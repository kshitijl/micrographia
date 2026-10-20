use bytemuck::{Pod, Zeroable};
use gpu_prims::prims::BinPlan;
use gpu_prims::util::{self, Dispatch, WG};
use gpu_prims::{fixed64, Gpu, Prims};
use md_core::system::MAX_SPECIES;
use md_core::{State, System};
use std::future::Future;

/// Mirrors `struct Params` in `md.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Params {
    pub n: u32,
    pub ncx: u32,
    pub ncy: u32,
    pub ncell: u32,
    pub box_x: [u32; 2],
    pub box_y: [u32; 2],
    pub half_x: [u32; 2],
    pub half_y: [u32; 2],
    pub box_f: [f32; 2],
    pub cell_scale: [f32; 2],
    pub rc2: f32,
    pub eps4: f32,
    pub sigma2: f32,
    pub shift: f32,
    pub kick_c: [f32; 4],
    pub flip_index: u32,
    pub flip_word: u32,
    pub flip_mask: u32,
    pub _pad: u32,
}

impl Params {
    pub fn new(sys: &System, n: u32) -> Self {
        let [ncx, ncy] = sys.cells();
        let b = sys.box_f64();
        let (rc2, eps4, sigma2, shift) = sys.pair.gpu_constants();
        let mut kick_c = [0f32; 4];
        for s in 0..sys.masses.len() {
            kick_c[s] = sys.kick_coeff(s as u32) as f32;
        }
        Self {
            n,
            ncx,
            ncy,
            ncell: ncx * ncy,
            box_x: fixed64::split(sys.box_len[0]),
            box_y: fixed64::split(sys.box_len[1]),
            half_x: fixed64::split(sys.box_len[0] / 2),
            half_y: fixed64::split(sys.box_len[1] / 2),
            box_f: [b[0] as f32, b[1] as f32],
            cell_scale: [(ncx as f64 / b[0]) as f32, (ncy as f64 / b[1]) as f32],
            rc2,
            eps4,
            sigma2,
            shift,
            kick_c,
            flip_index: 0,
            flip_word: 0,
            flip_mask: 0,
            _pad: 0,
        }
    }
}

/// Positions as the GPU stores them: (x.lo, x.hi, y.lo, y.hi).
pub fn pack_positions(pos: &[[i64; 2]]) -> Vec<[u32; 4]> {
    pos.iter()
        .map(|p| {
            let x = fixed64::split(p[0]);
            let y = fixed64::split(p[1]);
            [x[0], x[1], y[0], y[1]]
        })
        .collect()
}

pub fn unpack_positions(words: &[[u32; 4]]) -> Vec<[i64; 2]> {
    words.iter().map(|w| [fixed64::join([w[0], w[1]]), fixed64::join([w[2], w[3]])]).collect()
}

/// Steps per command submission; longer runs are split into several submissions.
pub const STEPS_PER_SUBMIT: usize = 256;

/// The GPU buffers of an engine, cheap to clone into a readback future.
#[derive(Clone)]
pub struct Buffers {
    pub gpu: Gpu,
    pub n: u32,
    pub params: wgpu::Buffer,
    pub pos: wgpu::Buffer,
    pub vel: wgpu::Buffer,
    pub species: wgpu::Buffer,
    pub keys: wgpu::Buffer,
    pub kicks: wgpu::Buffer,
    pub pe: wgpu::Buffer,
}

impl Buffers {
    /// Read the full state back (asynchronously; does not borrow the engine).
    pub fn read_state(&self) -> impl Future<Output = State> + 'static {
        let b = self.clone();
        async move {
            let n = b.n as usize;
            let pos: Vec<[u32; 4]> = util::read(&b.gpu, &b.pos, n).await;
            let vel: Vec<[i32; 2]> = util::read(&b.gpu, &b.vel, n).await;
            let species: Vec<u32> = util::read(&b.gpu, &b.species, n).await;
            State { pos: unpack_positions(&pos), vel, species }
        }
    }

    /// Kicks and per-particle potential energies from the most recent force evaluation.
    pub fn read_kicks(&self) -> impl Future<Output = (Vec<[i32; 2]>, Vec<f32>)> + 'static {
        let b = self.clone();
        async move {
            let n = b.n as usize;
            (util::read(&b.gpu, &b.kicks, n).await, util::read(&b.gpu, &b.pe, n).await)
        }
    }
}

/// `gpu-int-f32`.
pub struct GpuEngine {
    pub prims: Prims,
    pub sys: System,
    pub params: Params,
    pub buf: Buffers,
    bin: BinPlan,
    d_cells: Dispatch,
    d_force: Dispatch,
    d_integrate: Dispatch,
    d_reverse: Dispatch,
    d_flip: Dispatch,
    /// Net steps taken (decreases while running reversed).
    pub steps: i64,
}

struct Pipelines {
    cells: wgpu::ComputePipeline,
    force: wgpu::ComputePipeline,
    integrate: wgpu::ComputePipeline,
    reverse: wgpu::ComputePipeline,
    flip: wgpu::ComputePipeline,
}

impl Pipelines {
    fn new(gpu: &Gpu) -> Self {
        let m = util::shader(gpu, "md", &crate::md_source());
        Self {
            cells: util::pipeline(gpu, &m, "cells"),
            force: util::pipeline(gpu, &m, "force"),
            integrate: util::pipeline(gpu, &m, "integrate"),
            reverse: util::pipeline(gpu, &m, "reverse"),
            flip: util::pipeline(gpu, &m, "flip"),
        }
    }
}

