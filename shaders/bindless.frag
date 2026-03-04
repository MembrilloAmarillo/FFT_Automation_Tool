#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_shader_explicit_arithmetic_types : require

// Push constant with root pointer (using two 32-bit integers)
layout(push_constant) uniform RootPointer {
    uint root_ptr_lo;
    uint root_ptr_hi;
} pc;

// Define a buffer reference struct for fragment data
layout(buffer_reference, scalar) buffer FragmentData {
    vec4 tint_color;
    float intensity;
};

layout(location = 0) in vec4 v_color;
layout(location = 0) out vec4 out_color;

void main() {
    // Reconstruct 64-bit pointer from two 32-bit integers
    uint64_t root_ptr = (uint64_t(pc.root_ptr_hi) << 32) | uint64_t(pc.root_ptr_lo);
    
    // Cast root pointer to our fragment data struct
    FragmentData data = FragmentData(root_ptr);
    
    // Apply tint and intensity from root data
    out_color = v_color * data.tint_color * data.intensity;
}