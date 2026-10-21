//! The CPU engine for pages: the same 2D integer engine as the GPU (md-ref's `IntEngine`: the
//! same arithmetic as the GPU kernels, which the GPU matches except where its float division
//! rounds differently), compiled to WebAssembly with SIMD, and
//! drawn with a 2D canvas. It needs no WebGPU. For small systems it avoids the GPU's fixed cost
//! per dispatch, and readouts are free (the state is already in memory).

use crate::sims::options;
use crate::{err, obj};
use js_sys::{Float32Array, Uint32Array};
use md_core::fixed;
use md_ref::IntEngine;
use scenarios::Finish;
use wasm_bindgen::prelude::*;

/// A 2D particle simulation on this computer's processor, drawn into a canvas (2D context).
#[wasm_bindgen]
pub struct CpuDiscSim {
    engine: IntEngine<f32>,
    ctx: web_sys::CanvasRenderingContext2d,
    size: [u32; 2],
    radii: Vec<f64>,
    /// Four species colours, the highlight and the background, as CSS strings.
    colors: Vec<String>,
    view: [f32; 4],
    /// Overlay rectangles, 8 floats each: x0, y0, x1, y1 (σ), r, g, b, a.
    rects: Vec<f32>,
}

fn css(c: &[f32]) -> String {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("rgba({},{},{},{})", b(c[0]), b(c[1]), b(c[2]), c[3])
}

#[wasm_bindgen]
impl CpuDiscSim {
    /// Build scenario `name` with numeric `options`, run its warm-up, and draw into `canvas`.
    pub fn create(canvas: web_sys::HtmlCanvasElement, name: String, opts: JsValue) -> Result<CpuDiscSim, JsError> {
        let o = options(&opts);
        let sc = scenarios::build(&name, &o).map_err(err)?;
        let mut engine = IntEngine::<f32>::new(sc.sys.clone(), sc.state.clone());
        if sc.warmup > 0 || sc.finish != Finish::None {
            engine.step(sc.warmup);
            let mut state = engine.state.clone();
            sc.finish.apply(&mut state, &sc.sys);
            engine.state = state;
        }
        let ctx = canvas
            .get_context("2d")
            .map_err(|_| JsError::new("no 2D canvas context"))?
            .ok_or_else(|| JsError::new("no 2D canvas context"))?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .map_err(|_| JsError::new("no 2D canvas context"))?;
        let b = engine.sys.box_f64();
        let grey = "rgba(200,200,200,1)".to_string();
        Ok(CpuDiscSim {
            engine,
            ctx,
            size: [canvas.width(), canvas.height()],
            radii: sc.radii,
            colors: vec![grey.clone(), grey.clone(), grey.clone(), grey.clone(), grey, "rgba(0,0,0,1)".into()],
            view: [0.0, 0.0, b[0] as f32, b[1] as f32],
            rects: vec![],
        })
    }

    /// Spatial dimensions.
    pub fn dims(&self) -> u32 {
        2
    }

    pub fn step(&mut self, k: u32) {
        self.engine.step(k as usize);
    }

    /// Resolves at once: the work is already done.
    pub fn sync(&self) -> js_sys::Promise {
        js_sys::Promise::resolve(&JsValue::UNDEFINED)
    }

    pub fn reverse(&mut self) {
        self.engine.reverse();
    }

    pub fn steps(&self) -> f64 {
        self.engine.steps as f64
    }

    pub fn n(&self) -> u32 {
        self.engine.state.n() as u32
    }

    pub fn dt(&self) -> f64 {
        self.engine.sys.dt
    }

    pub fn box_size(&self) -> Vec<f64> {
        self.engine.sys.box_f64().to_vec()
    }

    pub fn masses(&self) -> Vec<f64> {
        self.engine.sys.masses.clone()
    }

    /// Resolves to `{pos, species}`: positions (σ, interleaved) and species, in id order.
    pub fn all_positions(&self) -> js_sys::Promise {
        let s = &self.engine.state;
        let pos: Vec<f32> = s.pos.iter().flat_map(|p| [fixed::to_f64(p[0]) as f32, fixed::to_f64(p[1]) as f32]).collect();
        let v = obj(&[("pos", Float32Array::from(&pos[..]).into()), ("species", Uint32Array::from(&s.species[..]).into())]);
        js_sys::Promise::resolve(&v)
    }

    /// Resolves to the positions (σ, interleaved, as f64, which holds every fixed-point position
    /// exactly) of the particles with the given ids.
    pub fn positions_of(&self, ids: Vec<u32>) -> js_sys::Promise {
        let p = &self.engine.state.pos;
        let out: Vec<f64> = ids.iter().flat_map(|&i| [fixed::to_f64(p[i as usize][0]), fixed::to_f64(p[i as usize][1])]).collect();
        js_sys::Promise::resolve(&JsValue::from(js_sys::Float64Array::from(&out[..])))
    }

    /// Resolves to the positions (σ, interleaved) of the particles of one species, in id order.
    pub fn positions(&self, species: u32) -> js_sys::Promise {
        let s = &self.engine.state;
        let pos: Vec<f32> = (0..s.n()).filter(|&i| s.species[i] == species).flat_map(|i| [fixed::to_f64(s.pos[i][0]) as f32, fixed::to_f64(s.pos[i][1]) as f32]).collect();
        js_sys::Promise::resolve(&JsValue::from(Float32Array::from(&pos[..])))
    }

