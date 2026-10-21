//! `gpu-int-f32` in 3D (`md3.wgsl`): the 2D engine's integer leapfrog and exact walls, with a
//! cell list over 3×3×3 cells. Its own kernels and buffers, so the 2D engine is unchanged.

use bytemuck::{Pod, Zeroable};
use gpu_prims::prims::{BinPlan, ReducePlan, Scalar};
use gpu_prims::util::{self, Dispatch, WG};
use gpu_prims::{fixed64, Gpu, Prims};
use md_core::system::MAX_SPECIES;
use md_core::three::{State3, System3, WALLS3};
use md_core::walls::LEDGER_SCALE;
use md_core::Wall;
use std::future::Future;

/// Mirrors `struct Params3` in `md3.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Params3 {
    pub n: u32,
    pub ncx: u32,
    pub ncy: u32,
    pub ncz: u32,
    pub ncell: u32,
    pub periodic: u32,
    pub nspecies: u32,
    pub pinned: u32,
    pub box_x: [u32; 2],
    pub box_y: [u32; 2],
    pub box_z: [u32; 2],
    pub half_x: [u32; 2],
    pub half_y: [u32; 2],
    pub half_z: [u32; 2],
    pub seed_lo: u32,
    pub seed_hi: u32,
    pub vstep: f32,
    pub dt: f32,
    pub features: u32,
    pub thermo_species: u32,
    pub _p0: u32,
    pub _p1: u32,
    pub cell_scale: [f32; 4],
    pub gravity: [i32; 4],
    pub kick_c: [f32; 4],
    pub mass: [f32; 4],
    pub thermo: [f32; 4],
    pub thermo_sd: [f32; 4],
    pub wall_kind: [[u32; 4]; 2],
    pub wall_sd: [[f32; 4]; 6],
    pub pairs: [[f32; 4]; 16],
}

impl Params3 {
    pub fn new(sys: &System3, n: u32) -> Self {
        let b = sys.box_f64();
        let [ncx, ncy, ncz] = sys.cells_for(sys.rc_max());
        let ns = sys.species();
        let mut kick_c = [0f32; 4];
        let mut mass = [1f32; 4];
        for s in 0..ns {
            kick_c[s] = sys.kick_coeff(s as u32) as f32;
            mass[s] = sys.masses[s].min(1e30) as f32;
        }
        let mut pairs = [[0f32; 4]; 16];
        for a in 0..ns {
            for bb in 0..ns {
                let (rc2, eps4, sigma2, shift) = sys.pair(a as u32, bb as u32).gpu_constants();
                pairs[a * 4 + bb] = [rc2, eps4, sigma2, shift];
            }
        }
        let mut wall_kind = [[0u32; 4]; 2];
        let mut wall_sd = [[0f32; 4]; 6];
        for axis in 0..3 {
            if let Some(ws) = sys.walls[axis] {
                for side in 0..2 {
                    let w = 2 * axis + side;
                    let (k, t) = match ws[side] {
                        Wall::Hard => (0, 0.0),
                        Wall::Soft => (1, 0.0),
                        Wall::Thermal(t) => (2, t),
                    };
                    wall_kind[w / 4][w % 4] = k;
                    for s in 0..ns {
                        wall_sd[w][s] = (t / sys.masses[s]).sqrt() as f32;
                    }
                }
            }
        }
        let g = sys.gravity_kick();
        let split = |a: usize| fixed64::split(sys.box_len[a]);
        let half = |a: usize| fixed64::split(sys.box_len[a] / 2);
        let mut thermo_sd = [0f32; 4];
        if let Some(t) = sys.thermostat {
            for s in 0..ns {
                thermo_sd[s] = ((t.t / sys.masses[s]).sqrt() * sys.dt * md_core::ONE as f64) as f32;
            }
        }
        Self {
            n,
            ncx,
            ncy,
            ncz,
            ncell: ncx * ncy * ncz,
            periodic: (0..3).map(|a| (sys.periodic(a) as u32) << a).sum(),
            nspecies: ns as u32,
            pinned: sys.pinned,
            box_x: split(0),
            box_y: split(1),
            box_z: split(2),
            half_x: half(0),
            half_y: half(1),
            half_z: half(2),
            seed_lo: sys.seed as u32,
            seed_hi: (sys.seed >> 32) as u32,
            vstep: (sys.dt * md_core::ONE as f64) as f32,
            dt: sys.dt as f32,
            features: sys.has_soft_walls() as u32,
            thermo_species: sys.thermostat.map_or(0, |t| t.species),
            _p0: 0,
            _p1: 0,
            cell_scale: [(ncx as f64 / b[0]) as f32, (ncy as f64 / b[1]) as f32, (ncz as f64 / b[2]) as f32, 0.0],
            gravity: [g[0], g[1], g[2], 0],
            kick_c,
            mass,
            thermo: match sys.thermostat {
                Some(t) => [t.t as f32, (t.rate * sys.dt) as f32, 0.0, 0.0],
                None => [0.0; 4],
            },
            thermo_sd,
            wall_kind,
            wall_sd,
            pairs,
        }
    }
}

