#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec3 tangent;
layout(location = 3) in vec2 bc_tex;
layout(location = 4) in vec2 rm_tex;
layout(location = 5) in vec2 ao_tex;
layout(location = 6) in vec2 em_tex;
layout(location = 7) in vec2 nm_tex;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
    mat4 view_inv;
} cam;

layout(set = 1, binding = 0) uniform samplerCube envMap;
layout(set = 1, binding = 1) uniform samplerCube spcMap;
layout(set = 1, binding = 2) uniform sampler2D lutMap;

layout(set = 2, binding = 0) uniform Factors {
    vec4 bc;
    vec3 em;
    float ao;
    vec2 rm;
} f;
layout(set = 2, binding = 1) uniform sampler2D bc_sampler;
layout(set = 2, binding = 2) uniform sampler2D rm_sampler;
layout(set = 2, binding = 3) uniform sampler2D ao_sampler;
layout(set = 2, binding = 4) uniform sampler2D em_sampler;
layout(set = 2, binding = 5) uniform sampler2D nm_sampler;

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

vec3 fresnel_shlick(float cos_theta, vec3 f0, float roughness) {
    return f0 + ((1.0 - roughness) - f0) * pow(1.0 - cos_theta, 5.0);
}

vec3 pbr_neutral_tone_mapping(vec3 color) {
    const float startCompression = 0.8 - 0.04;
    const float desaturation = 0.15;

    float x = min(color.r, min(color.g, color.b));
    float offset = x < 0.08 ? x - 6.25 * x * x : 0.04;
    color -= offset;

    float peak = max(color.r, max(color.g, color.b));
    if (peak < startCompression) return color;

    const float d = 1. - startCompression;
    float newPeak = 1. - d * d / (peak + d - startCompression);
    color *= newPeak / peak;

    float g = 1. - 1. / (desaturation * (peak - newPeak) + 1.);
    return mix(color, newPeak * vec3(1, 1, 1), g);
}

void main() {
    vec3 n = normalize(normal);
    vec3 t = normalize(tangent);
    vec3 b = cross(n, t);
    mat3 tbn = mat3(t, b, n);

    float ao = texture(ao_sampler, ao_tex).r * f.ao;
    vec3 albedo = texture(bc_sampler, bc_tex).rgb * f.bc.rgb;
    vec2 rm = texture(rm_sampler, rm_tex).gb * f.rm;
    vec3 em = texture(em_sampler, em_tex).rgb * f.em;

    vec3 V = normalize(cam.view_inv[3].xyz - position);
    vec3 N = tbn * (texture(nm_sampler, nm_tex).rgb * 2.0 - 1.0);
    vec3 R = reflect(-V, N);
    vec3 f0 = mix(vec3(0.04), albedo, rm.y);

    float n_dot_v = max(dot(N, V), 0.0);

    vec3 f = fresnel_shlick(n_dot_v, f0, rm.x);
    vec3 kd = (1.0 - f) * (1.0 - rm.y);

    vec3 diffuse = texture(envMap, N).rgb * albedo * kd;

    const float MAX_REFLECTION_LOD = 4.0;
    vec2 brdf = texture(lutMap, vec2(n_dot_v, rm.x)).rg;
    vec3 specular = textureLod(spcMap, R, rm.x * MAX_REFLECTION_LOD).rgb * (f * brdf.x + brdf.y);

    vec3 ambient = (diffuse + specular) * ao;
    vec3 color = ambient + em;
    f_color = vec4(pbr_neutral_tone_mapping(color), 1.0);
    // f_color = vec4((N + 1.0) / 2.0, 1.0);
}
