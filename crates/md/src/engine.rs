use bytemuck::{Pod, Zeroable};
use gpu_prims::prims::{BinPlan, Gated};
use gpu_prims::util::{self, Dispatch, WG};
use gpu_prims::{fixed64, Gpu, Prims};
use md_core::system::MAX_SPECIES;
use md_core::walls::{CompartmentLedger, Ledger, LEDGER_SCALE, WALLS};
use md_core::{BondType, PistonDrive, State, System, Wall};
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
    pub pinned: u32,
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
    pub membrane: [u32; 4],
    pub pores: [u32; 4],
    pub piston_mode: u32,
    pub piston_kc: f32,
    pub piston_fext: f32,
    pub thermo_species: u32,
    pub piston_min: [u32; 2],
    pub piston_max: [u32; 2],
    pub itab_len: u32,
    pub features: u32,
    pub thermo_drift: i32,
    pub _p4: u32,
    pub itab_rc2: [u32; 16],
    pub bond_types: [[f32; 4]; 4],
    pub pairs: [[f32; 4]; 16],
    pub fan: [u32; 4],
    pub thermo_sd: [f32; 4],
    pub wall_sd: [[f32; 4]; 4],
}

/// Words of the control buffer (see `params.wgsl`).
pub mod ctl {
    use md_core::system::MAX_COMPARTMENTS;
    use md_core::walls::WALLS;
    pub const REBUILD: u32 = 0;
    pub const OVERFLOW: u32 = 1;
    pub const CLOCK: u32 = 2;
    pub const REBUILDS: u32 = 3;
    pub const PISTON_POS: usize = 8;
    pub const PISTON_VEL: usize = 10;
    pub const PISTON_FORCE: usize = 12;
    pub const PISTON_WORK: usize = 14;
    pub const LEDGER: usize = 16;
    /// Words before the ledger (flags, stats, piston).
    pub const HEADER: usize = 16;
    pub const WORDS: usize = LEDGER + 6 * WALLS * MAX_COMPARTMENTS;

    /// Index of the first word of a ledger field (0 energy, 1 impulse, 2 hits).
    pub fn ledger(wall: usize, compartment: usize, field: usize) -> usize {
        LEDGER + 6 * (wall * MAX_COMPARTMENTS + compartment) + 2 * field
    }
}