impl GpuEngine {
    pub fn new(prims: &Prims, sys: System, state: &State) -> Self {
        assert!(sys.masses.len() <= MAX_SPECIES);
        let gpu = &prims.gpu;
        let n = state.n() as u32;
        let params = Params::new(&sys, n);
        let p = Pipelines::new(gpu);
        let buf = Buffers {
            gpu: gpu.clone(),
            n,
            params: util::uniform_init(gpu, "md params", &params),
            pos: util::storage_init(gpu, "pos", &pack_positions(&state.pos)),
            vel: util::storage_init(gpu, "vel", &state.vel),
            species: util::storage_init(gpu, "species", &state.species),
            keys: util::storage(gpu, "keys", n as u64 * 4),
            kicks: util::storage(gpu, "kicks", n as u64 * 8),
            pe: util::storage(gpu, "pe", n as u64 * 4),
        };
        let bin = BinPlan::new(prims, &buf.keys, n, params.ncell);
        let g = util::groups(n, WG);
        let b = &buf;
        Self {
            d_cells: Dispatch::new_at(gpu, &p.cells, &[(0, &b.params), (1, &b.pos), (4, &b.keys)], g),
            d_force: Dispatch::new_at(
                gpu,
                &p.force,
                &[
                    (0, &b.params),
                    (1, &b.pos),
                    (3, &b.species),
                    (5, &bin.starts),
                    (6, &bin.counts),
                    (7, &bin.sorted),
                    (8, &b.kicks),
                    (9, &b.pe),
                ],
                g,
            ),
            d_integrate: Dispatch::new_at(gpu, &p.integrate, &[(0, &b.params), (1, &b.pos), (2, &b.vel), (8, &b.kicks)], g),
            d_reverse: Dispatch::new_at(gpu, &p.reverse, &[(0, &b.params), (2, &b.vel), (8, &b.kicks)], g),
            d_flip: Dispatch::new_at(gpu, &p.flip, &[(0, &b.params), (1, &b.pos)], 1),
            prims: prims.clone(),
            sys,
            params,
            buf,
            bin,
            steps: 0,
        }
    }

    pub fn n(&self) -> u32 {
        self.buf.n
    }

    pub fn gpu(&self) -> &Gpu {
        &self.prims.gpu
    }

    /// Record a force evaluation at the current positions: cell keys, counting sort, kicks.
    pub fn encode_forces(&self, pass: &mut wgpu::ComputePass<'_>) {
        self.d_cells.encode(pass);
        self.bin.encode(pass);
        self.d_force.encode(pass);
    }

    pub fn encode_step(&self, pass: &mut wgpu::ComputePass<'_>) {
        self.encode_forces(pass);
        self.d_integrate.encode(pass);
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

    /// Advance `steps` steps (submitted, not waited for).
    pub fn step(&mut self, steps: usize) {
        let mut left = steps;
        while left > 0 {
            let k = left.min(STEPS_PER_SUBMIT);
            self.submit(|pass| {
                for _ in 0..k {
                    self.encode_step(pass);
                }
            });
            left -= k;
        }
        self.steps += steps as i64;
    }

    /// Evaluate forces at the current positions without moving (for `read_kicks`).
    pub fn compute_forces(&self) {
        self.submit(|pass| self.encode_forces(pass));
    }

    /// Time reversal, `v ← −(v + kick(x))`; see `md_ref::IntEngine::reverse`.
    pub fn reverse(&mut self) {
        self.submit(|pass| {
            self.encode_forces(pass);
            self.d_reverse.encode(pass);
        });
        self.steps = -self.steps;
    }

    /// XOR bit `bit` (0 = least significant, 2⁻³² σ) of coordinate `axis` of a particle.
    pub fn flip_bit(&mut self, particle: u32, axis: usize, bit: u32) {
        let mut p = self.params;
        p.flip_index = particle;
        p.flip_word = (2 * axis as u32) + bit / 32;
        p.flip_mask = 1 << (bit % 32);
        self.gpu().queue.write_buffer(&self.buf.params, 0, bytemuck::bytes_of(&p));
        self.submit(|pass| self.d_flip.encode(pass));
    }

    /// Replace the state (same particle count and species layout).
    pub fn upload(&mut self, state: &State) {
        assert_eq!(state.n() as u32, self.n());
        let q = &self.gpu().queue;
        q.write_buffer(&self.buf.pos, 0, bytemuck::cast_slice(&pack_positions(&state.pos)));
        q.write_buffer(&self.buf.vel, 0, bytemuck::cast_slice(&state.vel));
        q.write_buffer(&self.buf.species, 0, bytemuck::cast_slice(&state.species));
        self.steps = 0;
    }

    pub fn read_state(&self) -> impl Future<Output = State> + 'static {
        self.buf.read_state()
    }

    /// Integer kicks at the current positions and the total potential energy.
    pub fn kicks(&self) -> impl Future<Output = (Vec<[i32; 2]>, f64)> + 'static {
        self.compute_forces();
        let fut = self.buf.read_kicks();
        async move {
            let (k, pe) = fut.await;
            (k, pe.iter().map(|&x| x as f64).sum())
        }
    }
}
