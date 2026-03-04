# Vulkan Abstraction API - Feature Summary

This document outlines the current capabilities of the Rust Vulkan abstraction API built on top of raw Vulkan bindings.

## Core Architecture

The API provides a **high-level abstraction** inspired by "No Graphics API" blog post, offering simplified GPU memory management with `gpuMalloc/gpuFree` style semantics while exposing low-level Vulkan control where needed.

---

## 1. Instance & Device Management

### `VulkanInstance`
- **Purpose**: Initialize Vulkan instance with validation layers
- **Features**:
  - Automatic validation layer setup (debug builds)
  - Debug messenger callback integration
  - Supports multiple SDL2 extensions
  - Vulkan 1.2 API version support

### `VulkanDevice`
- **Purpose**: Logical device creation and queue management
- **Features**:
  - Physical device enumeration
  - Graphics queue family detection
  - Present queue family support (for window rendering)
  - Automatic command pool creation
  - Buffer device address features (for bindless access)

### `VulkanSurface`
- **Purpose**: SDL2 window surface integration
- **Features**:
  - SDL2/Vulkan surface creation
  - Automatic cleanup

### `SdlWindow` & `SdlContext`
- **Purpose**: Window and SDL2 lifecycle management
- **Features**:
  - Window creation with Vulkan support
  - SDL2 initialization and cleanup

---

## 2. Memory Management

### `GraphicsContext`
- **Purpose**: Central context for all GPU operations
- **Features**:
  - Device handles and queues
  - Memory type queries
  - Physical device properties
  - Simplified memory allocation methods

### `GpuAllocation`
- **Purpose**: GPU memory with both CPU and GPU pointers
- **Features**:
  - `cpu_ptr`: CPU-mapped pointer for writes
  - `gpu_ptr`: GPU virtual address (64-bit)
  - `gpu_malloc()`: Allocate with size, alignment, and memory type
  - `write()`: Copy CPU data to GPU memory
  - `host_to_device_ptr()`: Convert CPU pointers to GPU addresses
  - Automatic cleanup via Drop trait

### `GpuBumpAllocator`
- **Purpose**: Fast temporary GPU allocations
- **Features**:
  - Efficient linear allocation pattern
  - `allocate<T>()`: Allocate memory for type T
  - `reset()`: Reset offset without freeing
  - Useful for per-frame allocations

### Memory Types
```rust
pub enum MemoryType {
    CpuMapped,    // Write-combined, fast for CPU writes
    GpuOnly,      // Optimal for textures
    CpuCached,    // For GPU readback
}
```

---

## 3. Buffers & Textures

### `Buffer`
- **Purpose**: Typed GPU buffers with flexible usage
- **Features**:
  - Configurable memory type (CPU/GPU)
  - Multiple usage flags supported
  - `cpu_ptr()`: Optional CPU access
  - `write()`: CPU-to-GPU copy

### `BufferUsage` Flags
```rust
pub struct BufferUsage: u32 {
    VERTEX,
    INDEX,
    UNIFORM,
    STORAGE,
    TRANSFER_SRC,
    TRANSFER_DST,
}
```

### `Texture`
- **Purpose**: 2D image resources
- **Features**:
  - Optimal GPU-local tiling
  - Automatic image view creation
  - Multiple format support
  - Configurable usage patterns

### `TextureUsage` Flags
```rust
pub struct TextureUsage: u32 {
    SAMPLED,
    RENDER_TARGET,
    DEPTH_STENCIL,
    TRANSFER_SRC,
    TRANSFER_DST,
}
```

### `Format` Support
- R8_UNORM, R8G8_UNORM
- R8G8B8A8_UNORM, B8G8R8A8_UNORM (SRGB variants)
- R32_FLOAT, R32G32_FLOAT, R32G32B32A32_FLOAT
- D32_SFLOAT (depth)
- Automatic aspect mask calculation

---

## 4. Shaders & Pipelines

### `ShaderModule`
- **Purpose**: Compiled SPIR-V shader wrapper
- **Features**:
  - Direct SPIR-V bytecode loading
  - Automatic module cleanup

### `SpecializationConstants`
- **Purpose**: Compile-time shader specialization
- **Features**:
  - `add_u32()`: Add u32 constants
  - Builder pattern for flexible setup
  - Optional specialization info generation

### `GraphicsPipeline`
- **Purpose**: Rendering pipeline
- **Features**:
  - Vertex/Fragment shader stages
  - Hardcoded triangle topology (configurable)
  - Color blending support
  - Depth testing enabled by default
  - Viewport and scissor management
  - Dynamic state support placeholder