/// The arithmetic of pair forces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arith {
    /// f32 pair forces, rounded to integer kicks per pair (`gpu-int-f32`): bit-exact on one
    /// device, exactly reversible.
    F32,
    /// All-integer pair forces from tables (`gpu-int-int`): bit-identical on every device and
    /// to `md_ref::IntEngine::integer`. Pair forces only (no soft walls or piston in the same
    /// kernel).
    Int,
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
            pinned: sys.pinned,
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
            membrane: match sys.membrane {
                Some(m) => {
                    let w = fixed64::split(m.position);
                    [m.axis as u32, m.blocks, w[0], w[1]]
                }
                None => [u32::MAX, 0, 0, 0],
            },
            pores: match sys.membrane {
                Some(m) if m.pore_shift != 0 => {
                    let w = fixed64::split(m.pore_width);
                    [m.pore_shift, w[0], w[1], 0]
                }
                _ => [0; 4],
            },
            piston_mode: match sys.piston.map(|p| p.drive) {
                None => 0,
                Some(PistonDrive::Speed(_)) => 1,
                Some(PistonDrive::Free { .. }) => 2,
            },
            piston_kc: match sys.piston.map(|p| p.drive) {
                Some(PistonDrive::Free { mass, .. }) => (sys.dt * sys.dt / mass * md_core::ONE as f64) as f32,
                _ => 0.0,
            },
            piston_fext: match sys.piston.map(|p| p.drive) {
                Some(PistonDrive::Free { force, .. }) => force as f32,
                _ => 0.0,
            },
            thermo_species: sys.thermostat.map_or(0, |t| t.species),
            piston_min: fixed64::split(sys.piston.map_or(0, |p| md_core::fixed::to_fixed(p.min))),
            piston_max: fixed64::split(sys.piston.map_or(0, |p| md_core::fixed::to_fixed(p.max))),
            itab_len: 0,
            features: sys.has_soft_walls() as u32 | ((sys.compartment_walls == md_core::system::CompartmentWalls::Diathermal) as u32) << 1,
            thermo_drift: sys.thermostat.map_or(0, |t| sys.to_step_displacement(t.drift)),
            _p4: 0,
            itab_rc2: [0; 16],
            bond_types: {
                let mut t = [[0f32; 4]; 4];
                for (k, b) in sys.bond_types.iter().enumerate() {
                    t[k] = match *b {
                        BondType::Harmonic { k, r0 } => [0.0, k as f32, r0 as f32, 0.0],
                        BondType::Fene { k, r0 } => [1.0, k as f32, r0 as f32, 0.0],
                    };
                }
                t
            },
            pairs,
            // The bath's x range, clamped to the box so that unsigned comparisons work.
            fan: {
                let r = sys.thermostat.map_or([0, 0], |t| t.region).map(|x| x.clamp(0, sys.box_len[0]));
                let (lo, hi) = (fixed64::split(r[0]), fixed64::split(r[1]));
                [lo[0], lo[1], hi[0], hi[1]]
            },
            thermo_sd: {
                let mut sd = [0f32; 4];
                if let Some(t) = sys.thermostat {
                    for s in 0..ns {
                        sd[s] = ((t.t / sys.masses[s]).sqrt() * sys.dt * md_core::ONE as f64) as f32;
                    }
                }
                sd
            },
            wall_sd: {
                let mut sd = [[0f32; 4]; 4];
                for w in 0..4 {
                    for s in 0..ns {
                        sd[w][s] = (wall_t[w] as f64 / sys.masses[s]).sqrt() as f32;
                    }
                }
                sd
            },
        }
    }
}

/// Arguments of the `kick_region` kernel.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct RegionCmd {
    lo: [f32; 2],
    hi: [f32; 2],
    scale: f32,
    _pad: u32,
    add: [i32; 2],
}

/// `set_species`'s command, in the same uniform buffer as [`RegionCmd`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct SetCmd {
    count: u32,
    to: u32,
    _pad: [u32; 6],
}

