#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 bc_tex;
layout(location = 3) in vec2 rm_tex;
layout(location = 4) in vec2 ao_tex;

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} cam;

layout(location = 0) out vec3 f_normal;
layout(location = 1) out vec2 f_bc_tex;
layout(location = 2) out vec2 f_rm_tex;
layout(location = 3) out vec2 f_ao_tex;

void main() {
    gl_Position = cam.proj * cam.view * vec4(position, 1.0);
    f_normal = normal;
    f_bc_tex = bc_tex;
    f_rm_tex = rm_tex;
    f_ao_tex = ao_tex;
}