### `ComputePipeline`
- **Purpose**: Compute shader execution
- **Features**:
  - Single compute stage setup
  - Root argument passing support
  - Specialization constant support

---

## 5. Pipeline Layouts & Resource Binding

### `PipelineLayout`
- **Purpose**: Define resource binding structure
- **Features**:
  - **Simple layout**: No descriptors, no push constants
  - **Root argument layouts**: 64-bit pointer for data access
  - **Push constant layouts**: Flexible size (8, 16, 64 bytes)
    - `with_mat4_push_constants()`: 64-byte layout
    - `with_vec4_push_constants()`: 16-byte layout
  - **Separate root arguments**: Vertex + Fragment (2 × 64-bit)
  - **Bindless texture layout**: For descriptor buffers
  - **Descriptor set layouts**: Standard descriptor binding

### `RootArguments`
- **Purpose**: Single-pointer GPU data passing
- **Features**:
  - `gpu_address()`: 64-bit pointer for shader access
  - `cpu_ptr<T>()`: Typed CPU access
  - `write<T>()`: Write structured data

### `DescriptorSetLayout`
- **Purpose**: Descriptor binding description
- **Features**:
  - Bindless texture support (variable descriptor count)
  - Descriptor buffer extension support

---

## 6. Bindless Texturing (VK_EXT_descriptor_buffer)

### `TextureDescriptorHeap`
- **Purpose**: Bindless texture array management
- **Features**:
  - Hardware descriptor encoding
  - `allocate()`: Get texture index
  - `write_descriptor()`: Store texture descriptor
  - `gpu_address()`: Get heap address for shaders
  - Dynamic capacity management
  - GPU device address support

### `DescriptorHeap`
- **Purpose**: Generic descriptor buffer management
- **Features**:
  - Combined image sampler descriptors
  - `add_texture()`: Register texture
  - Automatic image view creation
  - Host-visible memory for CPU updates

---

## 7. Synchronization & Frame Management

### `Fence`
- **Purpose**: CPU-GPU synchronization
- **Features**:
  - `wait()`: With timeout (nanoseconds)
  - `wait_forever()`: Blocking wait
  - `is_signaled()`: Non-blocking query
  - `reset()`: Return to unsignaled state

### `Swapchain`
- **Purpose**: Window presentation with double buffering
- **Features**:
  - Automatic swapchain creation
  - **Double-buffering**: 2 frames in flight
  - Depth buffer management (D32_SFLOAT)
  - `begin_frame()`: Acquire image, reset command buffer
  - `end_frame()`: Submit and present
  - Automatic frame synchronization
  - Render pass with color + depth attachments

### `FrameData`
- **Purpose**: Per-frame GPU resources
- **Features**:
  - Command buffer allocation
  - Fence creation
  - Semaphore pair (image_available, render_finished)
  - Automatic wait/submit/present

### `CommandBuffer`
- **Purpose**: Recording rendering/compute commands
- **Features**:
  - **Memory operations**:
    - `copy_buffer()`: Buffer-to-buffer copy
    - `copy_buffer_to_texture()`: Staging uploads
  - **Image transitions**:
    - `transition_to_transfer_dst()`
    - `transition_to_transfer_src()`
    - `transition_to_shader_read()`
    - `transition_to_render_target()`
    - `transition_to_depth_target()`
  - **Rendering**:
    - `begin_render_pass()`
    - `bind_graphics_pipeline()`
    - `bind_vertex_buffer()`
    - `bind_index_buffer()`
    - `draw()`
    - `draw_indexed()`
    - `end_render_pass()`
  - **Compute**:
    - `bind_compute_pipeline()`
    - `dispatch()`
  - **Resource binding**:
    - `set_graphics_root_arguments()`: Pass GPU pointer
    - `set_compute_root_arguments()`: Pass GPU pointer
    - `push_constants()`: Raw push constant data
    - `bind_texture_heap_graphics()`
    - `bind_texture_heap_compute()`
  - **Descriptor buffers**:
    - `bind_descriptor_buffer()`
    - `set_descriptor_buffer_offset()`

### Semaphores
- `create_semaphore()`: Binary semaphores
- `destroy_semaphore()`: Cleanup
- Used for GPU-GPU synchronization in submission

---

## 8. Rendering Components

