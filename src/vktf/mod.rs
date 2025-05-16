use loader::{PrimitiveVertex, VktfDocument};
use material::{MaterialPush, Materials};
use mesh::{Instance, Mesh};
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::{allocator::DescriptorSetAllocator, layout::DescriptorSetLayout},
    device::Device,
    image::SampleCount,
    memory::allocator::MemoryAllocator,
    pipeline::{
        DynamicState, GraphicsPipeline, Pipeline, PipelineLayout, PipelineShaderStageCreateInfo,
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
        layout::{PipelineLayoutCreateInfo, PushConstantRange},
    },
    render_pass::Subpass,
    shader::ShaderStages,
};

pub mod loader;
pub mod material;
pub mod mesh;

#[derive(Clone)]
pub struct GltfRenderInfo {
    pub meshes: Vec<Mesh>,
    pub materials: Materials,
    pub vktf: Arc<VktfDocument>,
}
impl GltfRenderInfo {
    pub fn new_default(
        mem_allocator: Arc<dyn MemoryAllocator>,
        set_allocator: Arc<dyn DescriptorSetAllocator>,
        layout: Arc<DescriptorSetLayout>,
        vktf: VktfDocument,
    ) -> GltfRenderInfo {
        let materials = Materials::new(set_allocator, layout, &vktf);

        let scene = vktf.document.default_scene().unwrap();
        let mut builder = GltfRenderInfoBuilder { instances: vec![] };
        Self::iter_nodes(scene.nodes(), &glm::identity(), &mut builder);

        let meshes = builder
            .instances
            .into_iter()
            .map(|(index, instances)| {
                let primitives = vktf
                    .document
                    .meshes()
                    .nth(index)
                    .unwrap()
                    .primitives()
                    .zip(vktf.vktf.get_mesh(index).unwrap().iter().cloned());
                Mesh::new(mem_allocator.clone(), primitives, instances)
            })
            .collect();

        Self {
            meshes,
            materials,
            vktf: Arc::new(vktf),
        }
    }
    fn iter_nodes<'a>(
        nodes: impl Iterator<Item = gltf::Node<'a>>,
        transform: &glm::Mat4,
        builder: &mut GltfRenderInfoBuilder,
    ) {
        for node in nodes {
            let transform = transform * glm::Mat4::from(node.transform().matrix());
            if let Some(mesh) = node.mesh() {
                builder.add_mesh(mesh.index(), transform);
            }
            Self::iter_nodes(node.children(), &transform, builder);
        }
    }
}

struct GltfRenderInfoBuilder {
    instances: Vec<(usize, Vec<glm::Mat4>)>,
}
impl GltfRenderInfoBuilder {
    pub fn add_mesh(&mut self, index: usize, transform: glm::Mat4) {
        match self.instances.binary_search_by_key(&index, |(i, _)| *i) {
            Ok(i) => {
                self.instances[i].1.push(transform);
            }
            Err(i) => {
                self.instances.insert(i, (index, vec![transform]));
            }
        }
    }
}

#[derive(Clone)]
pub struct GltfPipeline {
    pub pipeline: Arc<GraphicsPipeline>,
}
impl GltfPipeline {
    pub fn new(
        device: Arc<Device>,
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

        let vertex_input_state = [PrimitiveVertex::per_vertex(), Instance::per_instance()]
            .definition(&vs)
            .unwrap();

        let stages = [
            PipelineShaderStageCreateInfo::new(vs),
            PipelineShaderStageCreateInfo::new(fs),
        ];

        let layout = PipelineLayout::new(
            device.clone(),
            PipelineLayoutCreateInfo {
                set_layouts,
                push_constant_ranges: vec![PushConstantRange {
                    stages: ShaderStages::FRAGMENT,
                    offset: 0,
                    size: std::mem::size_of::<MaterialPush>() as u32,
                }],
                ..Default::default()
            },
        )
        .unwrap();

        let pipeline = GraphicsPipeline::new(
            device,
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
                    front_face: FrontFace::CounterClockwise,
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
    pub fn render<L>(&self, info: GltfRenderInfo, builder: &mut AutoCommandBufferBuilder<L>) {
        builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .unwrap();
        // TODO: dont rebind and repush materials when not needed
        for mesh in info.meshes {
            mesh.render(builder, &info.materials, self.pipeline.layout());
        }
    }
}

mod vs {
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
