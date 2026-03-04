#version 460

layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_color;
layout(location = 0) out vec4 outColor;

void main() {
    vec2 tiled = fract(v_uv * 20.0);
    float grid = step(0.9, tiled.x) + step(0.9, tiled.y);
    vec3 accent = mix(v_color.rgb, vec3(1.0, 1.0, 1.0), min(grid, 1.0) * 0.5);
    outColor = vec4(accent, v_color.a);
}
