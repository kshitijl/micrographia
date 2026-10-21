//! The fast pair-force path of the CPU engine: a cell list by counting sort, a half shell of
//! neighbour cells (each pair once), and for each cell one fused, branch-free loop per particle
//! over its neighbourhood gathered into small contiguous buffers, which the compiler turns into
//! SIMD (wasm `simd128`, NEON, SSE/AVX).
//!
//! Bit-exact with the reference loop (whose f32 operations are the GPU kernel's, though a GPU
//! may round a division differently, so the GPU tracks it closely rather than exactly): each
//! pair's separation is converted
//! to float exactly as the reference converts it, and its force factor −U'(r)/r is computed with
//! the same operations in the same order. That factor depends only on r², so it is the same seen
//! from either end; each end's kick is then rounded with its own mass coefficient, exactly as the
//! reference rounds it for that end. Integer kicks sum the same in any order. Only the potential
//! energy, a float sum, comes out in a different order (it does not feed the dynamics).
//!
//! The separations: within a cell or to a neighbour cell, t's position minus s's, shifted by a
//! box length across a periodic edge. For any pair within the cutoff that is the minimum image,
//! as the reference takes (cells are at least a cutoff wide).

use crate::real::{PairConsts, Real};
use md_core::{fixed, State, System};

/// Vector width the neighbourhoods are padded to (two 4-wide f32 vectors).
const LANES: usize = 8;

/// Reusable buffers, so a step allocates nothing.
pub struct Scratch<F> {
    /// Particles sorted by cell: position, species, original index; and each cell's start.
    sx: Vec<i64>,
    sy: Vec<i64>,
    ss: Vec<u32>,
    sid: Vec<u32>,
    start: Vec<u32>,
    fill: Vec<u32>,
    key: Vec<u32>,
    /// Kicks by sorted index.
    kx: Vec<i32>,
    ky: Vec<i32>,
    /// One cell's neighbourhood (its own particles first, then its half shell's), positions
    /// shifted across periodic edges, with their sorted indices and kicks accumulated here.
    nx: Vec<i64>,
    ny: Vec<i64>,
    ni: Vec<u32>,
    nkx: Vec<i32>,
    nky: Vec<i32>,
    /// One row's separations as floats.
    rx: Vec<F>,
    ry: Vec<F>,
}

impl<F> Default for Scratch<F> {
    fn default() -> Self {
        Self {
            sx: vec![],
            sy: vec![],
            ss: vec![],
            sid: vec![],
            start: vec![],
            fill: vec![],
            key: vec![],
            kx: vec![],
            ky: vec![],
            nx: vec![],
            ny: vec![],
            ni: vec![],
            nkx: vec![],
            nky: vec![],
            rx: vec![],
            ry: vec![],
        }
    }
}

/// Whether the fast path applies: every pair of particles may interact (no insulating
/// compartment walls between them), and cells are small enough that every candidate pair's
/// separation (up to two cells) is within the 8 σ a float separation holds.
pub fn eligible(sys: &System, cells: [u32; 2]) -> bool {
    let b = sys.box_f64();
    let side = (b[0] / cells[0] as f64).max(b[1] / cells[1] as f64);
    (sys.compartments == [0, 0] || sys.compartment_walls == md_core::system::CompartmentWalls::Diathermal) && 2.0 * side < 7.9
}

