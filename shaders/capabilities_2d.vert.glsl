#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_shader_explicit_arithmetic_types : require

layout(push_constant) uniform RootPointer {
    uint root_ptr_lo;
    uint root_ptr_hi;
} pc;

struct Vertex {
    vec3 pos;
    vec3 normal;
    vec2 uv;
};

layout(buffer_reference, scalar) readonly buffer VertexBuffer {
    Vertex data[];
};

layout(buffer_reference, scalar) readonly buffer DrawData2D {
    uint64_t vertex_ptr;
    vec4 color;
    mat4 mvp;
};

layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec4 v_color;

void main() {
    uint64_t root_ptr = (uint64_t(pc.root_ptr_hi) << 32) | uint64_t(pc.root_ptr_lo);
    DrawData2D draw = DrawData2D(root_ptr);
    VertexBuffer vb = VertexBuffer(draw.vertex_ptr);

    Vertex v = vb.data[gl_VertexIndex];
    v_uv = v.uv;
    v_color = draw.color;

    gl_Position = draw.mvp * vec4(v.pos, 1.0);
}
