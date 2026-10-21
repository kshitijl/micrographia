//! A 2D lattice gas under Kawasaki Monte Carlo (`gas.wgsl`), and its CPU twin.
//!
//! Each site of a `w × h` grid holds a molecule or nothing; neighbouring molecules have energy
//! −ε (ε = 1 sets the units), and gravity adds `g` per molecule per row of height. A move swaps
//! a neighbouring molecule–hole pair with the Metropolis probability at temperature T, so the
//! number of molecules is conserved and they move by hopping, as molecules diffuse. This is
//! the canonical ensemble (the moves are a heat bath at T).
//!
//! The lattice gas is the Ising model in disguise (occupied = spin up), so with periodic
//! boundaries and no gravity its coexisting liquid and vapour densities are (1 ± M)/2, with
//! Onsager's M = (1 − sinh(ε/2kT)⁻⁴)^(1/8), up to the critical point kT_c = ε / (2 ln(1 + √2))
//! ≈ 0.567 ε.

use bytemuck::{Pod, Zeroable};
use gpu_prims::threefry::threefry2x32;
use gpu_prims::util::{self, Dispatch};
use gpu_prims::Gpu;
use std::future::Future;

/// The exact critical temperature of the 2D lattice gas (ε = 1).
pub fn critical_temperature() -> f64 {
    1.0 / (2.0 * (1.0 + 2f64.sqrt()).ln())
}

/// Onsager's coexisting (vapour, liquid) densities at temperature `t` (below the critical one).
pub fn coexistence(t: f64) -> Option<(f64, f64)> {
    let s = (0.5 / t).sinh();
    let m4 = 1.0 - s.powi(-4);
    if m4 <= 0.0 {
        return None;
    }
    let m = m4.powf(0.125);
    Some((0.5 * (1.0 - m), 0.5 * (1.0 + m)))
}

/// Boundaries and seed. Sizes: `w` a multiple of 6; `h` even, and a multiple of 6 if periodic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GasConfig {
    pub w: u32,
    pub h: u32,
    pub periodic_y: bool,
    /// What the walls below and above count as, when y is not periodic: true if the liquid wets
    /// them (a wall site counts as a molecule), false if not.
    pub wet_floor: bool,
    pub wet_ceiling: bool,
    pub seed: u64,
}