/// The host's commands (`kick_region`, `set_species`), mirroring `Cmd3`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct Cmd3 {
    lo: [f32; 4],
    hi: [f32; 4],
    add: [i32; 4],
    scale: f32,
    count: u32,
    to: u32,
    _p: u32,
}

const SET_CHUNK: usize = 1024;
/// Words of the control buffer: header (clock at word 2), then 7 ledger slots of 6 words.
const CTL_WORDS: usize = 16 + 6 * WALLS3;
const CTL_LEDGER: usize = 16;

pub fn pack_positions3(pos: &[[i64; 3]]) -> Vec<[u32; 6]> {
    pos.iter()
        .map(|p| {
            let (x, y, z) = (fixed64::split(p[0]), fixed64::split(p[1]), fixed64::split(p[2]));
            [x[0], x[1], y[0], y[1], z[0], z[1]]
        })
        .collect()
}

pub fn unpack_positions3(words: &[[u32; 6]]) -> Vec<[i64; 3]> {
    words.iter().map(|w| [fixed64::join([w[0], w[1]]), fixed64::join([w[2], w[3]]), fixed64::join([w[4], w[5]])]).collect()
}

/// Energy, impulse and hits per wall (2·axis + side) and the bath (slot 6).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ledger3 {
    pub energy: [f64; WALLS3],
    pub impulse: [f64; WALLS3],
    pub hits: [u64; WALLS3],
}

/// The GPU buffers of a 3D engine, cheap to clone into a readback future.
#[derive(Clone)]
pub struct Buffers3 {
    pub gpu: Gpu,
    pub n: u32,
    pub params: wgpu::Buffer,
    pub pos: wgpu::Buffer,
    pub vel: wgpu::Buffer,
    pub species: wgpu::Buffer,
    pub ids: wgpu::Buffer,
    pub keys: wgpu::Buffer,
    pub kicks: wgpu::Buffer,
    /// Per particle: potential energy and the soft walls' force (x, y, z).
    pub aux: wgpu::Buffer,
    pub ctl: wgpu::Buffer,
    pub cmd: wgpu::Buffer,
    /// Kinetic energy per slot (after `measure_kinetic`).
    pub ke: wgpu::Buffer,
}

fn by_id<T: Copy>(ids: &[u32], values: Vec<T>) -> Vec<T> {
    let mut out = values.clone();
    for (slot, &id) in ids.iter().enumerate() {
        out[id as usize] = values[slot];
    }
    out
}

