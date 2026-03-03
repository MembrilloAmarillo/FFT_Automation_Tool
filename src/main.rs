use rust_and_vulkan::{SdlContext, SdlWindow, VulkanInstance};

fn main() -> Result<(), String> {
    let sdl = SdlContext::init()?;
    let window = SdlWindow::new("Rust + SDL3 + Vulkan", 800, 600)?;
    let _instance = VulkanInstance::create(&sdl, &window)?;

    println!("SDL3 and Vulkan initialized successfully!");

    // Simple event loop
    let mut quit = false;
    while !quit {
        unsafe {
            let mut event = std::mem::zeroed();
            while rust_and_vulkan::SDL_PollEvent(&mut event) {
                if event.type_ == rust_and_vulkan::SDL_EventType::SDL_EVENT_QUIT as u32 {
                    quit = true;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    Ok(())
}
