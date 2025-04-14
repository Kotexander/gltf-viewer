use std::{collections::BTreeMap, sync::Arc};

use crate::{
    cubemap::CubemapPipeline,
    viewer::{GltfPipeline, GltfRenderInfo},
};
use vulkano::{
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::{
        DescriptorSet,
        layout::{
            DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo,
            DescriptorType,
        },
    },
    device::DeviceOwned,
    memory::allocator::StandardMemoryAllocator,
    render_pass::Subpass,
    shader::ShaderStages,
};

#[derive(Clone)]
pub struct Renderer {
    pub camera_set_layout: Arc<DescriptorSetLayout>,
    pub cubemap_set_layout: Arc<DescriptorSetLayout>,

    pub skybox_pipeline: CubemapPipeline,

    pub equi_set: Option<Arc<DescriptorSet>>,
    pub cube_set: Option<Arc<DescriptorSet>>,
    pub cube_mode: bool,

    pub gltf_pipeline: GltfPipeline,
    pub gltf_info: Option<GltfRenderInfo>,
}
impl Renderer {
    pub fn new(allocator: Arc<StandardMemoryAllocator>, subpass: Subpass) -> Self {
        let device = allocator.device();

        let camera_set_layout = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings: BTreeMap::from([(
                    0,
                    DescriptorSetLayoutBinding {
                        stages: ShaderStages::VERTEX,
                        ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer)
                    },
                )]),
                ..Default::default()
            },
        )
        .unwrap();
        let cubemap_set_layout = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings: BTreeMap::from([(
                    0,
                    DescriptorSetLayoutBinding {
                        stages: ShaderStages::FRAGMENT,
                        ..DescriptorSetLayoutBinding::descriptor_type(
                            DescriptorType::CombinedImageSampler,
                        )
                    },
                )]),
                ..Default::default()
            },
        )
        .unwrap();

        let gltf_pipeline = GltfPipeline::new(
            device.clone(),
            vec![camera_set_layout.clone()],
            subpass.clone(),
        );
        let skybox_pipeline = CubemapPipeline::new(
            allocator,
            vec![camera_set_layout.clone(), cubemap_set_layout.clone()],
            subpass,
        );

        Self {
            camera_set_layout,
            cubemap_set_layout,
            skybox_pipeline,
            equi_set: None,
            cube_set: None,
            cube_mode: false,
            gltf_pipeline,
            gltf_info: None,
        }
    }
    pub fn render<L>(self, builder: &mut AutoCommandBufferBuilder<L>) {
        if let Some(gltf_info) = self.gltf_info {
            self.gltf_pipeline.render(gltf_info, builder);
        }
        if self.cube_mode {
            if let Some(cube_set) = self.cube_set {
                self.skybox_pipeline.render_cube(builder, cube_set);
            }
        } else {
            if let Some(equi_set) = self.equi_set {
                self.skybox_pipeline.render_equi(builder, equi_set);
            }
        }
    }
}