impl Buffers3 {
    pub fn read_state(&self) -> impl Future<Output = State3> + 'static {
        let b = self.clone();
        async move {
            let n = b.n as usize;
            let ids: Vec<u32> = util::read(&b.gpu, &b.ids, n).await;
            let pos: Vec<[u32; 6]> = util::read(&b.gpu, &b.pos, n).await;
            let vel: Vec<[i32; 3]> = util::read(&b.gpu, &b.vel, n).await;
            let species: Vec<u32> = util::read(&b.gpu, &b.species, n).await;
            State3 { pos: by_id(&ids, unpack_positions3(&pos)), vel: by_id(&ids, vel), species: by_id(&ids, species) }
        }
    }

    /// Kicks and per-particle potential energies from the last force evaluation, in id order.
    pub fn read_kicks(&self) -> impl Future<Output = (Vec<[i32; 3]>, Vec<f32>)> + 'static {
        let b = self.clone();
        async move {
            let n = b.n as usize;
            let ids: Vec<u32> = util::read(&b.gpu, &b.ids, n).await;
            let kicks: Vec<[i32; 3]> = util::read(&b.gpu, &b.kicks, n).await;
            let aux: Vec<[f32; 4]> = util::read(&b.gpu, &b.aux, n).await;
            (by_id(&ids, kicks), by_id(&ids, aux.iter().map(|a| a[0]).collect()))
        }
    }

    /// Kinetic energies (from `measure_kinetic`) and species, in id order.
    pub fn read_kinetic(&self) -> impl Future<Output = (Vec<f32>, Vec<u32>)> + 'static {
        let b = self.clone();
        async move {
            let n = b.n as usize;
            let ids: Vec<u32> = util::read(&b.gpu, &b.ids, n).await;
            let ke: Vec<f32> = util::read(&b.gpu, &b.ke, n).await;
            let sp: Vec<u32> = util::read(&b.gpu, &b.species, n).await;
            (by_id(&ids, ke), by_id(&ids, sp))
        }
    }

    pub fn read_ledger(&self) -> impl Future<Output = Ledger3> + 'static {
        let b = self.clone();
        async move {
            let w: Vec<u32> = util::read(&b.gpu, &b.ctl, CTL_WORDS).await;
            let word = |k: usize| fixed64::join([w[k], w[k + 1]]);
            let mut l = Ledger3::default();
            for s in 0..WALLS3 {
                let base = CTL_LEDGER + 6 * s;
                l.energy[s] = word(base) as f64 / LEDGER_SCALE;
                l.impulse[s] = word(base + 2) as f64 / LEDGER_SCALE;
                l.hits[s] = word(base + 4) as u64;
            }
            l
        }
    }
}

/// `gpu-int-f32` in 3D.
pub struct GpuEngine3 {
    pub prims: Prims,
    pub sys: System3,
    pub params: Params3,
    pub buf: Buffers3,
    cells: Dispatch,
    bin: BinPlan,
    force: Dispatch,
    integrate: Dispatch,
    tick: Option<Dispatch>,
    reverse: Dispatch,
    kick_region: Dispatch,
    set_species: Dispatch,
    set_list: wgpu::Buffer,
    kinetic: Dispatch,
    ke_sum: ReducePlan,
    /// Reorder storage into Morton order: keys, sort, gather, copy back.
    reorder: Vec<Dispatch>,
    _reorder_bin: BinPlan,
    _tmp: [wgpu::Buffer; 4],
    /// Reorder every this many steps (0 = never).
    pub sort_interval: u32,
    since_sort: u32,
    /// Net steps taken (decreases while running reversed).
    pub steps: i64,
}

