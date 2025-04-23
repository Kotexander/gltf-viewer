use super::{CubemapPipelineBuilder, CubemapVertexShader};
use std::sync::Arc;
use vulkano::{
    descriptor_set::layout::DescriptorSetLayout,
    device::DeviceOwned,
    pipeline::{
        PipelineLayout,
        layout::{PipelineLayoutCreateInfo, PushConstantRange},
    },
    shader::ShaderStages,
};

pub fn filter_pipeline_layout(
    camera_set_layout: Arc<DescriptorSetLayout>,
    texture_set_layout: Arc<DescriptorSetLayout>,
) -> Arc<PipelineLayout> {
    let device = camera_set_layout.device();
    PipelineLayout::new(
        device.clone(),
        PipelineLayoutCreateInfo {
            set_layouts: vec![camera_set_layout, texture_set_layout],
            push_constant_ranges: vec![PushConstantRange {
                stages: ShaderStages::FRAGMENT,
                offset: 0,
                size: 4,
            }],
            ..Default::default()
        },
    )
    .unwrap()
}

impl CubemapPipelineBuilder {
    pub fn new_filt(vertex: CubemapVertexShader) -> Self {
        let device = vertex.vs.module().device();
        let fs = fs::load(device.clone())
            .unwrap()
            .entry_point("main")
            .unwrap();

        Self {
            vs: vertex.vs,
            vis: vertex.vis,
            fs,
        }
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: r#"
#version 450

layout(location = 0) in vec3 v_position;
layout(set = 1, binding = 0) uniform samplerCube envMap;
layout(push_constant) uniform PushConstants {
    float roughness;
} push;

layout(location = 0) out vec4 f_color;

const float PI = 3.14159265358979323846264338327950288;

float distribution_ggx(vec3 N, vec3 H, float roughness) {
    float a = roughness*roughness;
    float a2 = a*a;
    float n_dot_h = max(dot(N, H), 0.0);
    float n_dot_h2 = n_dot_h*n_dot_h;

    float num   = a2;
    float denum = (n_dot_h2 * (a2 - 1.0) + 1.0);
    denum = PI * denum * denum;

    return num / denum;
}
float radical_inverse_vdc(uint bits) {
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return float(bits) * 2.3283064365386963e-10; // / 0x100000000
}
vec2 hammersley(uint i, uint N){
    return vec2(float(i)/float(N), radical_inverse_vdc(i));
}
vec3 importance_sample_ggx(vec2 Xi, vec3 N, float roughness) {
	float a = roughness*roughness;

	float phi = 2.0 * PI * Xi.x;
	float cosTheta = sqrt((1.0 - Xi.y) / (1.0 + (a*a - 1.0) * Xi.y));
	float sinTheta = sqrt(1.0 - cosTheta*cosTheta);

	vec3 H;
	H.x = cos(phi) * sinTheta;
	H.y = sin(phi) * sinTheta;
	H.z = cosTheta;

	vec3 up          = abs(N.z) < 0.99999 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);
	vec3 tangent   = normalize(cross(up, N));
	vec3 bitangent = cross(N, tangent);

	vec3 sample_vec = tangent * H.x + bitangent * H.y + N * H.z;
	return normalize(sample_vec);
}

void main(){
    vec3 N = normalize(v_position);
    vec3 R = N;
    vec3 V = R;

    const uint SAMPLE_COUNT = 1024u;
    vec3 prefiltered_color = vec3(0.0);
    float total_weight = 0.0;

    for(uint i = 0u; i < SAMPLE_COUNT; i++){
        vec2 Xi = hammersley(i, SAMPLE_COUNT);
        vec3 H = importance_sample_ggx(Xi, N, push.roughness);
        vec3 L  = normalize(2.0 * dot(V, H) * H - V);

        float n_dot_l = max(dot(N, L), 0.0);
        if(n_dot_l > 0.0) {
            float D   = distribution_ggx(N, H, push.roughness);
            float n_dot_h = max(dot(N, H), 0.0);
            float h_dot_v = max(dot(H, V), 0.0);
            float pdf = D * n_dot_h / (4.0 * h_dot_v) + 0.0001;

            float resolution = 512.0; // resolution of source cubemap (per face)
            float sa_texel  = 4.0 * PI / (6.0 * resolution * resolution);
            float sa_sample = 1.0 / (float(SAMPLE_COUNT) * pdf + 0.0001);

            float mipLevel = push.roughness == 0.0 ? 0.0 : 0.5 * log2(sa_sample / sa_texel);

            prefiltered_color += textureLod(envMap, L, mipLevel).rgb * n_dot_l;
            total_weight      += n_dot_l;
        }
    }

    prefiltered_color /= total_weight;

    f_color = vec4(prefiltered_color, 1.0);
}
        "#
    }
}
