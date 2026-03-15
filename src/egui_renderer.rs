//! egui Vulkan renderer using device addresses and descriptor-buffer bindless textures.
//!
//! Converts egui tessellated output into Vulkan draw calls.
//! Uses device address buffers for vertex/index data, and a `TextureDescriptorHeap`
//! for the font texture (compatible with VK_EXT_descriptor_buffer pipelines).

use egui::ClippedPrimitive;

use crate::simple::{
    Buffer, BufferUsage, CommandBuffer, DescriptorPool, DescriptorSet, DescriptorSetLayout, Format,
    GraphicsContext, GraphicsPipeline, GraphicsPipelineConfig, MemoryType, PipelineLayout,
    RasterizationState, ShaderModule, Texture, TextureDescriptorHeap, TextureUsage,
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct UIVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: u32, // pre-multiplied sRGB packed as ABGR (little-endian RGBA)
}

/// Push constants for the egui pipeline (20 bytes, same layout in both shaders).
///
/// Layout (std430):
///   offset  0: vertex_ptr    (u64, 8 bytes)
///   offset  8: window_width  (f32, 4 bytes)
///   offset 12: window_height (f32, 4 bytes)
///   offset 16: texture_index (u32, 4 bytes)
///   total: 20 bytes
#[repr(C)]
#[derive(Clone, Copy)]
struct UIPushConstants {
    vertex_ptr: u64,    // 8 bytes
    window_width: f32,  // 4 bytes
    window_height: f32, // 4 bytes
    texture_index: u32, // 4 bytes — bindless heap index for font atlas
}

/// One scissored draw call produced by `prepare()` and consumed by `render()`.
#[derive(Clone, Copy)]
struct DrawCall {
    /// First index in the index buffer.
    first_index: u32,
    /// Number of indices to draw.
    index_count: u32,
    /// Scissor rect in screen-space pixels (already clamped to the viewport).
    scissor_x: i32,
    scissor_y: i32,
    scissor_w: u32,
    scissor_h: u32,
}

pub struct EguiRenderer {
    pipeline: GraphicsPipeline,
    layout: PipelineLayout,
    device: crate::VkDevice,
    use_descriptor_buffer: bool,
    use_mapped_ui_buffers: bool,
    // Font texture + descriptor binding state
    font_texture: Option<Texture>,
    font_texture_width: u32,
    font_texture_height: u32,
    font_sampler: crate::VkSampler,
    font_heap: Option<TextureDescriptorHeap>,
    _font_descriptor_pool: Option<DescriptorPool>,
    font_descriptor_set: Option<DescriptorSet>,
    font_texture_index: u32,
    font_descriptor_ready: bool,
    // Geometry buffers
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
    vertex_capacity: usize,
    index_capacity: usize,
    // Reused CPU-side scratch buffers to avoid per-frame Vec allocations.
    scratch_vertices: Vec<UIVertex>,
    scratch_indices: Vec<u32>,
    // Per-primitive draw calls (populated by prepare, consumed by render)
    draws: Vec<DrawCall>,
}

