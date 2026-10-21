//! Shan–Chen lattice Boltzmann (D2Q9) with a Carnahan–Starling equation of state (`lbm.wgsl`):
//! a fluid that flows (bubbles rise, drops fall and merge) and separates into liquid and vapour
//! by itself. Optionally the temperature evolves too: carried by the flow, diffusing from a hot
//! plate, and taken up where liquid expands into vapour, so a heated pool boils.
//!
//! Units are lattice units (Δx = Δt = 1). The equation of state is
//! p = ρT (1 + η + η² − η³)/(1 − η)³ − aρ², with η = bρ/4; for a = 1, b = 4 its critical point is
//! at T_c ≈ 0.0943, ρ_c ≈ 0.130.

use bytemuck::{Pod, Zeroable};
use gpu_prims::util::{self, Dispatch};
use gpu_prims::Gpu;
use std::future::Future;

/// The Carnahan–Starling equation of state (R = 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eos {
    pub a: f64,
    pub b: f64,
}

impl Default for Eos {
    fn default() -> Self {
        Self { a: 1.0, b: 4.0 }
    }
}

impl Eos {
    fn z(&self, r: f64) -> f64 {
        let e = self.b * r / 4.0;
        (1.0 + e + e * e - e * e * e) / (1.0 - e).powi(3)
    }

    pub fn pressure(&self, r: f64, t: f64) -> f64 {
        r * t * self.z(r) - self.a * r * r
    }

    /// The Shan–Chen potential that gives this pressure (G = −1, c_s² = 1/3).
    pub fn psi(&self, r: f64, t: f64) -> f64 {
        (2.0 * r - 6.0 * self.pressure(r, t)).max(0.0).sqrt()
    }

    /// The temperature at which ∂p/∂ρ = 0 at density r (the spinodal).
    fn spinodal_t(&self, r: f64) -> f64 {
        let h = 1e-6;
        let d = ((r + h) * self.z(r + h) - (r - h) * self.z(r - h)) / (2.0 * h);
        2.0 * self.a * r / d
    }

    /// (T_c, ρ_c): the highest spinodal temperature.
    pub fn critical(&self) -> (f64, f64) {
        let (mut lo, mut hi) = (1e-4, 3.9 / self.b);
        for _ in 0..200 {
            let m1 = lo + (hi - lo) / 3.0;
            let m2 = hi - (hi - lo) / 3.0;
            if self.spinodal_t(m1) < self.spinodal_t(m2) {
                lo = m1;
            } else {
                hi = m2;
            }
        }
        let r = 0.5 * (lo + hi);
        (self.spinodal_t(r), r)
    }

    /// Coexisting (vapour, liquid) densities at temperature `t`, for equal pressures and
    /// ∫ (p₀ − p) w(ρ) dρ = 0 with weight `w`: 1/ρ² for thermodynamics (Maxwell's equal areas),
    /// ψ'/ψ² for the Shan–Chen model's mechanical balance (which this lattice Boltzmann follows).
    fn coexistence_with(&self, t: f64, w: impl Fn(f64) -> f64) -> Option<(f64, f64)> {
        let (tc, rc) = self.critical();
        if t >= tc {
            return None;
        }
        // The spinodal densities bracket the loop: p has a local max at r1 and min at r2.
        let rmax = 3.95 / self.b;
        let dp = |r: f64| (self.pressure(r + 1e-7, t) - self.pressure(r - 1e-7, t)) / 2e-7;
        let root = |f: &dyn Fn(f64) -> f64, mut a: f64, mut b: f64| {
            for _ in 0..200 {
                let m = 0.5 * (a + b);
                if (f(a) > 0.0) == (f(m) > 0.0) {
                    a = m;
                } else {
                    b = m;
                }
            }
            0.5 * (a + b)
        };
        let r1 = root(&|r| dp(r), 1e-6, rc);
        let r2 = root(&|r| dp(r), rc, rmax);
        let (pmax, pmin) = (self.pressure(r1, t), self.pressure(r2, t)).max_min();
        // (The vapour root's bracket starts at ρ = 0, where p = 0 is below any p₀ > 0.)
        let densities = |p0: f64| (root(&|r| self.pressure(r, t) - p0, 0.0, r1), root(&|r| self.pressure(r, t) - p0, r2, rmax));
        let integral = |p0: f64| {
            let (rv, rl) = densities(p0);
            // Simpson's rule in ln ρ (the weights grow steeply at low density): dρ = ρ d(ln ρ).
            let n = 4000;
            let (u0, u1) = (rv.ln(), rl.ln());
            let h = (u1 - u0) / n as f64;
            (0..=n)
                .map(|k| {
                    let r = (u0 + k as f64 * h).exp();
                    let c = if k == 0 || k == n { 1.0 } else if k % 2 == 1 { 4.0 } else { 2.0 };
                    c * (p0 - self.pressure(r, t)) * w(r) * r
                })
                .sum::<f64>()
                * h
                / 3.0
        };
        let p0 = root(&integral, pmin.max(1e-12), pmax);
        Some(densities(p0))
    }

