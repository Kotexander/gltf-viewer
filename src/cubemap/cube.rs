use super::{CubemapPipelineBuilder, CubemapVertexShader};
use vulkano::device::DeviceOwned;

impl CubemapPipelineBuilder {
    pub fn new_cube(vertex: CubemapVertexShader) -> Self {
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
layout(set = 1, binding = 0) uniform samplerCube cubemap;

layout(location = 0) out vec4 f_color;

void main() {
    f_color = texture(cubemap, v_position);
}
        "#
    }
}
