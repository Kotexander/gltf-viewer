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

mod mesh;
pub mod renderer;

pub use mesh::CubeMesh;

pub struct CubemapShaders {
    pub vs: EntryPoint,
    pub vertex_input_state: VertexInputState,
    pub equi_fs: EntryPoint,
    pub cube_fs: EntryPoint,
    pub conv_fs: EntryPoint,
}
impl CubemapShaders {
    pub fn new(device: Arc<Device>) -> Self {
        let vs = vs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let vertex_input_state = CubemapVertex::per_vertex().definition(&vs).unwrap();

        let cube_fs = cube_fs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let equi_fs = equi_fs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let conv_fs = conv_fs::load(device).unwrap().entry_point("main").unwrap();

        Self {
            vs,
            vertex_input_state,
            equi_fs,
            cube_fs,
            conv_fs,
        }
    }
}

#[derive(Clone)]
pub struct CubemapPipelineLayout {
    pub layout: Arc<PipelineLayout>,
}
impl CubemapPipelineLayout {
    pub fn new(
        device: Arc<Device>,
        camera_layout: Arc<DescriptorSetLayout>,
        texture_layout: Arc<DescriptorSetLayout>,
    ) -> Self {
        let layout = PipelineLayout::new(
            device,
            PipelineLayoutCreateInfo {
                set_layouts: vec![camera_layout, texture_layout],
                ..Default::default()
            },
        )
        .unwrap();

        Self { layout }
    }
    pub fn create_pipeline(
        self,
        vs: EntryPoint,
        fs: EntryPoint,
        vertex_input_state: VertexInputState,
        subpass: Subpass,
    ) -> Arc<GraphicsPipeline> {
        let stages = [
            PipelineShaderStageCreateInfo::new(vs),
            PipelineShaderStageCreateInfo::new(fs),
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
            self.layout.device().clone(),
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
                depth_stencil_state,
                dynamic_state: [DynamicState::Viewport, DynamicState::Scissor]
                    .into_iter()
                    .collect(),
                subpass: Some(subpass.into()),
                ..GraphicsPipelineCreateInfo::layout(self.layout)
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
layout(set = 1, binding = 0) uniform samplerCube cubemap;

layout(location = 0) out vec4 f_color;

void main() {
    f_color = texture(cubemap, v_position);
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
layout(set = 1, binding = 0) uniform sampler2D equiTex;

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
    vec4 color = texture(equiTex, uv);
    // f_color = color / (color + 1);

    // f_color = vec4(pow(color.rgb, vec3(1.0/2.2)), color.a);
    f_color = color;
}
        "#
    }
}
mod conv_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: r#"
#version 450

layout(location = 0) in vec3 v_position;
layout(set = 1, binding = 0) uniform samplerCube envMap;

layout(location = 0) out vec4 f_color;

const float PI = 3.14159265358979323846264338327950288;

void main() {
    vec3 N = normalize(v_position);
    vec3 irradiance = vec3(0.0);

    vec3 up    = vec3(0.0, 1.0, 0.0);
    vec3 right = normalize(cross(up, N));
    up         = normalize(cross(N, right));

    float samples = 0.0;
    for(float phi = 0.0; phi < 2.0 * PI; phi += 0.01){
        float cos_phi = cos(phi);
        float sin_phi = sin(phi);

        for(float theta = 0.0; theta < 0.5 * PI; theta += 0.01){
            float cos_theta = cos(theta);
            float sin_theta = sin(theta);

            vec3 temp = cos_phi * right + sin_phi * up;
            vec3 sample_dir = cos_theta * N + sin_theta * temp;
            irradiance += texture(envMap, sample_dir).rgb * cos_theta * sin_theta;
            samples += 1.0;
        }
    }
    irradiance = PI * irradiance / samples;
    f_color = vec4(irradiance, 1.0);
}
        "#
    }
}
