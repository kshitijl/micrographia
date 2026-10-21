//! The Shan–Chen lattice Boltzmann fluid (`lattice::lbm`) for pages, drawn from its GPU
//! buffers. Methods mirror `DiscSim`'s where they mean the same thing, so `SimHost` runs it.

use crate::sims::options;
use crate::{browser_gpu, err, obj};
use bytemuck::{Pod, Zeroable};
use gpu_prims::Gpu;
use js_sys::Float32Array;
use lattice::lbm::{Eos, LbmConditions, LbmConfig, ShanChen};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct F {
    w: u32,
    h: u32,
    mode: u32,
    _p: u32,
    view: [f32; 4],
    rho_v: f32,
    rho_l: f32,
    t_lo: f32,
    t_hi: f32,
    liquid: [f32; 4],
    vapour: [f32; 4],
    cool: [f32; 4],
    hot: [f32; 4],
    wall: [f32; 4],
}

struct FieldRenderer {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    groups: [wgpu::BindGroup; 2],
    params_buf: wgpu::Buffer,
    params: F,
}

impl FieldRenderer {
    fn new(gpu: &Gpu, surface: wgpu::Surface<'static>, sim: &ShanChen, width: u32, height: u32) -> Self {
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
        let module = gpu_prims::util::shader(gpu, "field", include_str!("../shaders/field.wgsl"));
        let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("field"),
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
        let (w, h) = (sim.config.w, sim.config.h);
        let params = F {
            w,
            h,
            mode: 0,
            _p: 0,
            view: [0.0, 0.0, w as f32, h as f32],
            rho_v: 0.0,
            rho_l: 0.35,
            t_lo: 0.0,
            t_hi: 0.1,
            liquid: [0.3, 0.5, 0.8, 1.0],
            vapour: [0.1, 0.08, 0.06, 1.0],
            cool: [0.25, 0.45, 0.9, 1.0],
            hot: [0.95, 0.3, 0.2, 1.0],
            wall: [0.4, 0.4, 0.4, 1.0],
        };
        let params_buf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("field params"), contents: bytemuck::bytes_of(&params), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST });
        let group = |t: &wgpu::Buffer| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: params_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: sim.rho.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: t.as_entire_binding() },
                ],
            })
        };
        let groups = [group(&sim.temp[0]), group(&sim.temp[1])];
        let mut me = Self { gpu: gpu.clone(), surface, config, pipeline, groups, params_buf, params };
        me.fit();
        me
    }

    fn fit(&mut self) {
        let (w, h) = (self.params.w as f32, self.params.h as f32);
        let (cw, ch) = (self.config.width as f32, self.config.height as f32);
        let scale = (w / cw).max(h / ch);
        let (vw, vh) = (cw * scale, ch * scale);
        self.params.view = [0.5 * (w - vw), 0.5 * (h - vh), 0.5 * (w + vw), 0.5 * (h + vh)];
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.gpu.device, &self.config);
        self.fit();
    }

    fn render(&mut self, current: usize) {
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
                label: Some("field"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.groups[current], &[]);
            pass.draw(0..3, 0..1);
        }
        self.gpu.queue.submit([enc.finish()]);
        self.gpu.queue.present(frame);
    }
}

/// A 2D Shan–Chen lattice Boltzmann fluid, drawn into a canvas.
#[wasm_bindgen]
pub struct FluidSim {
    sim: ShanChen,
    renderer: FieldRenderer,
    /// The bulk temperature (lattice units) and its coexisting (vapour, liquid) densities.
    t: f64,
    coexist: (f64, f64),
}