/// Integer pair kicks for every particle (by id), and the pairs' potential energy.
pub(crate) fn pair_kicks<F: Real>(
    sys: &System,
    state: &State,
    consts: &[PairConsts<F>],
    kick_c: &[F],
    cells: [u32; 2],
    sc: &mut Scratch<F>,
    kicks: &mut [[i32; 2]],
) -> f64 {
    let n = state.n();
    let ns = sys.species();
    let [cnx, cny] = cells;
    let ncell = (cnx * cny) as usize;
    let b = sys.box_f64();
    let periodic = [sys.periodic(0), sys.periodic(1)];
    let l = sys.box_len;

    // Counting sort into cells.
    sc.key.clear();
    sc.key.extend(state.pos.iter().map(|p| {
        let cx = crate::cells::cell_of(fixed::to_f64(p[0]), b[0], cnx);
        let cy = crate::cells::cell_of(fixed::to_f64(p[1]), b[1], cny);
        cy * cnx + cx
    }));
    sc.start.clear();
    sc.start.resize(ncell + 1, 0);
    for &k in &sc.key {
        sc.start[k as usize + 1] += 1;
    }
    for c in 0..ncell {
        sc.start[c + 1] += sc.start[c];
    }
    for v in [&mut sc.sx, &mut sc.sy] {
        v.clear();
        v.resize(n, 0);
    }
    for v in [&mut sc.ss, &mut sc.sid] {
        v.clear();
        v.resize(n, 0);
    }
    sc.fill.clear();
    sc.fill.extend_from_slice(&sc.start);
    for i in 0..n {
        let k = sc.key[i] as usize;
        let s = sc.fill[k] as usize;
        sc.fill[k] += 1;
        sc.sx[s] = state.pos[i][0];
        sc.sy[s] = state.pos[i][1];
        sc.ss[s] = state.species[i];
        sc.sid[s] = i as u32;
    }
    for v in [&mut sc.kx, &mut sc.ky] {
        v.clear();
        v.resize(n, 0);
    }

    // The half shell: the cell itself (t after s) and four of its eight neighbours.
    const SHELL: [(i32, i32); 4] = [(1, 0), (-1, 1), (0, 1), (1, 1)];
    // A neighbour cell coordinate, and the shift of its positions across a periodic edge; with
    // walls, a wrapped neighbour is never within reach.
    let wrap = |t: i32, m: u32, axis: usize| -> Option<(u32, i64)> {
        if t < 0 {
            periodic[axis].then_some(((t + m as i32) as u32, -l[axis]))
        } else if t >= m as i32 {
            periodic[axis].then_some(((t - m as i32) as u32, l[axis]))
        } else {
            Some((t as u32, 0))
        }
    };
    let mut pe = F::from_f64(0.0);
    for cy in 0..cny {
        for cx in 0..cnx {
            let c = (cy * cnx + cx) as usize;
            let (a0, a1) = (sc.start[c] as usize, sc.start[c + 1] as usize);
            if a0 == a1 {
                continue;
            }
            // Gather the neighbourhood.
            sc.nx.clear();
            sc.ny.clear();
            sc.ni.clear();
            let mut ranges = [(a0, a1, [0i64; 2]); 5];
            let mut nr = 1;
            for &(ox, oy) in &SHELL {
                let (Some((dx, shx)), Some((dy, shy))) = (wrap(cx as i32 + ox, cnx, 0), wrap(cy as i32 + oy, cny, 1)) else { continue };
                let d = (dy * cnx + dx) as usize;
                ranges[nr] = (sc.start[d] as usize, sc.start[d + 1] as usize, [shx, shy]);
                nr += 1;
            }
            for &(b0, b1, sh) in &ranges[..nr] {
                sc.nx.extend(sc.sx[b0..b1].iter().map(|&x| x + sh[0]));
                sc.ny.extend(sc.sy[b0..b1].iter().map(|&y| y + sh[1]));
                sc.ni.extend(b0 as u32..b1 as u32);
            }
            // Padded to whole vectors with copies of the last entry (masked off below), so that
            // every row runs full vector widths with no scalar remainder.
            let m = sc.nx.len();
            let mp = m.div_ceil(LANES) * LANES;
            let (lx, ly, li) = (sc.nx[m - 1], sc.ny[m - 1], sc.ni[m - 1]);
            sc.nx.resize(mp, lx);
            sc.ny.resize(mp, ly);
            sc.ni.resize(mp, li);
            sc.nkx.clear();
            sc.nkx.resize(mp, 0);
            sc.nky.clear();
            sc.nky.resize(mp, 0);
            // Each of the cell's particles against the whole neighbourhood, counting only the
            // entries after it (lo..m): its own cell's later particles, and the shell's.
            for p in 0..a1 - a0 {
                let s = a0 + p;
                // The separations, as the reference converts them (a loop of its own: i64 lanes
                // and f32 lanes do not vectorise well together).
                let (xs, ys) = (sc.sx[s], sc.sy[s]);
                sc.rx.clear();
                sc.rx.extend(sc.nx.iter().map(|&x| F::sep_near(x - xs)));
                sc.ry.clear();
                sc.ry.extend(sc.ny.iter().map(|&y| F::sep_near(y - ys)));
                let (kx, ky, e) = if ns == 1 {
                    row_uniform(&consts[0], kick_c[0], p + 1, m, &sc.rx, &sc.ry, &mut sc.nkx, &mut sc.nky)
                } else {
                    let si = sc.ss[s] as usize;
                    row_mixture(&consts[si * ns..(si + 1) * ns], kick_c, si, p + 1, m, &sc.rx, &sc.ry, &sc.ni, &sc.ss, &mut sc.nkx, &mut sc.nky)
                };
                sc.kx[s] = sc.kx[s].wrapping_add(kx);
                sc.ky[s] = sc.ky[s].wrapping_add(ky);
                pe += e;
            }
            // Scatter the neighbourhood's kicks back, once per cell.
            for k in 0..m {
                let t = sc.ni[k] as usize;
                sc.kx[t] = sc.kx[t].wrapping_add(sc.nkx[k]);
                sc.ky[t] = sc.ky[t].wrapping_add(sc.nky[k]);
            }
        }
    }
    for s in 0..n {
        kicks[sc.sid[s] as usize] = [sc.kx[s], sc.ky[s]];
    }
    pe.to_f64()
}

