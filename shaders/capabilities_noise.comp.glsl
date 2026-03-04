#version 460
#extension GL_EXT_buffer_reference : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_shader_explicit_arithmetic_types : require

layout(push_constant) uniform RootPointer {
    uint root_ptr_lo;
    uint root_ptr_hi;
} pc;

layout(buffer_reference, scalar) buffer PixelBuffer {
    uint data[];
};

layout(buffer_reference, scalar) readonly buffer NoiseParams {
    uint64_t out_ptr;
    uint width;
    uint height;
    uint mode;
    float time;
};

layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

float value_noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);

    float a = hash12(i + vec2(0.0, 0.0));
    float b = hash12(i + vec2(1.0, 0.0));
    float c = hash12(i + vec2(0.0, 1.0));
    float d = hash12(i + vec2(1.0, 1.0));

    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm(vec2 p) {
    float sum = 0.0;
    float amp = 0.5;
    for (int i = 0; i < 6; ++i) {
        sum += amp * value_noise(p);
        p *= 2.02;
        amp *= 0.5;
    }
    return sum;
}

void main() {
    uint64_t root_ptr = (uint64_t(pc.root_ptr_hi) << 32) | uint64_t(pc.root_ptr_lo);
    NoiseParams params = NoiseParams(root_ptr);

    uint x = gl_GlobalInvocationID.x;
    uint y = gl_GlobalInvocationID.y;
    if (x >= params.width || y >= params.height) {
        return;
    }

    vec2 uv = vec2(float(x) / float(params.width), float(y) / float(params.height));
    vec3 color;

    if (params.mode == 0u) {
        // Value noise marble
        float n = value_noise(uv * 18.0 + vec2(params.time * 0.2, params.time * 0.15));
        float m = sin((uv.x + n * 0.75) * 40.0);
        color = mix(vec3(0.08, 0.15, 0.45), vec3(0.9, 0.95, 1.0), m * 0.5 + 0.5);
    } else if (params.mode == 1u) {
        // fBm terrain
        float n = fbm(uv * 10.0 + vec2(params.time * 0.05));
        color = mix(vec3(0.05, 0.12, 0.04), vec3(0.85, 0.8, 0.55), n);
    } else {
        // Voronoi-like cell pattern
        vec2 cell = floor(uv * 16.0);
        vec2 local = fract(uv * 16.0);
        float d = 1.0;
        for (int j = -1; j <= 1; ++j) {
            for (int i = -1; i <= 1; ++i) {
                vec2 o = vec2(float(i), float(j));
                vec2 p = o + vec2(
                    hash12(cell + o),
                    hash12(cell + o + 17.0)
                );
                d = min(d, length(local - p));
            }
        }
        color = mix(vec3(0.15, 0.02, 0.02), vec3(1.0, 0.45, 0.2), smoothstep(0.0, 0.45, d));
    }

    uvec4 rgba = uvec4(clamp(color, 0.0, 1.0) * 255.0, 255.0);
    uint packed = (rgba.r) | (rgba.g << 8) | (rgba.b << 16) | (rgba.a << 24);
    uint idx = y * params.width + x;

    PixelBuffer out_buf = PixelBuffer(params.out_ptr);
    out_buf.data[idx] = packed;
}