#[wasm_bindgen]
impl FluidSim {
    /// Options: w, h (sites; rows 0 and h − 1 are walls), temperature (as a fraction of the
    /// critical temperature), start (0 a pool, 1 a quench from uniform density, 2 a drop), fill
    /// (the pool's depth, or the quench's density relative to the critical one), gravity,
    /// wetting, seed. `name` is unused.
    pub async fn create(canvas: web_sys::HtmlCanvasElement, _name: String, opts: JsValue) -> Result<FluidSim, JsError> {
        let o = options(&opts);
        let (w, h) = (o.get("w", 256.0) as u32, o.get("h", 256.0) as u32);
        let eos = Eos::default();
        let (tc, rc) = eos.critical();
        let t = o.get("temperature", 0.8).clamp(0.6, 0.99) * tc;
        let (rv, rl) = eos.coexistence_sc(t).ok_or_else(|| err("above the critical point"))?;
        let fill = o.get("fill", 0.4);
        let seed = o.get("seed", 1.0) as u64;
        let rho: Vec<f32> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f64, (i / w) as f64);
                let noise = gpu_prims::threefry::uniform(seed, 21, i) - 0.5;
                match o.get("start", 0.0) as u32 {
                    1 => (rc * fill * (1.0 + 0.05 * noise)) as f32,
                    2 => {
                        let d = ((x - 0.5 * w as f64).powi(2) + (y - 0.55 * h as f64).powi(2)).sqrt();
                        if d < fill * 0.5 * w.min(h) as f64 { rl as f32 } else { rv as f32 }
                    }
                    _ => (if y < fill * h as f64 { rl } else { rv }) as f32,
                }
            })
            .collect();
        let temp = vec![t as f32; (w * h) as usize];
        let cond = LbmConditions { gravity: o.get("gravity", 0.0), wetting: o.get("wetting", 1.0), t_floor: t, t_ceiling: t, plate: (0, w), ..Default::default() };
        let (cw, ch) = (canvas.width(), canvas.height());
        let (gpu, surface) = browser_gpu(Some(canvas)).await?;
        let sim = ShanChen::new(&gpu, LbmConfig { w, h, eos }, cond, &rho, &temp);
        let mut renderer = FieldRenderer::new(&gpu, surface.ok_or_else(|| err("no surface"))?, &sim, cw, ch);
        renderer.params.rho_v = rv as f32;
        renderer.params.rho_l = rl as f32;
        renderer.params.t_lo = (0.6 * tc) as f32;
        renderer.params.t_hi = (1.1 * tc) as f32;
        Ok(FluidSim { sim, renderer, t, coexist: (rv, rl) })
    }

    pub fn dims(&self) -> u32 {
        2
    }

    pub fn step(&mut self, k: u32) {
        self.sim.step(k as usize);
    }

    pub fn sync(&self) -> js_sys::Promise {
        let (tx, rx) = futures_channel::oneshot::channel::<()>();
        self.sim.gpu().queue.on_submitted_work_done(move || {
            let _ = tx.send(());
        });
        future_to_promise(async move {
            let _ = rx.await;
            Ok(JsValue::UNDEFINED)
        })
    }

    pub fn steps(&self) -> f64 {
        self.sim.steps as f64
    }

    pub fn dt(&self) -> f64 {
        1.0
    }

    pub fn width(&self) -> u32 {
        self.sim.config.w
    }

    pub fn height(&self) -> u32 {
        self.sim.config.h
    }

    /// The critical temperature in lattice units.
    pub fn critical_temperature(&self) -> f64 {
        self.sim.config.eos.critical().0
    }

    /// The model's coexisting [vapour, liquid] densities at the bulk temperature.
    pub fn coexistence(&self) -> Vec<f64> {
        vec![self.coexist.0, self.coexist.1]
    }

    /// Resolves to `{rho, temp}` (Float32Arrays, row-major from the bottom).
    pub fn fields(&self) -> js_sys::Promise {
        let (r, t) = (self.sim.read_rho(), self.sim.read_temp());
        future_to_promise(async move { Ok(obj(&[("rho", Float32Array::from(&r.await[..]).into()), ("temp", Float32Array::from(&t.await[..]).into())])) })
    }

    /// Set the bulk temperature (a fraction of the critical one) everywhere: the temperature
    /// field is reset to it, and the walls are held at it.
    pub fn set_temperature(&mut self, fraction: f64) {
        let tc = self.critical_temperature();
        self.t = fraction.clamp(0.6, 0.99) * tc;
        if let Some(c) = self.sim.config.eos.coexistence_sc(self.t) {
            self.coexist = c;
            self.renderer.params.rho_v = c.0 as f32;
            self.renderer.params.rho_l = c.1 as f32;
        }
        let n = (self.sim.config.w * self.sim.config.h) as usize;
        self.sim.set_temperature_field(&vec![self.t as f32; n]);
        let k = LbmConditions { t_ceiling: self.t, t_floor: if self.sim.conditions.thermal { self.sim.conditions.t_floor } else { self.t }, ..self.sim.conditions };
        self.sim.set_conditions(k);
    }

    pub fn set_gravity(&mut self, g: f64) {
        let k = LbmConditions { gravity: g, ..self.sim.conditions };
        self.sim.set_conditions(k);
    }

    /// A hot plate across the middle `width` fraction of the floor at `fraction` of the critical
    /// temperature, with the temperature field then evolving (heat flows, and evaporation takes
    /// it up); 0 turns it off (the field stops evolving and keeps its last values).
    pub fn set_hot_plate(&mut self, fraction: f64, width: f64) {
        let tc = self.critical_temperature();
        let w = self.sim.config.w;
        let half = (0.5 * width.clamp(0.0, 1.0) * w as f64) as u32;
        let k = if fraction > 0.0 {
            LbmConditions { thermal: true, t_floor: fraction * tc, plate: (w / 2 - half, w / 2 + half), t_ceiling: self.t, ..self.sim.conditions }
        } else {
            LbmConditions { thermal: false, ..self.sim.conditions }
        };
        self.sim.set_conditions(k);
    }

    /// Put a drop of liquid (`liquid` true) or a bubble of vapour of radius `r` at (x, y) (sites,
    /// y up), at the coexisting densities.
    pub fn paint(&mut self, x: f32, y: f32, r: f32, liquid: bool) {
        let d = if liquid { self.coexist.1 } else { self.coexist.0 };
        self.sim.paint(x, y, r, d as f32);
    }

    /// 0: colour by density; 1: by temperature.
    pub fn color_mode(&mut self, mode: u32) {
        self.renderer.params.mode = mode;
    }

    pub fn render(&mut self) {
        self.renderer.render(self.sim.current());
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    pub fn view(&self) -> Vec<f32> {
        self.renderer.params.view.to_vec()
    }

    /// 24 floats as for `DiscSim`: species 0 is the liquid's colour, species 1 the walls', species
    /// 2 and 3 the cool and hot ends of the temperature colours, the last the vapour's.
    pub fn set_colors(&mut self, c: Vec<f32>) {
        let p = &mut self.renderer.params;
        p.liquid.copy_from_slice(&c[0..4]);
        p.wall.copy_from_slice(&c[4..8]);
        p.cool.copy_from_slice(&c[8..12]);
        p.hot.copy_from_slice(&c[12..16]);
        p.vapour.copy_from_slice(&c[20..24]);
    }

    pub fn describe(&self, _names: Vec<String>) -> Vec<String> {
        let c = &self.sim.config;
        let k = &self.sim.conditions;
        let (tc, _) = c.eos.critical();
        let mut out = vec![
            format!("Method: lattice Boltzmann (D2Q9, BGK collisions), {} × {} sites: each site carries the numbers of fluid particles moving in 9 directions, which stream to the neighbouring sites and relax towards local equilibrium, giving the Navier–Stokes equations for a flowing fluid.", c.w, c.h),
            format!("Forces: the Shan–Chen pseudopotential, an attraction between neighbouring sites chosen so the fluid's pressure follows the Carnahan–Starling equation of state (a = {}, b = {}; critical temperature {tc:.4} in lattice units), which makes it split into liquid and vapour below the critical point.", c.eos.a, c.eos.b),
            "Boundaries: periodic left–right; solid walls top and bottom (bounce-back: the fluid sticks to them).".into(),
        ];
        if k.gravity != 0.0 {
            out.push(format!("Gravity: {} (lattice units).", k.gravity));
        }
        out.push(if k.thermal {
            format!("Heat: a temperature field carried by the flow, diffusing (diffusivity {}), and cooled where liquid expands into vapour (the latent heat); the hot plate is held at {:.2} T_c and the ceiling at {:.2} T_c.", k.chi, k.t_floor / tc, k.t_ceiling / tc)
        } else {
            format!("Temperature: held at {:.2} T_c everywhere (an isothermal fluid).", self.t / tc)
        });
        out.push("Ensemble: mass is conserved; momentum too, apart from gravity and the walls. With the hot plate on, heat flows in through it and out through the ceiling.".into());
        out
    }

    pub fn energy_note(&self) -> String {
        "The fluid's temperature is held (or set by the hot plate and the ceiling): heat flows in and out through them.".into()
    }
}
