//! Draw particles as instanced discs straight from the engine's GPU buffers, plus overlay
//! rectangles (box outlines, highlights) supplied by the page.

use bytemuck::{Pod, Zeroable};
use gpu_prims::Gpu;
use md::GpuEngine;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct RParams {
    pub box_f: [f32; 2],
    pub min_radius: f32,
    pub highlight: u32,
    pub radii: [f32; 4],
    pub colors: [[f32; 4]; 4],
    pub highlight_color: [f32; 4],
}

pub const NO_HIGHLIGHT: u32 = u32::MAX;

/// Floats per overlay rectangle: x0, y0, x1, y1 (σ), r, g, b, a.
pub const RECT_FLOATS: usize = 8;

pub struct Renderer {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    discs: wgpu::RenderPipeline,
    highlight: wgpu::RenderPipeline,
    rects: wgpu::RenderPipeline,
    disc_group: wgpu::BindGroup,
    highlight_group: wgpu::BindGroup,
    rect_group: Option<(wgpu::BindGroup, u32)>,
    params_buf: wgpu::Buffer,
    pub params: RParams,
    pub background: wgpu::Color,
    n: u32,
}

impl Renderer {
    pub fn new(gpu: &Gpu, surface: wgpu::Surface<'static>, engine: &GpuEngine, radii: &[f64], width: u32, height: u32) -> Self {
        let caps = surface.get_capabilities(&gpu.adapter);
        // Colours are given in sRGB, so prefer a non-sRGB target and write them unchanged.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&gpu.device, &config);
        let module = gpu_prims::util::shader(gpu, "discs", include_str!("../shaders/discs.wgsl"));
        let pipeline = |vs: &str, fs: &str, blend: Option<wgpu::BlendState>, highlight: bool| {
            let constants: &[(&str, f64)] = if highlight { &[("highlight_only", 1.0)] } else { &[] };
            gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(vs),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    compilation_options: wgpu::PipelineCompilationOptions { constants, ..Default::default() },
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let discs = pipeline("vs", "fs", None, false);
        let highlight = pipeline("vs", "fs", None, true);
        let rects = pipeline("vs_rect", "fs_rect", Some(wgpu::BlendState::ALPHA_BLENDING), false);
        let b = engine.sys.box_f64();
        let mut r = [0.5f32; 4];
        for (k, v) in radii.iter().enumerate().take(4) {
            r[k] = *v as f32;
        }
        let params = RParams {
            box_f: [b[0] as f32, b[1] as f32],
            min_radius: 0.0,
            highlight: NO_HIGHLIGHT,
            radii: r,
            colors: [[0.36, 0.22, 0.14, 1.0], [0.96, 0.90, 0.78, 1.0], [0.3, 0.5, 0.8, 1.0], [0.8, 0.3, 0.3, 1.0]],
            highlight_color: [0.9, 0.2, 0.2, 1.0],
        };
        let params_buf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("render params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let group_for = |p: &wgpu::RenderPipeline| {
            let entries = [
                wgpu::BindGroupEntry { binding: 0, resource: params_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: engine.buf.pos.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: engine.buf.species.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: engine.buf.ids.as_entire_binding() },
            ];
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &p.get_bind_group_layout(0), entries: &entries })
        };
        let disc_group = group_for(&discs);
        let highlight_group = group_for(&highlight);
        let mut me = Self {
            gpu: gpu.clone(),
            surface,
            config,
            discs,
            highlight,
            rects,
            disc_group,
            highlight_group,
            rect_group: None,
            params_buf,
            params,
            background: wgpu::Color { r: 0.1, g: 0.08, b: 0.06, a: 1.0 },
            n: engine.n(),
        };
        me.update_min_radius();
        me
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.gpu.device, &self.config);
        self.update_min_radius();
    }

    /// Keep discs at least about 0.7 px in radius.
    fn update_min_radius(&mut self) {
        let px_per_sigma = self.config.width as f32 / self.params.box_f[0];
        self.params.min_radius = 0.7 / px_per_sigma;
    }

    /// Replace the overlay rectangles (`RECT_FLOATS` floats each).
    pub fn set_rects(&mut self, data: &[f32]) {
        let count = (data.len() / RECT_FLOATS) as u32;
        if count == 0 {
            self.rect_group = None;
            return;
        }
        let buf = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rects"),
            contents: bytemuck::cast_slice(&data[..count as usize * RECT_FLOATS]),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let group = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.rects.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: buf.as_entire_binding() },
            ],
        });
        self.rect_group = Some((group, count));
    }

    pub fn render(&mut self) {
        self.gpu.queue.write_buffer(&self.params_buf, 0, bytemuck::bytes_of(&self.params));
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                self.surface.configure(&self.gpu.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("discs"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(self.background), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.discs);
            pass.set_bind_group(0, &self.disc_group, &[]);
            pass.draw(0..6, 0..self.n);
            if self.params.highlight != NO_HIGHLIGHT {
                pass.set_pipeline(&self.highlight);
                pass.set_bind_group(0, &self.highlight_group, &[]);
                pass.draw(0..6, 0..self.n);
            }
            if let Some((group, count)) = &self.rect_group {
                pass.set_pipeline(&self.rects);
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..6, 0..*count);
            }
        }
        self.gpu.queue.submit([enc.finish()]);
        self.gpu.queue.present(frame);
    }
}