    /// Resolves to the velocities (σ/τ, interleaved), in id order.
    pub fn velocities(&self) -> js_sys::Promise {
        let sys = &self.engine.sys;
        let v: Vec<f32> = self.engine.state.vel.iter().flat_map(|v| [sys.velocity(v[0]) as f32, sys.velocity(v[1]) as f32]).collect();
        js_sys::Promise::resolve(&JsValue::from(Float32Array::from(&v[..])))
    }

    /// Resolves to `{ke, species}`: each particle's kinetic energy (ε, from its half-step
    /// velocity, as the GPU engine reports it) and species.
    pub fn kinetic_energies(&self) -> js_sys::Promise {
        let (sys, s) = (&self.engine.sys, &self.engine.state);
        let ke: Vec<f32> = (0..s.n())
            .map(|i| {
                let m = sys.masses[s.species[i] as usize];
                let (vx, vy) = (sys.velocity(s.vel[i][0]), sys.velocity(s.vel[i][1]));
                (0.5 * m * (vx * vx + vy * vy)) as f32
            })
            .collect();
        js_sys::Promise::resolve(&obj(&[("ke", Float32Array::from(&ke[..]).into()), ("species", Uint32Array::from(&s.species[..]).into())]))
    }

    /// Resolves to the total potential energy (ε).
    pub fn potential_energy(&self) -> js_sys::Promise {
        js_sys::Promise::resolve(&JsValue::from_f64(self.engine.kicks().1))
    }

    /// Kick, freeze or shove the unpinned particles in the rectangle (x0, y0)–(x1, y1) (σ):
    /// velocities are multiplied by `scale`, then (vx, vy) σ/τ is added (as the GPU does it).
    #[allow(clippy::too_many_arguments)]
    pub fn kick_region(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, scale: f64, vx: f64, vy: f64) {
        let (lo, hi) = ([x0.min(x1) as f32, y0.min(y1) as f32], [x0.max(x1) as f32, y0.max(y1) as f32]);
        let sys = &self.engine.sys;
        let add = [sys.to_step_displacement(vx), sys.to_step_displacement(vy)];
        let s = &mut self.engine.state;
        for i in 0..s.n() {
            if sys.is_pinned(s.species[i]) {
                continue;
            }
            let (x, y) = (fixed::to_f64(s.pos[i][0]) as f32, fixed::to_f64(s.pos[i][1]) as f32);
            if x < lo[0] || x >= hi[0] || y < lo[1] || y >= hi[1] {
                continue;
            }
            for a in 0..2 {
                s.vel[i][a] = ((s.vel[i][a] as f32 * scale as f32).round_ties_even() as i32).wrapping_add(add[a]);
            }
        }
    }

    /// Set the species of the particles with these ids.
    pub fn set_species(&mut self, ids: Vec<u32>, species: u32) {
        for i in ids {
            self.engine.state.species[i as usize] = species;
        }
    }

    /// Retune the bulk heat bath (the scenario must have one).
    pub fn set_thermostat(&mut self, t: f64, rate: f64) {
        if self.engine.sys.thermostat.is_some() {
            self.engine.sys = self.engine.sys.clone().with_thermostat(t, rate);
        }
    }

    /// "About this simulation", as the GPU engine's, saying which engine ran.
    pub fn describe(&self, names: Vec<String>) -> Vec<String> {
        let mut d = self.engine.sys.describe(self.engine.state.n(), &names);
        d.push("Engine: this computer's processor (WebAssembly with SIMD), one thread, with the same integer positions and the same float force arithmetic as the GPU engine.".into());
        d
    }

    pub fn energy_note(&self) -> String {
        self.engine.sys.energy_note()
    }

    pub fn view(&self) -> Vec<f32> {
        self.view.to_vec()
    }

    pub fn set_view(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.view = [x0, y0, x1, y1];
    }

    /// Overlay rectangles, 8 floats each: x0, y0, x1, y1 (σ), r, g, b, a.
    pub fn set_overlay(&mut self, rects: Vec<f32>) {
        self.rects = rects;
    }

    /// 24 floats: four species colours, the highlight colour, the background (RGBA, sRGB).
    pub fn set_colors(&mut self, c: Vec<f32>) {
        self.colors = (0..6).map(|k| css(&c[4 * k..4 * k + 4])).collect();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.size = [width, height];
    }

    /// Draw the particles, species by species, and the overlay.
    pub fn render(&mut self) {
        let [w, h] = self.size.map(|x| x as f64);
        let ctx = &self.ctx;
        let [x0, y0, x1, y1] = self.view.map(|x| x as f64);
        let (sx, sy) = (w / (x1 - x0), h / (y1 - y0));
        let px = |x: f64| (x - x0) * sx;
        let py = |y: f64| h - (y - y0) * sy;
        ctx.set_fill_style_str(&self.colors[5]);
        ctx.fill_rect(0.0, 0.0, w, h);
        let s = &self.engine.state;
        for sp in 0..4u32 {
            let r = self.radii.get(sp as usize).copied().unwrap_or(0.5) * sx;
            ctx.set_fill_style_str(&self.colors[sp as usize]);
            ctx.begin_path();
            for i in 0..s.n() {
                if s.species[i] != sp {
                    continue;
                }
                let (x, y) = (px(fixed::to_f64(s.pos[i][0])), py(fixed::to_f64(s.pos[i][1])));
                ctx.move_to(x + r, y);
                let _ = ctx.arc(x, y, r.max(0.7), 0.0, std::f64::consts::TAU);
            }
            ctx.fill();
        }
        for q in self.rects.chunks_exact(8) {
            ctx.set_fill_style_str(&css(&q[4..8]));
            let (a, b) = (px(q[0] as f64), py(q[3] as f64));
            ctx.fill_rect(a, b, px(q[2] as f64) - a, py(q[1] as f64) - b);
        }
    }
}
