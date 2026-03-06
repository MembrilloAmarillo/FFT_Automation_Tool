//! egui Vulkan renderer using device addresses.
//!
//! Converts egui tessellated output into Vulkan draw calls.
//! Uses device address buffers for minimal overhead.

use egui::ClippedPrimitive;

use crate::simple::{
    Buffer, BufferUsage, CommandBuffer, Format, GraphicsContext, GraphicsPipeline, MemoryType,
    PipelineLayout, ShaderModule,
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct UIVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: u32, // sRGB packed as RGBA
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UIPushConstants {
    vertex_ptr: u64,
    window_width: f32,
    window_height: f32,
    _padding: u32,
}

pub struct EguiRenderer {
    pipeline: GraphicsPipeline,
    layout: PipelineLayout,
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
    vertex_count: usize,
    index_count: usize,
    vertex_capacity: usize,
    index_capacity: usize,
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

        // Create pipeline layout with push constants
        let layout = PipelineLayout::with_push_constants_size(
            context,
            crate::simple::SHADER_STAGE_VERTEX,
            std::mem::size_of::<UIPushConstants>() as u32,
        )
        .map_err(|e| e.to_string())?;

        // Create graphics pipeline with alpha blending for UI transparency.
        // Must also set VK_PIPELINE_CREATE_DESCRIPTOR_BUFFER_BIT_EXT because the
        // scene pipeline (rendered before egui) uses descriptor buffers, and Vulkan
        // requires all pipelines in that command buffer to carry the same flag.
        let pipeline = GraphicsPipeline::new_with_blend_descriptor_buffer(
            context,
            &vs,
            &fs,
            &layout,
            render_pass,
            Format::Rgba8Unorm,
            None,
            None,
        )
        .map_err(|e| e.to_string())?;

        Ok(EguiRenderer {
            pipeline,
            layout,
            vertex_buffer: None,
            index_buffer: None,
            vertex_count: 0,
            index_count: 0,
            vertex_capacity: 0,
            index_capacity: 0,
        })
    }

    /// Update buffers with new egui output
    pub fn prepare(
        &mut self,
        context: &GraphicsContext,
        clipped_primitives: Vec<ClippedPrimitive>,
    ) -> Result<(), String> {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        eprintln!(
            "[egui] prepare() called with {} clipped primitives",
            clipped_primitives.len()
        );

        for ClippedPrimitive { primitive, .. } in clipped_primitives {
            match primitive {
                egui::epaint::Primitive::Mesh(mesh) => {
                    eprintln!(
                        "[egui]   Mesh with {} vertices, {} indices",
                        mesh.vertices.len(),
                        mesh.indices.len()
                    );
                    let index_offset = vertices.len() as u32;

                    // Convert egui mesh to our vertex format
                    for (vi, vertex) in mesh.vertices.iter().enumerate() {
                        let color = vertex.color;
                        let [r, g, b, a] = color.to_srgba_unmultiplied();
                        // Pre-multiply alpha for the pre-multiplied alpha blend equation:
                        //   srcFactor=ONE, dstFactor=ONE_MINUS_SRC_ALPHA
                        let a_f = a as f32 / 255.0;
                        let pr = ((r as f32 / 255.0) * a_f * 255.0 + 0.5) as u8;
                        let pg = ((g as f32 / 255.0) * a_f * 255.0 + 0.5) as u8;
                        let pb = ((b as f32 / 255.0) * a_f * 255.0 + 0.5) as u8;
                        let packed_color = ((a as u32) << 24)
                            | ((pb as u32) << 16)
                            | ((pg as u32) << 8)
                            | (pr as u32);

                        if vi < 3 {
                            eprintln!(
                                "[egui]   Vertex {}: pos=({:.2}, {:.2}), color=rgba({}, {}, {}, {}), packed=0x{:08x}",
                                vi, vertex.pos.x, vertex.pos.y, pr, pg, pb, a, packed_color
                            );
                        }

                        vertices.push(UIVertex {
                            position: [vertex.pos.x, vertex.pos.y],
                            uv: [vertex.uv.x, vertex.uv.y],
                            color: packed_color,
                        });
                    }

                    // Add indices with offset
                    for index in &mesh.indices {
                        indices.push(index_offset + index);
                    }
                }
                egui::epaint::Primitive::Callback(_) => {
                    // Skip callbacks for now
                }
            }
        }

        self.vertex_count = vertices.len();
        self.index_count = indices.len();

        // Update or create vertex buffer (only if size changed)
        if !vertices.is_empty() {
            let needed_size = vertices.len() * std::mem::size_of::<UIVertex>();

            // Only recreate if size changed significantly (with 10% headroom to reduce reallocations)
            if self.vertex_capacity < needed_size || self.vertex_capacity > needed_size * 2 {
                self.vertex_capacity = (needed_size as f32 * 1.2) as usize;

                let buffer = Buffer::new(
                    context,
                    self.vertex_capacity,
                    BufferUsage::VERTEX,
                    MemoryType::CpuMapped,
                )
                .map_err(|e| format!("Failed to create vertex buffer: {}", e))?;

                // Write vertex data
                let vertex_bytes = unsafe {
                    std::slice::from_raw_parts(
                        vertices.as_ptr() as *const u8,
                        vertices.len() * std::mem::size_of::<UIVertex>(),
                    )
                };
                buffer
                    .write(vertex_bytes)
                    .map_err(|e| format!("Failed to write vertex buffer: {}", e))?;

                self.vertex_buffer = Some(buffer);
            } else {
                // Reuse existing buffer - just update data
                if let Some(ref buf) = self.vertex_buffer {
                    let vertex_bytes = unsafe {
                        std::slice::from_raw_parts(
                            vertices.as_ptr() as *const u8,
                            vertices.len() * std::mem::size_of::<UIVertex>(),
                        )
                    };
                    buf.write(vertex_bytes)
                        .map_err(|e| format!("Failed to write vertex buffer: {}", e))?;
                }
            }
        }

        // Update or create index buffer (only if size changed)
        if !indices.is_empty() {
            let needed_size = indices.len() * std::mem::size_of::<u32>();

            // Only recreate if size changed significantly (with 10% headroom to reduce reallocations)
            if self.index_capacity < needed_size || self.index_capacity > needed_size * 2 {
                self.index_capacity = (needed_size as f32 * 1.2) as usize;

                let buffer = Buffer::new(
                    context,
                    self.index_capacity,
                    BufferUsage::INDEX,
                    MemoryType::CpuMapped,
                )
                .map_err(|e| format!("Failed to create index buffer: {}", e))?;

                // Write index data
                let index_bytes = unsafe {
                    std::slice::from_raw_parts(
                        indices.as_ptr() as *const u8,
                        indices.len() * std::mem::size_of::<u32>(),
                    )
                };
                buffer
                    .write(index_bytes)
                    .map_err(|e| format!("Failed to write index buffer: {}", e))?;

                self.index_buffer = Some(buffer);
            } else {
                // Reuse existing buffer - just update data
                if let Some(ref buf) = self.index_buffer {
                    let index_bytes = unsafe {
                        std::slice::from_raw_parts(
                            indices.as_ptr() as *const u8,
                            indices.len() * std::mem::size_of::<u32>(),
                        )
                    };
                    buf.write(index_bytes)
                        .map_err(|e| format!("Failed to write index buffer: {}", e))?;
                }
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
        if self.vertex_buffer.is_none() || self.index_buffer.is_none() || self.index_count == 0 {
            eprintln!("[egui] render() skipped - no geometry");
            return Ok(());
        }

        eprintln!(
            "[egui] render() drawing {} indices, {} vertices, screen ({:.1}x{:.1})",
            self.index_count, self.vertex_count, screen_width, screen_height
        );

        cmd.bind_pipeline(&self.pipeline);

        let pc = UIPushConstants {
            vertex_ptr: self.vertex_buffer.as_ref().unwrap().device_address(),
            window_width: screen_width,
            window_height: screen_height,
            _padding: 0,
        };

        let pc_bytes = unsafe {
            std::slice::from_raw_parts(
                (&pc as *const UIPushConstants) as *const u8,
                std::mem::size_of::<UIPushConstants>(),
            )
        };
        cmd.push_constants(&self.layout, pc_bytes);

        cmd.bind_index_buffer(
            self.index_buffer.as_ref().unwrap(),
            0,
            crate::simple::IndexType::U32,
        );
        cmd.draw_indexed(self.index_count as u32, 1, 0, 0, 0);

        Ok(())
    }

    pub fn pipeline(&self) -> &GraphicsPipeline {
        &self.pipeline
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
