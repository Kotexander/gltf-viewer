use loader::{
    mesh::{Primitive, PrimitiveVertex},
    node::Node,
    scene::Scene,
};
use std::sync::Arc;
use vulkano::{
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::{DescriptorSet, layout::DescriptorSetLayout},
    device::Device,
    image::SampleCount,
    pipeline::{
        DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineLayout,
        PipelineShaderStageCreateInfo,
        graphics::{
            GraphicsPipelineCreateInfo,
            color_blend::{ColorBlendAttachmentState, ColorBlendState},
            depth_stencil::{DepthState, DepthStencilState},
            input_assembly::InputAssemblyState,
            multisample::MultisampleState,
            rasterization::{CullMode, FrontFace, RasterizationState},
            vertex_input::{Vertex, VertexDefinition},
            viewport::ViewportState,
        },
        layout::PipelineLayoutCreateInfo,
    },
    render_pass::Subpass,
};

pub mod loader;

#[derive(Clone)]
pub struct GltfRenderInfo {
    pub meshes: Vec<Primitive>,
    pub sets: Arc<DescriptorSet>,
}
impl GltfRenderInfo {
    pub fn from_scene(scene: &Scene, sets: Arc<DescriptorSet>) -> GltfRenderInfo {
        let mut meshes = vec![];
        Self::iter_nodes(&scene.nodes, &mut meshes);
        Self { meshes, sets }
    }
    fn iter_nodes(nodes: &[Arc<Node>], meshes: &mut Vec<Primitive>) {
        for node in nodes {
            if let Some(mesh) = &node.mesh {
                meshes.extend_from_slice(&mesh.primitives);
            }
            Self::iter_nodes(&node.children, meshes);
        }
    }
}

#[derive(Clone)]
pub struct GltfPipeline {
    pipeline: Arc<GraphicsPipeline>,
}
impl GltfPipeline {
    pub fn new(
        device: &Arc<Device>,
        set_layouts: Vec<Arc<DescriptorSetLayout>>,
        subpass: Subpass,
    ) -> Self {
        let vs = vs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let fs = fs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();

        let vertex_input_state = PrimitiveVertex::per_vertex().definition(&vs).unwrap();

        let stages = [
            PipelineShaderStageCreateInfo::new(vs),
            PipelineShaderStageCreateInfo::new(fs),
        ];

        let layout = PipelineLayout::new(
            device.clone(),
            PipelineLayoutCreateInfo {
                set_layouts,
                ..Default::default()
            },
        )
        .unwrap();

        let pipeline = GraphicsPipeline::new(
            device.clone(),
            None,
            GraphicsPipelineCreateInfo {
                stages: stages.into_iter().collect(),
                vertex_input_state: Some(vertex_input_state),
                input_assembly_state: Some(InputAssemblyState::default()),
                viewport_state: Some(ViewportState::default()),
                multisample_state: Some(MultisampleState {
                    rasterization_samples: subpass.num_samples().unwrap_or(SampleCount::Sample1),
                    ..Default::default()
                }),
                rasterization_state: Some(RasterizationState {
                    front_face: FrontFace::Clockwise,
                    cull_mode: CullMode::Back,
                    ..Default::default()
                }),
                color_blend_state: Some(ColorBlendState::with_attachment_states(
                    subpass.num_color_attachments(),
                    ColorBlendAttachmentState::default(),
                )),
                depth_stencil_state: Some(DepthStencilState {
                    depth: Some(DepthState::simple()),
                    ..Default::default()
                }),
                dynamic_state: [DynamicState::Viewport, DynamicState::Scissor]
                    .into_iter()
                    .collect(),
                subpass: Some(subpass.into()),
                ..GraphicsPipelineCreateInfo::layout(layout)
            },
        )
        .unwrap();

        Self { pipeline }
    }
    pub fn render<L>(self, info: GltfRenderInfo, builder: &mut AutoCommandBufferBuilder<L>) {
        builder
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                info.sets,
            )
            .unwrap();
        builder.bind_pipeline_graphics(self.pipeline).unwrap();
        for mesh in info.meshes {
            mesh.render(builder);
        }
    }
}

pub mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "shaders/gltf.vert"
    }
}
mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "shaders/gltf.frag"
    }
}
