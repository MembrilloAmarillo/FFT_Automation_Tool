#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_nonuniform_qualifier : require
#extension GL_EXT_descriptor_buffer : require

// Push constant with root pointer
layout(push_constant) uniform RootPointer {
    uint64_t root_ptr;
} pc;

// Define a buffer reference struct for fragment data
layout(buffer_reference, scalar) buffer FragmentData {
    uint texture_index;
    float intensity;
};

// Bindless texture descriptor heap
// Using descriptor buffer extension
layout(set = 0, binding = 0) uniform texture2D textures[];

layout(location = 0) in vec4 v_color;
layout(location = 0) out vec4 out_color;

void main() {
    // Cast root pointer to our fragment data struct
    FragmentData data = FragmentData(pc.root_ptr);
    
    // Default color from vertex shader
    vec4 color = v_color;
    
    // If we have a texture index, sample from texture
    if (data.texture_index > 0) {
        // Use nonuniform qualifier for bindless texture access
        // Note: In a real implementation, we'd need a sampler too
        // color = texture(textures[nonuniformEXT(data.texture_index)], gl_FragCoord.xy / 512.0);
        color = v_color * data.intensity;
    }
    
    out_color = color;
}