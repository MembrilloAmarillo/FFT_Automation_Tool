#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_nonuniform_qualifier : require
#extension GL_EXT_descriptor_buffer : require

// Push constant with root pointer
layout(push_constant) uniform RootPointer {
    uint64_t root_ptr;
} pc;

// Define a buffer reference struct for vertex data
layout(buffer_reference, scalar) buffer VertexData {
    vec2 positions[3];
    vec4 color;
};

layout(location = 0) out vec4 v_color;

void main() {
    // Cast root pointer to our vertex data struct
    VertexData data = VertexData(pc.root_ptr);
    
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