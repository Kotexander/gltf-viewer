use nalgebra_glm as glm;
use std::sync::Arc;
use vulkano::{
    Validated,
    buffer::{
        AllocateBufferError, Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer,
    },
    command_buffer::AutoCommandBufferBuilder,
    descriptor_set::{DescriptorSetsCollection, layout::DescriptorSetLayout},
    device::DeviceOwned,
    image::SampleCount,
    memory::allocator::{
        AllocationCreateInfo, MemoryAllocator, MemoryTypeFilter, StandardMemoryAllocator,
    },
    pipeline::{
        DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineLayout,
        PipelineShaderStageCreateInfo,
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
};

#[repr(C)]
#[derive(BufferContents, Vertex)]
struct CubemapVertex {
    #[format(R32G32B32_SFLOAT)]
    position: glm::Vec3,
}

#[rustfmt::skip]
const VERTICES: [CubemapVertex; 8] = [
    CubemapVertex { position: glm::Vec3::new(-0.5, -0.5, -0.5) },
    CubemapVertex { position: glm::Vec3::new( 0.5, -0.5, -0.5) },
    CubemapVertex { position: glm::Vec3::new( 0.5,  0.5, -0.5) },
    CubemapVertex { position: glm::Vec3::new(-0.5,  0.5, -0.5) },
    CubemapVertex { position: glm::Vec3::new(-0.5, -0.5,  0.5) },
    CubemapVertex { position: glm::Vec3::new( 0.5, -0.5,  0.5) },
    CubemapVertex { position: glm::Vec3::new( 0.5,  0.5,  0.5) },
    CubemapVertex { position: glm::Vec3::new(-0.5,  0.5,  0.5) },
];

#[rustfmt::skip]
const INDICES: [u16; 36] = [
    // back face (z+)
    6, 5, 4,
    4, 7, 6,

    // front face (z-)
    2, 3, 0,
    0, 1, 2,

    // left face (x-)
    7, 4, 0,
    0, 3, 7,

    // right face (x+)
    6, 2, 1,
    1, 5, 6,

    // top face (y+)
    6, 7, 3,
    3, 2, 6,

    // bottom face (y-)
    5, 1, 0,
    0, 4, 5,
];

pub struct CubeMesh {
    vbuf: Subbuffer<[CubemapVertex]>,
    ibuf: Subbuffer<[u16]>,
    ilen: u32,
}
impl CubeMesh {
    pub fn new(
        allocator: Arc<dyn MemoryAllocator>,
    ) -> Result<Self, Validated<AllocateBufferError>> {
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
            VERTICES,
        )?;
        let ibuf = Buffer::from_iter(
            allocator,
            BufferCreateInfo {
                usage: BufferUsage::INDEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            INDICES,
        )?;

        Ok(Self {
            vbuf,
            ibuf,
            ilen: INDICES.len() as u32,
        })
    }
}

#[derive(Clone)]
pub struct CubemapPipeline {
    pub mesh: Arc<CubeMesh>,
    pub cube_pipeline: Arc<GraphicsPipeline>,
    pub equi_pipeline: Arc<GraphicsPipeline>,
}
impl CubemapPipeline {
    pub fn new(
        alloc: Arc<StandardMemoryAllocator>,
        set_layouts: Vec<Arc<DescriptorSetLayout>>,
        subpass: Subpass,
    ) -> Self {
        let vs = vs::load(alloc.device().clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let cube_fs = cube_fs::load(alloc.device().clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let equi_fs = equi_fs::load(alloc.device().clone())
            .unwrap()
            .entry_point("main")
            .unwrap();

        let layout = PipelineLayout::new(
            alloc.device().clone(),
            PipelineLayoutCreateInfo {
                set_layouts,
                ..Default::default()
            },
        )
        .unwrap();

        let vertex_input_state = CubemapVertex::per_vertex().definition(&vs).unwrap();

        let cube_stages = [
            PipelineShaderStageCreateInfo::new(vs.clone()),
            PipelineShaderStageCreateInfo::new(cube_fs),
        ];
        let equi_stages = [
            PipelineShaderStageCreateInfo::new(vs),
            PipelineShaderStageCreateInfo::new(equi_fs),
        ];

        let equi_pipeline = Self::create_graphics_pipeline(
            &alloc,
            subpass.clone(),
            vertex_input_state.clone(),
            equi_stages,
            layout.clone(),
        );
        let cube_pipeline = Self::create_graphics_pipeline(
            &alloc,
            subpass,
            vertex_input_state,
            cube_stages,
            layout,
        );

        let mesh = Arc::new(CubeMesh::new(alloc).unwrap());

        Self {
            mesh,
            cube_pipeline,
            equi_pipeline,
        }
    }

    pub fn render_cube<L>(
        &self,
        builder: &mut AutoCommandBufferBuilder<L>,
        sets: impl DescriptorSetsCollection,
    ) {
        builder
            .bind_pipeline_graphics(self.cube_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.cube_pipeline.layout().clone(),
                1,
                sets,
            )
            .unwrap()
            .bind_vertex_buffers(0, self.mesh.vbuf.clone())
            .unwrap()
            .bind_index_buffer(self.mesh.ibuf.clone())
            .unwrap();

        unsafe { builder.draw_indexed(self.mesh.ilen, 1, 0, 0, 0).unwrap() };
    }
    pub fn render_equi<L>(
        &self,
        builder: &mut AutoCommandBufferBuilder<L>,
        sets: impl DescriptorSetsCollection,
    ) {
        builder
            .bind_pipeline_graphics(self.equi_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.equi_pipeline.layout().clone(),
                1,
                sets,
            )
            .unwrap()
            .bind_vertex_buffers(0, self.mesh.vbuf.clone())
            .unwrap()
            .bind_index_buffer(self.mesh.ibuf.clone())
            .unwrap();

        unsafe { builder.draw_indexed(self.mesh.ilen, 1, 0, 0, 0).unwrap() };
    }

    fn create_graphics_pipeline(
        alloc: &Arc<StandardMemoryAllocator>,
        subpass: Subpass,
        vertex_input_state: VertexInputState,
        stages: [PipelineShaderStageCreateInfo; 2],
        layout: Arc<PipelineLayout>,
    ) -> Arc<GraphicsPipeline> {
        GraphicsPipeline::new(
            alloc.device().clone(),
            None,
            GraphicsPipelineCreateInfo {
                stages: stages.into_iter().collect(),
                vertex_input_state: Some(vertex_input_state),
                input_assembly_state: Some(InputAssemblyState::default()),
                viewport_state: Some(ViewportState::default()),
                rasterization_state: Some(RasterizationState {
                    front_face: FrontFace::CounterClockwise,
                    cull_mode: CullMode::Back,
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
                depth_stencil_state: Some(DepthStencilState {
                    depth: Some(DepthState {
                        write_enable: true,
                        compare_op: CompareOp::LessOrEqual,
                    }),
                    ..Default::default()
                }),
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
    f_position = vec3(-position.x, position.y, position.z);
}
        "#
    }
}
mod cube_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: r#"
#version 450

layout(location = 0) in vec3 v_position;
layout(set = 1, binding = 0) uniform samplerCube texSampler;

layout(location = 0) out vec4 f_color;

void main() {
    f_color = texture(texSampler, v_position);
}
        "#
    }
}
mod equi_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: r#"
#version 450

layout(location = 0) in vec3 v_pos;
layout(set = 1, binding = 0) uniform sampler2D texSampler;

layout(location = 0) out vec4 f_color;

const float PI = 3.14159265358979323846264338327950288;

vec2 sampleSphericalMap(vec3 dir) {
    float phi = atan(dir.z, dir.x);
    float theta = asin(dir.y);
    float u = (phi + PI) / (2.0 * PI);
    float v = (theta + PI / 2.0) / PI;
    return 1.0 - vec2(u, v);
}

void main() {
    vec3 dir = normalize(v_pos);
    vec2 uv = sampleSphericalMap(dir);
    vec4 color = texture(texSampler, uv);
    // f_color = color / (color + 1);

    f_color = vec4(pow(color.rgb, vec3(1.0/2.2)), color.a);
}
        "#
    }
}
