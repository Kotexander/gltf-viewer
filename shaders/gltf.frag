#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec3 tangent;
layout(location = 3) in vec3 bitangent;
layout(location = 4) in vec2 uv_0;
layout(location = 5) in vec2 uv_1;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
    mat4 view_inv;
} cam;

layout(set = 1, binding = 0) uniform samplerCube envMap;
layout(set = 1, binding = 1) uniform samplerCube spcMap;
layout(set = 1, binding = 2) uniform sampler2D lutMap;

layout(push_constant) uniform Material {
    vec4 bc;
    vec3 em;
    float ao;
    vec2 rm;
    float nm;

    int bc_set;
    int rm_set;
    int ao_set;
    int em_set;
    int nm_set;
} m;
layout(set = 2, binding = 0) uniform sampler2D bc_sampler;
layout(set = 2, binding = 1) uniform sampler2D rm_sampler;
layout(set = 2, binding = 2) uniform sampler2D ao_sampler;
layout(set = 2, binding = 3) uniform sampler2D em_sampler;
layout(set = 2, binding = 4) uniform sampler2D nm_sampler;

vec2 get_uv(uint set) {
    if (set == 0) {
        return uv_0;
    }
    else {
        return uv_1;
    }
}
vec4 get_base_color() {
    vec4 bc = vec4(1.0);
    if (m.bc_set >= 0) {
        bc = texture(bc_sampler, get_uv(m.bc_set));
    }
    return bc * m.bc;
}
vec2 get_roughness_metallic() {
    vec2 rm = vec2(1.0);
    if (m.rm_set >= 0) {
        rm = texture(rm_sampler, get_uv(m.rm_set)).gb;
    }
    return rm * m.rm;
}
float get_ambient_occlusion() {
    float ao = 1.0;
    if (m.ao_set >= 0) {
        ao = texture(ao_sampler, get_uv(m.ao_set)).r;
    }
    return 1.0 + m.ao * (ao - 1.0);
}
vec3 get_emmissive() {
    vec3 em = vec3(1.0);
    if (m.em_set >= 0) {
        em = texture(em_sampler, get_uv(m.em_set)).rgb;
    }
    return em * m.em;
}
vec3 get_normal() {
    vec3 n = normalize(normal);
    if (m.nm_set >= 0) {
        vec3 t = normalize(tangent);
        vec3 b = normalize(bitangent);
        mat3 tbn = mat3(t, b, n);
        vec3 nm = (texture(nm_sampler, get_uv(m.nm_set)).rgb * 2.0 - 1.0);
        nm.xy *= m.nm;
        return tbn * normalize(nm);
    }
    return n;
}

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
    vec3 bc = get_base_color().rgb;
    float ao = get_ambient_occlusion();
    vec2 rm = get_roughness_metallic();
    vec3 em = get_emmissive();

    vec3 N = get_normal();
    vec3 V = normalize(cam.view_inv[3].xyz - position);
    vec3 R = reflect(-V, N);
    vec3 f0 = mix(vec3(0.04), bc, rm.y);

    float n_dot_v = max(dot(N, V), 0.0);

    vec3 f = fresnel_shlick(n_dot_v, f0, rm.x);
    vec3 kd = (1.0 - f) * (1.0 - rm.y);

    vec3 diffuse = texture(envMap, N).rgb * bc * kd;

    const float MAX_REFLECTION_LOD = 4.0;
    vec2 brdf = texture(lutMap, vec2(n_dot_v, rm.x)).rg;
    vec3 specular = textureLod(spcMap, R, rm.x * MAX_REFLECTION_LOD).rgb * (f * brdf.x + brdf.y);

    vec3 ambient = (diffuse + specular) * ao;
    vec3 color = ambient + em;
    f_color = vec4(pbr_neutral_tone_mapping(color), 1.0);

    // vec3 t = normalize(tangent);
    // vec3 b = normalize(bitangent);
    // f_color = vec4(t / 2.0 + 0.5, 1.0);
}
