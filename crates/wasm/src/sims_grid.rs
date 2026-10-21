//! Lattice simulations for pages: the lattice gas (`lattice::gas`), drawn as an image straight
//! from its GPU buffer. Its methods mirror `DiscSim`'s where they mean the same thing (`step`,
//! `sync`, `render`, ...), so `SimHost` can run it; one step is one Monte Carlo sweep.

use crate::sims::options;
use crate::{browser_gpu, err, obj};
use bytemuck::{Pod, Zeroable};
use gpu_prims::Gpu;
use js_sys::Uint32Array;
use lattice::gas::{GasConditions, GasConfig, LatticeGas, Start};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct G {
    w: u32,
    h: u32,
    k: u32,
    hot_rows: u32,
    view: [f32; 4],
    filled: [f32; 4],
    empty: [f32; 4],
    hot: [f32; 4],
}

struct GridRenderer {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    params_buf: wgpu::Buffer,
    params: G,
}

impl GridRenderer {
    fn new(gpu: &Gpu, surface: wgpu::Surface<'static>, occ: &wgpu::Buffer, w: u32, h: u32, width: u32, height: u32) -> Self {
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
        let module = gpu_prims::util::shader(gpu, "grid", include_str!("../shaders/grid.wgsl"));
        let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grid"),
            layout: None,
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let params = G { w, h, k: 1, hot_rows: 0, view: [0.0, 0.0, w as f32, h as f32], filled: [0.3, 0.5, 0.8, 1.0], empty: [0.1, 0.08, 0.06, 1.0], hot: [0.9, 0.3, 0.2, 1.0] };
        let params_buf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("grid params"), contents: bytemuck::bytes_of(&params), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST });
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: params_buf.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: occ.as_entire_binding() }],
        });
        let mut me = Self { gpu: gpu.clone(), surface, config, pipeline, group, params_buf, params };
        me.fit();
        me
    }

    /// Show the whole lattice, letterboxed to the canvas's shape, averaging the sites under
    /// each pixel.
    fn fit(&mut self) {
        let (w, h) = (self.params.w as f32, self.params.h as f32);
        let (cw, ch) = (self.config.width as f32, self.config.height as f32);
        let scale = (w / cw).max(h / ch);
        let (vw, vh) = (cw * scale, ch * scale);
        self.params.view = [0.5 * (w - vw), 0.5 * (h - vh), 0.5 * (w + vw), 0.5 * (h + vh)];
        self.params.k = (scale.round() as u32).clamp(1, 4);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.gpu.device, &self.config);
        self.fit();
    }

    fn render(&mut self) {
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
                label: Some("grid"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.gpu.queue.submit([enc.finish()]);
        self.gpu.queue.present(frame);
    }
}

/// A 2D lattice gas under Kawasaki Monte Carlo, drawn into a canvas.
#[wasm_bindgen]
pub struct GasSim {
    gas: LatticeGas,
    renderer: GridRenderer,
}

#[wasm_bindgen]
impl GasSim {
    /// Options: w, h (sites; w a multiple of 6, h even), periodic (1: y periodic, else walls),
    /// wet (1: the floor is wetted), temperature, gravity (ε per row), hot_rows, hot (its kT),
    /// start (0 slab, 1 pool, 2 random), fill, seed. `name` is unused (one model).
    pub async fn create(canvas: web_sys::HtmlCanvasElement, _name: String, opts: JsValue) -> Result<GasSim, JsError> {
        let o = options(&opts);
        let w = ((o.get("w", 300.0) as u32) / 6).max(1) * 6;
        let periodic = o.get("periodic", 0.0) > 0.5;
        let h = if periodic { ((o.get("h", 300.0) as u32) / 6).max(1) * 6 } else { ((o.get("h", 300.0) as u32) / 2).max(3) * 2 };
        let config = GasConfig { w, h, periodic_y: periodic, wet_floor: o.get("wet", 0.0) > 0.5, wet_ceiling: false, seed: o.get("seed", 1.0) as u64 };
        let cond = GasConditions { t: o.get("temperature", 0.45), g: o.get("gravity", 0.0), hot_rows: o.get("hot_rows", 0.0) as u32, t_hot: o.get("hot", 0.0) };
        let start = match o.get("start", 0.0) as u32 {
            1 => Start::Pool,
            2 => Start::Random,
            _ => Start::Slab,
        };
        let occ = lattice::gas::initial(&config, start, o.get("fill", 0.5));
        let (cw, ch) = (canvas.width(), canvas.height());
        let (gpu, surface) = browser_gpu(Some(canvas)).await?;
        let gas = LatticeGas::new(&gpu, config, cond, &occ);
        let mut renderer = GridRenderer::new(&gpu, surface.ok_or_else(|| err("no surface"))?, &gas.occ, w, h, cw, ch);
        renderer.params.hot_rows = cond.hot_rows;
        Ok(GasSim { gas, renderer })
    }

    pub fn dims(&self) -> u32 {
        2
    }

    /// Run `k` Monte Carlo sweeps (every bond tried once per sweep).
    pub fn step(&mut self, k: u32) {
        self.gas.sweep(k as usize);
    }

