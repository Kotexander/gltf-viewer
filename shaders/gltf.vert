#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 bc_tex;
layout(location = 3) in vec2 rm_tex;
layout(location = 4) in vec2 ao_tex;
layout(location = 5) in vec2 em_tex;
layout(location = 6) in vec2 nm_tex;

layout(location = 7) in vec4 model_x;
layout(location = 8) in vec4 model_y;
layout(location = 9) in vec4 model_z;
layout(location = 10) in vec4 model_w;

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
    mat4 view_inv;
} cam;

layout(location = 0) out vec3 f_position;
layout(location = 1) out vec3 f_normal;
layout(location = 2) out vec2 f_bc_tex;
layout(location = 3) out vec2 f_rm_tex;
layout(location = 4) out vec2 f_ao_tex;
layout(location = 5) out vec2 f_em_tex;
layout(location = 6) out vec2 f_nm_tex;

void main() {
    mat4 model = mat4(model_x, model_y, model_z, model_w);
    vec4 pos = model * vec4(position, 1.0);

    f_position = pos.xyz;
    f_normal = transpose(inverse(mat3(model))) * normal;
    f_bc_tex = bc_tex;
    f_rm_tex = rm_tex;
    f_ao_tex = ao_tex;
    f_em_tex = em_tex;
    f_nm_tex = nm_tex;

    gl_Position = cam.proj * cam.view * pos;
}