impl EguiRenderer {
    pub fn new(
        context: &GraphicsContext,
        render_pass: crate::VkRenderPass,
    ) -> Result<Self, String> {
        // Load shaders
        let vert_spv = load_spirv_u32("shaders/ui.vert.spv")?;
        let frag_spv = load_spirv_u32("shaders/ui.frag.spv")?;

        let vs = ShaderModule::new(context, &vert_spv).map_err(|e| e.to_string())?;
        let fs = ShaderModule::new(context, &frag_spv).map_err(|e| e.to_string())?;

        let use_descriptor_buffer = context.descriptor_buffer_supported();

        let set_layout = if use_descriptor_buffer {
            DescriptorSetLayout::new_bindless_textures(context, 1).map_err(|e| e.to_string())?
        } else {
            DescriptorSetLayout::new_texture_array(context, 1).map_err(|e| e.to_string())?
        };

        // Pipeline layout: descriptor set 0 + push constants (20 bytes).
        let layout = PipelineLayout::with_descriptor_set_layouts_and_push_size(
            context,
            std::slice::from_ref(&set_layout),
            crate::simple::SHADER_STAGE_VERTEX | crate::simple::SHADER_STAGE_FRAGMENT,
            std::mem::size_of::<UIPushConstants>() as u32,
        )
        .map_err(|e| e.to_string())?;

        // Alpha-blend pipeline with VK_PIPELINE_CREATE_DESCRIPTOR_BUFFER_BIT_EXT.
        // UI rendering with blending enabled and culling disabled (both sides visible).
        let ui_config = GraphicsPipelineConfig::transparent_ui().with_rasterization(
            RasterizationState::default()
                .with_cull_mode(crate::VkCullModeFlagBits::VK_CULL_MODE_NONE as u32),
        );

        let mut pipeline_builder =
            GraphicsPipeline::builder(context, &vs, &fs, &layout, render_pass)
                .with_config(ui_config);
        if use_descriptor_buffer {
            pipeline_builder = pipeline_builder.with_descriptor_buffer();
        }
        let pipeline = pipeline_builder.build().map_err(|e| e.to_string())?;

        // Sampler for font atlas
        let font_sampler = context
            .create_default_sampler()
            .map_err(|e| e.to_string())?;

        let (font_heap, font_descriptor_pool, font_descriptor_set, font_texture_index) =
            if use_descriptor_buffer {
                let mut font_heap =
                    TextureDescriptorHeap::new(context, 1).map_err(|e| e.to_string())?;
                let font_texture_index = font_heap.allocate().map_err(|e| e.to_string())?;
                (Some(font_heap), None, None, font_texture_index)
            } else {
                let descriptor_pool =
                    DescriptorPool::new(context, 1, 1).map_err(|e| e.to_string())?;
                let descriptor_set = descriptor_pool
                    .allocate(&set_layout)
                    .map_err(|e| e.to_string())?;
                (None, Some(descriptor_pool), Some(descriptor_set), 0)
            };

        Ok(EguiRenderer {
            pipeline,
            layout,
            device: context.vk_device(),
            use_descriptor_buffer,
            use_mapped_ui_buffers: true,
            font_texture: None,
            font_texture_width: 0,
            font_texture_height: 0,
            font_sampler,
            font_heap,
            _font_descriptor_pool: font_descriptor_pool,
            font_descriptor_set,
            font_texture_index,
            font_descriptor_ready: false,
            vertex_buffer: None,
            index_buffer: None,
            vertex_capacity: 0,
            index_capacity: 0,
            scratch_vertices: Vec::new(),
            scratch_indices: Vec::new(),
            draws: Vec::new(),
        })
    }

    fn create_ui_vertex_buffer(
        &mut self,
        context: &GraphicsContext,
        needed: usize,
    ) -> Result<Buffer, String> {
        if self.use_mapped_ui_buffers {
            match Buffer::new(
                context,
                needed.max(1),
                BufferUsage::VERTEX,
                MemoryType::CpuMapped,
            ) {
                Ok(buf) => return Ok(buf),
                Err(crate::simple::Error::Unsupported) => {
                    self.use_mapped_ui_buffers = false;
                }
                Err(e) => return Err(format!("vertex buffer: {e}")),
            }
        }

        Buffer::from_data(
            context,
            BufferUsage::STORAGE | BufferUsage::TRANSFER_DST,
            as_bytes(&self.scratch_vertices),
        )
        .map_err(|e| format!("vertex upload buffer: {e}"))
    }

    fn create_ui_index_buffer(
        &mut self,
        context: &GraphicsContext,
        needed: usize,
    ) -> Result<Buffer, String> {
        if self.use_mapped_ui_buffers {
            match Buffer::new(
                context,
                needed.max(1),
                BufferUsage::INDEX,
                MemoryType::CpuMapped,
            ) {
                Ok(buf) => return Ok(buf),
                Err(crate::simple::Error::Unsupported) => {
                    self.use_mapped_ui_buffers = false;
                }
                Err(e) => return Err(format!("index buffer: {e}")),
            }
        }

        Buffer::from_data(
            context,
            BufferUsage::INDEX | BufferUsage::TRANSFER_DST,
            as_bytes(&self.scratch_indices),
        )
        .map_err(|e| format!("index upload buffer: {e}"))
    }

