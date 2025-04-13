#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} cam;

layout(location = 0) out vec3 f_normal;

void main() {
    gl_Position = cam.proj * cam.view * vec4(position, 1.0);
    f_normal = normal;
}