### `GraphicsContext` Capabilities
```rust
// Memory
pub fn gpu_malloc() -> GpuAllocation
pub fn gpu_malloc_simple<T>() -> GpuAllocation

// Textures
pub fn upload_texture() -> Texture
pub fn texture_size_align() -> (size, alignment)

// Samplers
pub fn create_default_sampler() -> VkSampler
pub fn destroy_sampler()

// Synchronization
pub fn create_semaphore() -> VkSemaphore
pub fn submit() -> Fence
pub fn submit_with_semaphores() -> Fence
pub fn wait_idle() -> Result<()>
```

### `Sampler` Support
- **Default sampler**: Linear filtering, repeat wrap mode
- Anisotropy: Disabled (can be enabled)
- Comparison: Disabled
- Mipmap: Linear mode

---

## 9. Advanced Features

### Pipeline Stages (for barriers & synchronization)
```rust
STAGE_TRANSFER
STAGE_COMPUTE
STAGE_GRAPHICS
STAGE_ALL
STAGE_HOST
STAGE_VERTEX_SHADER
STAGE_PIXEL_SHADER (Fragment)
STAGE_RASTER_COLOR_OUT
STAGE_RASTER_DEPTH_OUT
STAGE_DRAW_INDIRECT
```

### Shader Stage Flags
```rust
SHADER_STAGE_VERTEX
SHADER_STAGE_FRAGMENT
SHADER_STAGE_COMPUTE
SHADER_STAGE_ALL_GRAPHICS
```

### Hazard Flags
```rust
pub struct HazardFlags: u32 {
    DRAW_ARGUMENTS,
    DESCRIPTORS,
    DEPTH_STENCIL,
}
```

### Buffer Device Address
- **Enabled by default** in logical device
- Required for shader-side pointer access
- Automatic capture with memory allocation flags

---

## 10. Example Usage Patterns

### Simple Triangle Rendering
```rust
// Setup
let ctx = GraphicsContext::new(...)?;
let shader = ShaderModule::new(&ctx, spirv_bytes)?;
let layout = PipelineLayout::with_root_argument(&ctx, SHADER_STAGE_VERTEX)?;
let pipeline = GraphicsPipeline::new(&ctx, shader, shader, &layout, ...)?;

// Rendering
let mut cmd = CommandBuffer::allocate(&ctx)?;
cmd.begin()?;
cmd.begin_render_pass(...);
cmd.bind_graphics_pipeline(&pipeline);
cmd.draw(3, 1, 0, 0);
cmd.end_render_pass();
cmd.end()?;

let fence = ctx.submit(&cmd)?;
fence.wait_forever()?;
```

### Bindless Texturing
```rust
let mut heap = TextureDescriptorHeap::new(&ctx, 256)?;
let idx = heap.allocate()?;
heap.write_descriptor(&ctx, idx, &texture, sampler)?;

// In shader: use descriptors[idx] to access texture
```

### Compute Shader
```rust
let compute = ComputePipeline::new(&ctx, shader, &layout, None)?;
let root_args = RootArguments::new::<MyData>(&ctx)?;
root_args.write(&my_data)?;

let mut cmd = CommandBuffer::allocate(&ctx)?;
cmd.begin()?;
cmd.bind_compute_pipeline(&compute);
cmd.set_compute_root_arguments(&layout, &root_args);
cmd.dispatch(&compute, &layout, root_args.gpu_address(), [8, 8, 1]);
cmd.end()?;
```

---

## Summary of Key Strengths

✅ **Simple GPU Memory Model**: `gpuMalloc`/`gpuFree` style with CPU/GPU pointers
✅ **Bindless Texturing**: VK_EXT_descriptor_buffer support for array indexing
✅ **Root Arguments**: Single 64-bit pointer for flexible data passing
✅ **Double Buffering**: Automatic frame synchronization in swapchain
✅ **Structured Pipelines**: Simple layouts for graphics and compute
✅ **SPIR-V Support**: Direct bytecode loading
✅ **Specialization Constants**: Compile-time shader variations
✅ **Buffer Device Address**: GPU pointer support for shaders
✅ **Image Transitions**: Explicit layout management
✅ **Validation Layers**: Integrated debug callback

---

## Limitations & Not Yet Implemented

❌ Dynamic pipeline state (viewport, scissor, etc.)
❌ Multi-sample anti-aliasing (MSAA)
❌ Traditional descriptor sets (limited support)
❌ Ray tracing extensions
❌ Mesh shaders
❌ VK_EXT_descriptor_buffer full feature exposure
⚠️ Limited sampler configuration
⚠️ Single viewport/scissor per pipeline
