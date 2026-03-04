#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_shader_explicit_arithmetic_types : require

// Push constant with root pointer (two 32-bit integers)
layout(push_constant) uniform RootPointer {
    uint root_ptr_lo;
    uint root_ptr_hi;
} pc;

// Define a buffer reference struct matching CPU-side struct
layout(buffer_reference, scalar) buffer RootData {
    uint32_t inValue;
    uint32_t outValue;
};

layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;

void main() {
    // Reconstruct 64-bit pointer from two 32-bit integers
    uint64_t root_ptr = (uint64_t(pc.root_ptr_hi) << 32) | uint64_t(pc.root_ptr_lo);
    
    // Cast root pointer to our struct
    RootData data = RootData(root_ptr);
    
    // Simple copy: outValue = inValue
    // Only first invocation writes to avoid race
    if (gl_GlobalInvocationID.x == 0) {
        data.outValue = data.inValue;
    }
}