    pub fn sync(&self) -> js_sys::Promise {
        let (tx, rx) = futures_channel::oneshot::channel::<()>();
        self.gas.gpu().queue.on_submitted_work_done(move || {
            let _ = tx.send(());
        });
        future_to_promise(async move {
            let _ = rx.await;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Sweeps so far.
    pub fn steps(&self) -> f64 {
        self.gas.sweeps as f64
    }

    /// The time unit is one sweep.
    pub fn dt(&self) -> f64 {
        1.0
    }

    pub fn width(&self) -> u32 {
        self.gas.config.w
    }

    pub fn height(&self) -> u32 {
        self.gas.config.h
    }

    pub fn molecules_total(&self) -> js_sys::Promise {
        let fut = self.gas.read_rows();
        future_to_promise(async move { Ok((fut.await.molecules.iter().map(|&m| m as f64).sum::<f64>()).into()) })
    }

    /// Resolves to `{molecules, bonds, energy}`: molecules and bonds per row (bottom first), and
    /// the total energy (ε).
    pub fn rows(&self) -> js_sys::Promise {
        let fut = self.gas.read_rows();
        let g = self.gas.conditions.g;
        future_to_promise(async move {
            let r = fut.await;
            let e = r.energy(g);
            Ok(obj(&[("molecules", Uint32Array::from(&r.molecules[..]).into()), ("bonds", Uint32Array::from(&r.bonds[..]).into()), ("energy", e.into())]))
        })
    }

    /// Resolves to a Uint32Array: molecules in each 6 × 6 block, row-major from the bottom,
    /// `width / 6` blocks per row.
    pub fn blocks(&self) -> js_sys::Promise {
        let fut = self.gas.read_blocks();
        future_to_promise(async move { Ok(Uint32Array::from(&fut.await[..]).into()) })
    }

    pub fn set_temperature(&mut self, t: f64) {
        let k = GasConditions { t, ..self.gas.conditions };
        self.gas.set_conditions(k);
    }

    pub fn set_gravity(&mut self, g: f64) {
        let k = GasConditions { g, ..self.gas.conditions };
        self.gas.set_conditions(k);
    }

    /// Hold the bottom `rows` rows at temperature `t` (0 rows: no hot plate).
    pub fn set_hot_plate(&mut self, rows: u32, t: f64) {
        let k = GasConditions { hot_rows: rows, t_hot: t, ..self.gas.conditions };
        self.gas.set_conditions(k);
        self.renderer.params.hot_rows = rows;
    }

    /// Add molecules (`molecule` true) or take them away within `r` sites of (x, y) (sites, y
    /// up), each site with probability `prob`.
    pub fn paint(&mut self, x: f32, y: f32, r: f32, molecule: bool, prob: f32) {
        self.gas.paint(x, y, r, molecule, prob);
    }

    pub fn render(&mut self) {
        self.renderer.render();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    /// The visible region in sites: [x0, y0, x1, y1] (y up), for turning pointer positions
    /// into sites.
    pub fn view(&self) -> Vec<f32> {
        self.renderer.params.view.to_vec()
    }

    /// 24 floats as for `DiscSim`: species 0 is the molecules' colour, the highlight the hot
    /// plate's, the last the empty sites'.
    pub fn set_colors(&mut self, c: Vec<f32>) {
        let p = &mut self.renderer.params;
        p.filled.copy_from_slice(&c[0..4]);
        p.hot.copy_from_slice(&c[16..20]);
        p.empty.copy_from_slice(&c[20..24]);
    }

    pub fn describe(&self, _names: Vec<String>) -> Vec<String> {
        let c = &self.gas.config;
        let k = &self.gas.conditions;
        let mut out = vec![
            format!("Method: Monte Carlo on a lattice (a lattice gas, equivalent to the Ising model): {} × {} sites, each empty or holding one molecule. A step is one sweep: every pair of neighbouring sites is tried once, and a molecule hops to an empty neighbour with the Metropolis probability min(1, e^(−ΔE/kT)) (Kawasaki dynamics, which conserves the molecules).", c.w, c.h),
            "Forces: neighbouring molecules attract with energy −ε each; nothing else (no velocities: the moves stand in for thermal motion).".into(),
            format!("Boundaries: periodic left–right; {}.", if c.periodic_y { "periodic top–bottom".to_string() } else { format!("walls top and bottom (the floor {} wetted by the liquid)", if c.wet_floor { "is" } else { "is not" }) }),
        ];
        if k.g != 0.0 {
            out.push(format!("Gravity: {} ε per molecule per row of height.", k.g));
        }
        if k.hot_rows > 0 {
            out.push(format!("Hot plate: the bottom {} rows are held at kT = {}.", k.hot_rows, k.t_hot));
        }
        out.push(format!("Ensemble: canonical (constant N, V, T): the Monte Carlo moves are a heat bath at kT = {}; energy flows in and out of it. The exact critical point of this model is kT_c = 0.567 ε (Onsager).", k.t));
        out
    }

    pub fn energy_note(&self) -> String {
        "A heat bath (constant temperature, the canonical ensemble): Monte Carlo moves at the set temperature exchange energy with it.".into()
    }
}
