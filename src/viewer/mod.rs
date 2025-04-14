use loader::{
    mesh::{Primitive, PrimitiveVertex},
    node::Node,
    scene::Scene,
};
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::{DescriptorSet, layout::DescriptorSetLayout},
    device::Device,
    image::SampleCount,
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
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

#[derive(Clone)]
pub struct InstancedMesh {
    primitives: Vec<Primitive>,
    instances: Subbuffer<[Instance]>,
}

#[derive(Clone)]
pub struct GltfRenderInfo {
    pub meshes: Vec<InstancedMesh>,
    pub sets: Arc<DescriptorSet>,
}
impl GltfRenderInfo {
    pub fn from_scene(
        allocator: &Arc<StandardMemoryAllocator>,
        scene: &Scene,
        sets: Arc<DescriptorSet>,
    ) -> GltfRenderInfo {
        let mut meshes = vec![];
        Self::iter_nodes(allocator, &scene.nodes, &glm::identity(), &mut meshes);
        Self { meshes, sets }
    }
    fn iter_nodes(
        allocator: &Arc<StandardMemoryAllocator>,
        nodes: &[Arc<Node>],
        transform: &glm::Mat4,
        meshes: &mut Vec<InstancedMesh>,
    ) {
        for node in nodes {
            let transform = transform * node.transform;
            if let Some(mesh) = &node.mesh {
                let instance = InstancedMesh {
                    primitives: mesh.primitives.clone(),
                    instances: Buffer::from_iter(
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
                        [Instance {
                            model_x: transform.data.0[0],
                            model_y: transform.data.0[1],
                            model_z: transform.data.0[2],
                            model_w: transform.data.0[3],
                        }],
                    )
                    .unwrap(),
                };
                meshes.push(instance);
            }
            Self::iter_nodes(allocator, &node.children, &transform, meshes);
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
            builder
                .bind_vertex_buffers(1, mesh.instances.clone())
                .unwrap();
            for primitive in mesh.primitives {
                primitive.render(1, builder);
            }
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
