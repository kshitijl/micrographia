//! Draw particles as instanced discs straight from the engine's GPU buffers.

use bytemuck::{Pod, Zeroable};
use gpu_prims::Gpu;
use md::GpuEngine;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct RParams {
    pub box_f: [f32; 2],
    pub radius: f32,
    pub highlight: u32,
    pub colors: [[f32; 4]; 4],
    pub highlight_color: [f32; 4],
}

pub const NO_HIGHLIGHT: u32 = u32::MAX;

pub struct Renderer {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params_buf: wgpu::Buffer,
    pub params: RParams,
    pub background: wgpu::Color,
    n: u32,
}

impl Renderer {
    pub fn new(gpu: &Gpu, surface: wgpu::Surface<'static>, engine: &GpuEngine, width: u32, height: u32) -> Self {
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
        let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("discs"),
            layout: None,
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let b = engine.sys.box_f64();
        let params = RParams {
            box_f: [b[0] as f32, b[1] as f32],
            radius: 0.5,
            highlight: NO_HIGHLIGHT,
            colors: [[0.36, 0.22, 0.14, 1.0], [0.96, 0.90, 0.78, 1.0], [0.3, 0.5, 0.8, 1.0], [0.8, 0.3, 0.3, 1.0]],
            highlight_color: [0.9, 0.2, 0.2, 1.0],
        };
        let params_buf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("render params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: params_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: engine.buf.pos.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: engine.buf.species.as_entire_binding() },
            ],
        });
        Self {
            gpu: gpu.clone(),
            surface,
            config,
            pipeline,
            bind_group,
            params_buf,
            params,
            background: wgpu::Color { r: 0.1, g: 0.08, b: 0.06, a: 1.0 },
            n: engine.n(),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.gpu.device, &self.config);
    }

    /// Disc radius in σ, at least about one pixel.
    pub fn set_radius(&mut self, sigma_radius: f32) {
        let px_per_sigma = self.config.width as f32 / self.params.box_f[0];
        self.params.radius = sigma_radius.max(0.7 / px_per_sigma);
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..6, 0..self.n);
            if self.params.highlight < self.n {
                let h = self.params.highlight;
                pass.draw(0..6, h..h + 1);
            }
        }
        self.gpu.queue.submit([enc.finish()]);
        self.gpu.queue.present(frame);
    }
}
