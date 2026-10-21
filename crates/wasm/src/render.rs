//! Draw particles as instanced discs straight from the engine's GPU buffers, plus overlay
//! rectangles (box outlines, highlights) supplied by the page.

use bytemuck::{Pod, Zeroable};
use gpu_prims::Gpu;
use md::GpuEngine;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct RParams {
    pub view_lo: [f32; 2],
    pub view_hi: [f32; 2],
    pub min_radius: f32,
    pub highlight: u32,
    pub speed_scale: f32,
    /// Nonzero: colour by the running average of each molecule's speed (see `smooth_heat`).
    pub smoothed: f32,
    pub radii: [f32; 4],
    pub colors: [[f32; 4]; 4],
    pub highlight_color: [f32; 4],
    pub cool_color: [f32; 4],
    pub hot_color: [f32; 4],
    /// Middle of the speed ramp; unused while its alpha is zero.
    pub mid_color: [f32; 4],
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
    /// The region the page asked to see; the drawn view is widened to the canvas's shape.
    requested_view: Option<([f32; 2], [f32; 2])>,
    heat_params: wgpu::Buffer,
    smooth: gpu_prims::util::Dispatch,
    /// The running average has not been started (the next update replaces it).
    heat_fresh: bool,
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
            view_lo: [0.0, 0.0],
            view_hi: [b[0] as f32, b[1] as f32],
            min_radius: 0.0,
            highlight: NO_HIGHLIGHT,
            speed_scale: 0.0,
            smoothed: 0.0,
            radii: r,
            colors: [[0.36, 0.22, 0.14, 1.0], [0.96, 0.90, 0.78, 1.0], [0.3, 0.5, 0.8, 1.0], [0.8, 0.3, 0.3, 1.0]],
            highlight_color: [0.9, 0.2, 0.2, 1.0],
            cool_color: [0.25, 0.45, 0.9, 1.0],
            hot_color: [0.95, 0.3, 0.2, 1.0],
            mid_color: [0.0; 4],
        };
        // The running average of each molecule's squared speed, by id (see heat.wgsl).
        let heat = gpu_prims::util::storage(gpu, "heat", engine.n() as u64 * 4);
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
                wgpu::BindGroupEntry { binding: 5, resource: engine.buf.vel.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: heat.as_entire_binding() },
            ];
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &p.get_bind_group_layout(0), entries: &entries })
        };
        let heat_params = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heat params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let heat_module = gpu_prims::util::shader(gpu, "heat", include_str!("../shaders/heat.wgsl"));
        let smooth = gpu_prims::util::Dispatch::new_at(
            gpu,
            &gpu_prims::util::pipeline(gpu, &heat_module, "smooth_heat"),
            &[(0, &heat_params), (3, &engine.buf.ids), (5, &engine.buf.vel), (6, &heat)],
            gpu_prims::util::groups(engine.n(), 256),
        );
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
            requested_view: None,
            heat_params,
            smooth,
            heat_fresh: true,
        };
        me.update_min_radius();
        me
    }

    /// Fold the current velocities into the running average of each molecule's squared speed,
    /// after `elapsed` time units, averaging over about `window` time units.
    pub fn smooth_heat(&mut self, elapsed: f64, window: f64) {
        let alpha = if self.heat_fresh { 1.0 } else { 1.0 - (-elapsed / window).exp() } as f32;
        self.heat_fresh = false;
        let p: [u32; 4] = [self.n, alpha.to_bits(), 0, 0];
        self.gpu.queue.write_buffer(&self.heat_params, 0, bytemuck::cast_slice(&p));
        gpu_prims::util::run(&self.gpu, &[&self.smooth]);
    }

    /// Start the running average afresh at the next update.
    pub fn reset_heat(&mut self) {
        self.heat_fresh = true;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.gpu.device, &self.config);
        if let Some((lo, hi)) = self.requested_view {
            self.set_view(lo, hi);
        }
        self.update_min_radius();
    }

    /// Keep discs at least about 0.7 px in radius.
    fn update_min_radius(&mut self) {
        let px_per_sigma = self.config.width as f32 / (self.params.view_hi[0] - self.params.view_lo[0]);
        self.params.min_radius = 0.7 / px_per_sigma;
    }

    /// Show the region [lo, hi] of the box (σ), widened in one direction (about its centre) to
    /// the canvas's aspect ratio so discs stay round.
    pub fn set_view(&mut self, lo: [f32; 2], hi: [f32; 2]) {
        self.requested_view = Some((lo, hi));
        let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
        let canvas = self.config.width as f32 / self.config.height as f32;
        let (mut lo, mut hi) = (lo, hi);
        if w / h < canvas {
            let extra = 0.5 * (h * canvas - w);
            lo[0] -= extra;
            hi[0] += extra;
        } else {
            let extra = 0.5 * (w / canvas - h);
            lo[1] -= extra;
            hi[1] += extra;
        }
        self.params.view_lo = lo;
        self.params.view_hi = hi;
        self.update_min_radius();
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