/// What can change while running: temperatures, gravity and the hot plate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GasConditions {
    pub t: f64,
    /// Energy per molecule per row of height (ε).
    pub g: f64,
    /// Rows at the bottom held at `t_hot` (a hot plate); 0 for none.
    pub hot_rows: u32,
    pub t_hot: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GasParams {
    w: u32,
    h: u32,
    periodic_y: u32,
    wall_lo: u32,
    wall_hi: u32,
    hot_rows: u32,
    seed_lo: u32,
    seed_hi: u32,
    table: [u32; 48],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Sub {
    dir: u32,
    px: u32,
    py: u32,
    id: u32,
}

/// The twelve sub-sweeps of one sweep: six sublattices of x-bonds, then six of y-bonds.
const SUBS: [Sub; 12] = {
    let mut s = [Sub { dir: 0, px: 0, py: 0, id: 0 }; 12];
    let mut k = 0;
    while k < 12 {
        let dir = (k / 6) as u32;
        let j = (k % 6) as u32;
        // x-bonds: px in 0..3, py in 0..2; y-bonds: px in 0..2, py in 0..3.
        let (px, py) = if dir == 0 { (j % 3, j / 3) } else { (j % 2, j / 2) };
        s[k] = Sub { dir, px, py, id: k as u32 };
        k += 1;
    }
    s
};

/// Acceptance thresholds: for each change dN in occupied neighbours (−3..3) and of row dy
/// (−1..1), min(1, e^(−ΔE/kT)) with ΔE = −dN + g·dy, as a fraction of 2^32 (u32::MAX: always).
fn thresholds(t: f64, g: f64) -> [u32; 21] {
    let mut out = [0u32; 21];
    for dn in -3i32..=3 {
        for dy in -1i32..=1 {
            let de = -(dn as f64) + g * dy as f64;
            let p = (-de / t).exp();
            out[((dn + 3) * 3 + dy + 1) as usize] = if p >= 1.0 { u32::MAX } else { (p * 4294967296.0) as u32 };
        }
    }
    out
}

impl GasParams {
    fn new(c: &GasConfig, k: &GasConditions) -> Self {
        let mut table = [0u32; 48];
        table[..21].copy_from_slice(&thresholds(k.t, k.g));
        table[24..45].copy_from_slice(&thresholds(if k.hot_rows > 0 { k.t_hot } else { k.t }, k.g));
        Self {
            w: c.w,
            h: c.h,
            periodic_y: c.periodic_y as u32,
            wall_lo: c.wet_floor as u32,
            wall_hi: c.wet_ceiling as u32,
            hot_rows: k.hot_rows,
            seed_lo: c.seed as u32,
            seed_hi: (c.seed >> 32) as u32,
            table,
        }
    }
}

fn check(c: &GasConfig) {
    assert!(c.w % 6 == 0 && c.w >= 6, "width must be a multiple of 6");
    assert!(c.h % 2 == 0 && c.h >= 6, "height must be even");
    assert!(!c.periodic_y || c.h % 6 == 0, "a periodic height must be a multiple of 6");
}

/// Number of pair candidates (threads) in a sub-sweep.
fn pairs(c: &GasConfig, s: &Sub) -> u32 {
    if s.dir == 0 { (c.w / 3) * (c.h / 2) } else { (c.w / 2) * c.h.div_ceil(3) }
}

/// Sweeps per command submission.
const SWEEPS_PER_SUBMIT: usize = 32;

/// The lattice gas on the GPU.
pub struct LatticeGas {
    gpu: Gpu,
    pub config: GasConfig,
    pub conditions: GasConditions,
    params: wgpu::Buffer,
    pub occ: wgpu::Buffer,
    ctl: wgpu::Buffer,
    rows: wgpu::Buffer,
    subs: Vec<Dispatch>,
    tick: Dispatch,
    count: Dispatch,
    paint: Dispatch,
    paint_buf: wgpu::Buffer,
    blocks: wgpu::Buffer,
    count_blocks: Dispatch,
    paints: u32,
    _sub_bufs: Vec<wgpu::Buffer>,
    /// Sweeps so far.
    pub sweeps: u64,
}

impl LatticeGas {
    /// `occ`: 0 or 1 per site, row-major from the bottom row.
    pub fn new(gpu: &Gpu, config: GasConfig, conditions: GasConditions, occ: &[u32]) -> Self {
        check(&config);
        assert_eq!(occ.len(), (config.w * config.h) as usize);
        let params = util::uniform_init(gpu, "gas params", &GasParams::new(&config, &conditions));
        let occ_buf = util::storage_init(gpu, "gas occ", occ);
        let ctl = util::storage(gpu, "gas ctl", 16);
        let rows = util::storage(gpu, "gas rows", 2 * config.h as u64 * 4);
        let m = util::shader(gpu, "gas", &crate::gas_source());
        let pl = |e: &str| util::pipeline(gpu, &m, e);
        let kawasaki = pl("kawasaki");
        let sub_bufs: Vec<wgpu::Buffer> = SUBS.iter().map(|s| util::uniform_init(gpu, "gas sub", s)).collect();
        let subs = SUBS
            .iter()
            .zip(&sub_bufs)
            .map(|(s, b)| Dispatch::new_at(gpu, &kawasaki, &[(0, &params), (1, b), (2, &occ_buf), (3, &ctl)], util::groups(pairs(&config, s), util::WG)))
            .collect();
        let paint_buf = util::uniform_init(gpu, "gas paint", &[0u32; 8]);
        let nblocks = (config.w / 6) * (config.h / 6);
        let blocks = util::storage(gpu, "gas blocks", nblocks as u64 * 4);
        Self {
            count_blocks: Dispatch::new_at(gpu, &pl("count_blocks"), &[(0, &params), (2, &occ_buf), (6, &blocks)], util::groups(nblocks, util::WG)),
            blocks,
            paint: Dispatch::new_at(gpu, &pl("paint"), &[(0, &params), (2, &occ_buf), (5, &paint_buf)], util::groups(config.w * config.h, util::WG)),
            paint_buf,
            paints: 0,
            tick: Dispatch::new_at(gpu, &pl("tick"), &[(3, &ctl)], 1),
            count: Dispatch::new_at(gpu, &pl("count_rows"), &[(0, &params), (2, &occ_buf), (4, &rows)], util::groups(config.h, 64)),
            gpu: gpu.clone(),
            config,
            conditions,
            params,
            occ: occ_buf,
            ctl,
            rows,
            subs,
            _sub_bufs: sub_bufs,
            sweeps: 0,
        }
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Change the temperature, gravity or hot plate.
    pub fn set_conditions(&mut self, k: GasConditions) {
        self.conditions = k;
        self.gpu.queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&GasParams::new(&self.config, &k)));
    }

    /// Run `n` sweeps (every bond tried once per sweep); submitted, not waited for.
    pub fn sweep(&mut self, n: usize) {
        let mut left = n;
        while left > 0 {
            let k = left.min(SWEEPS_PER_SUBMIT);
            let mut enc = self.gpu.device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                for _ in 0..k {
                    for d in &self.subs {
                        d.encode(&mut pass);
                    }
                    self.tick.encode(&mut pass);
                }
            }
            self.gpu.queue.submit([enc.finish()]);
            left -= k;
        }
        self.sweeps += n as u64;
    }

    /// Set the sites within `r` of (`cx`, `cy`) (in sites, y up) to `molecule` (true) or empty,
    /// each with probability `prob`: a dropper adding molecules, or a sponge taking them away.
    pub fn paint(&mut self, cx: f32, cy: f32, r: f32, molecule: bool, prob: f32) {
        let words: [u32; 8] = [cx.to_bits(), cy.to_bits(), r.to_bits(), prob.to_bits(), molecule as u32, self.paints, 0, 0];
        self.paints += 1;
        self.gpu.queue.write_buffer(&self.paint_buf, 0, bytemuck::cast_slice(&words));
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            self.paint.encode(&mut pass);
        }
        self.gpu.queue.submit([enc.finish()]);
    }

    /// Replace the sites (same size), e.g. after the page adds or removes molecules.
    pub fn upload(&mut self, occ: &[u32]) {
        assert_eq!(occ.len(), (self.config.w * self.config.h) as usize);
        self.gpu.queue.write_buffer(&self.occ, 0, bytemuck::cast_slice(occ));
    }

    pub fn read_occ(&self) -> impl Future<Output = Vec<u32>> + 'static {
        let (gpu, occ, n) = (self.gpu.clone(), self.occ.clone(), (self.config.w * self.config.h) as usize);
        async move { util::read(&gpu, &occ, n).await }
    }

    /// Molecules and bonds in each row (measures now).
    pub fn read_rows(&self) -> impl Future<Output = Rows> + 'static {
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            self.count.encode(&mut pass);
        }
        self.gpu.queue.submit([enc.finish()]);
        let (gpu, rows, h) = (self.gpu.clone(), self.rows.clone(), self.config.h as usize);
        async move {
            let r: Vec<u32> = util::read(&gpu, &rows, 2 * h).await;
            Rows { molecules: r[..h].to_vec(), bonds: r[h..].to_vec() }
        }
    }
}

