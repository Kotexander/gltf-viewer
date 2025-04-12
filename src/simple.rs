use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::{
        DescriptorSet, DescriptorSetsCollection, WriteDescriptorSet,
        allocator::StandardDescriptorSetAllocator,
    },
    device::Device,
    image::SampleCount,
    memory::allocator::{AllocationCreateInfo, MemoryAllocator, MemoryTypeFilter},
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
        layout::PipelineDescriptorSetLayoutCreateInfo,
    },
    render_pass::Subpass,
};

#[repr(C)]
#[derive(BufferContents, Vertex)]
struct SimpleVertex {
    #[format(R32G32B32_SFLOAT)]
    position: glm::Vec3,
    #[format(R32G32B32_SFLOAT)]
    normal: glm::Vec3,
}

#[derive(Clone)]
pub struct SimpleMesh {
    vbuf: Subbuffer<[SimpleVertex]>,
    ibuf: Subbuffer<[u32]>,
    ilen: u32,
}
impl SimpleMesh {
    pub fn new(allocator: Arc<dyn MemoryAllocator>, path: &str) -> Vec<Self> {
        let (tobj, _) = tobj::load_obj(path, &tobj::GPU_LOAD_OPTIONS).unwrap();

        let models = tobj
            .into_iter()
            .map(|model| {
                let vertex_iter = model
                    .mesh
                    .positions
                    .chunks_exact(3)
                    .map(|p| glm::vec3(p[0], p[1], p[2]))
                    .zip(
                        model
                            .mesh
                            .normals
                            .chunks_exact(3)
                            .map(|n| glm::vec3(n[0], n[1], n[2])),
                    )
                    .map(|(position, normal)| SimpleVertex { position, normal });

                let vbuf = Buffer::from_iter(
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
                    vertex_iter,
                )
                .unwrap();
                let ilen = model.mesh.indices.len() as u32;
                let ibuf = Buffer::from_iter(
                    allocator.clone(),
                    BufferCreateInfo {
                        usage: BufferUsage::INDEX_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    model.mesh.indices,
                )
                .unwrap();

                Self { vbuf, ibuf, ilen }
            })
            .collect();

        models
    }
}

#[derive(Clone)]
pub struct SimpleRenderer {
    pub pipeline: Arc<GraphicsPipeline>,
}
impl SimpleRenderer {
    pub fn new(device: Arc<Device>, subpass: Subpass) -> Self {
        let vs = vs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let fs = fs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();

        let vertex_input_state = SimpleVertex::per_vertex().definition(&vs).unwrap();

        let stages = [
            PipelineShaderStageCreateInfo::new(vs),
            PipelineShaderStageCreateInfo::new(fs),
        ];

        let layout = PipelineLayout::new(
            device.clone(),
            PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
                .into_pipeline_layout_create_info(device.clone())
                .unwrap(),
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
    pub fn create_sets(
        &self,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
        camera_buffer: Subbuffer<[glm::Mat4; 2]>,
    ) -> Arc<DescriptorSet> {
        let camera_set = DescriptorSet::new(
            descriptor_set_allocator.clone(),
            self.pipeline.layout().set_layouts()[0].clone(),
            [WriteDescriptorSet::buffer(0, camera_buffer)],
            [],
        )
        .unwrap();

        camera_set
    }
    pub fn render<L>(
        &self,
        builder: &mut AutoCommandBufferBuilder<L>,
        mesh: &SimpleMesh,
        sets: impl DescriptorSetsCollection,
    ) {
        builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                sets,
            )
            .unwrap()
            .bind_vertex_buffers(0, mesh.vbuf.clone())
            .unwrap()
            .bind_index_buffer(mesh.ibuf.clone())
            .unwrap();

        unsafe { builder.draw_indexed(mesh.ilen, 1, 0, 0, 0).unwrap() };
    }
}

pub mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "shaders/simple.vert"
    }
}
mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "shaders/simple.frag"
    }
}
