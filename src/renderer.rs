use crate::{
    Allocators,
    cubemap::{
        CubeMesh, CubemapPipelineLayout, CubemapShaders,
        renderer::{CubeRenderPass, CubeRendererPipeline},
    },
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
    device::{Device, DeviceOwned},
    pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint},
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
pub struct SetLayouts {
    pub camera: Arc<DescriptorSetLayout>,
    pub texture: Arc<DescriptorSetLayout>,
    pub material: Arc<DescriptorSetLayout>,
}
impl SetLayouts {
    pub fn new(device: Arc<Device>) -> Self {
        let camera = DescriptorSetLayout::new(
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
        let material = DescriptorSetLayout::new(
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
        let texture_set_layout = DescriptorSetLayout::new(
            device,
            DescriptorSetLayoutCreateInfo {
                bindings: BTreeMap::from([texture_layout(0)]),
                ..Default::default()
            },
        )
        .unwrap();

        Self {
            camera,
            texture: texture_set_layout,
            material,
        }
    }
}

#[derive(Clone)]
pub struct Renderer {
    pub set_layouts: SetLayouts,
    pub skybox_pipeline: Arc<GraphicsPipeline>,
    pub equi_pipeline: Arc<GraphicsPipeline>,

    pub equi_set: Option<Arc<DescriptorSet>>,
    pub cube_set: Option<Arc<DescriptorSet>>,
    pub cube_mode: bool,

    pub gltf_pipeline: GltfPipeline,
    pub gltf_info: Option<GltfRenderInfo>,

    pub equi_renderer: CubeRendererPipeline,
    pub conv_renderer: CubeRendererPipeline,
    pub cube: CubeMesh,
}
impl Renderer {
    pub fn new(
        allocators: Allocators,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        subpass: Subpass,
    ) -> Self {
        let device = allocators.mem.device();

        let set_layouts = SetLayouts::new(device.clone());

        let cubemap_pipeline_layout = CubemapPipelineLayout::new(
            device.clone(),
            set_layouts.camera.clone(),
            set_layouts.texture.clone(),
        );
        let cubemap_shaders = CubemapShaders::new(device.clone());
        let skybox_pipeline = cubemap_pipeline_layout.clone().create_pipeline(
            cubemap_shaders.vs.clone(),
            cubemap_shaders.cube_fs,
            cubemap_shaders.vertex_input_state.clone(),
            subpass.clone(),
        );

        let gltf_pipeline = GltfPipeline::new(
            device.clone(),
            vec![set_layouts.camera.clone(), set_layouts.material.clone()],
            subpass.clone(),
        );

        let cube = CubeMesh::new(allocators.mem.clone(), builder);

        let cube_render_pass = CubeRenderPass::new(
            device.clone(),
            allocators.mem.clone(),
            allocators.set.clone(),
            set_layouts.camera.clone(),
        );
        let equi_renderer = CubeRendererPipeline {
            pipeline: cubemap_pipeline_layout.clone().create_pipeline(
                cubemap_shaders.vs.clone(),
                cubemap_shaders.equi_fs.clone(),
                cubemap_shaders.vertex_input_state.clone(),
                cube_render_pass.subpass.clone(),
            ),
            renderer: cube_render_pass.clone(),
            cube: cube.clone(),
        };
        let conv_renderer = CubeRendererPipeline {
            pipeline: cubemap_pipeline_layout.clone().create_pipeline(
                cubemap_shaders.vs.clone(),
                cubemap_shaders.conv_fs.clone(),
                cubemap_shaders.vertex_input_state.clone(),
                cube_render_pass.subpass.clone(),
            ),
            renderer: cube_render_pass,
            cube: cube.clone(),
        };
        let equi_pipeline = cubemap_pipeline_layout.clone().create_pipeline(
            cubemap_shaders.vs,
            cubemap_shaders.equi_fs,
            cubemap_shaders.vertex_input_state,
            subpass.clone(),
        );

        Self {
            skybox_pipeline,
            equi_set: None,
            cube_set: None,
            cube_mode: false,
            gltf_pipeline,
            gltf_info: None,
            cube,
            set_layouts,
            equi_renderer,
            equi_pipeline,
            conv_renderer,
        }
    }
    pub fn render<L>(self, builder: &mut AutoCommandBufferBuilder<L>) {
        if let Some(gltf_info) = self.gltf_info {
            self.gltf_pipeline.render(gltf_info, builder);
        }
        if self.cube_mode {
            if let Some(cube) = self.cube_set {
                let layout = self.skybox_pipeline.layout().clone();
                builder
                    .bind_pipeline_graphics(self.skybox_pipeline)
                    .unwrap()
                    .bind_descriptor_sets(PipelineBindPoint::Graphics, layout, 1, cube)
                    .unwrap();
                self.cube.render(builder);
            }
        } else {
            if let Some(equi) = self.equi_set {
                let layout = self.equi_pipeline.layout().clone();
                builder
                    .bind_pipeline_graphics(self.equi_pipeline)
                    .unwrap()
                    .bind_descriptor_sets(PipelineBindPoint::Graphics, layout, 1, equi)
                    .unwrap();
                self.cube.render(builder);
            }
        }
    }
}