/// How to fill the lattice at the start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Start {
    /// A full slab across the middle half of the height (with periodic y: two flat interfaces).
    Slab,
    /// The bottom `fill` of the rows full (a pool), the rest empty.
    Pool,
    /// Every site filled with probability `fill`: a quench into the two-phase region, which then
    /// separates by itself (spinodal decomposition, or nucleation if `fill` is small).
    Random,
}

/// Initial sites for `config`.
pub fn initial(config: &GasConfig, start: Start, fill: f64) -> Vec<u32> {
    let (w, h) = (config.w, config.h);
    (0..w * h)
        .map(|i| {
            let y = i / w;
            match start {
                Start::Slab => (y >= h / 4 && y < 3 * h / 4) as u32,
                Start::Pool => ((y as f64) < fill * h as f64) as u32,
                Start::Random => (gpu_prims::threefry::uniform(config.seed, 11, i) < fill) as u32,
            }
        })
        .collect()
}

impl LatticeGas {
    /// Molecules in each 6 × 6 block, row-major from the bottom ((w/6) × ⌊h/6⌋; measures now).
    pub fn read_blocks(&self) -> impl Future<Output = Vec<u32>> + 'static {
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            self.count_blocks.encode(&mut pass);
        }
        self.gpu.queue.submit([enc.finish()]);
        let (gpu, blocks, n) = (self.gpu.clone(), self.blocks.clone(), ((self.config.w / 6) * (self.config.h / 6)) as usize);
        async move { util::read(&gpu, &blocks, n).await }
    }
}

