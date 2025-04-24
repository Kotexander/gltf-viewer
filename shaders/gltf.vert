#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec4 tangent;
layout(location = 3) in vec2 uv_0;
layout(location = 4) in vec2 uv_1;

layout(location = 5) in vec4 model_x;
layout(location = 6) in vec4 model_y;
layout(location = 7) in vec4 model_z;
layout(location = 8) in vec4 model_w;

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
    mat4 view_inv;
} cam;

layout(location = 0) out vec3 f_position;
layout(location = 1) out vec3 f_normal;
layout(location = 2) out vec3 f_tangent;
layout(location = 3) out vec3 f_bitangent;
layout(location = 4) out vec2 f_uv_0;
layout(location = 5) out vec2 f_uv_1;

void main() {
    mat4 model = mat4(model_x, model_y, model_z, model_w);
    mat3 model_inv_t = transpose(inverse(mat3(model)));
    vec4 pos = model * vec4(position, 1.0);

    f_position = pos.xyz;
    f_normal = model_inv_t * normal;
    f_tangent = model_inv_t * tangent.xyz;
    f_bitangent = cross(f_normal, f_tangent);
    f_uv_0 = uv_0;
    f_uv_1 = uv_1;

    gl_Position = cam.proj * cam.view * pos;
}
