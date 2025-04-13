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

#[repr(C)]
#[derive(BufferContents, Vertex)]
struct GltfVertex {
    #[format(R32G32B32_SFLOAT)]
    position: glm::Vec3,
    #[format(R32G32B32_SFLOAT)]
    normal: glm::Vec3,
}

#[derive(Clone)]
pub struct GltfMesh {
    vbuf: Subbuffer<[GltfVertex]>,
    ibuf: Subbuffer<[u32]>,
    ilen: u32,
}
impl GltfMesh {
    pub fn render<L>(self, builder: &mut AutoCommandBufferBuilder<L>) {
        builder.bind_vertex_buffers(0, self.vbuf).unwrap();
        builder.bind_index_buffer(self.ibuf).unwrap();
        unsafe { builder.draw_indexed(self.ilen, 1, 0, 0, 0) }.unwrap();
    }
}

#[derive(Clone)]
pub struct GltfRenderInfo {
    pub meshes: Vec<GltfMesh>,
    pub sets: Arc<DescriptorSet>,
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

        let vertex_input_state = GltfVertex::per_vertex().definition(&vs).unwrap();

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
                    cull_mode: CullMode::None,
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

pub fn load_gltf(alloc: &Arc<StandardMemoryAllocator>, path: &str) -> Vec<GltfMesh> {
    let (document, buffers, _images) = gltf::import(path).unwrap();

    let mut meshes = vec![];
    for mesh in document.meshes() {
        for primative in mesh.primitives() {
            if primative.mode() != gltf::mesh::Mode::Triangles {
                log::warn!("Only triangle primatives supported. Skipping primative.");
                continue;
            }

            let reader =
                primative.reader(|buffer| buffers.get(buffer.index()).map(|d| d.0.as_slice()));

            let vertices = reader
                .read_positions()
                .unwrap()
                .zip(reader.read_normals().unwrap())
                .map(|(pos, norm)| GltfVertex {
                    position: pos.into(),
                    normal: norm.into(),
                });
            let indices = reader.read_indices().unwrap().into_u32();

            let vbuf = Buffer::from_iter(
                alloc.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                vertices,
            )
            .unwrap();
            let ibuf = Buffer::from_iter(
                alloc.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::INDEX_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                indices,
            )
            .unwrap();
            let ilen = ibuf.len() as u32;

            let mesh = GltfMesh { vbuf, ibuf, ilen };
            meshes.push(mesh);
        }
    }
    meshes
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