    /// Upload (or re-upload) the egui texture atlas.
    /// Should be called each frame before `render()`, passing the `TexturesDelta`
    /// returned by `egui::Context::end_frame()`.
    pub fn update_textures(
        &mut self,
        context: &GraphicsContext,
        textures_delta: &egui::TexturesDelta,
    ) -> Result<(), String> {
        for id in &textures_delta.free {
            // We only own the default egui atlas texture in this renderer.
            if *id == egui::TextureId::default() {
                self.font_texture = None;
                self.font_texture_width = 0;
                self.font_texture_height = 0;
                self.font_descriptor_ready = false;
            }
        }

        for (id, delta) in &textures_delta.set {
            // We only handle the built-in font atlas (TextureId::default() == Managed(0))
            if *id != egui::TextureId::default() {
                continue;
            }

            if let Some(pos) = delta.pos {
                // Partial update: only some region of the texture changed
                if let Some(texture) = &self.font_texture {
                    // Upload the partial region to the existing texture
                    self.update_texture_region(context, texture, &delta.image, pos)?;
                }
                // If texture doesn't exist yet, we'll get a full update later
            } else {
                // Full texture update
                let (width, height, rgba_bytes) = image_delta_to_rgba(&delta.image);

                let texture = context
                    .upload_texture(
                        &rgba_bytes,
                        width,
                        height,
                        Format::Rgba8Unorm,
                        TextureUsage::SAMPLED | TextureUsage::TRANSFER_DST,
                    )
                    .map_err(|e| e.to_string())?;

                // Write (or re-write) the descriptor in the heap.
                if self.use_descriptor_buffer {
                    self.font_heap
                        .as_mut()
                        .ok_or_else(|| "missing font descriptor heap".to_string())?
                        .write_descriptor(
                            context,
                            self.font_texture_index,
                            &texture,
                            self.font_sampler,
                        )
                        .map_err(|e| e.to_string())?;
                } else {
                    self.font_descriptor_set
                        .as_ref()
                        .ok_or_else(|| "missing font descriptor set".to_string())?
                        .write_textures(context, &[&texture], self.font_sampler)
                        .map_err(|e| e.to_string())?;
                }
                self.font_descriptor_ready = true;

                self.font_texture_width = width;
                self.font_texture_height = height;
                self.font_texture = Some(texture);
            }
        }
        Ok(())
    }

