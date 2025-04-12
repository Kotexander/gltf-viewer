#version 450

layout(location = 0) in vec3 v_position;
layout(set = 1, binding = 0) uniform samplerCube texSampler;

layout(location = 0) out vec4 f_color;

void main() {
    f_color = texture(texSampler, v_position);
}
