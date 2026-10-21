//! Draw a 3D simulation as lit spheres straight from the engine's GPU buffers (see
//! `spheres.wgsl`), seen by an orbiting camera, with the box's edges.

use bytemuck::{Pod, Zeroable};
use gpu_prims::Gpu;
use md::GpuEngine3;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct R3 {
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub radii: [f32; 4],
    pub colors: [[f32; 4]; 4],
    pub highlight_color: [f32; 4],
    pub cool_color: [f32; 4],
    pub mid_color: [f32; 4],
    pub hot_color: [f32; 4],
    pub background: [f32; 4],
    pub edge_color: [f32; 4],
    pub box_size: [f32; 4],
    pub slab: [f32; 4],
    pub color_mode: u32,
    pub speed_scale: f32,
    pub highlight: u32,
    pub _p: u32,
}

pub const NO_HIGHLIGHT: u32 = u32::MAX;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Where the camera is: angles about the box centre, and distance in box diagonals.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

/// World → view (column-major), camera at `eye` looking at `target`, y up.
fn look_at(eye: V3, target: V3) -> [[f32; 4]; 4] {
    let f = normalize(sub(target, eye));
    let s = normalize(cross(f, [0.0, 1.0, 0.0]));
    let u = cross(s, f);
    [[s[0], u[0], -f[0], 0.0], [s[1], u[1], -f[1], 0.0], [s[2], u[2], -f[2], 0.0], [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0]]
}

/// View → clip for WebGPU (depth 0 at `near`, 1 at `far`), column-major.
fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (0.5 * fovy).tan();
    let r = far / (near - far);
    [[f / aspect, 0.0, 0.0, 0.0], [0.0, f, 0.0, 0.0], [0.0, 0.0, r, -1.0], [0.0, 0.0, r * near, 0.0]]
}

pub struct Renderer3 {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    spheres: wgpu::RenderPipeline,
    highlight: wgpu::RenderPipeline,
    edges: wgpu::RenderPipeline,
    sphere_group: wgpu::BindGroup,
    highlight_group: wgpu::BindGroup,
    edge_group: wgpu::BindGroup,
    depth: wgpu::TextureView,
    params_buf: wgpu::Buffer,
    /// The page's per-particle values (by id), for `color_mode` 2.
    pub values: wgpu::Buffer,
    pub params: R3,
    pub camera: Camera,
    /// Fraction of the box cut away from the front (0: none), so the inside shows.
    pub cut: f32,
    n: u32,
    box_size: V3,
}

