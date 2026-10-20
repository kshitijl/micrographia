use bytemuck::{Pod, Zeroable};
use gpu_prims::prims::{BinPlan, Gated};
use gpu_prims::util::{self, Dispatch, WG};
use gpu_prims::{fixed64, Gpu, Prims};
use md_core::system::MAX_SPECIES;
use md_core::walls::{CompartmentLedger, Ledger, LEDGER_SCALE, WALLS};
use md_core::{State, System, Wall};
use std::future::Future;

/// Mirrors `struct Params` in `params.wgsl`.
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
    pub flip_index: u32,
    pub flip_word: u32,
    pub flip_mask: u32,
    pub cap: u32,
    pub list_r2: f32,
    pub half_skin2: f32,
    pub mbits: u32,
    pub periodic: u32,
    pub nspecies: u32,
    pub seed_lo: u32,
    pub seed_hi: u32,
    pub _p0: u32,
    pub gravity: [i32; 2],
    pub vstep: f32,
    pub dt: f32,
    pub kick_c: [f32; 4],
    pub mass: [f32; 4],
    pub wall_kind: [u32; 4],
    pub wall_t: [f32; 4],
    pub comp_shift: [u32; 2],
    pub ncomp: [u32; 2],
    pub thermo: [f32; 4],
    pub pairs: [[f32; 4]; 16],
}

/// Words of the control buffer (see `params.wgsl`).
pub mod ctl {
    use md_core::system::MAX_COMPARTMENTS;
    use md_core::walls::WALLS;
    pub const REBUILD: u32 = 0;
    pub const OVERFLOW: u32 = 1;
    pub const CLOCK: u32 = 2;
    pub const REBUILDS: u32 = 3;
    pub const LEDGER: usize = 8;
    /// Words for list flags and stats only.
    pub const HEADER: usize = 8;
    pub const WORDS: usize = LEDGER + 6 * WALLS * MAX_COMPARTMENTS;

    /// Index of the first word of a ledger field (0 energy, 1 impulse, 2 hits).
    pub fn ledger(wall: usize, compartment: usize, field: usize) -> usize {
        LEDGER + 6 * (wall * MAX_COMPARTMENTS + compartment) + 2 * field
    }
}

/// How pair forces find their partners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ForceMode {
    /// Scan the 3×3 neighbouring cells every step (cells at least rc wide).
    CellList,
    /// Verlet lists with a skin, rebuilt (on the GPU's own decision) when some particle has
    /// moved half the skin. `cap` is the list capacity per particle.
    Verlet { skin: f64, cap: u32 },
}

impl ForceMode {
    /// Below this many particles the per-step cost of the (mostly idle) list-rebuild
    /// dispatches outweighs the savings, and a cell list every step is faster (measured on an
    /// M1 Pro; see bench/results.jsonl).
    pub const VERLET_MIN_N: usize = 24_000;

    /// The faster mode for this size.
    pub fn auto(sys: &System, n: usize) -> Self {
        if n < Self::VERLET_MIN_N { ForceMode::CellList } else { Self::verlet_for(sys, n) }
    }

    /// Verlet lists with a 0.4 σ skin (fastest for LJ at density 0.7 in our benchmarks) and
    /// room for twice the mean number of neighbours.
    pub fn verlet_for(sys: &System, n: usize) -> Self {
        Self::verlet_with_skin(sys, n, 0.4)
    }

    pub fn verlet_with_skin(sys: &System, n: usize, skin: f64) -> Self {
        let density = n as f64 / sys.area();
        let r = sys.rc_max() + skin;
        let mut cap = (2.0 * std::f64::consts::PI * r * r * density).ceil() as u32 + 8;
        // Keep the list within one storage binding (128 MiB by default).
        let max_cap = ((128u64 << 20) / (4 * n.max(1) as u64)) as u32;
        cap = cap.min(max_cap);
        ForceMode::Verlet { skin, cap }
    }
}

