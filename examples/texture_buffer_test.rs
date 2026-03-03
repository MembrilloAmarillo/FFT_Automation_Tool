//! Test texture and buffer creation using the simple API.

use rust_and_vulkan::simple::{Buffer, BufferUsage, Format, MemoryType, Texture, TextureUsage};
use rust_and_vulkan::{SdlContext, SdlWindow, VulkanDevice, VulkanInstance, VulkanSurface};

fn main() -> Result<(), String> {
    println!("Texture and Buffer Test");
    println!("======================");

    // Initialize SDL3 and Vulkan
    let sdl = SdlContext::init()?;
    let window = SdlWindow::new("Texture/Buffer Test", 800, 600)?;
    let instance = VulkanInstance::create(&sdl, &window)?;

    // Create surface
    let surface = VulkanSurface::create(&window, &instance)?;

    // Create Vulkan device
    let device = VulkanDevice::create(instance, Some(surface))?;

    // Create graphics context for simple API
    let context = device
        .graphics_context()
        .map_err(|e| format!("Failed to create graphics context: {}", e))?;

    println!("Graphics context created successfully.");

    // Test buffer creation
    println!("Testing buffer creation...");
    let buffer = Buffer::new(
        &context,
        1024,
        BufferUsage::VERTEX | BufferUsage::TRANSFER_DST,
        MemoryType::CpuMapped,
    )
    .map_err(|e| format!("Failed to create buffer: {}", e))?;

    println!(
        "Buffer created successfully (size: {} bytes).",
        buffer.size()
    );

    // Test writing to buffer
    let test_data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    buffer
        .write(&test_data)
        .map_err(|e| format!("Failed to write to buffer: {}", e))?;
    println!("Data written to buffer.");

    // Test texture creation
    println!("Testing texture creation...");
    let texture = Texture::new(
        &context,
        256,
        256,
        Format::Rgba8Unorm,
        TextureUsage::SAMPLED | TextureUsage::TRANSFER_DST,
    )
    .map_err(|e| format!("Failed to create texture: {}", e))?;

    println!(
        "Texture created successfully ({}x{}, format: {:?}).",
        texture.width(),
        texture.height(),
        texture.format()
    );

    println!("All tests passed!");

    // Event loop (brief)
    let mut quit = false;
    let mut frames = 0;
    while !quit && frames < 30 {
        // Run for ~0.5 seconds at 60 FPS
        unsafe {
            let mut event = std::mem::zeroed();
            while rust_and_vulkan::SDL_PollEvent(&mut event) {
                if event.type_ == rust_and_vulkan::SDL_EventType::SDL_EVENT_QUIT as u32 {
                    quit = true;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
        frames += 1;
    }

    println!("Test completed successfully.");
    Ok(())
}
