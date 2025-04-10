#version 450

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) in vec3 position;

layout(location = 0) out vec3 f_pos;

void main() {
    gl_Position = (ubo.proj * ubo.view * vec4(position, 0.0)).xyww;
    f_pos = position;
}
