//! Gibbs-ensemble Monte Carlo (`mc::gibbs`) for pages. It runs on the CPU (see `mc` for why),
//! so it has no renderer here: the page draws the two boxes from `positions`.

use js_sys::Float32Array;
use mc::gibbs::Gibbs;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct GibbsSim {
    g: Gibbs,
}

#[wasm_bindgen]
impl GibbsSim {
    /// `n` molecules at overall density `density` in two equal boxes, at temperature `t`
    /// (2D Lennard-Jones, cut and shifted at 2.5 σ, as on the molecular-dynamics pages).
    #[wasm_bindgen(constructor)]
    pub fn new(n: usize, density: f64, t: f64, seed: u64) -> GibbsSim {
        GibbsSim { g: Gibbs::new(n, density, t, 2.5, seed) }
    }

    /// Run `cycles` cycles (each: N molecule moves, one area exchange, N/4 transfer attempts).
    pub fn run(&mut self, cycles: u32) {
        self.g.run(cycles as usize);
    }

    pub fn cycles(&self) -> f64 {
        self.g.cycles as f64
    }

    pub fn set_temperature(&mut self, t: f64) {
        self.g.set_temperature(t);
    }

    pub fn temperature(&self) -> f64 {
        self.g.t
    }

    /// Side lengths of the two boxes (σ).
    pub fn sides(&self) -> Vec<f64> {
        vec![self.g.boxes[0].side, self.g.boxes[1].side]
    }

    /// Molecules in each box.
    pub fn counts(&self) -> Vec<u32> {
        vec![self.g.boxes[0].n() as u32, self.g.boxes[1].n() as u32]
    }

    /// [density, pressure] of box 0, then of box 1.
    pub fn measure(&self) -> Vec<f64> {
        let m = self.g.measure();
        vec![m[0].0, m[0].1, m[1].0, m[1].1]
    }

    /// (x, y) of every molecule in box `b` (σ).
    pub fn positions(&self, b: usize) -> Float32Array {
        let p: Vec<f32> = self.g.boxes[b].pos.iter().flat_map(|q| [q[0] as f32, q[1] as f32]).collect();
        Float32Array::from(&p[..])
    }

    /// Acceptance rates: [molecule moves, area exchanges, transfers].
    pub fn acceptance(&self) -> Vec<f64> {
        let s = self.g.stats;
        let r = |(t, a): (u64, u64)| if t == 0 { 0.0 } else { a as f64 / t as f64 };
        vec![r(s.moves), r(s.areas), r(s.transfers)]
    }
}