impl Params {
    pub fn new(sys: &System, n: u32, mode: ForceMode) -> Self {
        let b = sys.box_f64();
        let (skin, cap) = match mode {
            ForceMode::CellList => (0.0, 0),
            ForceMode::Verlet { skin, cap } => (skin, cap),
        };
        // Cells must hold every pair within the list radius.
        let rc = sys.rc_max();
        let [ncx, ncy] = sys.cells_for(rc + skin);
        let ns = sys.species();
        let mut kick_c = [0f32; 4];
        let mut mass = [1f32; 4];
        for s in 0..ns {
            kick_c[s] = sys.kick_coeff(s as u32) as f32;
            mass[s] = sys.masses[s] as f32;
        }
        let mut pairs = [[0f32; 4]; 16];
        for a in 0..ns {
            for bb in 0..ns {
                let (rc2, eps4, sigma2, shift) = sys.pair(a as u32, bb as u32).gpu_constants();
                pairs[a * 4 + bb] = [rc2, eps4, sigma2, shift];
            }
        }
        let mut wall_kind = [0u32; 4];
        let mut wall_t = [0f32; 4];
        for axis in 0..2 {
            if let Some(ws) = sys.walls[axis] {
                for side in 0..2 {
                    let (k, t) = match ws[side] {
                        Wall::Hard => (0, 0.0),
                        Wall::Soft => (1, 0.0),
                        Wall::Thermal(t) => (2, t),
                    };
                    wall_kind[2 * axis + side] = k;
                    wall_t[2 * axis + side] = t as f32;
                }
            }
        }
        let mbits = 32 - (ncx.max(ncy) - 1).leading_zeros();
        // Conservative margins against f32 rounding in the list tests.
        let list_r = (rc + skin) * (1.0 + 1e-4);
        let half_skin = 0.5 * skin * (1.0 - 1e-3);
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
            flip_index: 0,
            flip_word: 0,
            flip_mask: 0,
            cap,
            list_r2: (list_r * list_r) as f32,
            half_skin2: (half_skin * half_skin) as f32,
            mbits,
            periodic: sys.periodic(0) as u32 | (sys.periodic(1) as u32) << 1,
            nspecies: ns as u32,
            seed_lo: sys.seed as u32,
            seed_hi: (sys.seed >> 32) as u32,
            _p0: 0,
            gravity: sys.gravity_kick(),
            vstep: (sys.dt * md_core::ONE as f64) as f32,
            dt: sys.dt as f32,
            kick_c,
            mass,
            wall_kind,
            wall_t,
            comp_shift: sys.compartments,
            ncomp: {
                let c = sys.compartment_counts();
                [c[0] as u32, c[1] as u32]
            },
            thermo: match sys.thermostat {
                Some(t) => [t.t as f32, (t.rate * sys.dt) as f32, 0.0, 0.0],
                None => [0.0; 4],
            },
            pairs,
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
    /// Particle id at each storage slot (storage is periodically reordered for locality).
    pub ids: wgpu::Buffer,
    pub keys: wgpu::Buffer,
    pub kicks: wgpu::Buffer,
    pub pe: wgpu::Buffer,
    /// Control words: list flags, clock, wall ledger (see [`ctl`]).
    pub ctl: wgpu::Buffer,
    /// Force on each particle from soft walls (for the ledger).
    pub wall_f: wgpu::Buffer,
}

/// Put per-slot values into id order.
fn by_id<T: Copy>(ids: &[u32], values: Vec<T>) -> Vec<T> {
    let mut out = values.clone();
    for (slot, &id) in ids.iter().enumerate() {
        out[id as usize] = values[slot];
    }
    out
}

impl Buffers {
    /// Read the full state back in particle-id order (asynchronously; does not borrow the
    /// engine).
    pub fn read_state(&self) -> impl Future<Output = State> + 'static {
        let b = self.clone();
        async move {
            let n = b.n as usize;
            let ids: Vec<u32> = util::read(&b.gpu, &b.ids, n).await;
            let pos: Vec<[u32; 4]> = util::read(&b.gpu, &b.pos, n).await;
            let vel: Vec<[i32; 2]> = util::read(&b.gpu, &b.vel, n).await;
            let species: Vec<u32> = util::read(&b.gpu, &b.species, n).await;
            State { pos: by_id(&ids, unpack_positions(&pos)), vel: by_id(&ids, vel), species: by_id(&ids, species) }
        }
    }

