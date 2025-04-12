#version 450

layout(location = 0) in vec3 v_normal;

layout(location = 0) out vec4 f_color;

void main() {
    f_color = vec4((normalize(v_normal) + 1.0) / 2.0, 1.0);
    // f_color = vec4(normalize(v_normal), 1.0);
}
