#version 450

layout(location = 0) in vec3 v_pos;
layout(set = 1, binding = 0) uniform sampler2D texSampler;

layout(location = 0) out vec4 f_color;

const vec2 invAtan = vec2(0.1591, 0.3183);
vec2 SampleSphericalMap(vec3 v) {
    vec2 uv = vec2(atan(v.z, v.x), asin(v.y));
    uv *= invAtan;
    uv += 0.5;
    return uv;
}

void main() {
    vec2 uv = SampleSphericalMap(normalize(v_pos));
    // f_color = vec4(uv, 0.0, 1.0);
    f_color = texture(texSampler, uv);
}