    /// Kicks and per-particle potential energies from the most recent force evaluation, in
    /// particle-id order.
    pub fn read_kicks(&self) -> impl Future<Output = (Vec<[i32; 2]>, Vec<f32>)> + 'static {
        let b = self.clone();
        async move {
            let n = b.n as usize;
            let ids: Vec<u32> = util::read(&b.gpu, &b.ids, n).await;
            let kicks: Vec<[i32; 2]> = util::read(&b.gpu, &b.kicks, n).await;
            let pe: Vec<f32> = util::read(&b.gpu, &b.pe, n).await;
            (by_id(&ids, kicks), by_id(&ids, pe))
        }
    }

    /// (neighbour-list rebuilds so far, whether a list ever overflowed).
    pub fn read_list_stats(&self) -> impl Future<Output = (u32, bool)> + 'static {
        let b = self.clone();
        async move {
            let w: Vec<u32> = util::read(&b.gpu, &b.ctl, ctl::HEADER).await;
            (w[ctl::REBUILDS as usize], w[ctl::OVERFLOW as usize] != 0)
        }
    }

    /// The wall ledger per wall and per compartment along it (`per_wall` compartments).
    pub fn read_compartment_ledger(&self, per_wall: usize) -> impl Future<Output = CompartmentLedger> + 'static {
        let b = self.clone();
        async move {
            let w: Vec<u32> = util::read(&b.gpu, &b.ctl, ctl::WORDS).await;
            let word = |k: usize| fixed64::join([w[k], w[k + 1]]);
            let mut l = CompartmentLedger::new(per_wall);
            for wall in 0..WALLS {
                for c in 0..per_wall {
                    l.energy[wall][c] = word(ctl::ledger(wall, c, 0)) as f64 / LEDGER_SCALE;
                    l.impulse[wall][c] = word(ctl::ledger(wall, c, 1)) as f64 / LEDGER_SCALE;
                    l.hits[wall][c] = word(ctl::ledger(wall, c, 2)) as u64;
                }
            }
            l
        }
    }

    /// The wall ledger totals (energy and impulse exchanged with each wall, and hits).
    pub fn read_ledger(&self) -> impl Future<Output = Ledger> + 'static {
        let fut = self.read_compartment_ledger(md_core::system::MAX_COMPARTMENTS);
        async move { fut.await.totals() }
    }
}

/// Permute particle storage into Morton order of cells. Its counting sort by Morton cell
/// index also serves as the cell list for building neighbour lists.
struct Reorder {
    keys: Dispatch,
    bin: BinPlan,
    passes: [Dispatch; 4],
}

impl Reorder {
    fn new(prims: &Prims, buf: &Buffers, params: &Params) -> Self {
        let gpu = &prims.gpu;
        let n = buf.n;
        let m = util::shader(gpu, "reorder", &crate::reorder_source());
        let pl = |e: &str| util::pipeline(gpu, &m, e);
        let keys = util::storage(gpu, "morton keys", n as u64 * 4);
        let bin = BinPlan::new(prims, &keys, n, 1 << (2 * params.mbits));
        let tmp_a = util::storage(gpu, "tmp pos", n as u64 * 16);
        let tmp_b = util::storage(gpu, "tmp vel", n as u64 * 8);
        let tmp_c = util::storage(gpu, "tmp species", n as u64 * 4);
        let tmp_d = util::storage(gpu, "tmp ids", n as u64 * 4);
        let g = util::groups(n, WG);
        let b = buf;
        Self {
            keys: Dispatch::new_at(gpu, &pl("morton_keys"), &[(0, &b.params), (1, &b.pos), (5, &keys)], g),
            passes: [
                Dispatch::new_at(gpu, &pl("gather_motion"), &[(0, &b.params), (1, &b.pos), (2, &b.vel), (6, &bin.sorted), (7, &tmp_a), (8, &tmp_b)], g),
                Dispatch::new_at(gpu, &pl("gather_meta"), &[(0, &b.params), (3, &b.species), (4, &b.ids), (6, &bin.sorted), (9, &tmp_c), (10, &tmp_d)], g),
                Dispatch::new_at(gpu, &pl("copy_back_motion"), &[(0, &b.params), (1, &b.pos), (2, &b.vel), (7, &tmp_a), (8, &tmp_b)], g),
                Dispatch::new_at(gpu, &pl("copy_back_meta"), &[(0, &b.params), (3, &b.species), (4, &b.ids), (9, &tmp_c), (10, &tmp_d)], g),
            ],
            bin,
        }
    }