/// How many ids `set_species` sends to the GPU at a time.
const SET_CHUNK: usize = 1024;

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
    /// Per particle: potential energy, soft-wall force (x, y), force on the piston.
    pub aux: wgpu::Buffer,
    /// Control words: list flags, clock, piston, wall ledger (see [`ctl`]).
    pub ctl: wgpu::Buffer,
    /// Bond partners by id, and the storage slot of each id.
    pub partners: wgpu::Buffer,
    pub slot_of: wgpu::Buffer,
    /// All-integer force tables (a placeholder unless `Arith::Int`).
    pub itab: wgpu::Buffer,
    /// Arguments of the region command (`kick_region`).
    pub cmd: wgpu::Buffer,
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

    /// Positions of all particles in id order (reads only ids and positions).
    pub fn read_positions(&self) -> impl Future<Output = Vec<[i64; 2]>> + 'static {
        let b = self.clone();
        async move {
            let n = b.n as usize;
            let ids: Vec<u32> = util::read(&b.gpu, &b.ids, n).await;
            let pos: Vec<[u32; 4]> = util::read(&b.gpu, &b.pos, n).await;
            by_id(&ids, unpack_positions(&pos))
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
            let aux: Vec<[f32; 4]> = util::read(&b.gpu, &b.aux, n).await;
            (by_id(&ids, kicks), by_id(&ids, aux.iter().map(|a| a[0]).collect()))
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

    /// The piston: (height σ, velocity σ/τ, gas force on it ε/σ, work done on it by the gas ε).
    pub fn read_piston(&self, dt: f64) -> impl Future<Output = [f64; 4]> + 'static {
        let b = self.clone();
        async move {
            let w: Vec<u32> = util::read(&b.gpu, &b.ctl, ctl::HEADER).await;
            let word = |k: usize| fixed64::join([w[k], w[k + 1]]);
            [
                md_core::fixed::to_f64(word(ctl::PISTON_POS)),
                (w[ctl::PISTON_VEL] as i32) as f64 * md_core::fixed::UNIT / dt,
                word(ctl::PISTON_FORCE) as f64 / md_core::walls::PISTON_FORCE_SCALE,
                word(ctl::PISTON_WORK) as f64 / LEDGER_SCALE,
            ]
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
    /// Refresh the id → slot map (only needed with bonds).
    invert: Option<Dispatch>,
}

impl Reorder {
    fn new(prims: &Prims, buf: &Buffers, params: &Params, invert: Option<Dispatch>) -> Self {
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
            invert,
        }
    }

    fn dispatches(&self) -> Vec<Dispatch> {
        let mut v = vec![self.keys.clone()];
        v.extend(self.bin.dispatches());
        v.extend(self.passes.iter().cloned());
        v.extend(self.invert.clone());
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

/// Initial control words: a list rebuild is pending, and the piston at its start.
fn initial_ctl(sys: &System) -> Vec<u32> {
    let mut w = vec![0u32; ctl::WORDS];
    w[ctl::REBUILD as usize] = 1;
    if let Some(p) = sys.piston {
        let pos = fixed64::split(md_core::fixed::to_fixed(p.start));
        w[ctl::PISTON_POS] = pos[0];
        w[ctl::PISTON_POS + 1] = pos[1];
        if let PistonDrive::Speed(u) = p.drive {
            w[ctl::PISTON_VEL] = sys.to_step_displacement(u) as u32;
        }
    }
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
    pub arith: Arith,
    pub params: Params,
    pub buf: Buffers,
    path: Path,
    reorder: Reorder,
    d_reverse: Dispatch,
    d_flip: Dispatch,
    d_flip_species: Dispatch,
    d_kick_region: Dispatch,
    d_set_species: Dispatch,
    set_list: wgpu::Buffer,
    /// Advances the clock for stochastic boundaries (only when there are any).
    d_tick: Option<Dispatch>,
    d_set_velocity: Dispatch,
    /// Bond forces (after the pair forces), if there are bonds.
    d_bonds: Option<Dispatch>,
    /// Piston kernels: clear its force sum, move it, reverse it.
    d_piston: Option<[Dispatch; 3]>,
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
        Self::with_options(prims, sys, state, mode, Arith::F32)
    }

    pub fn with_options(prims: &Prims, sys: System, state: &State, mode: ForceMode, arith: Arith) -> Self {
        assert!(sys.masses.len() <= MAX_SPECIES);
        let gpu = &prims.gpu;
        let n = state.n() as u32;
        let mut params = Params::new(&sys, n, mode);
        let tables = (arith == Arith::Int).then(|| {
            assert!(sys.piston.is_none() && !sys.has_soft_walls(), "integer forces cover pair forces only");
            md_core::table::IntTables::new(&sys)
        });
        if let Some(t) = &tables {
            params.itab_len = t.len as u32;
            for a in 0..t.ns {
                for b in 0..t.ns {
                    params.itab_rc2[a * 4 + b] = t.rc2[a * t.ns + b];
                }
            }
        }
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
            aux: util::storage(gpu, "aux", n as u64 * 16),
            ctl: util::storage_init(gpu, "ctl", &initial_ctl(&sys)),
            partners: util::storage_init(gpu, "partners", &sys.partner_table(n as usize)),
            slot_of: util::storage_init(gpu, "slot of", &(0..n).collect::<Vec<u32>>()),
            itab: match &tables {
                Some(t) => util::storage_init(gpu, "int tables", &t.data),
                None => util::storage(gpu, "int tables", 4),
            },
            cmd: util::uniform_init(gpu, "region command", &RegionCmd::default()),
        };
        let g = util::groups(n, WG);
        let set_list = util::storage(gpu, "set species list", SET_CHUNK as u64 * 4);
        let b = &buf;
        let md = util::shader(gpu, "md", &crate::md_source());
        let pl = |e: &str| util::pipeline(gpu, &md, e);
        let has_bonds = !sys.bonds.is_empty();
        let invert = has_bonds.then(|| Dispatch::new_at(gpu, &pl("invert_ids"), &[(0, &b.params), (10, &b.ids), (14, &b.slot_of)], g));
        let reorder = Reorder::new(prims, b, &params, invert);
        let path = match mode {
            ForceMode::CellList => {
                let bin = BinPlan::new(prims, &b.keys, n, params.ncell);
                let force = if arith == Arith::Int {
                    Dispatch::new_at(
                        gpu,
                        &pl("force_int"),
                        &[(0, &b.params), (1, &b.pos), (3, &b.species), (5, &bin.starts), (6, &bin.counts), (7, &bin.sorted), (8, &b.kicks), (9, &b.aux), (15, &b.itab)],
                        g,
                    )
                } else {
                    Dispatch::new_at(
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
                            (9, &b.aux),
                            (11, &b.ctl),
                        ],
                        g,
                    )
                };
                Path::Cells(CellPath {
                    cells: Dispatch::new_at(gpu, &pl("cells"), &[(0, &b.params), (1, &b.pos), (4, &b.keys)], g),
                    force,
                    integrate: Dispatch::new_at(
                        gpu,
                        &pl("integrate"),
                        &[(0, &b.params), (1, &b.pos), (2, &b.vel), (3, &b.species), (8, &b.kicks), (9, &b.aux), (10, &b.ids), (11, &b.ctl)],
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
                    force: if arith == Arith::Int {
                        Dispatch::new_at(
                            gpu,
                            &npl("force_list_int"),
                            &[(0, &b.params), (1, &b.pos), (3, &b.species), (6, &nbr), (7, &nbr_count), (10, &b.kicks), (11, &b.aux), (14, &b.itab)],
                            g,
                        )
                    } else {
                        Dispatch::new_at(
                            gpu,
                            &npl("force_list"),
                            &[(0, &b.params), (1, &b.pos), (3, &b.species), (6, &nbr), (7, &nbr_count), (9, &b.ctl), (10, &b.kicks), (11, &b.aux)],
                            g,
                        )
                    },
                    integrate: Dispatch::new_at(
                        gpu,
                        &npl("integrate_list"),
                        &[(0, &b.params), (1, &b.pos), (2, &b.vel), (3, &b.species), (8, &ref_pos), (9, &b.ctl), (10, &b.kicks), (11, &b.aux), (12, &b.ids)],
                        g,
                    ),
                    _lists: [nbr, nbr_count, ref_pos],
                })
            }
        };
        Self {
            d_reverse: Dispatch::new_at(gpu, &pl("reverse"), &[(0, &b.params), (2, &b.vel), (3, &b.species), (8, &b.kicks)], g),
            // Any walls could be made thermal later (update_system), so keep a clock for them.
            d_tick: (!sys.periodic(0) || !sys.periodic(1) || sys.thermostat.is_some()).then(|| Dispatch::new_at(gpu, &pl("tick"), &[(11, &b.ctl)], 1)),
            d_bonds: has_bonds.then(|| {
                Dispatch::new_at(
                    gpu,
                    &pl("bond_force"),
                    &[(0, &b.params), (1, &b.pos), (3, &b.species), (8, &b.kicks), (9, &b.aux), (10, &b.ids), (13, &b.partners), (14, &b.slot_of)],
                    g,
                )
            }),
            d_piston: sys.piston.is_some().then(|| {
                let one = |e: &str| Dispatch::new_at(gpu, &pl(e), &[(0, &b.params), (11, &b.ctl)], 1);
                [Dispatch::new_at(gpu, &pl("piston_clear"), &[(11, &b.ctl)], 1), one("piston_move"), one("piston_flip")]
            }),
            d_flip: Dispatch::new_at(gpu, &pl("flip"), &[(0, &b.params), (1, &b.pos), (10, &b.ids)], g),
            d_flip_species: Dispatch::new_at(gpu, &pl("flip_species"), &[(0, &b.params), (1, &b.pos), (3, &b.species)], g),
            d_kick_region: Dispatch::new_at(gpu, &pl("kick_region"), &[(0, &b.params), (1, &b.pos), (2, &b.vel), (3, &b.species), (12, &b.cmd)], g),
            d_set_velocity: Dispatch::new_at(gpu, &pl("set_species_velocity"), &[(0, &b.params), (2, &b.vel), (3, &b.species)], g),
            d_set_species: Dispatch::new_at(
                gpu,
                &util::pipeline(gpu, &util::shader(gpu, "reorder", &crate::reorder_source()), "set_species"),
                &[(0, &b.params), (3, &b.species), (4, &b.ids), (11, &b.cmd), (12, &set_list)],
                g,
            ),
            set_list,
            reorder,
            path,
            sort_interval: 100,
            since_sort: 0,
            prims: prims.clone(),
            sys,
            mode,
            arith,
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
        if let Some(p) = &self.d_piston {
            p[0].encode(pass);
        }
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
        if let Some(b) = &self.d_bonds {
            b.encode(pass);
        }
    }

    pub fn encode_step(&self, pass: &mut wgpu::ComputePass<'_>) {
        self.encode_forces(pass);
        match &self.path {
            Path::Cells(c) => c.integrate.encode(pass),
            Path::List(l) => l.integrate.encode(pass),
        }
        if let Some(p) = &self.d_piston {
            p[1].encode(pass);
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
            if let Some(p) = &self.d_piston {
                p[2].encode(pass);
            }
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
        assert!(sys.bonds == self.sys.bonds || *sys.bonds == *self.sys.bonds, "bonds cannot change at run time");
        assert_eq!(sys.walls.map(|w| w.is_some()), self.sys.walls.map(|w| w.is_some()));
        assert_eq!(sys.thermostat.is_some(), self.sys.thermostat.is_some(), "the thermostat can be retuned, not added or removed");
        assert_eq!(sys.piston.map(|p| std::mem::discriminant(&p.drive)), self.sys.piston.map(|p| std::mem::discriminant(&p.drive)));
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
        p.itab_len = self.params.itab_len;
        // Soft walls cannot appear at run time unless the engine read wall forces from the start.
        p.features |= self.params.features;
        p.itab_rc2 = self.params.itab_rc2;
        assert!(self.arith == Arith::F32 || sys.pairs == self.sys.pairs, "integer tables are fixed at creation");
        self.gpu().queue.write_buffer(&self.buf.params, 0, bytemuck::bytes_of(&p));
        self.params = p;
        self.sys = sys;
    }

    /// Set a fixed-speed piston's speed (σ/τ; positive opens the box).
    pub fn set_piston_speed(&mut self, u: f64) {
        let v = self.sys.to_step_displacement(u);
        self.gpu().queue.write_buffer(&self.buf.ctl, 4 * ctl::PISTON_VEL as u64, bytemuck::bytes_of(&v));
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

    /// (height σ, velocity σ/τ, gas force ε/σ, work done by the gas ε).
    pub fn read_piston(&self) -> impl Future<Output = [f64; 4]> + 'static {
        self.buf.read_piston(self.sys.dt)
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

    /// Break the bond between particles `a` and `b` (ids), for good: from the next step it exerts
    /// no force. Returns whether there was such a bond. (Bonds that break make a run irreversible.)
    pub fn remove_bond(&mut self, a: u32, b: u32) -> bool {
        let before = self.sys.bonds.len();
        let bonds: Vec<[u32; 3]> = self.sys.bonds.iter().copied().filter(|x| !((x[0] == a && x[1] == b) || (x[0] == b && x[1] == a))).collect();
        if bonds.len() == before {
            return false;
        }
        self.sys.bonds = std::sync::Arc::new(bonds);
        let table = self.sys.partner_table(self.n() as usize);
        let row = std::mem::size_of_val(&table[0]) as u64;
        for id in [a, b] {
            self.gpu().queue.write_buffer(&self.buf.partners, id as u64 * row, bytemuck::cast_slice(&table[id as usize..id as usize + 1]));
        }
        true
    }

    /// XOR bit `bit` of coordinate `axis` of every particle whose species is in `species` (a
    /// bitmask), in one dispatch.
    pub fn flip_species_bit(&mut self, species: u32, axis: usize, bit: u32) {
        let mut p = self.params;
        p.flip_index = species;
        p.flip_word = (2 * axis as u32) + bit / 32;
        p.flip_mask = 1 << (bit % 32);
        self.gpu().queue.write_buffer(&self.buf.params, 0, bytemuck::bytes_of(&p));
        self.submit(|pass| self.d_flip_species.encode(pass));
        self.gpu().queue.write_buffer(&self.buf.params, 0, bytemuck::bytes_of(&self.params));
    }

    /// For every unpinned particle inside the rectangle `lo`–`hi` (σ): multiply its velocity by
    /// `scale`, then add `add` (σ/τ). A kick (scale > 1), a freeze (scale 0) or a shove.
    pub fn kick_region(&mut self, lo: [f64; 2], hi: [f64; 2], scale: f64, add: [f64; 2]) {
        let c = RegionCmd {
            lo: lo.map(|x| x as f32),
            hi: hi.map(|x| x as f32),
            scale: scale as f32,
            _pad: 0,
            add: add.map(|v| self.sys.to_step_displacement(v)),
        };
        self.gpu().queue.write_buffer(&self.buf.cmd, 0, bytemuck::bytes_of(&c));
        self.submit(|pass| self.d_kick_region.encode(pass));
    }

    /// Set the species of the particles with these ids. Masses, sizes and forces follow the new
    /// species from the next step; positions and velocities are kept. Pages use it to add or
    /// remove salt, say (choosing molecules with room for the new size, so nothing overlaps).
    /// Particles in bonds or of pinned species should not be changed.
    pub fn set_species(&mut self, ids: &[u32], species: u32) {
        let mut ids = ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        for chunk in ids.chunks(SET_CHUNK) {
            let c = SetCmd { count: chunk.len() as u32, to: species, _pad: [0; 6] };
            self.gpu().queue.write_buffer(&self.buf.cmd, 0, bytemuck::bytes_of(&c));
            self.gpu().queue.write_buffer(&self.set_list, 0, bytemuck::cast_slice(chunk));
            self.submit(|pass| self.d_set_species.encode(pass));
        }
    }

    /// Give every particle of `species` the velocity `v` (σ/τ). For pinned species this sets a
    /// steady motion (anchors pulled at constant speed).
    pub fn set_species_velocity(&mut self, species: u32, v: [f64; 2]) {
        let mut p = self.params;
        p.flip_index = species;
        p.flip_word = self.sys.to_step_displacement(v[0]) as u32;
        p.flip_mask = self.sys.to_step_displacement(v[1]) as u32;
        self.gpu().queue.write_buffer(&self.buf.params, 0, bytemuck::bytes_of(&p));
        self.submit(|pass| self.d_set_velocity.encode(pass));
        self.gpu().queue.write_buffer(&self.buf.params, 0, bytemuck::bytes_of(&self.params));
    }

    /// Replace the state (same particle count).
    pub fn upload(&mut self, state: &State) {
        assert_eq!(state.n() as u32, self.n());
        let q = &self.gpu().queue;
        q.write_buffer(&self.buf.pos, 0, bytemuck::cast_slice(&pack_positions(&state.pos)));
        q.write_buffer(&self.buf.vel, 0, bytemuck::cast_slice(&state.vel));
        q.write_buffer(&self.buf.species, 0, bytemuck::cast_slice(&state.species));
        q.write_buffer(&self.buf.ids, 0, bytemuck::cast_slice(&(0..self.n()).collect::<Vec<u32>>()));
        q.write_buffer(&self.buf.slot_of, 0, bytemuck::cast_slice(&(0..self.n()).collect::<Vec<u32>>()));
        self.request_rebuild();
        self.steps = 0;
        self.since_sort = 0;
    }

    pub fn read_state(&self) -> impl Future<Output = State> + 'static {
        self.buf.read_state()
    }

    /// Positions in id order, without velocities or species.
    pub fn read_positions(&self) -> impl Future<Output = Vec<[i64; 2]>> + 'static {
        self.buf.read_positions()
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
