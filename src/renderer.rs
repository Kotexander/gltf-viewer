use crate::{
    cubemap::{CubeMesh, CubemapPipeline},
    viewer::{GltfPipeline, GltfRenderInfo},
};
use std::{collections::BTreeMap, sync::Arc};
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
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

fn texture_layout(set: u32) -> (u32, DescriptorSetLayoutBinding) {
    (
        set,
        DescriptorSetLayoutBinding {
            stages: ShaderStages::FRAGMENT,
            ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::CombinedImageSampler)
        },
    )
}

#[derive(Clone)]
pub struct Renderer {
    pub camera_set_layout: Arc<DescriptorSetLayout>,
    pub cubemap_set_layout: Arc<DescriptorSetLayout>,
    pub material_set_layout: Arc<DescriptorSetLayout>,

    pub skybox_pipeline: CubemapPipeline,

    pub equi_set: Option<Arc<DescriptorSet>>,
    pub cube_set: Option<Arc<DescriptorSet>>,
    pub cube_mode: bool,

    pub gltf_pipeline: GltfPipeline,
    pub gltf_info: Option<GltfRenderInfo>,
    pub cube: CubeMesh,
}
impl Renderer {
    pub fn new(
        allocator: Arc<StandardMemoryAllocator>,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        subpass: Subpass,
    ) -> Self {
        let device = allocator.device();

        let camera_set_layout = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings: BTreeMap::from([(
                    0,
                    DescriptorSetLayoutBinding {
                        stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                        ..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::UniformBuffer)
                    },
                )]),
                ..Default::default()
            },
        )
        .unwrap();
        let material_set_layout = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings: BTreeMap::from([
                    (
                        0,
                        DescriptorSetLayoutBinding {
                            stages: ShaderStages::FRAGMENT,
                            ..DescriptorSetLayoutBinding::descriptor_type(
                                DescriptorType::UniformBuffer,
                            )
                        },
                    ),
                    texture_layout(1),
                    texture_layout(2),
                    texture_layout(3),
                    texture_layout(4),
                    texture_layout(5),
                ]),
                ..Default::default()
            },
        )
        .unwrap();
        let cubemap_set_layout = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings: BTreeMap::from([texture_layout(0)]),
                ..Default::default()
            },
        )
        .unwrap();

        let gltf_pipeline = GltfPipeline::new(
            device.clone(),
            vec![camera_set_layout.clone(), material_set_layout.clone()],
            subpass.clone(),
        );
        let skybox_pipeline = CubemapPipeline::new(
            allocator.clone(),
            vec![camera_set_layout.clone(), cubemap_set_layout.clone()],
            subpass,
        );

        let cube = CubeMesh::new(allocator, builder);

        Self {
            camera_set_layout,
            cubemap_set_layout,
            material_set_layout,
            skybox_pipeline,
            equi_set: None,
            cube_set: None,
            cube_mode: false,
            gltf_pipeline,
            gltf_info: None,
            cube,
        }
    }
    pub fn render<L>(self, builder: &mut AutoCommandBufferBuilder<L>) {
        if let Some(gltf_info) = self.gltf_info {
            self.gltf_pipeline.render(gltf_info, builder);
        }
        if let Some(equi) = self.equi_set {
            CubemapPipeline::render(builder, self.skybox_pipeline.equi_pipeline, self.cube, equi);
        }
    }
}
