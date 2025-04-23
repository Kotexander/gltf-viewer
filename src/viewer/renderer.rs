use crate::{
    Allocators,
    gltf::{GltfPipeline, GltfRenderInfo},
    set_layouts::SetLayouts,
};
use image::EncodableLayout;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{AutoCommandBufferBuilder, CopyBufferToImageInfo},
    descriptor_set::{DescriptorSet, WriteDescriptorSet, allocator::DescriptorSetAllocator},
    device::DeviceOwned,
    format::Format,
    image::{
        Image, ImageCreateFlags, ImageCreateInfo, ImageUsage,
        sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo},
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    pipeline::{Pipeline, PipelineBindPoint},
    render_pass::Subpass,
};

#[derive(Clone)]
pub struct ViewerRenderer {
    pub pipeline: GltfPipeline,
    pub env_set: Arc<DescriptorSet>,
    pub info: Option<GltfRenderInfo>,
    pub sampler: Arc<Sampler>,
    pub lut_write: WriteDescriptorSet,
    pub set_allocator: Arc<dyn DescriptorSetAllocator>,
}
impl ViewerRenderer {
    pub fn new<L>(
        allocators: &Allocators,
        builder: &mut AutoCommandBufferBuilder<L>,
        set_layouts: &SetLayouts,
        subpass: Subpass,
    ) -> Self {
        let device = allocators.mem.device();
        let pipeline = GltfPipeline::new(
            device.clone(),
            vec![
                set_layouts.camera.clone(),
                set_layouts.environment.clone(),
                set_layouts.material.clone(),
            ],
            subpass.clone(),
        );

        let env_image = Image::new(
            allocators.mem.clone(),
            ImageCreateInfo {
                format: Format::R16G16B16A16_SFLOAT,
                usage: ImageUsage::SAMPLED,
                flags: ImageCreateFlags::CUBE_COMPATIBLE,
                array_layers: 6,
                extent: [1, 1, 1],
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )
        .unwrap();
        let env_view = ImageView::new(
            env_image.clone(),
            ImageViewCreateInfo {
                view_type: ImageViewType::Cube,
                ..ImageViewCreateInfo::from_image(&env_image)
            },
        )
        .unwrap();

        let brdf = image::load_from_memory(include_bytes!("lut_ggx.png"))
            .unwrap()
            .to_rgba8();
        let stage_brdf = Buffer::from_iter(
            allocators.mem.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            brdf.as_bytes().iter().copied(),
        )
        .unwrap();
        let brdf = Image::new(
            allocators.mem.clone(),
            ImageCreateInfo {
                usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                format: Format::R8G8B8A8_UNORM,
                extent: [brdf.width(), brdf.height(), 1],
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )
        .unwrap();
        builder
            .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
                stage_brdf,
                brdf.clone(),
            ))
            .unwrap();
        let brdf = ImageView::new_default(brdf).unwrap();

        let sampler =
            Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear()).unwrap();
        let lut_write = WriteDescriptorSet::image_view_sampler(
            2,
            brdf.clone(),
            Sampler::new(
                device.clone(),
                SamplerCreateInfo {
                    mag_filter: Filter::Linear,
                    min_filter: Filter::Linear,
                    address_mode: [
                        SamplerAddressMode::ClampToEdge,
                        SamplerAddressMode::ClampToEdge,
                        SamplerAddressMode::ClampToEdge,
                    ],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        let env_set = DescriptorSet::new(
            allocators.set.clone(),
            set_layouts.environment.clone(),
            [
                WriteDescriptorSet::image_view_sampler(0, env_view.clone(), sampler.clone()),
                WriteDescriptorSet::image_view_sampler(1, env_view, sampler.clone()),
                WriteDescriptorSet::image_view_sampler(2, brdf.clone(), sampler.clone()),
            ],
            [],
        )
        .unwrap();

        Self {
            pipeline,
            info: None,
            env_set,
            sampler,
            set_allocator: allocators.set.clone(),
            lut_write,
        }
    }

    pub fn render<L>(&self, builder: &mut AutoCommandBufferBuilder<L>) {
        if let Some(gltf_info) = self.info.clone() {
            let layout = self.pipeline.pipeline.layout().clone();
            builder
                .bind_descriptor_sets(PipelineBindPoint::Graphics, layout, 1, self.env_set.clone())
                .unwrap();
            self.pipeline.render(gltf_info, builder);
        }
    }

    pub fn new_env(&mut self, diffuse: Arc<Image>, specular: Arc<Image>) {
        let diffuse_view = ImageView::new(
            diffuse.clone(),
            ImageViewCreateInfo {
                view_type: ImageViewType::Cube,
                ..ImageViewCreateInfo::from_image(&diffuse)
            },
        )
        .unwrap();
        let specular_view = ImageView::new(
            specular.clone(),
            ImageViewCreateInfo {
                view_type: ImageViewType::Cube,
                ..ImageViewCreateInfo::from_image(&specular)
            },
        )
        .unwrap();

        let env_set = DescriptorSet::new(
            self.set_allocator.clone(),
            self.pipeline.pipeline.layout().set_layouts()[1].clone(),
            [
                WriteDescriptorSet::image_view_sampler(0, diffuse_view, self.sampler.clone()),
                WriteDescriptorSet::image_view_sampler(1, specular_view, self.sampler.clone()),
                self.lut_write.clone(),
            ],
            [],
        )
        .unwrap();
        self.env_set = env_set;
    }
}
