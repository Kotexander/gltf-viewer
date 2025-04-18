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
        DescriptorSet, WriteDescriptorSet,
        layout::{
            DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo,
            DescriptorType,
        },
    },
    device::{Device, DeviceOwned},
    format::Format,
    image::{
        Image, ImageCreateFlags, ImageCreateInfo, ImageUsage,
        sampler::{Sampler, SamplerCreateInfo},
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
    },
    memory::allocator::AllocationCreateInfo,
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

    pub cube_set: Option<Arc<DescriptorSet>>,
    pub conv_set: Arc<DescriptorSet>,

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
            vec![
                set_layouts.camera.clone(),
                set_layouts.texture.clone(),
                set_layouts.material.clone(),
            ],
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
                cubemap_shaders.equi_fs,
                cubemap_shaders.vertex_input_state.clone(),
                cube_render_pass.subpass.clone(),
            ),
            renderer: cube_render_pass.clone(),
            cube: cube.clone(),
        };
        let conv_renderer = CubeRendererPipeline {
            pipeline: cubemap_pipeline_layout.clone().create_pipeline(
                cubemap_shaders.vs,
                cubemap_shaders.conv_fs,
                cubemap_shaders.vertex_input_state,
                cube_render_pass.subpass.clone(),
            ),
            renderer: cube_render_pass,
            cube: cube.clone(),
        };

        let conv_image = Image::new(
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
        let conv_view = ImageView::new(
            conv_image.clone(),
            ImageViewCreateInfo {
                view_type: ImageViewType::Cube,
                ..ImageViewCreateInfo::from_image(&conv_image)
            },
        )
        .unwrap();
        let conv_set = DescriptorSet::new(
            allocators.set,
            set_layouts.texture.clone(),
            [WriteDescriptorSet::image_view_sampler(
                0,
                conv_view,
                Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear()).unwrap(),
            )],
            [],
        )
        .unwrap();

        Self {
            skybox_pipeline,
            cube_set: None,
            gltf_pipeline,
            gltf_info: None,
            cube,
            set_layouts,
            equi_renderer,
            conv_renderer,
            conv_set,
        }
    }
    pub fn render<L>(self, builder: &mut AutoCommandBufferBuilder<L>) {
        if let Some(gltf_info) = self.gltf_info {
            let layout = self.gltf_pipeline.pipeline.layout().clone();
            builder
                .bind_descriptor_sets(PipelineBindPoint::Graphics, layout, 1, self.conv_set)
                .unwrap();
            self.gltf_pipeline.render(gltf_info, builder);
        }
        if let Some(cube) = self.cube_set {
            let layout = self.skybox_pipeline.layout().clone();
            builder
                .bind_pipeline_graphics(self.skybox_pipeline)
                .unwrap()
                .bind_descriptor_sets(PipelineBindPoint::Graphics, layout, 1, cube)
                .unwrap();
            self.cube.render(builder);
        }
    }
}