impl GpuEngine3 {
    pub fn new(prims: &Prims, sys: System3, state: &State3) -> Self {
        assert!(sys.masses.len() <= MAX_SPECIES);
        let gpu = &prims.gpu;
        let n = state.n() as u32;
        let params = Params3::new(&sys, n);
        let mut ctl = vec![0u32; CTL_WORDS];
        ctl[0] = 0;
        let buf = Buffers3 {
            gpu: gpu.clone(),
            n,
            params: util::uniform_init(gpu, "md3 params", &params),
            pos: util::storage_init(gpu, "pos3", &pack_positions3(&state.pos)),
            vel: util::storage_init(gpu, "vel3", &state.vel),
            species: util::storage_init(gpu, "species3", &state.species),
            ids: util::storage_init(gpu, "ids3", &(0..n).collect::<Vec<u32>>()),
            keys: util::storage(gpu, "keys3", n as u64 * 4),
            kicks: util::storage(gpu, "kicks3", n as u64 * 12),
            aux: util::storage(gpu, "aux3", n as u64 * 16),
            ctl: util::storage_init(gpu, "ctl3", &ctl),
            cmd: util::uniform_init(gpu, "cmd3", &Cmd3::default()),
            ke: util::storage(gpu, "ke3", n as u64 * 4),
        };
        let b = &buf;
        let m = util::shader(gpu, "md3", &crate::md3_source());
        let pl = |e: &str| util::pipeline(gpu, &m, e);
        let g = util::groups(n, WG);
        let bin = BinPlan::new(prims, &b.keys, n, params.ncell);
        let set_list = util::storage(gpu, "set list3", SET_CHUNK as u64 * 4);
        // Morton reorder: its own keys and sort, then gather into temporaries and copy back.
        let mkeys = util::storage(gpu, "morton keys3", n as u64 * 4);
        let mbits = 32 - (params.ncx.max(params.ncy).max(params.ncz) - 1).leading_zeros();
        assert!(mbits <= 10, "too many cells for 3D Morton keys");
        let rbin = BinPlan::new(prims, &mkeys, n, 1 << (3 * mbits.max(1)));
        let tmp = [
            util::storage(gpu, "tmp pos3", n as u64 * 24),
            util::storage(gpu, "tmp vel3", n as u64 * 12),
            util::storage(gpu, "tmp species3", n as u64 * 4),
            util::storage(gpu, "tmp ids3", n as u64 * 4),
        ];
        let mut reorder = vec![Dispatch::new_at(gpu, &pl("morton_keys"), &[(0, &b.params), (1, &b.pos), (4, &mkeys)], g)];
        reorder.extend(rbin.dispatches());
        reorder.extend([
            Dispatch::new_at(gpu, &pl("gather_motion"), &[(0, &b.params), (1, &b.pos), (2, &b.vel), (14, &rbin.sorted), (15, &tmp[0]), (16, &tmp[1])], g),
            Dispatch::new_at(gpu, &pl("gather_meta"), &[(0, &b.params), (3, &b.species), (10, &b.ids), (14, &rbin.sorted), (17, &tmp[2]), (18, &tmp[3])], g),
            Dispatch::new_at(gpu, &pl("copy_back_motion"), &[(0, &b.params), (1, &b.pos), (2, &b.vel), (15, &tmp[0]), (16, &tmp[1])], g),
            Dispatch::new_at(gpu, &pl("copy_back_meta"), &[(0, &b.params), (3, &b.species), (10, &b.ids), (17, &tmp[2]), (18, &tmp[3])], g),
        ]);
        let stochastic = (0..3).any(|a| !sys.periodic(a)) || sys.thermostat.is_some();
        Self {
            cells: Dispatch::new_at(gpu, &pl("cells"), &[(0, &b.params), (1, &b.pos), (4, &b.keys)], g),
            force: Dispatch::new_at(
                gpu,
                &pl("force"),
                &[(0, &b.params), (1, &b.pos), (3, &b.species), (5, &bin.starts), (6, &bin.counts), (7, &bin.sorted), (8, &b.kicks), (9, &b.aux)],
                g,
            ),
            integrate: Dispatch::new_at(
                gpu,
                &pl("integrate"),
                &[(0, &b.params), (1, &b.pos), (2, &b.vel), (3, &b.species), (8, &b.kicks), (9, &b.aux), (10, &b.ids), (11, &b.ctl)],
                g,
            ),
            // Walls could be made thermal later (update_system), so keep a clock for them.
            tick: stochastic.then(|| Dispatch::new_at(gpu, &pl("tick"), &[(11, &b.ctl)], 1)),
            reverse: Dispatch::new_at(gpu, &pl("reverse"), &[(0, &b.params), (2, &b.vel), (3, &b.species), (8, &b.kicks)], g),
            kick_region: Dispatch::new_at(gpu, &pl("kick_region"), &[(0, &b.params), (1, &b.pos), (2, &b.vel), (3, &b.species), (12, &b.cmd)], g),
            set_species: Dispatch::new_at(gpu, &pl("set_species"), &[(0, &b.params), (3, &b.species), (10, &b.ids), (12, &b.cmd), (13, &set_list)], g),
            kinetic: Dispatch::new_at(gpu, &pl("kinetic"), &[(0, &b.params), (2, &b.vel), (3, &b.species), (19, &b.ke)], g),
            ke_sum: ReducePlan::new(prims, Scalar::F32, &b.ke, n),
            bin,
            set_list,
            reorder,
            _reorder_bin: rbin,
            _tmp: tmp,
            sort_interval: 100,
            since_sort: 100,
            prims: prims.clone(),
            sys,
            params,
            buf,
            steps: 0,
        }
    }

