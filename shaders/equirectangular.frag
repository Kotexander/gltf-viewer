#version 450

layout(location = 0) in vec3 v_pos;
layout(set = 1, binding = 0) uniform sampler2D texSampler;

layout(location = 0) out vec4 f_color;

const float PI = 3.14159265358979323846264338327950288;

vec2 sampleSphericalMap(vec3 dir) {
    float phi = atan(dir.z, dir.x);
    float theta = asin(dir.y);
    float u = (phi + PI) / (2.0 * PI);
    float v = (theta + PI / 2.0) / PI;
    return 1.0 - vec2(u, v);
}

void main() {
    vec3 dir = normalize(v_pos);
    vec2 uv = sampleSphericalMap(dir);
    // vec4 color = texture(texSampler, uv);
    // f_color = color / (color + 1);

    f_color = texture(texSampler, uv);
}
