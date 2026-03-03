#version 450
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_scalar_block_layout : require

layout(buffer_reference, scalar) buffer ColorBlock {
    vec4 color;
};

layout(push_constant) uniform PushConstants {
    ColorBlock ptr;
} push;

layout(location = 0) out vec4 v_color;

void main() {
    vec2 positions[3] = vec2[](
        vec2(-1.0, -1.0),
        vec2(3.0, -1.0),
        vec2(-1.0, 3.0)
    );
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    v_color = push.ptr.color;
}