    fn dispatches(&self) -> Vec<Dispatch> {
        let mut v = vec![self.keys.clone()];
        v.extend(self.bin.dispatches());
        v.extend(self.passes.iter().cloned());
        v
    }

    fn encode(&self, pass: &mut wgpu::ComputePass<'_>) {
        for d in self.dispatches() {
            d.encode(pass);
        }
    }
}

/// Cell-list force path: cell keys, counting sort by cell, gather over 3×3 cells.
struct CellPath {
    cells: Dispatch,
    bin: BinPlan,
    force: Dispatch,
    integrate: Dispatch,
}

/// Initial control words: a list rebuild is pending.
fn initial_ctl() -> Vec<u32> {
    let mut w = vec![0u32; ctl::WORDS];
    w[ctl::REBUILD as usize] = 1;
    w
}

/// Verlet-list force path: gated (reorder + build list), force over the list.
struct ListPath {
    rebuild: Gated,
    force: Dispatch,
    integrate: Dispatch,
    _lists: [wgpu::Buffer; 3],
}

enum Path {
    Cells(CellPath),
    List(ListPath),
}

/// `gpu-int-f32`.
pub struct GpuEngine {
    pub prims: Prims,
    pub sys: System,
    pub mode: ForceMode,
    pub params: Params,
    pub buf: Buffers,
    path: Path,
    reorder: Reorder,
    d_reverse: Dispatch,
    d_flip: Dispatch,
    /// Advances the clock for stochastic boundaries (only when there are any).
    d_tick: Option<Dispatch>,
    /// Cell-list mode only: reorder storage into Morton order every this many steps (0 =
    /// never). Verlet mode reorders at every list rebuild.
    pub sort_interval: u32,
    since_sort: u32,
    /// Net steps taken (decreases while running reversed).
    pub steps: i64,
}

impl GpuEngine {
    /// The default engine, with the force mode chosen by size (see [`ForceMode::auto`]).
    pub fn new(prims: &Prims, sys: System, state: &State) -> Self {
        let mode = ForceMode::auto(&sys, state.n());
        Self::with_mode(prims, sys, state, mode)
    }

