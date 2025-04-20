pub fn generate_lut() {
    todo!()
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: r#"
#version 450

const vec2 pos[] = {
    vec2(-1.0, -1.0),
    vec2(3.0, -1.0),
    vec2(-1.0, 3.0),
};

layout(location = 0) out vec2 uv;

void main() {
    gl_Position = vec4(pos[gl_VertexIndex], 0.0, 1.0);
    uv = (pos[gl_VertexIndex] + 1.0) / 2.0;
}
        "#
    }
}
mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: r#"
#version 450

layout(location = 0) out vec2 f_color;
layout(location = 0) in vec2 uv;

const float PI = 3.14159265358979323846264338327950288;

float radical_inverse_vdc(uint bits) {
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return float(bits) * 2.3283064365386963e-10; // / 0x100000000
}
vec2 hammersley(uint i, uint N){
    return vec2(float(i)/float(N), radical_inverse_vdc(i));
}
vec3 importance_sample_ggx(vec2 Xi, vec3 N, float roughness) {
	float a = roughness*roughness;

	float phi = 2.0 * PI * Xi.x;
	float cosTheta = sqrt((1.0 - Xi.y) / (1.0 + (a*a - 1.0) * Xi.y));
	float sinTheta = sqrt(1.0 - cosTheta*cosTheta);

	vec3 H;
	H.x = cos(phi) * sinTheta;
	H.y = sin(phi) * sinTheta;
	H.z = cosTheta;

	vec3 up          = abs(N.z) < 0.99999 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);
	vec3 tangent   = normalize(cross(up, N));
	vec3 bitangent = cross(N, tangent);

	vec3 sample_vec = tangent * H.x + bitangent * H.y + N * H.z;
	return normalize(sample_vec);
}

float geometry_shlick_ggx(float n_dot_v, float k) {
    float num = n_dot_v;
    float denum = n_dot_v * (1.0 - k) + k;

    return num / denum;
}
float geometry_smith(float n_dot_v, float n_dot_l, float roughness) {
    float r = roughness;
    float k = (r * r) / 2.0;
    float ggx1 = geometry_shlick_ggx(n_dot_v, k);
    float ggx2 = geometry_shlick_ggx(n_dot_l, k);

    return ggx1 * ggx2;
}

vec2 integrate_brdf(float n_dot_v, float roughness){
    vec3 V;
    V.x = sqrt(1.0 - n_dot_v*n_dot_v);
    V.y = 0.0;
    V.z = n_dot_v;

    float A = 0.0;
    float B = 0.0;

    vec3 N = vec3(0.0, 0.0, 1.0);

    const uint SAMPLE_COUNT = 1024u;
    for(uint i = 0u; i < SAMPLE_COUNT; ++i) {
        vec2 Xi = hammersley(i, SAMPLE_COUNT);
        vec3 H  = importance_sample_ggx(Xi, N, roughness);
        vec3 L  = normalize(2.0 * dot(V, H) * H - V);

        float n_dot_l = max(L.z, 0.0);
        float n_dot_h = max(H.z, 0.0);
        float v_dot_h = max(dot(V, H), 0.0);

        if(n_dot_l > 0.0) {
            float G = geometry_smith(n_dot_v, n_dot_l, roughness);
            float G_Vis = (G * v_dot_h) / (n_dot_h * n_dot_v);
            float Fc = pow(1.0 - v_dot_h, 5.0);

            A += (1.0 - Fc) * G_Vis;
            B += Fc * G_Vis;
        }
    }
    A /= float(SAMPLE_COUNT);
    B /= float(SAMPLE_COUNT);
    return vec2(A, B);
}

void main() {
    vec2 integrated_brdf = integrate_brdf(uv.x, uv.y);
    f_color = vec2(integrated_brdf);
}
        "#
    }
}
