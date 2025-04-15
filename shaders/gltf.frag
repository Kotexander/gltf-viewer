#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 bc_tex;
layout(location = 3) in vec2 rm_tex;
layout(location = 4) in vec2 ao_tex;
layout(location = 5) in vec2 em_tex;
layout(location = 6) in vec2 nm_tex;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0, std140) uniform Camera {
    mat4 view;
    mat4 proj;
    mat4 view_inv;
} cam;

layout(set = 1, binding = 0) uniform sampler2D bc_sampler;
layout(set = 1, binding = 1) uniform sampler2D rm_sampler;
layout(set = 1, binding = 2) uniform sampler2D ao_sampler;
layout(set = 1, binding = 3) uniform sampler2D em_sampler;
layout(set = 1, binding = 4) uniform sampler2D nm_sampler;

const float PI = 3.14159265358979323846264338327950288;

float distribution_ggx(float n_dot_h, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float n_dot_h2 = n_dot_h * n_dot_h;

    float num = a2;
    float denum = (n_dot_h2 * (a2 - 1.0) + 1.0);
    denum = PI * denum * denum;

    return num / denum;
}

float geometry_shlick_ggx(float n_dot_v, float k) {
    float num = n_dot_v;
    float denum = n_dot_v * (1.0 - k) + k;

    return num / denum;
}
float geometry_smith(float n_dot_v, float n_dot_l, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    float ggx1 = geometry_shlick_ggx(n_dot_v, k);
    float ggx2 = geometry_shlick_ggx(n_dot_l, k);

    return ggx1 * ggx2;
}

vec3 fresnel_shlick(float cos_theta, vec3 f0) {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}

void main() {
    // f_color = vec4((normalize(normal) + 1.0) / 2.0, 1.0);
    // f_color = vec4(normalize(normal), 1.0);

    // f_color = texture(bc_sampler, bc_tex);
    // f_color = texture(rm_sampler, rm_tex);
    // f_color = texture(ao_sampler, ao_tex);
    // f_color = texture(em_sampler, em_tex);
    // f_color = texture(nm_sampler, nm_tex);

    float ao = texture(ao_sampler, ao_tex).r;
    vec3 albedo = texture(bc_sampler, bc_tex).rgb;
    vec2 rm = texture(rm_sampler, rm_tex).gb;
    vec3 em = texture(em_sampler, em_tex).rgb;

    vec3 N = normalize(normal);
    vec3 V = normalize(cam.view_inv[3].xyz - position);
    vec3 f0 = mix(vec3(0.04), albedo, rm.y);

    vec3 light_colour = vec3(10.0);
    vec3 L = normalize(vec3(1.0, 1.0, 1.0));
    vec3 H = normalize(L + V);

    float n_dot_v = max(dot(N, V), 0.0000001);
    float n_dot_l = max(dot(N, L), 0.0000001);
    float n_dot_h = max(dot(N, H), 0.0);
    float h_dot_v = max(dot(H, V), 0.0);

    float d = distribution_ggx(n_dot_h, rm.x);
    float g = geometry_smith(n_dot_v, n_dot_l, rm.x);
    vec3 f = fresnel_shlick(h_dot_v, f0);

    vec3 specular_num = d * g * f;
    float specular_denum = 4.0 * n_dot_v * n_dot_l;
    vec3 specular = specular_num / specular_denum;

    vec3 kd = vec3(1.0) - f;
    kd *= 1.0 - rm.y;

    vec3 lambert = albedo / PI;

    vec3 brdf = kd * lambert + specular;
    vec3 color = brdf * light_colour * n_dot_l + em;
    vec3 ambient = vec3(0.5) * albedo * ao;
    color += ambient;
    vec3 tone_map = color / (color + vec3(1.0));
    f_color = vec4(tone_map, 1.0);
}
