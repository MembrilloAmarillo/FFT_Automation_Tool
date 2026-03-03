//! Demonstration of the simple graphics API.
//! Shows how to allocate GPU memory and write data to it.

use rust_and_vulkan::simple::{GpuBumpAllocator, MemoryType};
use rust_and_vulkan::{SdlContext, SdlWindow, VulkanDevice, VulkanInstance, VulkanSurface};

fn main() -> Result<(), String> {
    println!("Simple Graphics API Demo");
    println!("========================");

    // Initialize SDL3 and Vulkan
    let sdl = SdlContext::init()?;
    let window = SdlWindow::new("Simple API Demo", 800, 600)?;
    let instance = VulkanInstance::create(&sdl, &window)?;

    // Create surface (optional for memory allocation)
    let surface = VulkanSurface::create(&window, &instance)?;

    // Create Vulkan device (with surface for presentation)
    let device = VulkanDevice::create(instance, Some(surface))?;

    // Create graphics context for simple API
    let context = device
        .graphics_context()
        .map_err(|e| format!("Failed to create graphics context: {}", e))?;

    println!("Graphics context created successfully.");

    // Test GPU memory allocation
    println!("Testing gpu_malloc...");
    let allocation = context
        .gpu_malloc(1024, 16, MemoryType::CpuMapped)
        .map_err(|e| format!("gpu_malloc failed: {}", e))?;
    println!("Allocation GPU address: 0x{:x}", allocation.gpu_ptr);

    // Write some data
    let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    allocation
        .write(&data)
        .map_err(|e| format!("write failed: {}", e))?;
    println!("Data written to GPU memory.");

    // Test bump allocator
    println!("Testing GpuBumpAllocator...");
    let mut bump_alloc = GpuBumpAllocator::new(&context, 4096)
        .map_err(|e| format!("Failed to create bump allocator: {}", e))?;

    let (_cpu_ptr, _gpu_ptr) =
        bump::<u32>(&mut bump_alloc, 10).map_err(|e| format!("bump allocation failed: {}", e))?;
    println!("Allocated 10 u32s via bump allocator.");

    // Wait a bit for user to see output
    std::thread::sleep(std::time::Duration::from_millis(2000));

    println!("Demo completed.");
    Ok(())
}

// Helper to use generic allocate method
fn bump<T>(
    allocator: &mut GpuBumpAllocator,
    count: usize,
) -> Result<(*mut T, u64), rust_and_vulkan::simple::Error> {
    allocator.allocate(count)
}
