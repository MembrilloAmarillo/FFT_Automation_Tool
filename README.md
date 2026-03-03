# Rust + SDL3 + Vulkan Bindings

This project demonstrates using SDL3 and Vulkan with Rust via `bindgen` to generate FFI bindings at compile time.

## Prerequisites

- Rust toolchain (rustc, cargo)
- SDL3 development libraries
- Vulkan development libraries

### Install SDL3

SDL3 is still in development; you may need to build from source. Ensure headers and libraries are installed in system paths.

### Install Vulkan

Install Vulkan SDK or distribution packages (e.g., `vulkan-devel` on Fedora, `libvulkan-dev` on Debian).

## Building

```bash
cargo build
```

The build script (`build.rs`) will generate bindings for SDL3 and Vulkan headers.

## Running

```bash
cargo run
```

A window should open with SDL3 and Vulkan initialized.

## Project Structure

- `Cargo.toml`: Dependencies and configuration
- `build.rs`: Build script generating bindings
- `wrapper.h`: C headers included for bindgen
- `src/lib.rs`: Safe Rust wrappers around generated bindings
- `src/main.rs`: Example application
- `src/simple.rs`: Simple graphics API abstraction (inspired by "No Graphics API" blog post)

## Simple Graphics API

The project includes a simple graphics API abstraction inspired by the ["No Graphics API"](https://www.sebastianaaltonen.com/blog/no-graphics-api) blog post. This API aims to provide a simpler interface to Vulkan with concepts like:

- `gpu_malloc`/`gpu_free` style memory management
- CPU-mapped GPU memory allocations
- Bump allocator for temporary allocations
- Simplified texture and buffer management (planned)
- Simplified pipeline creation (planned)

### Example Usage

```rust
use rust_and_vulkan::simple::{GraphicsContext, MemoryType};

// After initializing Vulkan...
// let context = GraphicsContext::new(instance, physical_device, device, ...);

// Allocate CPU-mapped GPU memory
// let allocation = context.gpu_malloc(1024, 16, MemoryType::CpuMapped)?;
// allocation.write(&[1, 2, 3, 4]);

// Use a bump allocator for temporary data
// let mut bump_allocator = GpuBumpAllocator::new(&context, 1024 * 1024)?;
// let (cpu_ptr, gpu_ptr) = bump_allocator.allocate::<u32>(256)?;
```

See `examples/simple_triangle.rs` and `examples/simple_api_demo.rs` for complete examples.

## Notes

- The bindings are generated at compile time; no pre‑existing `-sys` crates are used.
- The safe wrappers are minimal; extend them as needed.
- Error handling is basic; production code should provide more detailed diagnostics.

## License

MIT