    pub fn n(&self) -> u32 {
        self.buf.n
    }

    pub fn gpu(&self) -> &Gpu {
        &self.prims.gpu
    }

    fn submit(&self, f: impl FnOnce(&mut wgpu::ComputePass<'_>)) {
        let gpu = self.gpu();
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            f(&mut pass);
        }
        gpu.queue.submit([enc.finish()]);
    }

    fn encode_forces(&self, pass: &mut wgpu::ComputePass<'_>) {
        self.cells.encode(pass);
        self.bin.encode(pass);
        self.force.encode(pass);
    }

    /// Advance `steps` steps (submitted, not waited for).
    pub fn step(&mut self, steps: usize) {
        let mut left = steps;
        while left > 0 {
            let k = left.min(crate::engine::STEPS_PER_SUBMIT);
            let interval = self.sort_interval;
            let mut since = self.since_sort;
            self.submit(|pass| {
                for _ in 0..k {
                    if interval > 0 && since >= interval {
                        for d in &self.reorder {
                            d.encode(pass);
                        }
                        since = 0;
                    }
                    self.encode_forces(pass);
                    self.integrate.encode(pass);
                    if let Some(t) = &self.tick {
                        t.encode(pass);
                    }
                    since += 1;
                }
            });
            self.since_sort = since;
            left -= k;
        }
        self.steps += steps as i64;
    }

    /// Time reversal, `v ← −(v + kick(x) + g)`.
    pub fn reverse(&mut self) {
        self.submit(|pass| {
            self.encode_forces(pass);
            self.reverse.encode(pass);
        });
        self.steps = -self.steps;
    }

    /// Change interactions, masses, wall kinds or temperatures, gravity, the bath's temperature or
    /// the seed while running. The box and species count must stay the same, no cutoff may grow,
    /// and the bath can be retuned but not added or removed.
    pub fn update_system(&mut self, sys: System3) {
        assert_eq!(sys.box_len, self.sys.box_len);
        assert_eq!(sys.species(), self.sys.species());
        assert_eq!(sys.walls.map(|w| w.is_some()), self.sys.walls.map(|w| w.is_some()));
        assert_eq!(sys.thermostat.is_some(), self.sys.thermostat.is_some(), "the bath can be retuned, not added or removed");
        assert!(sys.rc_max() <= self.sys.rc_max() + 1e-12, "cutoffs cannot grow at run time");
        let mut p = Params3::new(&sys, self.n());
        // Keep the cell geometry the buffers were sized for.
        (p.ncx, p.ncy, p.ncz, p.ncell, p.cell_scale) = (self.params.ncx, self.params.ncy, self.params.ncz, self.params.ncell, self.params.cell_scale);
        self.gpu().queue.write_buffer(&self.buf.params, 0, bytemuck::bytes_of(&p));
        self.params = p;
        self.sys = sys;
    }