    pub fn with_mode(prims: &Prims, sys: System, state: &State, mode: ForceMode) -> Self {
        assert!(sys.masses.len() <= MAX_SPECIES);
        let gpu = &prims.gpu;
        let n = state.n() as u32;
        let params = Params::new(&sys, n, mode);
        let buf = Buffers {
            gpu: gpu.clone(),
            n,
            params: util::uniform_init(gpu, "md params", &params),
            pos: util::storage_init(gpu, "pos", &pack_positions(&state.pos)),
            vel: util::storage_init(gpu, "vel", &state.vel),
            species: util::storage_init(gpu, "species", &state.species),
            ids: util::storage_init(gpu, "ids", &(0..n).collect::<Vec<u32>>()),
            keys: util::storage(gpu, "keys", n as u64 * 4),
            kicks: util::storage(gpu, "kicks", n as u64 * 8),
            pe: util::storage(gpu, "pe", n as u64 * 4),
            ctl: util::storage_init(gpu, "ctl", &initial_ctl()),
            wall_f: util::storage(gpu, "wall force", n as u64 * 8),
        };
        let g = util::groups(n, WG);
        let b = &buf;
        let md = util::shader(gpu, "md", &crate::md_source());
        let pl = |e: &str| util::pipeline(gpu, &md, e);
        let reorder = Reorder::new(prims, b, &params);
        let path = match mode {
            ForceMode::CellList => {
                let bin = BinPlan::new(prims, &b.keys, n, params.ncell);
                Path::Cells(CellPath {
                    cells: Dispatch::new_at(gpu, &pl("cells"), &[(0, &b.params), (1, &b.pos), (4, &b.keys)], g),
                    force: Dispatch::new_at(
                        gpu,
                        &pl("force"),
                        &[
                            (0, &b.params),
                            (1, &b.pos),
                            (3, &b.species),
                            (5, &bin.starts),
                            (6, &bin.counts),
                            (7, &bin.sorted),
                            (8, &b.kicks),
                            (9, &b.pe),
                            (12, &b.wall_f),
                        ],
                        g,
                    ),
                    integrate: Dispatch::new_at(
                        gpu,
                        &pl("integrate"),
                        &[(0, &b.params), (1, &b.pos), (2, &b.vel), (3, &b.species), (8, &b.kicks), (10, &b.ids), (11, &b.ctl), (12, &b.wall_f)],
                        g,
                    ),
                    bin,
                })
            }
            ForceMode::Verlet { cap, .. } => {
                let nm = util::shader(gpu, "neighbors", &crate::neighbors_source());
                let npl = |e: &str| util::pipeline(gpu, &nm, e);
                let nbr = util::storage(gpu, "nbr", n as u64 * cap as u64 * 4);
                let nbr_count = util::storage(gpu, "nbr count", n as u64 * 4);
                let ref_pos = util::storage(gpu, "ref pos", n as u64 * 16);
                let build = Dispatch::new_at(
                    gpu,
                    &npl("build_list"),
                    &[(0, &b.params), (1, &b.pos), (4, &reorder.bin.starts), (5, &reorder.bin.counts), (6, &nbr), (7, &nbr_count), (8, &ref_pos), (9, &b.ctl)],
                    g,
                );
                let mut list = reorder.dispatches();
                list.push(build);
                Path::List(ListPath {
                    rebuild: Gated::new(prims, &b.ctl, ctl::REBUILD, ctl::REBUILDS, list),
                    force: Dispatch::new_at(
                        gpu,
                        &npl("force_list"),
                        &[(0, &b.params), (1, &b.pos), (3, &b.species), (6, &nbr), (7, &nbr_count), (10, &b.kicks), (11, &b.pe), (13, &b.wall_f)],
                        g,
                    ),
                    integrate: Dispatch::new_at(
                        gpu,
                        &npl("integrate_list"),
                        &[(0, &b.params), (1, &b.pos), (2, &b.vel), (3, &b.species), (8, &ref_pos), (9, &b.ctl), (10, &b.kicks), (12, &b.ids), (13, &b.wall_f)],
                        g,
                    ),
                    _lists: [nbr, nbr_count, ref_pos],
                })
            }
        };
        Self {
            d_reverse: Dispatch::new_at(gpu, &pl("reverse"), &[(0, &b.params), (2, &b.vel), (8, &b.kicks)], g),
            // Any walls could be made thermal later (update_system), so keep a clock for them.
            d_tick: (!sys.periodic(0) || !sys.periodic(1) || sys.thermostat.is_some()).then(|| Dispatch::new_at(gpu, &pl("tick"), &[(11, &b.ctl)], 1)),
            d_flip: Dispatch::new_at(gpu, &pl("flip"), &[(0, &b.params), (1, &b.pos), (10, &b.ids)], g),
            reorder,
            path,
            sort_interval: 100,
            since_sort: 0,
            prims: prims.clone(),
            sys,
            mode,
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

    /// Names of the phases of one step, for per-phase timing.
    pub fn phase_names(&self) -> &'static [&'static str] {
        match self.path {
            Path::Cells(_) => &["cells", "bin", "force", "integrate"],
            Path::List(_) => &["rebuild", "force", "integrate"],
        }
    }

    pub fn encode_phase(&self, pass: &mut wgpu::ComputePass<'_>, phase: usize) {
        match (&self.path, phase) {
            (Path::Cells(c), 0) => c.cells.encode(pass),
            (Path::Cells(c), 1) => c.bin.encode(pass),
            (Path::Cells(c), 2) => c.force.encode(pass),
            (Path::Cells(c), 3) => c.integrate.encode(pass),
            (Path::List(l), 0) => l.rebuild.encode(pass),
            (Path::List(l), 1) => l.force.encode(pass),
            (Path::List(l), 2) => l.integrate.encode(pass),
            _ => panic!("no phase {phase}"),
        }
    }

    /// Record a force evaluation at the current positions.
    pub fn encode_forces(&self, pass: &mut wgpu::ComputePass<'_>) {
        match &self.path {
            Path::Cells(c) => {
                c.cells.encode(pass);
                c.bin.encode(pass);
                c.force.encode(pass);
            }
            Path::List(l) => {
                l.rebuild.encode(pass);
                l.force.encode(pass);
            }
        }
    }

    pub fn encode_step(&self, pass: &mut wgpu::ComputePass<'_>) {
        self.encode_forces(pass);
        match &self.path {
            Path::Cells(c) => c.integrate.encode(pass),
            Path::List(l) => l.integrate.encode(pass),
        }
        if let Some(t) = &self.d_tick {
            t.encode(pass);
        }
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
        let cells = matches!(self.path, Path::Cells(_));
        let mut left = steps;
        while left > 0 {
            let k = left.min(STEPS_PER_SUBMIT);
            let interval = self.sort_interval;
            let mut since = self.since_sort;
            self.submit(|pass| {
                for _ in 0..k {
                    if cells && interval > 0 && since >= interval {
                        self.reorder.encode(pass);
                        since = 0;
                    }
                    self.encode_step(pass);
                    since += 1;
                }
            });
            self.since_sort = since;
            left -= k;
        }
        self.steps += steps as i64;
    }

    /// Evaluate forces at the current positions without moving (for `kicks`).
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

    /// Change interactions, masses, wall kinds or temperatures, gravity or the seed while
    /// running. The box, species count and compartments must stay the same, and no cutoff may
    /// grow beyond the largest one the engine was built with (cells and lists are sized by it).
    pub fn update_system(&mut self, sys: System) {
        assert_eq!(sys.box_len, self.sys.box_len);
        assert_eq!(sys.species(), self.sys.species());
        assert_eq!(sys.compartments, self.sys.compartments);
        assert_eq!(sys.walls.map(|w| w.is_some()), self.sys.walls.map(|w| w.is_some()));
        assert_eq!(sys.thermostat.is_some(), self.sys.thermostat.is_some(), "the thermostat can be retuned, not added or removed");
        assert!(sys.rc_max() <= self.sys.rc_max() + 1e-12, "cutoffs cannot grow at run time");
        let mut p = Params::new(&sys, self.n(), self.mode);
        // Keep the geometry the buffers were sized for.
        p.ncx = self.params.ncx;
        p.ncy = self.params.ncy;
        p.ncell = self.params.ncell;
        p.cell_scale = self.params.cell_scale;
        p.mbits = self.params.mbits;
        p.list_r2 = self.params.list_r2;
        p.half_skin2 = self.params.half_skin2;
        self.gpu().queue.write_buffer(&self.buf.params, 0, bytemuck::bytes_of(&p));
        self.params = p;
        self.sys = sys;
    }

    /// Reorder particle storage into Morton order now (and rebuild lists, if any).
    pub fn sort_now(&mut self) {
        self.submit(|pass| self.reorder.encode(pass));
        self.request_rebuild();
        self.since_sort = 0;
    }

    fn request_rebuild(&self) {
        self.gpu().queue.write_buffer(&self.buf.ctl, 4 * ctl::REBUILD as u64, bytemuck::bytes_of(&1u32));
    }

    pub fn read_ledger(&self) -> impl Future<Output = Ledger> + 'static {
        self.buf.read_ledger()
    }