    /// Thermodynamic coexistence (Maxwell's equal areas).
    pub fn coexistence_maxwell(&self, t: f64) -> Option<(f64, f64)> {
        self.coexistence_with(t, |r| 1.0 / (r * r))
    }

    /// The coexistence the Shan–Chen model reaches (its mechanical balance, Shan & Chen 1994).
    pub fn coexistence_sc(&self, t: f64) -> Option<(f64, f64)> {
        let h = 1e-7;
        self.coexistence_with(t, |r| {
            let p = self.psi(r, t);
            ((self.psi(r + h, t) - self.psi(r - h, t)) / (2.0 * h)) / (p * p)
        })
    }
}

trait MaxMin {
    fn max_min(self) -> (f64, f64);
}
impl MaxMin for (f64, f64) {
    fn max_min(self) -> (f64, f64) {
        (self.0.max(self.1), self.0.min(self.1))
    }
}

/// Size (rows 0 and h − 1 are walls; x is periodic) and the equation of state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LbmConfig {
    pub w: u32,
    pub h: u32,
    pub eos: Eos,
}

/// What can change while running.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LbmConditions {
    /// Evolve the temperature field (else it stays as set).
    pub thermal: bool,
    /// 1/τ (1: kinematic viscosity 1/6).
    pub omega: f64,
    /// Downward acceleration (lattice units).
    pub gravity: f64,
    /// The floor's temperature over x in [plate.0, plate.1) (elsewhere the floor insulates), and
    /// the ceiling's (≤ 0: insulating).
    pub t_floor: f64,
    pub plate: (u32, u32),
    pub t_ceiling: f64,
    pub chi: f64,
    pub cv: f64,
    /// ψ of the walls relative to the fluid beside them (1: neutral; < 1: the liquid avoids them).
    pub wetting: f64,
}