/// Per-row counts from the GPU.
#[derive(Clone, Debug)]
pub struct Rows {
    pub molecules: Vec<u32>,
    pub bonds: Vec<u32>,
}

impl Rows {
    /// Total energy (ε): −1 per bond, plus g per molecule per row of height.
    pub fn energy(&self, g: f64) -> f64 {
        let bonds: u64 = self.bonds.iter().map(|&b| b as u64).sum();
        let height: f64 = self.molecules.iter().enumerate().map(|(y, &m)| y as f64 * m as f64).sum();
        -(bonds as f64) + g * height
    }
}

/// The CPU twin: the same moves with the same random numbers, so it agrees with the GPU bit
/// for bit.
pub struct GasRef {
    pub config: GasConfig,
    pub conditions: GasConditions,
    pub occ: Vec<u32>,
    pub sweeps: u32,
}

impl GasRef {
    pub fn new(config: GasConfig, conditions: GasConditions, occ: Vec<u32>) -> Self {
        check(&config);
        Self { config, conditions, occ, sweeps: 0 }
    }

    fn neighbours(&self, x: u32, y: u32, skip: u32) -> i32 {
        let (w, h) = (self.config.w, self.config.h);
        let at = |x: u32, y: u32| self.occ[(y * w + x) as usize];
        let mut n = 0;
        for xx in [(x + w - 1) % w, (x + 1) % w] {
            if y * w + xx != skip {
                n += at(xx, y);
            }
        }
        if y > 0 {
            if (y - 1) * w + x != skip {
                n += at(x, y - 1);
            }
        } else if self.config.periodic_y {
            if (h - 1) * w + x != skip {
                n += at(x, h - 1);
            }
        } else {
            n += self.config.wet_floor as u32;
        }
        if y + 1 < h {
            if (y + 1) * w + x != skip {
                n += at(x, y + 1);
            }
        } else if self.config.periodic_y {
            if x != skip {
                n += at(x, 0);
            }
        } else {
            n += self.config.wet_ceiling as u32;
        }
        n as i32
    }

    pub fn sweep(&mut self, n: usize) {
        let c = self.config;
        let p = GasParams::new(&c, &self.conditions);
        for _ in 0..n {
            for s in &SUBS {
                for k in 0..pairs(&c, s) {
                    let (xa, ya, xb, mut yb);
                    if s.dir == 0 {
                        let nx = c.w / 3;
                        ya = 2 * (k / nx) + s.py;
                        if ya >= c.h {
                            continue;
                        }
                        xa = 3 * (k % nx) + s.px;
                        xb = (xa + 1) % c.w;
                        yb = ya;
                    } else {
                        let nx = c.w / 2;
                        ya = 3 * (k / nx) + s.py;
                        if ya >= c.h {
                            continue;
                        }
                        xa = 2 * (k % nx) + s.px;
                        xb = xa;
                        yb = ya + 1;
                        if yb == c.h {
                            if !c.periodic_y {
                                continue;
                            }
                            yb = 0;
                        }
                    }
                    let (a, b) = (ya * c.w + xa, yb * c.w + xb);
                    let na = self.occ[a as usize];
                    if na == self.occ[b as usize] {
                        continue;
                    }
                    let ((xs, ys, sidx), (xd, yd, didx)) = if na == 1 { ((xa, ya, a), (xb, yb, b)) } else { ((xb, yb, b), (xa, ya, a)) };
                    let dn = self.neighbours(xd, yd, sidx) - self.neighbours(xs, ys, didx);
                    let dy = if s.dir == 1 { if yd == ys + 1 || (ys == c.h - 1 && yd == 0) { 1 } else { -1 } } else { 0 };
                    let hot = ya.min(yb) < p.hot_rows;
                    let idx = ((dn + 3) * 3 + dy + 1) as usize + if hot { 24 } else { 0 };
                    let thr = p.table[idx];
                    let r = threefry2x32([k, self.sweeps], [p.seed_lo ^ (s.id << 24), p.seed_hi]);
                    if thr == u32::MAX || r[0] < thr {
                        self.occ[a as usize] = 1 - na;
                        self.occ[b as usize] = na;
                    }
                }
            }
            self.sweeps = self.sweeps.wrapping_add(1);
        }
    }
}