impl Renderer3 {
    pub fn new(gpu: &Gpu, surface: wgpu::Surface<'static>, engine: &GpuEngine3, radii: &[f64], width: u32, height: u32) -> Self {
        let caps = surface.get_capabilities(&gpu.adapter);
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
        let module = gpu_prims::util::shader(gpu, "spheres", include_str!("../shaders/spheres.wgsl"));
        let pipeline = |vs: &str, fs: &str, topology: wgpu::PrimitiveTopology, highlight: bool| {
            let constants: &[(&str, f64)] = if highlight { &[("highlight_only", 1.0)] } else { &[] };
            gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(vs),
                layout: None,
                vertex: wgpu::VertexState { module: &module, entry_point: Some(vs), compilation_options: wgpu::PipelineCompilationOptions { constants, ..Default::default() }, buffers: &[] },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let spheres = pipeline("vs", "fs", wgpu::PrimitiveTopology::TriangleList, false);
        let highlight = pipeline("vs", "fs", wgpu::PrimitiveTopology::TriangleList, true);
        let edges = pipeline("vs_edges", "fs_edges", wgpu::PrimitiveTopology::LineList, false);
        let b = engine.sys.box_f64();
        let mut r = [0.5f32; 4];
        for (k, v) in radii.iter().enumerate().take(4) {
            r[k] = *v as f32;
        }
        let ident = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        let params = R3 {
            view: ident,
            proj: ident,
            radii: r,
            colors: [[0.36, 0.22, 0.14, 1.0], [0.96, 0.90, 0.78, 1.0], [0.3, 0.5, 0.8, 1.0], [0.8, 0.3, 0.3, 1.0]],
            highlight_color: [0.9, 0.2, 0.2, 1.0],
            cool_color: [0.25, 0.45, 0.9, 1.0],
            mid_color: [0.0; 4],
            hot_color: [0.95, 0.3, 0.2, 1.0],
            background: [0.1, 0.08, 0.06, 1.0],
            edge_color: [1.0, 1.0, 1.0, 0.35],
            box_size: [b[0] as f32, b[1] as f32, b[2] as f32, 0.0],
            slab: [-1e9, 1e9, -1.0, 1.0],
            color_mode: 0,
            speed_scale: 0.0,
            highlight: NO_HIGHLIGHT,
            _p: 0,
        };
        let params_buf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("render3 params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let values = gpu_prims::util::storage(gpu, "render3 values", engine.n() as u64 * 4);
        let group_for = |p: &wgpu::RenderPipeline| {
            let e = &engine.buf;
            let entries = [
                wgpu::BindGroupEntry { binding: 0, resource: params_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: e.pos.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: e.species.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: e.ids.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: e.vel.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: values.as_entire_binding() },
            ];
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &p.get_bind_group_layout(0), entries: &entries })
        };
        let sphere_group = group_for(&spheres);
        let highlight_group = group_for(&highlight);
        let edge_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &edges.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: params_buf.as_entire_binding() }],
        });
        let depth = Self::depth_view(gpu, &config);
        Self {
            gpu: gpu.clone(),
            surface,
            config,
            spheres,
            highlight,
            edges,
            sphere_group,
            highlight_group,
            edge_group,
            depth,
            params_buf,
            values,
            params,
            camera: Camera { yaw: 0.6, pitch: 0.35, distance: 1.25 },
            cut: 0.0,
            n: engine.n(),
            box_size: [b[0] as f32, b[1] as f32, b[2] as f32],
        }
    }

    fn depth_view(gpu: &Gpu, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
        let t = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: config.width, height: config.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        t.create_view(&Default::default())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.gpu.device, &self.config);
        self.depth = Self::depth_view(&self.gpu, &self.config);
    }

    /// Camera matrices, the cut-away slab and depth cueing from the camera and `cut`.
    fn update_view(&mut self) {
        let b = self.box_size;
        let centre = [0.5 * b[0], 0.5 * b[1], 0.5 * b[2]];
        let diag = dot(b, b).sqrt();
        let Camera { yaw, pitch, distance } = self.camera;
        let d = distance * diag;
        let eye = [centre[0] + d * pitch.cos() * yaw.sin(), centre[1] + d * pitch.sin(), centre[2] + d * pitch.cos() * yaw.cos()];
        let view = look_at(eye, centre);
        let aspect = self.config.width as f32 / self.config.height as f32;
        // Fit the box: the field of view grows when the canvas is narrow.
        let fovy = 2.0 * ((0.5 * diag / d) / aspect.min(1.0)).atan();
        let (near, far) = ((d - 0.75 * diag).max(0.05 * diag), d + 0.75 * diag);
        self.params.view = view;
        self.params.proj = perspective(fovy, aspect, near, far);
        // View-space z of the box centre is −d; its extent along the view axis is ±diag/2.
        let front = -d + 0.5 * diag;
        let back = -d - 0.5 * diag;
        self.params.slab = [-1e9, front - self.cut * diag, back, front];
        // Keep spheres at least ~0.7 px across.
        let px_per_sigma = self.config.height as f32 / (2.0 * d * (0.5 * fovy).tan());
        self.params.box_size[3] = 0.7 / px_per_sigma;
    }

    /// Upload the page's per-particle values (by id).
    pub fn gpu_write_values(&self, values: &[f32]) {
        self.gpu.queue.write_buffer(&self.values, 0, bytemuck::cast_slice(values));
    }

    pub fn render(&mut self) {
        self.update_view();
        self.gpu.queue.write_buffer(&self.params_buf, 0, bytemuck::bytes_of(&self.params));
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                self.surface.configure(&self.gpu.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let bg = self.params.background;
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("spheres"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: bg[0] as f64, g: bg[1] as f64, b: bg[2] as f64, a: bg[3] as f64 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.spheres);
            pass.set_bind_group(0, &self.sphere_group, &[]);
            pass.draw(0..6, 0..self.n);
            if self.params.highlight != NO_HIGHLIGHT {
                pass.set_pipeline(&self.highlight);
                pass.set_bind_group(0, &self.highlight_group, &[]);
                pass.draw(0..6, 0..self.n);
            }
            pass.set_pipeline(&self.edges);
            pass.set_bind_group(0, &self.edge_group, &[]);
            pass.draw(0..24, 0..1);
        }
        self.gpu.queue.submit([enc.finish()]);
        self.gpu.queue.present(frame);
    }
}