/// One particle (one species throughout) against its padded neighbourhood at separations
/// (rx, ry), counting only entries lo..hi: its kick, summed; theirs, added into (tkx, tky); and
/// the pairs' energy. Branch-free so that it vectorises: entries outside lo..hi and pairs beyond
/// the cutoff give zero. The float operations are the reference's (PairConsts::eval).
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn row_uniform<F: Real>(p: &PairConsts<F>, c: F, lo: usize, hi: usize, rx: &[F], ry: &[F], tkx: &mut [i32], tky: &mut [i32]) -> (i32, i32, F) {
    let m = rx.len();
    let (ry, tkx, tky) = (&ry[..m], &mut tkx[..m], &mut tky[..m]);
    let (zero, one, two, six) = (F::from_f64(0.0), F::from_f64(1.0), F::from_f64(2.0), F::from_f64(6.0));
    let (mut kx, mut ky, mut pe) = (0i32, 0i32, zero);
    for k in 0..m {
        let (x, y) = (rx[k], ry[k]);
        let r2 = x * x + y * y;
        let s2 = p.sigma2 / r2;
        let s6 = s2 * s2 * s2;
        let f = six * p.eps4 * s6 * (two * s6 - one) / r2;
        let e = p.eps4 * s6 * (s6 - one) - p.shift;
        let on = r2 < p.rc2 && k >= lo && k < hi;
        let a = -(if on { f } else { zero }) * c;
        kx = kx.wrapping_add((a * x).round_even().to_i32());
        ky = ky.wrapping_add((a * y).round_even().to_i32());
        tkx[k] = tkx[k].wrapping_add((a * -x).round_even().to_i32());
        tky[k] = tky[k].wrapping_add((a * -y).round_even().to_i32());
        pe += if on { e } else { zero };
    }
    (kx, ky, pe)
}

/// The same for a mixture: `row` holds the pair constants of s's species against each species,
/// and each end's kick uses its own species' coefficient.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn row_mixture<F: Real>(
    row: &[PairConsts<F>],
    kick_c: &[F],
    si: usize,
    lo: usize,
    hi: usize,
    rx: &[F],
    ry: &[F],
    ti: &[u32],
    species: &[u32],
    tkx: &mut [i32],
    tky: &mut [i32],
) -> (i32, i32, F) {
    let m = rx.len();
    let (ry, ti, tkx, tky) = (&ry[..m], &ti[..m], &mut tkx[..m], &mut tky[..m]);
    let (zero, one, two, six) = (F::from_f64(0.0), F::from_f64(1.0), F::from_f64(2.0), F::from_f64(6.0));
    let ci = kick_c[si];
    let (mut kx, mut ky, mut pe) = (0i32, 0i32, zero);
    for k in 0..m {
        let sj = species[ti[k] as usize] as usize;
        let p = &row[sj];
        let (x, y) = (rx[k], ry[k]);
        let r2 = x * x + y * y;
        let s2 = p.sigma2 / r2;
        let s6 = s2 * s2 * s2;
        let f = six * p.eps4 * s6 * (two * s6 - one) / r2;
        let e = p.eps4 * s6 * (s6 - one) - p.shift;
        let on = r2 < p.rc2 && k >= lo && k < hi;
        let f = if on { f } else { zero };
        let (ai, aj) = (-f * ci, -f * kick_c[sj]);
        kx = kx.wrapping_add((ai * x).round_even().to_i32());
        ky = ky.wrapping_add((ai * y).round_even().to_i32());
        tkx[k] = tkx[k].wrapping_add((aj * -x).round_even().to_i32());
        tky[k] = tky[k].wrapping_add((aj * -y).round_even().to_i32());
        pe += if on { e } else { zero };
    }
    (kx, ky, pe)
}
