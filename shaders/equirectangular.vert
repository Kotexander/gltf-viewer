#version 450

layout(location = 0) in vec3 position;

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} cam;

layout(location = 0) out vec3 f_pos;

void main() {
    gl_Position = (cam.proj * cam.view * vec4(position, 0.0)).xyww;
    f_pos = position;
}
