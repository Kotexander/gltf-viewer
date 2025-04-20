use mesh::CubemapVertex;
use std::sync::Arc;
use vulkano::{
    descriptor_set::layout::DescriptorSetLayout,
    device::{Device, DeviceOwned},
    image::{ImageAspects, SampleCount},
    pipeline::{
        DynamicState, GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo,
        graphics::{
            GraphicsPipelineCreateInfo,
            color_blend::{ColorBlendAttachmentState, ColorBlendState},
            depth_stencil::{CompareOp, DepthState, DepthStencilState},
            input_assembly::InputAssemblyState,
            multisample::MultisampleState,
            rasterization::{CullMode, FrontFace, RasterizationState},
            vertex_input::{Vertex, VertexDefinition, VertexInputState},
            viewport::ViewportState,
        },
        layout::PipelineLayoutCreateInfo,
    },
    render_pass::Subpass,
    shader::EntryPoint,
};

pub mod brdf;
pub mod conv;
pub mod cube;
pub mod equi;
pub mod filt;
mod mesh;
pub mod renderer;

pub use mesh::CubeMesh;

#[derive(Clone)]
pub struct CubemapVertexShader {
    pub vs: EntryPoint,
    pub vis: VertexInputState,
}
impl CubemapVertexShader {
    pub fn new(device: Arc<Device>) -> Self {
        let vs = vs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let vis = CubemapVertex::per_vertex().definition(&vs).unwrap();

        Self { vs, vis }
    }
}

pub fn cubemap_pipeline_layout(
    camera_set_layout: Arc<DescriptorSetLayout>,
    texture_set_layout: Arc<DescriptorSetLayout>,
) -> Arc<PipelineLayout> {
    let device = camera_set_layout.device();
    PipelineLayout::new(
        device.clone(),
        PipelineLayoutCreateInfo {
            set_layouts: vec![camera_set_layout, texture_set_layout],
            ..Default::default()
        },
    )
    .unwrap()
}

#[derive(Clone)]
pub struct CubemapPipelineBuilder {
    // layout: Arc<PipelineLayout>,
    vs: EntryPoint,
    fs: EntryPoint,
    vis: VertexInputState,
}
impl CubemapPipelineBuilder {
    pub fn build(self, layout: Arc<PipelineLayout>, subpass: Subpass) -> Arc<GraphicsPipeline> {
        let stages = [
            PipelineShaderStageCreateInfo::new(self.vs),
            PipelineShaderStageCreateInfo::new(self.fs),
        ];

        let has_depth_buffer = subpass
            .subpass_desc()
            .depth_stencil_attachment
            .as_ref()
            .is_some_and(|ar| {
                subpass.render_pass().attachments()[ar.attachment as usize]
                    .format
                    .aspects()
                    .intersects(ImageAspects::DEPTH)
            });

        let depth_stencil_state = if has_depth_buffer {
            Some(DepthStencilState {
                depth: Some(DepthState {
                    write_enable: true,
                    compare_op: CompareOp::LessOrEqual,
                }),
                ..Default::default()
            })
        } else {
            None
        };

        GraphicsPipeline::new(
            layout.device().clone(),
            None,
            GraphicsPipelineCreateInfo {
                stages: stages.into_iter().collect(),
                vertex_input_state: Some(self.vis),
                input_assembly_state: Some(InputAssemblyState::default()),
                viewport_state: Some(ViewportState::default()),
                rasterization_state: Some(RasterizationState {
                    front_face: FrontFace::CounterClockwise,
                    cull_mode: CullMode::None,
                    ..Default::default()
                }),
                multisample_state: Some(MultisampleState {
                    rasterization_samples: subpass.num_samples().unwrap_or(SampleCount::Sample1),
                    ..Default::default()
                }),
                color_blend_state: Some(ColorBlendState::with_attachment_states(
                    subpass.num_color_attachments(),
                    ColorBlendAttachmentState::default(),
                )),
                depth_stencil_state,
                dynamic_state: [DynamicState::Viewport, DynamicState::Scissor]
                    .into_iter()
                    .collect(),
                subpass: Some(subpass.into()),
                ..GraphicsPipelineCreateInfo::layout(layout)
            },
        )
        .unwrap()
    }
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: r#"
#version 450

layout(location = 0) in vec3 position;

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} cam;

layout(location = 0) out vec3 f_position;

void main() {
    gl_Position = (cam.proj * cam.view * vec4(position, 0.0)).xyww;
    f_position = position;
}
        "#
    }
}
