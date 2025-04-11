use super::{CubeMesh, CubemapVertex};
use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    buffer::Subbuffer,
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::{
        DescriptorSet, DescriptorSetsCollection, WriteDescriptorSet,
        allocator::StandardDescriptorSetAllocator,
    },
    device::{Device, DeviceOwned},
    image::{
        sampler::{Sampler, SamplerCreateInfo},
        view::ImageView,
    },
    memory::allocator::StandardMemoryAllocator,
    pipeline::{
        DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineLayout,
        PipelineShaderStageCreateInfo,
        graphics::{
            GraphicsPipelineCreateInfo,
            color_blend::{ColorBlendAttachmentState, ColorBlendState},
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

#[derive(Clone)]
pub struct CubemapRenderer {
    pub mesh: Arc<CubeMesh>,
    pub pipeline: Arc<GraphicsPipeline>,
}
impl CubemapRenderer {
    pub fn new(
        device: Arc<Device>,
        subpass: Subpass,
        memory_allocator: Arc<StandardMemoryAllocator>,
    ) -> Self {
        let vs = vs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let fs = fs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();

        let vertex_input_state = CubemapVertex::per_vertex().definition(&vs).unwrap();

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
                rasterization_state: Some(RasterizationState {
                    front_face: FrontFace::CounterClockwise,
                    cull_mode: CullMode::None,
                    ..Default::default()
                }),
                multisample_state: Some(MultisampleState::default()),
                color_blend_state: Some(ColorBlendState::with_attachment_states(
                    subpass.num_color_attachments(),
                    ColorBlendAttachmentState::default(),
                )),
                dynamic_state: [DynamicState::Viewport].into_iter().collect(),
                subpass: Some(subpass.into()),
                ..GraphicsPipelineCreateInfo::layout(layout)
            },
        )
        .unwrap();

        let mesh = Arc::new(CubeMesh::new(memory_allocator).unwrap());

        Self { mesh, pipeline }
    }
    pub fn create_sets(
        &self,
        camera_buffer: Subbuffer<[glm::Mat4; 2]>,
        view: Arc<ImageView>,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    ) -> [Arc<DescriptorSet>; 2] {
        let camera_set = DescriptorSet::new(
            descriptor_set_allocator.clone(),
            self.pipeline.layout().set_layouts()[0].clone(),
            [WriteDescriptorSet::buffer(0, camera_buffer)],
            [],
        )
        .unwrap();
        let sampler = Sampler::new(
            descriptor_set_allocator.device().clone(),
            SamplerCreateInfo::simple_repeat_linear_no_mipmap(),
        )
        .unwrap();
        let texture_set = DescriptorSet::new(
            descriptor_set_allocator,
            self.pipeline.layout().set_layouts()[1].clone(),
            [WriteDescriptorSet::image_view_sampler(0, view, sampler)],
            [],
        )
        .unwrap();

        [camera_set, texture_set]
    }
    pub fn render<L>(
        &self,
        builder: &mut AutoCommandBufferBuilder<L>,
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
            .bind_vertex_buffers(0, self.mesh.vbuf.clone())
            .unwrap()
            .bind_index_buffer(self.mesh.ibuf.clone())
            .unwrap();

        unsafe { builder.draw_indexed(self.mesh.ilen, 1, 0, 0, 0).unwrap() };
    }
}

pub mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "shaders/cubemap.vert"
    }
}
mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "shaders/cubemap.frag"
    }
}
