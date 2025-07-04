use image::EncodableLayout;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{
        AutoCommandBufferBuilder, BlitImageInfo, CopyBufferToImageInfo, CopyImageInfo, ImageBlit,
    },
    format::Format,
    image::{
        Image, ImageCreateInfo, ImageSubresourceLayers, ImageType, ImageUsage, sampler::Filter,
    },
    memory::allocator::{AllocationCreateInfo, MemoryAllocator, MemoryTypeFilter},
};

pub fn create_vk_image<L>(
    allocator: Arc<dyn MemoryAllocator>,
    builder: &mut AutoCommandBufferBuilder<L>,
    data: gltf::image::Data,
    is_srgb: bool,
) -> Arc<Image> {
    let w = data.width.next_power_of_two();
    let h = data.height.next_power_of_two();

    let rgba8 = convert_image(data)
        .resize_exact(w, h, image::imageops::FilterType::Lanczos3)
        .to_rgba8();

    let format = if is_srgb {
        Format::R8G8B8A8_SRGB
    } else {
        Format::R8G8B8A8_UNORM
    };

    let stage_buffer = Buffer::from_iter(
        allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        rgba8.as_bytes().iter().copied(),
    )
    .unwrap();

    let mips = w.max(h).ilog2() + 1;

    let stage_image = vulkano::image::Image::new(
        allocator.clone(),
        ImageCreateInfo {
            usage: ImageUsage::TRANSFER_DST | ImageUsage::TRANSFER_SRC | ImageUsage::SAMPLED,
            image_type: ImageType::Dim2d,
            format,
            mip_levels: mips,
            extent: [w, h, 1],
            ..Default::default()
        },
        AllocationCreateInfo::default(),
    )
    .unwrap();

    builder
        .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
            stage_buffer,
            stage_image.clone(),
        ))
        .unwrap();

    for mip in 1..mips {
        builder
            .blit_image(BlitImageInfo {
                filter: Filter::Linear,
                regions: [ImageBlit {
                    src_subresource: ImageSubresourceLayers {
                        mip_level: mip - 1,
                        ..stage_image.subresource_layers()
                    },
                    dst_subresource: ImageSubresourceLayers {
                        mip_level: mip,
                        ..stage_image.subresource_layers()
                    },
                    src_offsets: [
                        [0, 0, 0],
                        [(w >> (mip - 1)).max(1), (h >> (mip - 1)).max(1), 1],
                    ],
                    dst_offsets: [[0, 0, 0], [(w >> mip).max(1), (h >> mip).max(1), 1]],
                    ..Default::default()
                }]
                .into(),
                ..BlitImageInfo::images(stage_image.clone(), stage_image.clone())
            })
            .unwrap();
    }

    let vk_image = vulkano::image::Image::new(
        allocator.clone(),
        ImageCreateInfo {
            usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
            image_type: ImageType::Dim2d,
            format,
            mip_levels: mips,
            extent: [w, h, 1],
            ..Default::default()
        },
        AllocationCreateInfo::default(),
    )
    .unwrap();

    let mut info = CopyImageInfo::images(stage_image, vk_image.clone());
    for mip in 0..mips {
        info.regions[0].src_subresource.mip_level = mip;
        info.regions[0].dst_subresource.mip_level = mip;
        builder.copy_image(info.clone()).unwrap();
        info.regions[0].extent[0] = (info.regions[0].extent[0] >> 1).max(1);
        info.regions[0].extent[1] = (info.regions[0].extent[1] >> 1).max(1);
    }

    vk_image
}

fn convert_image(data: gltf::image::Data) -> image::DynamicImage {
    match data.format {
        gltf::image::Format::R8 => image::DynamicImage::ImageLuma8(
            image::ImageBuffer::from_vec(data.width, data.height, data.pixels).unwrap(),
        ),
        gltf::image::Format::R8G8 => image::DynamicImage::ImageLumaA8(
            image::ImageBuffer::from_vec(data.width, data.height, data.pixels).unwrap(),
        ),
        gltf::image::Format::R8G8B8 => image::DynamicImage::ImageRgb8(
            image::ImageBuffer::from_vec(data.width, data.height, data.pixels).unwrap(),
        ),
        gltf::image::Format::R8G8B8A8 => image::DynamicImage::ImageRgba8(
            image::ImageBuffer::from_vec(data.width, data.height, data.pixels).unwrap(),
        ),
        gltf::image::Format::R16 => image::DynamicImage::ImageLuma16(
            image::ImageBuffer::from_vec(data.width, data.height, bytemuck::cast_vec(data.pixels))
                .unwrap(),
        ),
        gltf::image::Format::R16G16 => image::DynamicImage::ImageLumaA16(
            image::ImageBuffer::from_vec(data.width, data.height, bytemuck::cast_vec(data.pixels))
                .unwrap(),
        ),
        gltf::image::Format::R16G16B16 => image::DynamicImage::ImageRgb16(
            image::ImageBuffer::from_vec(data.width, data.height, bytemuck::cast_vec(data.pixels))
                .unwrap(),
        ),
        gltf::image::Format::R16G16B16A16 => image::DynamicImage::ImageRgba16(
            image::ImageBuffer::from_vec(data.width, data.height, bytemuck::cast_vec(data.pixels))
                .unwrap(),
        ),
        gltf::image::Format::R32G32B32FLOAT => image::DynamicImage::ImageRgb32F(
            image::ImageBuffer::from_vec(data.width, data.height, bytemuck::cast_vec(data.pixels))
                .unwrap(),
        ),
        gltf::image::Format::R32G32B32A32FLOAT => image::DynamicImage::ImageRgba32F(
            image::ImageBuffer::from_vec(data.width, data.height, bytemuck::cast_vec(data.pixels))
                .unwrap(),
        ),
    }
}
