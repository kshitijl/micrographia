//! GPU instruments: coarse density histograms and kinetic energy, read back asynchronously.

use crate::engine::GpuEngine;
use bytemuck::{Pod, Zeroable};
use gpu_prims::prims::{ReducePlan, Scalar};
use gpu_prims::util::{self, Dispatch, WG};
use gpu_prims::Gpu;
use md_core::fixed::UNIT;
use std::future::Future;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ObsParams {
    n: u32,
    gx: u32,
    gy: u32,
    marked: u32,
    scale: [f32; 2],
    vel_scale: f32,
    _pad: f32,
    mass: [f32; 4],
}

/// A `gx × gy` histogram of all particles and of the `marked` species, plus total kinetic
/// energy.
pub struct Observer {
    gpu: Gpu,
    pub grid: [u32; 2],
    hist: wgpu::Buffer,
    clear: Dispatch,
    histogram: Dispatch,
    kinetic: Dispatch,
    ke_sum: ReducePlan,
}

/// One reading of the observer.
#[derive(Clone, Debug)]
pub struct Reading {
    pub all: Vec<u32>,
    pub marked: Vec<u32>,
    pub kinetic: f64,
}

impl Observer {
    pub fn new(engine: &GpuEngine, grid: [u32; 2], marked: u32) -> Self {
        let gpu = engine.gpu().clone();
        let b = engine.sys.box_f64();
        let mut mass = [1f32; 4];
        for (s, m) in engine.sys.masses.iter().enumerate() {
            mass[s] = *m as f32;
        }
        let params = ObsParams {
            n: engine.n(),
            gx: grid[0],
            gy: grid[1],
            marked,
            scale: [(grid[0] as f64 / b[0]) as f32, (grid[1] as f64 / b[1]) as f32],
            vel_scale: (UNIT / engine.sys.dt) as f32,
            _pad: 0.0,
            mass,
        };
        let m = util::shader(&gpu, "observe", &crate::observe_source());
        let p_hist = util::pipeline(&gpu, &m, "histogram");
        let p_ke = util::pipeline(&gpu, &m, "kinetic");
        let pbuf = util::uniform_init(&gpu, "observe", &params);
        let bins = grid[0] * grid[1] * 2;
        let hist = util::storage(&gpu, "hist", bins as u64 * 4);
        let ke = util::storage(&gpu, "ke", engine.n() as u64 * 4);
        let g = util::groups(engine.n(), WG);
        let e = &engine.buf;
        Self {
            clear: engine.prims.fill(&hist, bins, 0),
            histogram: Dispatch::new_at(&gpu, &p_hist, &[(0, &pbuf), (1, &e.pos), (2, &e.species), (3, &hist)], g),
            kinetic: Dispatch::new_at(&gpu, &p_ke, &[(0, &pbuf), (2, &e.species), (4, &e.vel), (5, &ke)], g),
            ke_sum: ReducePlan::new(&engine.prims, Scalar::F32, &ke, engine.n()),
            hist,
            grid,
            gpu,
        }
    }

    /// Measure now (submits work) and return a future for the reading.
    pub fn read(&self) -> impl Future<Output = Reading> + 'static {
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            self.clear.encode(&mut pass);
            self.histogram.encode(&mut pass);
            self.kinetic.encode(&mut pass);
            self.ke_sum.encode(&mut pass);
        }
        self.gpu.queue.submit([enc.finish()]);
        let gpu = self.gpu.clone();
        let hist = self.hist.clone();
        let ke = self.ke_sum.result().clone();
        let bins = (self.grid[0] * self.grid[1]) as usize;
        async move {
            let h: Vec<u32> = util::read(&gpu, &hist, 2 * bins).await;
            let k: Vec<f32> = util::read(&gpu, &ke, 1).await;
            Reading { all: h[..bins].to_vec(), marked: h[bins..].to_vec(), kinetic: k[0] as f64 }
        }
    }
}

/// Coarse-grained mixing entropy of the marked particles, normalized to [0, 1]: 0 when they
/// fill as few bins as possible, 1 when they are spread in proportion to all particles.
pub fn mixing(reading: &Reading) -> f64 {
    let total: f64 = reading.marked.iter().map(|&c| c as f64).sum();
    let all: f64 = reading.all.iter().map(|&c| c as f64).sum();
    if total == 0.0 || all == 0.0 {
        return 0.0;
    }
    // Relative entropy of the marked distribution with respect to the overall density:
    // D = Σ p ln(p / q). Fully mixed → 0; fully segregated in a fraction f of space → ln(1/f).
    let mut d = 0.0;
    for (m, a) in reading.marked.iter().zip(&reading.all) {
        if *m > 0 {
            let p = *m as f64 / total;
            let q = *a as f64 / all;
            d += p * (p / q).ln();
        }
    }
    let d_max = (all / total).ln();
    (1.0 - d / d_max).clamp(0.0, 1.0)
}
