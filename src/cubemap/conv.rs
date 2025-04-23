use super::{CubemapPipelineBuilder, CubemapVertexShader};
use vulkano::device::DeviceOwned;

impl CubemapPipelineBuilder {
    pub fn new_conv(vertex: CubemapVertexShader) -> Self {
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
    irradiance *= PI / samples;
    f_color = vec4(irradiance, 1.0);
}
        "#
    }
}