    /// Update a sub-region of the font texture.
    fn update_texture_region(
        &self,
        context: &GraphicsContext,
        texture: &Texture,
        image: &egui::ImageData,
        pos: [usize; 2],
    ) -> Result<(), String> {
        let (region_width, region_height, rgba_bytes) = image_delta_to_rgba(image);

        // Create staging buffer for this region's data
        let staging_size = rgba_bytes.len();
        let staging = context
            .gpu_malloc(staging_size, 16, crate::simple::MemoryType::CpuMapped)
            .map_err(|e| format!("Failed to allocate staging buffer: {e}"))?;

        staging
            .write(&rgba_bytes)
            .map_err(|e| format!("Failed to write to staging buffer: {e}"))?;

        // Use single-time-submit command buffer for the update
        let cmd = context
            .begin_single_time_commands()
            .map_err(|e| e.to_string())?;

        // Transition texture to transfer destination
        cmd.transition_to_transfer_dst(texture);

        // Copy the region to the texture
        self.copy_buffer_to_texture_region(
            &cmd,
            &staging,
            texture,
            pos,
            region_width,
            region_height,
        );

        // Transition texture back to shader read-only
        cmd.transition_to_shader_read(texture);

        // Submit and wait
        context
            .end_single_time_commands(cmd)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Copy a buffer to a specific region of a texture.
    fn copy_buffer_to_texture_region(
        &self,
        cmd: &crate::simple::CommandBuffer,
        src_buffer: &crate::simple::GpuAllocation,
        dst_texture: &Texture,
        region_pos: [usize; 2],
        region_width: u32,
        region_height: u32,
    ) {
        cmd.copy_buffer_to_texture_region(
            src_buffer,
            dst_texture,
            region_pos[0] as i32,
            region_pos[1] as i32,
            region_width,
            region_height,
        );
    }

    /// Update vertex/index buffers with new egui tessellated output.
    pub fn prepare(
        &mut self,
        context: &GraphicsContext,
        clipped_primitives: Vec<ClippedPrimitive>,
        screen_width: f32,
        screen_height: f32,
    ) -> Result<(), String> {
        self.scratch_vertices.clear();
        self.scratch_indices.clear();
        self.draws.clear();

        for ClippedPrimitive {
            primitive,
            clip_rect,
        } in clipped_primitives
        {
            match primitive {
                egui::epaint::Primitive::Mesh(mesh) => {
                    if mesh.indices.is_empty() {
                        continue;
                    }

                    let index_offset = self.scratch_vertices.len() as u32;
                    let first_index = self.scratch_indices.len() as u32;

                    for vertex in &mesh.vertices {
                        let [r, g, b, a] = vertex.color.to_srgba_unmultiplied();
                        // Pre-multiply alpha for (srcFactor=ONE, dstFactor=ONE_MINUS_SRC_ALPHA)
                        let a_f = a as u8;
                        let pr = r as u8;
                        let pg = g as u8;
                        let pb = b as u8;
                        let packed = ((a_f as u32) << 24)
                            | ((pb as u32) << 16)
                            | ((pg as u32) << 8)
                            | (pr as u32);

                        self.scratch_vertices.push(UIVertex {
                            position: [vertex.pos.x, vertex.pos.y],
                            uv: [vertex.uv.x, vertex.uv.y],
                            color: packed,
                        });
                    }

                    for index in &mesh.indices {
                        self.scratch_indices.push(index_offset + index);
                    }

                    // Clamp clip_rect to the viewport and convert to integer pixels.
                    let x0 = clip_rect.min.x.max(0.0).floor() as i32;
                    let y0 = clip_rect.min.y.max(0.0).floor() as i32;
                    let x1 = clip_rect.max.x.min(screen_width).ceil() as i32;
                    let y1 = clip_rect.max.y.min(screen_height).ceil() as i32;
                    let w = (x1 - x0).max(0) as u32;
                    let h = (y1 - y0).max(0) as u32;

                    self.draws.push(DrawCall {
                        first_index,
                        index_count: mesh.indices.len() as u32,
                        scissor_x: x0,
                        scissor_y: y0,
                        scissor_w: w,
                        scissor_h: h,
                    });
                }
                egui::epaint::Primitive::Callback(_) => {}
            }
        }

        if !self.scratch_vertices.is_empty() {
            let needed = self.scratch_vertices.len() * std::mem::size_of::<UIVertex>();
            // Grow-only strategy: avoids vkDeviceWaitIdle stalls when UI size oscillates.
            // On drivers without CPU-mapped compatible memory for these buffers, fall back
            // to recreating/uploading GPU buffers instead of failing startup.
            if self.vertex_capacity < needed || !self.use_mapped_ui_buffers {
                self.vertex_capacity = (needed as f32 * 1.5) as usize;
                let buf = self.create_ui_vertex_buffer(context, self.vertex_capacity)?;
                if buf.cpu_ptr().is_some() {
                    buf.write(as_bytes(&self.scratch_vertices))
                        .map_err(|e| format!("write vertices: {e}"))?;
                }
                self.vertex_buffer = Some(buf);
            } else if let Some(ref buf) = self.vertex_buffer {
                buf.write(as_bytes(&self.scratch_vertices))
                    .map_err(|e| format!("write vertices: {e}"))?;
            }
        }

        if !self.scratch_indices.is_empty() {
            let needed = self.scratch_indices.len() * std::mem::size_of::<u32>();
            // Grow-only strategy: avoids vkDeviceWaitIdle stalls when UI size oscillates.
            // On drivers without CPU-mapped compatible memory for these buffers, fall back
            // to recreating/uploading GPU buffers instead of failing startup.
            if self.index_capacity < needed || !self.use_mapped_ui_buffers {
                self.index_capacity = (needed as f32 * 1.5) as usize;
                let buf = self.create_ui_index_buffer(context, self.index_capacity)?;
                if buf.cpu_ptr().is_some() {
                    buf.write(as_bytes(&self.scratch_indices))
                        .map_err(|e| format!("write indices: {e}"))?;
                }
                self.index_buffer = Some(buf);
            } else if let Some(ref buf) = self.index_buffer {
                buf.write(as_bytes(&self.scratch_indices))
                    .map_err(|e| format!("write indices: {e}"))?;
            }
        }

        Ok(())
    }

    pub fn render(
        &self,
        cmd: &CommandBuffer,
        screen_width: f32,
        screen_height: f32,
    ) -> Result<(), String> {
        if self.vertex_buffer.is_none() || self.index_buffer.is_none() || self.draws.is_empty() {
            return Ok(());
        }

        if !self.font_descriptor_ready {
            return Ok(());
        }

        cmd.bind_pipeline(&self.pipeline);

        if self.use_descriptor_buffer {
            cmd.bind_texture_heap(
                self.font_heap
                    .as_ref()
                    .ok_or_else(|| "missing font descriptor heap".to_string())?,
                &self.layout,
                0,
                crate::VkPipelineBindPoint::VK_PIPELINE_BIND_POINT_GRAPHICS,
            );
        } else {
            cmd.bind_descriptor_sets(
                &self.layout,
                0,
                &[self
                    .font_descriptor_set
                    .as_ref()
                    .ok_or_else(|| "missing font descriptor set".to_string())?],
            );
        }

        let pc = UIPushConstants {
            vertex_ptr: self.vertex_buffer.as_ref().unwrap().device_address(),
            window_width: screen_width,
            window_height: screen_height,
            texture_index: self.font_texture_index,
        };
        cmd.push_constants(&self.layout, as_bytes(std::slice::from_ref(&pc)));

        cmd.bind_index_buffer(
            self.index_buffer.as_ref().unwrap(),
            0,
            crate::simple::IndexType::U32,
        );

        // Issue one draw call per clipped primitive with its scissor rect.
        for draw in &self.draws {
            cmd.set_scissor(
                draw.scissor_x,
                draw.scissor_y,
                draw.scissor_w,
                draw.scissor_h,
            );
            cmd.draw_indexed(draw.index_count, 1, draw.first_index, 0, 0);
        }

        Ok(())
    }

    pub fn pipeline(&self) -> &GraphicsPipeline {
        &self.pipeline
    }
}

impl Drop for EguiRenderer {
    fn drop(&mut self) {
        unsafe {
            crate::vkDestroySampler(self.device, self.font_sampler, std::ptr::null());
        }
    }
}

// ─── helpers ────────────────────────────────────────────────────────────────

fn as_bytes<T>(slice: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            slice.as_ptr() as *const u8,
            slice.len() * std::mem::size_of::<T>(),
        )
    }
}

/// Convert an egui `ImageData` to `(width, height, rgba8_bytes)`.
fn image_delta_to_rgba(image: &egui::ImageData) -> (u32, u32, Vec<u8>) {
    match image {
        egui::ImageData::Color(img) => {
            let w = img.size[0] as u32;
            let h = img.size[1] as u32;
            let bytes = img
                .pixels
                .iter()
                .flat_map(|c| {
                    let [r, g, b, a] = c.to_srgba_unmultiplied();
                    [r, g, b, a]
                })
                .collect();
            (w, h, bytes)
        }
    }
}

fn load_spirv_u32(path: &str) -> Result<Vec<u32>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read {path}: {e}"))?;
    if bytes.len() % 4 != 0 {
        return Err(format!("SPIR-V file not u32-aligned: {path}"));
    }
    let mut words = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        words.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(words)
}