impl Default for LbmConditions {
    fn default() -> Self {
        Self { thermal: false, omega: 1.0, gravity: 0.0, t_floor: 0.0, plate: (0, 0), t_ceiling: 0.0, chi: 0.03, cv: 6.0, wetting: 1.0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LbmParams {
    w: u32,
    h: u32,
    n: u32,
    thermal: u32,
    omega: f32,
    gravity: f32,
    a: f32,
    b: f32,
    t_floor: f32,
    t_ceiling: f32,
    plate_x0: u32,
    plate_x1: u32,
    chi: f32,
    cv: f32,
    wetting: f32,
    _p: u32,
}

impl LbmParams {
    fn new(c: &LbmConfig, k: &LbmConditions) -> Self {
        Self {
            w: c.w,
            h: c.h,
            n: c.w * c.h,
            thermal: k.thermal as u32,
            omega: k.omega as f32,
            gravity: k.gravity as f32,
            a: c.eos.a as f32,
            b: c.eos.b as f32,
            t_floor: k.t_floor as f32,
            t_ceiling: k.t_ceiling as f32,
            plate_x0: k.plate.0,
            plate_x1: k.plate.1,
            chi: k.chi as f32,
            cv: k.cv as f32,
            wetting: k.wetting as f32,
            _p: 0,
        }
    }
}

/// Steps per command submission.
const STEPS_PER_SUBMIT: usize = 64;

/// The fluid on the GPU.
pub struct ShanChen {
    gpu: Gpu,
    pub config: LbmConfig,
    pub conditions: LbmConditions,
    params: wgpu::Buffer,
    f: [wgpu::Buffer; 2],
    pub rho: wgpu::Buffer,
    pub psi: wgpu::Buffer,
    pub vel: wgpu::Buffer,
    pub temp: [wgpu::Buffer; 2],
    /// Per parity (which buffers are current): macro fields, wall ψ, collide and stream, heat.
    steps_by_parity: [[Dispatch; 4]; 2],
    paint: [Dispatch; 2],
    paint_buf: wgpu::Buffer,
    pub steps: u64,
}

impl ShanChen {
    /// `rho` and `temp`: one value per site, row-major from the bottom (walls' values ignored).
    pub fn new(gpu: &Gpu, config: LbmConfig, conditions: LbmConditions, rho: &[f32], temp: &[f32]) -> Self {
        let n = (config.w * config.h) as usize;
        assert!(config.h >= 4 && config.w >= 3);
        assert_eq!(rho.len(), n);
        assert_eq!(temp.len(), n);
        let w9 = [4.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0f32];
        let mut f0 = vec![0f32; 9 * n];
        for i in 0..9 {
            for s in 0..n {
                f0[i * n + s] = w9[i] * rho[s];
            }
        }
        let params = util::uniform_init(gpu, "lbm params", &LbmParams::new(&config, &conditions));
        let f = [util::storage_init(gpu, "lbm f a", &f0), util::storage_init(gpu, "lbm f b", &f0)];
        let rho_buf = util::storage_init(gpu, "lbm rho", rho);
        let psi = util::storage(gpu, "lbm psi", n as u64 * 4);
        let vel = util::storage(gpu, "lbm vel", n as u64 * 8);
        let temp_bufs = [util::storage_init(gpu, "lbm temp a", temp), util::storage_init(gpu, "lbm temp b", temp)];
        let m = util::shader(gpu, "lbm", crate::LBM_WGSL);
        let pl = |e: &str| util::pipeline(gpu, &m, e);
        let (p_macro, p_wall, p_collide, p_heat, p_paint) = (pl("macro_fields"), pl("wall_psi"), pl("collide_stream"), pl("heat"), pl("paint"));
        let g = util::groups(n as u32, util::WG);
        let paint_buf = util::uniform_init(gpu, "lbm paint", &[0f32; 4]);
        let for_parity = |p: usize| {
            let (fi, fo, ti, to) = (&f[p], &f[1 - p], &temp_bufs[p], &temp_bufs[1 - p]);
            [
                Dispatch::new_at(gpu, &p_macro, &[(0, &params), (1, fi), (3, &rho_buf), (4, &psi), (6, ti)], g),
                Dispatch::new_at(gpu, &p_wall, &[(0, &params), (4, &psi)], util::groups(config.w, util::WG)),
                Dispatch::new_at(gpu, &p_collide, &[(0, &params), (1, fi), (2, fo), (4, &psi), (5, &vel)], g),
                Dispatch::new_at(gpu, &p_heat, &[(0, &params), (3, &rho_buf), (5, &vel), (6, ti), (7, to)], g),
            ]
        };
        let steps_by_parity = [for_parity(0), for_parity(1)];
        let paint = [
            Dispatch::new_at(gpu, &p_paint, &[(0, &params), (2, &f[0]), (8, &paint_buf), (9, &f[1])], g),
            Dispatch::new_at(gpu, &p_paint, &[(0, &params), (2, &f[1]), (8, &paint_buf), (9, &f[0])], g),
        ];
        Self { gpu: gpu.clone(), config, conditions, params, f, rho: rho_buf, psi, vel, temp: temp_bufs, steps_by_parity, paint, paint_buf, steps: 0 }
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// The buffers holding the current populations and temperatures (they alternate each step).
    pub fn current(&self) -> usize {
        (self.steps % 2) as usize
    }

    pub fn set_conditions(&mut self, k: LbmConditions) {
        self.conditions = k;
        self.gpu.queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&LbmParams::new(&self.config, &k)));
    }

    /// Set the temperature everywhere (both buffers).
    pub fn set_temperature_field(&mut self, temp: &[f32]) {
        for b in &self.temp {
            self.gpu.queue.write_buffer(b, 0, bytemuck::cast_slice(temp));
        }
    }

    /// Run `n` steps (submitted, not waited for).
    pub fn step(&mut self, n: usize) {
        let mut left = n;
        while left > 0 {
            let k = left.min(STEPS_PER_SUBMIT);
            let mut enc = self.gpu.device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                for j in 0..k {
                    for d in &self.steps_by_parity[((self.steps + j as u64) % 2) as usize] {
                        d.encode(&mut pass);
                    }
                }
            }
            self.gpu.queue.submit([enc.finish()]);
            self.steps += k as u64;
            left -= k;
        }
    }

    /// Set a disc of radius `r` around (`cx`, `cy`) (sites, y up) to density `density` at rest:
    /// a drop of liquid, or a bubble of vapour.
    pub fn paint(&mut self, cx: f32, cy: f32, r: f32, density: f32) {
        self.gpu.queue.write_buffer(&self.paint_buf, 0, bytemuck::cast_slice(&[cx, cy, r, density]));
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            self.paint[self.current()].encode(&mut pass);
        }
        self.gpu.queue.submit([enc.finish()]);
    }

    /// Densities at the last step (walls: 0).
    pub fn read_rho(&self) -> impl Future<Output = Vec<f32>> + 'static {
        let (gpu, b, n) = (self.gpu.clone(), self.rho.clone(), (self.config.w * self.config.h) as usize);
        async move { util::read(&gpu, &b, n).await }
    }

    pub fn read_temp(&self) -> impl Future<Output = Vec<f32>> + 'static {
        let (gpu, b, n) = (self.gpu.clone(), self.temp[self.current()].clone(), (self.config.w * self.config.h) as usize);
        async move { util::read(&gpu, &b, n).await }
    }

    pub fn read_vel(&self) -> impl Future<Output = Vec<[f32; 2]>> + 'static {
        let (gpu, b, n) = (self.gpu.clone(), self.vel.clone(), (self.config.w * self.config.h) as usize);
        async move { util::read(&gpu, &b, n).await }
    }
}
