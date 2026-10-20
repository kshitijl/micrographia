//! Simulations exposed to pages.

use crate::render::{Renderer, NO_HIGHLIGHT};
use crate::{browser_gpu, err, obj};
use gpu_prims::Prims;
use md::observe::{mixing, Observer};
use md::GpuEngine;
use md_core::{init, Pair, State, System};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

fn opt(options: &JsValue, key: &str, default: f64) -> f64 {
    js_sys::Reflect::get(options, &JsValue::from_str(key)).ok().and_then(|v| v.as_f64()).unwrap_or(default)
}

/// A 2D disc fluid on the GPU with a renderer, an observer and a saved initial state.
#[wasm_bindgen]
pub struct DiscSim {
    engine: GpuEngine,
    observer: Observer,
    renderer: Renderer,
    initial: State,
}

#[wasm_bindgen]
impl DiscSim {
    /// Scenarios:
    /// - `"cream"`: a WCA fluid with a disc of marked ("cream") particles in the middle.
    ///   Options: `n`, `density`, `temperature`, `seed`, `warmup` (steps before marking),
    ///   `blob` (disc radius as a fraction of the shorter box side).
    pub async fn create(canvas: web_sys::HtmlCanvasElement, scenario: String, options: JsValue) -> Result<DiscSim, JsError> {
        let (w, h) = (canvas.width(), canvas.height());
        let (gpu, surface) = browser_gpu(Some(canvas)).await?;
        let prims = Prims::new(&gpu);
        let n = opt(&options, "n", 8000.0) as usize;
        let density = opt(&options, "density", 0.5);
        let temperature = opt(&options, "temperature", 1.0);
        let seed = opt(&options, "seed", 1.0) as u64;
        let warmup = opt(&options, "warmup", 1000.0) as usize;
        let (engine, initial) = match scenario.as_str() {
            "cream" => {
                let aspect = w as f64 / h as f64;
                let sys = System::new(init::box_for(n, density, aspect), init::DT, Pair::wca(), vec![1.0, 1.0]);
                let state = init::fluid(&sys, n, temperature, seed);
                let mut engine = GpuEngine::new(&prims, sys.clone(), &state);
                engine.step(warmup);
                let mut state = engine.read_state().await;
                init::set_temperature(&mut state, &sys, temperature);
                let b = sys.box_f64();
                let radius = opt(&options, "blob", 0.18) * b[0].min(b[1]);
                init::mark_disc(&mut state, &sys, [b[0] / 2.0, b[1] / 2.0], radius, 1);
                engine.upload(&state);
                (engine, state)
            }
            other => return Err(err(format!("unknown scenario {other}"))),
        };
        // About 64 particles per bin, so counting noise barely biases the mixing measure.
        let b = engine.sys.box_f64();
        let bins = engine.n() as f64 / 64.0;
        let gy = ((bins * b[1] / b[0]).sqrt().round() as u32).max(2);
        let gx = ((gy as f64 * b[0] / b[1]).round() as u32).max(2);
        let observer = Observer::new(&engine, [gx, gy], 1);
        let mut renderer = Renderer::new(&gpu, surface.expect("surface"), &engine, w, h);
        renderer.set_radius(0.5);
        Ok(DiscSim { engine, observer, renderer, initial })
    }

    pub fn step(&mut self, k: u32) {
        self.engine.step(k as usize);
    }

    pub fn reverse(&mut self) {
        self.engine.reverse();
    }

    /// Flip the least significant bit (2⁻³² σ) of particle `i`'s x coordinate.
    pub fn flip_bit(&mut self, i: u32) {
        self.engine.flip_bit(i, 0, 0);
    }

    /// Index of the marked particle nearest the box centre in the initial state.
    pub fn central_marked(&self) -> u32 {
        let b = self.engine.sys.box_len;
        let c = [b[0] / 2, b[1] / 2];
        (0..self.initial.n())
            .filter(|&i| self.initial.species[i] == 1)
            .min_by_key(|&i| {
                let d = [self.initial.pos[i][0] - c[0], self.initial.pos[i][1] - c[1]];
                (d[0] as i128).pow(2) + (d[1] as i128).pow(2)
            })
            .unwrap_or(0) as u32
    }

    /// Signed step count: positive while running forward, negative while running back.
    pub fn steps(&self) -> f64 {
        self.engine.steps as f64
    }

    pub fn n(&self) -> u32 {
        self.engine.n()
    }

    pub fn dt(&self) -> f64 {
        self.engine.sys.dt
    }

    pub fn box_size(&self) -> Vec<f64> {
        self.engine.sys.box_f64().to_vec()
    }

    /// Resolves to `{mixing, temperature}`.
    pub fn observe(&self) -> js_sys::Promise {
        let fut = self.observer.read();
        let dof = (self.engine.n() as f64 - 1.0).max(1.0);
        future_to_promise(async move {
            let r = fut.await;
            Ok(obj(&[("mixing", mixing(&r).into()), ("temperature", (r.kinetic / dof).into())]))
        })
    }

    /// Resolves to `{identical, differ, maxDeviation}`: whether the current state equals the
    /// initial state bit for bit, how many particles' positions differ, and by how much (σ).
    pub fn compare_to_initial(&self) -> js_sys::Promise {
        let fut = self.engine.read_state();
        let initial = self.initial.clone();
        let sys: System = self.engine.sys.clone();
        future_to_promise(async move {
            let s = fut.await;
            let (differ, max) = s.position_difference(&initial, &sys);
            Ok(obj(&[
                ("identical", (s == initial).into()),
                ("differ", (differ as u32).into()),
                ("maxDeviation", max.into()),
                ("hash", format!("{:016x}", s.hash()).into()),
                ("initialHash", format!("{:016x}", initial.hash()).into()),
            ]))
        })
    }

    pub fn render(&mut self) {
        self.renderer.render();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
        self.renderer.set_radius(0.5);
    }

    /// 24 floats: four species colours, the highlight colour, the background (RGBA, sRGB).
    pub fn set_colors(&mut self, c: Vec<f32>) {
        let p = &mut self.renderer.params;
        for s in 0..4 {
            p.colors[s].copy_from_slice(&c[4 * s..4 * s + 4]);
        }
        p.highlight_color.copy_from_slice(&c[16..20]);
        self.renderer.background = wgpu::Color { r: c[20] as f64, g: c[21] as f64, b: c[22] as f64, a: c[23] as f64 };
    }

    /// Draw particle `i` enlarged in the highlight colour (negative for none).
    pub fn set_highlight(&mut self, i: i32) {
        self.renderer.params.highlight = if i < 0 { NO_HIGHLIGHT } else { i as u32 };
    }
}
