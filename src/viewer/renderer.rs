use crate::{
    Allocators,
    gltf::{GltfPipeline, GltfRenderInfo},
    set_layouts::SetLayouts,
};
use std::sync::Arc;
use vulkano::{
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    device::DeviceOwned,
    format::Format,
    image::{
        Image, ImageCreateFlags, ImageCreateInfo, ImageUsage,
        sampler::{Sampler, SamplerCreateInfo},
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
    },
    memory::allocator::AllocationCreateInfo,
    pipeline::{Pipeline, PipelineBindPoint},
    render_pass::Subpass,
};

#[derive(Clone)]
pub struct ViewerRenderer {
    pub pipeline: GltfPipeline,
    pub env_set: Arc<DescriptorSet>,
    pub info: Option<GltfRenderInfo>,
}
impl ViewerRenderer {
    pub fn new(allocators: &Allocators, set_layouts: &SetLayouts, subpass: Subpass) -> Self {
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
        let env_set = DescriptorSet::new(
            allocators.set.clone(),
            set_layouts.texture.clone(),
            [WriteDescriptorSet::image_view_sampler(
                0,
                env_view,
                Sampler::new(
                    device.clone(),
                    SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
                )
                .unwrap(),
            )],
            [],
        )
        .unwrap();

        Self {
            pipeline,
            info: None,
            env_set,
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
}
