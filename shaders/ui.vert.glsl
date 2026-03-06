#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_shader_explicit_arithmetic_types : require

struct Vertex {
    vec2 position;
    vec2 uv;
    uint color;
};

layout(push_constant) uniform PushConstants {
    uint64_t vertex_ptr;
    float window_width;
    float window_height;
    uint padding;
} pc;

layout(buffer_reference, scalar) readonly buffer VertexBuffer {
    Vertex data[];
};

layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec4 v_color;

void main() {
    uint idx = uint(gl_VertexIndex);// % 6u;

    VertexBuffer vb = VertexBuffer(pc.vertex_ptr);
    Vertex v = vb.data[idx];

    // Normalize screen-space coordinates to NDC (-1 to 1)
    // egui coordinates: (0,0) at top-left, (width,height) at bottom-right
    // Vulkan NDC: (-1,-1) at bottom-left, (1,1) at top-right
    // Need to flip Y axis
    vec2 ndc = vec2(
        (2.0 * v.position.x / pc.window_width) - 1.0,
        1.0 - (2.0 * v.position.y / pc.window_height)
    );

    gl_Position = vec4(ndc, 0.0, 1.0);

    v_uv = v.uv;
    v_color = unpackUnorm4x8(v.color);
}
