use image::EncodableLayout;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{AutoCommandBufferBuilder, CopyBufferToImageInfo, PrimaryAutoCommandBuffer},
    format::Format,
    image::{ImageCreateInfo, ImageType, ImageUsage, view::ImageView},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
};

pub fn load_images(
    document: &gltf::Document,
    images: Vec<gltf::image::Data>,
    allocator: &Arc<StandardMemoryAllocator>,
    builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
) -> Vec<Arc<ImageView>> {
    let mut is_srgb = vec![false; document.images().len()];
    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();
        if let Some(base_color) = pbr.base_color_texture() {
            let i = base_color.texture().source().index();
            is_srgb[i] = true;
        }
    }

    images
        .into_iter()
        .zip(is_srgb)
        .map(|(data, is_srgb)| {
            let image = convert_image(data);

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
                image.as_bytes().iter().copied(),
            )
            .unwrap();

            let format = if is_srgb {
                Format::R8G8B8A8_SRGB
            } else {
                Format::R8G8B8A8_UNORM
            };

            let image = vulkano::image::Image::new(
                allocator.clone(),
                ImageCreateInfo {
                    usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                    image_type: ImageType::Dim2d,
                    format,
                    extent: [image.width(), image.height(), 1],
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap();

            builder
                .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
                    stage_buffer,
                    image.clone(),
                ))
                .unwrap();

            ImageView::new_default(image).unwrap()
        })
        .collect()
}

fn convert_image(data: gltf::image::Data) -> image::ImageBuffer<image::Rgba<u8>, Vec<u8>> {
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
    .to_rgba8()
}