    /// XOR bit `bit` (0 = least significant, 2⁻³² σ) of coordinate `axis` of the particle with
    /// id `particle`.
    pub fn flip_bit(&mut self, particle: u32, axis: usize, bit: u32) {
        let mut p = self.params;
        // The flip fields are only read by the flip kernel; later writes of `params` reset them.
        p.flip_index = particle;
        p.flip_word = (2 * axis as u32) + bit / 32;
        p.flip_mask = 1 << (bit % 32);
        self.gpu().queue.write_buffer(&self.buf.params, 0, bytemuck::bytes_of(&p));
        self.submit(|pass| self.d_flip.encode(pass));
    }

    /// Replace the state (same particle count).
    pub fn upload(&mut self, state: &State) {
        assert_eq!(state.n() as u32, self.n());
        let q = &self.gpu().queue;
        q.write_buffer(&self.buf.pos, 0, bytemuck::cast_slice(&pack_positions(&state.pos)));
        q.write_buffer(&self.buf.vel, 0, bytemuck::cast_slice(&state.vel));
        q.write_buffer(&self.buf.species, 0, bytemuck::cast_slice(&state.species));
        q.write_buffer(&self.buf.ids, 0, bytemuck::cast_slice(&(0..self.n()).collect::<Vec<u32>>()));
        self.request_rebuild();
        self.steps = 0;
        self.since_sort = 0;
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
