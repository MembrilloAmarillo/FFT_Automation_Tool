#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_shader_explicit_arithmetic_types : require

// Push constant with root pointer (using 64-bit integer)
// Note: uint64_t requires GL_EXT_shader_explicit_arithmetic_types
layout(push_constant) uniform RootPointer {
    uint root_ptr_lo;
    uint root_ptr_hi;
} pc;

// Define a buffer reference struct for vertex data
layout(buffer_reference, scalar) buffer VertexData {
    vec2 positions[3];
    vec4 color;
};

layout(location = 0) out vec4 v_color;

void main() {
    // Reconstruct 64-bit pointer from two 32-bit integers
    uint64_t root_ptr = (uint64_t(pc.root_ptr_hi) << 32) | uint64_t(pc.root_ptr_lo);
    
    // Cast root pointer to our vertex data struct
    VertexData data = VertexData(root_ptr);
    
    vec2 positions[3] = vec2[](
        vec2(-1.0, -1.0),
        vec2(3.0, -1.0),
        vec2(-1.0, 3.0)
    );
    
    // Use positions from buffer if available, otherwise use hardcoded
    if (data.positions[0].x != 0.0 || data.positions[0].y != 0.0) {
        gl_Position = vec4(data.positions[gl_VertexIndex], 0.0, 1.0);
    } else {
        gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    }
    
    v_color = data.color;
}