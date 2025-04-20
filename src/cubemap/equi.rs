use super::{CubemapPipelineBuilder, CubemapVertexShader};
use vulkano::device::DeviceOwned;

impl CubemapPipelineBuilder {
    pub fn new_equi(vertex: CubemapVertexShader) -> Self {
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

layout(location = 0) in vec3 v_pos;
layout(set = 1, binding = 0) uniform sampler2D equiTex;

layout(location = 0) out vec4 f_color;

const float PI = 3.14159265358979323846264338327950288;

vec2 sampleSphericalMap(vec3 dir) {
    float phi = atan(dir.z, dir.x);
    float theta = asin(dir.y);
    float u = (phi + PI) / (2.0 * PI);
    float v = (theta + PI / 2.0) / PI;
    return vec2(u, 1.0-v);
}

void main() {
    vec3 dir = normalize(v_pos);
    vec2 uv = sampleSphericalMap(dir);
    vec4 color = texture(equiTex, uv);
    f_color = color;
}

// const vec2 invAtan = vec2(0.1591, 0.3183); // 1/(2PI), 1/PI

// vec2 sampleSphericalMap(vec3 v) {
//     vec2 uv = vec2(atan(v.z, v.x), asin(v.y));
//     uv *= invAtan;
//     uv += 0.5;
//     return uv;
// }

// void main() {
//     vec3 dir = normalize(v_pos);
//     vec2 uv = sampleSphericalMap(dir);
//     vec3 color = texture(equiTex, uv).rgb;
//     f_color = vec4(color, 1.0);
// }
        "#
    }
}
