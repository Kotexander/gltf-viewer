#version 450

layout(location = 0) in vec3 normal;
layout(location = 1) in vec2 bc_tex;
layout(location = 2) in vec2 rm_tex;
layout(location = 3) in vec2 ao_tex;
layout(location = 4) in vec2 em_tex;
layout(location = 5) in vec2 nm_tex;

layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 0) uniform sampler2D bc_sampler;
layout(set = 1, binding = 1) uniform sampler2D rm_sampler;
layout(set = 1, binding = 2) uniform sampler2D ao_sampler;
layout(set = 1, binding = 3) uniform sampler2D em_sampler;
layout(set = 1, binding = 4) uniform sampler2D nm_sampler;

void main() {
    // f_color = vec4((normalize(normal) + 1.0) / 2.0, 1.0);
    // f_color = vec4(normalize(normal), 1.0);

    f_color = texture(bc_sampler, bc_tex);
    // f_color = texture(rm_sampler, rm_tex);
    // f_color = texture(ao_sampler, ao_tex);
    // f_color = texture(em_sampler, em_tex);
    // f_color = texture(nm_sampler, nm_tex);
}
