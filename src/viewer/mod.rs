use loader::mesh::{Mesh, Primitive, PrimitiveVertex};
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::layout::DescriptorSetLayout,
    device::Device,
    image::SampleCount,
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
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
        layout::PipelineLayoutCreateInfo,
    },
    render_pass::Subpass,
};

pub mod loader;

#[repr(C)]
#[derive(BufferContents, Vertex, Debug)]
pub struct Instance {
    #[format(R32G32B32A32_SFLOAT)]
    pub model_x: [f32; 4],
    #[format(R32G32B32A32_SFLOAT)]
    pub model_y: [f32; 4],
    #[format(R32G32B32A32_SFLOAT)]
    pub model_z: [f32; 4],
    #[format(R32G32B32A32_SFLOAT)]
    pub model_w: [f32; 4],
}
impl From<glm::Mat4> for Instance {
    fn from(value: glm::Mat4) -> Self {
        Self {
            model_x: value.data.0[0],
            model_y: value.data.0[1],
            model_z: value.data.0[2],
            model_w: value.data.0[3],
        }
    }
}

#[derive(Clone)]
pub struct InstancedMesh {
    primitives: Vec<Primitive>,
    instances: Subbuffer<[Instance]>,
    len: u32,
}

#[derive(Clone)]
pub struct GltfRenderInfo {
    pub meshes: Vec<InstancedMesh>,
}
impl GltfRenderInfo {
    pub fn from_scene(
        allocator: Arc<StandardMemoryAllocator>,
        scene: gltf::Scene,
        mut meshes: Vec<Option<Mesh>>,
    ) -> GltfRenderInfo {
        let mut builder = GltfRenderInfoBuilder { instances: vec![] };

        Self::iter_nodes(scene.nodes(), &glm::identity(), &mut builder);

        let meshes = builder
            .instances
            .into_iter()
            .map(|(index, instance)| {
                let mesh = meshes[index].take().unwrap();
                let instances = Buffer::from_iter(
                    allocator.clone(),
                    BufferCreateInfo {
                        usage: BufferUsage::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    instance.into_iter().map(Instance::from),
                )
                .unwrap();
                InstancedMesh {
                    len: instances.len() as u32,
                    primitives: mesh.primitives.clone(),
                    instances,
                }
            })
            .collect();

        Self { meshes }
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
    pipeline: Arc<GraphicsPipeline>,
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
    pub fn render<L>(self, info: GltfRenderInfo, builder: &mut AutoCommandBufferBuilder<L>) {
        let layout = self.pipeline.layout().clone();
        builder.bind_pipeline_graphics(self.pipeline).unwrap();
        for mesh in info.meshes {
            builder.bind_vertex_buffers(1, mesh.instances).unwrap();
            for primitive in mesh.primitives {
                primitive.render(layout.clone(), mesh.len, builder);
            }
        }
    }
    pub fn layout(&self) -> &Arc<PipelineLayout> {
        self.pipeline.layout()
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