    /// For every unpinned particle inside the box `lo`–`hi` (σ): multiply its velocity by
    /// `scale`, then add `add` (σ/τ). A kick, a freeze or a shove.
    pub fn kick_region(&mut self, lo: [f64; 3], hi: [f64; 3], scale: f64, add: [f64; 3]) {
        let a = add.map(|v| self.sys.to_step_displacement(v));
        let c = Cmd3 { lo: [lo[0] as f32, lo[1] as f32, lo[2] as f32, 0.0], hi: [hi[0] as f32, hi[1] as f32, hi[2] as f32, 0.0], add: [a[0], a[1], a[2], 0], scale: scale as f32, count: 0, to: 0, _p: 0 };
        self.gpu().queue.write_buffer(&self.buf.cmd, 0, bytemuck::bytes_of(&c));
        self.submit(|pass| self.kick_region.encode(pass));
    }

    /// Set the species of the particles with these ids (positions and velocities kept).
    pub fn set_species(&mut self, ids: &[u32], species: u32) {
        let mut ids = ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        for chunk in ids.chunks(SET_CHUNK) {
            let c = Cmd3 { count: chunk.len() as u32, to: species, ..Default::default() };
            self.gpu().queue.write_buffer(&self.buf.cmd, 0, bytemuck::bytes_of(&c));
            self.gpu().queue.write_buffer(&self.set_list, 0, bytemuck::cast_slice(chunk));
            self.submit(|pass| self.set_species.encode(pass));
        }
    }

    /// Replace the state (same particle count).
    pub fn upload(&mut self, state: &State3) {
        assert_eq!(state.n() as u32, self.n());
        let q = &self.gpu().queue;
        q.write_buffer(&self.buf.pos, 0, bytemuck::cast_slice(&pack_positions3(&state.pos)));
        q.write_buffer(&self.buf.vel, 0, bytemuck::cast_slice(&state.vel));
        q.write_buffer(&self.buf.species, 0, bytemuck::cast_slice(&state.species));
        q.write_buffer(&self.buf.ids, 0, bytemuck::cast_slice(&(0..self.n()).collect::<Vec<u32>>()));
        self.steps = 0;
        self.since_sort = self.sort_interval;
    }

    pub fn read_state(&self) -> impl Future<Output = State3> + 'static {
        self.buf.read_state()
    }

    /// Integer kicks at the current positions and the total potential energy.
    pub fn kicks(&self) -> impl Future<Output = (Vec<[i32; 3]>, f64)> + 'static {
        self.submit(|pass| self.encode_forces(pass));
        let fut = self.buf.read_kicks();
        async move {
            let (k, pe) = fut.await;
            (k, pe.iter().map(|&x| x as f64).sum())
        }
    }

    /// Compute every particle's kinetic energy (and their sum) from the half-step velocities.
    pub fn measure_kinetic(&self) {
        self.submit(|pass| {
            self.kinetic.encode(pass);
            self.ke_sum.encode(pass);
        });
    }

    /// Kinetic energies and species in id order (measures now).
    pub fn read_kinetic(&self) -> impl Future<Output = (Vec<f32>, Vec<u32>)> + 'static {
        self.measure_kinetic();
        self.buf.read_kinetic()
    }

    /// Total kinetic energy from the half-step velocities (measures now).
    pub fn read_kinetic_total(&self) -> impl Future<Output = f64> + 'static {
        self.measure_kinetic();
        let (gpu, sum) = (self.gpu().clone(), self.ke_sum.result().clone());
        async move { util::read::<f32>(&gpu, &sum, 1).await[0] as f64 }
    }

    /// Total potential energy at the last force evaluation (pairs and soft walls; not gravity).
    pub fn read_potential(&self) -> impl Future<Output = f64> + 'static {
        let fut = self.buf.read_kicks();
        async move { fut.await.1.iter().map(|&x| x as f64).sum() }
    }

    pub fn read_ledger(&self) -> impl Future<Output = Ledger3> + 'static {
        self.buf.read_ledger()
    }
}
