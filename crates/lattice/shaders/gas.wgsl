// Lattice gas under Kawasaki Monte Carlo. Requires threefry.wgsl. CPU twin: lattice::gas::GasRef.
//
// Sites (x, y), y = 0 at the bottom, are 0 (empty) or 1 (a molecule); neighbouring molecules
// have energy −ε each (ε = 1), and gravity adds g per row of height. A move tries to swap a
// neighbouring pair of sites (a molecule hops to an empty neighbour), accepted with the
// Metropolis probability min(1, e^(−ΔE/kT)).
//
// Moves run in parallel on sublattices of pairs far enough apart not to affect each other's
// ΔE: x-bonds (x, y)–(x+1, y) with x ≡ px (mod 3), y ≡ py (mod 2); y-bonds (x, y)–(x, y+1) with
// x ≡ px (mod 2), y ≡ py (mod 3). Twelve sub-sweeps (6 of each) try every bond once. The
// acceptance thresholds are computed on the CPU, so runs are bit-identical on every device.

struct GasParams {
    w: u32,
    h: u32,
    // 1 if y is periodic; else walls below and above, which count as `wall_lo` / `wall_hi`
    // (0 empty, 1 occupied: a wall the liquid wets).
    periodic_y: u32,
    wall_lo: u32,
    wall_hi: u32,
    // Rows below this use the hot table (a hot plate).
    hot_rows: u32,
    seed_lo: u32,
    seed_hi: u32,
    // Acceptance thresholds (u32 fraction of 2^32; 0xffffffff: always), index (dN + 3)·3 + (dy + 1),
    // where dN is the change in occupied neighbours of the hopping molecule and dy its change of
    // row; entries 0..20 for the bulk, 24..44 for the hot rows.
    table: array<vec4<u32>, 12>,
}

// One sub-sweep: bond direction (0 x, 1 y) and sublattice offsets.
struct Sub {
    dir: u32,
    px: u32,
    py: u32,
    id: u32,
}

@group(0) @binding(0) var<uniform> P: GasParams;
@group(0) @binding(1) var<uniform> S: Sub;
@group(0) @binding(2) var<storage, read_write> occ: array<u32>;
// ctl[0]: sweeps so far (the random-number counter).
@group(0) @binding(3) var<storage, read_write> ctl: array<atomic<u32>>;
// Per row: molecules, then bonds (pairs of neighbouring molecules, counting a wetting wall).
@group(0) @binding(4) var<storage, read_write> rows: array<u32>;

fn at(x: u32, y: u32) -> u32 {
    return occ[y * P.w + x];
}

// Occupied neighbours of (x, y), not counting the site `skip` (the hop's partner).
fn neighbours(x: u32, y: u32, skip: u32) -> i32 {
    var n = 0u;
    let xl = (x + P.w - 1u) % P.w;
    let xr = (x + 1u) % P.w;
    if (y * P.w + xl != skip) {
        n += at(xl, y);
    }
    if (y * P.w + xr != skip) {
        n += at(xr, y);
    }
    // Below.
    if (y > 0u) {
        if ((y - 1u) * P.w + x != skip) {
            n += at(x, y - 1u);
        }
    } else if (P.periodic_y != 0u) {
        if ((P.h - 1u) * P.w + x != skip) {
            n += at(x, P.h - 1u);
        }
    } else {
        n += P.wall_lo;
    }
    // Above.
    if (y + 1u < P.h) {
        if ((y + 1u) * P.w + x != skip) {
            n += at(x, y + 1u);
        }
    } else if (P.periodic_y != 0u) {
        if (x != skip) {
            n += at(x, 0u);
        }
    } else {
        n += P.wall_hi;
    }
    return i32(n);
}

@compute @workgroup_size(256)
fn kawasaki(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    var xa: u32;
    var ya: u32;
    var xb: u32;
    var yb: u32;
    if (S.dir == 0u) {
        let nx = P.w / 3u;
        let i = k % nx;
        let j = k / nx;
        ya = 2u * j + S.py;
        if (ya >= P.h) {
            return;
        }
        xa = 3u * i + S.px;
        xb = (xa + 1u) % P.w;
        yb = ya;
    } else {
        let nx = P.w / 2u;
        let i = k % nx;
        let j = k / nx;
        ya = 3u * j + S.py;
        if (ya >= P.h) {
            return;
        }
        xa = 2u * i + S.px;
        xb = xa;
        yb = ya + 1u;
        if (yb == P.h) {
            if (P.periodic_y == 0u) {
                return;
            }
            yb = 0u;
        }
    }
    let a = ya * P.w + xa;
    let b = yb * P.w + xb;
    let na = occ[a];
    if (na == occ[b]) {
        return;
    }
    // The molecule hops from src to dst.
    var xs = xa;
    var ys = ya;
    var xd = xb;
    var yd = yb;
    var s = a;
    var d = b;
    if (na == 0u) {
        xs = xb;
        ys = yb;
        xd = xa;
        yd = ya;
        s = b;
        d = a;
    }
    let dn = neighbours(xd, yd, s) - neighbours(xs, ys, d);
    // Row change: ±1 for a y-bond (across the periodic seam it is a wrap, counted as ±1 too, so
    // gravity needs walls).
    var dy = 0;
    if (S.dir == 1u) {
        dy = select(-1, 1, (yd == ys + 1u) || (ys == P.h - 1u && yd == 0u));
    }
    let hot = min(ya, yb) < P.hot_rows;
    let idx = u32((dn + 3) * 3 + (dy + 1)) + select(0u, 24u, hot);
    let thr = P.table[idx / 4u][idx % 4u];
    let r = threefry2x32(vec2<u32>(k, atomicLoad(&ctl[0])), vec2<u32>(P.seed_lo ^ (S.id << 24u), P.seed_hi));
    if (thr == 0xffffffffu || r.x < thr) {
        occ[a] = 1u - na;
        occ[b] = na;
    }
}

// Once per sweep, after the twelve sub-sweeps.
@compute @workgroup_size(1)
fn tick() {
    atomicAdd(&ctl[0], 1u);
}

// Per row: molecules, and bonds to the right and upward neighbours (plus the walls').
@compute @workgroup_size(64)
fn count_rows(@builtin(global_invocation_id) gid: vec3<u32>) {
    let y = gid.x;
    if (y >= P.h) {
        return;
    }
    var m = 0u;
    var bonds = 0u;
    for (var x = 0u; x < P.w; x++) {
        let o = at(x, y);
        m += o;
        if (o == 0u) {
            continue;
        }
        bonds += at((x + 1u) % P.w, y);
        if (y + 1u < P.h) {
            bonds += at(x, y + 1u);
        } else if (P.periodic_y != 0u) {
            bonds += at(x, 0u);
        } else {
            bonds += P.wall_hi;
        }
        if (y == 0u && P.periodic_y == 0u) {
            bonds += P.wall_lo;
        }
    }
    rows[y] = m;
    rows[P.h + y] = bonds;
}

// Set the sites within `r` of (cx, cy) to `value`, each with probability `prob` (the page adds
// or removes molecules: a dropper, or a sponge).
struct Paint {
    cx: f32,
    cy: f32,
    r: f32,
    prob: f32,
    value: u32,
    counter: u32,
    _p0: u32,
    _p1: u32,
}
@group(0) @binding(5) var<uniform> B: Paint;

@compute @workgroup_size(256)
fn paint(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.w * P.h) {
        return;
    }
    let x = f32(i % P.w) + 0.5;
    let y = f32(i / P.w) + 0.5;
    // Shortest x distance across the periodic seam.
    var dx = abs(x - B.cx);
    dx = min(dx, f32(P.w) - dx);
    let dy = y - B.cy;
    if (dx * dx + dy * dy > B.r * B.r) {
        return;
    }
    let r = threefry2x32(vec2<u32>(i, B.counter), vec2<u32>(P.seed_lo ^ 0x50414e54u, P.seed_hi));
    if (f32(r.x >> 8u) * (1.0 / 16777216.0) < B.prob) {
        occ[i] = B.value;
    }
}

// Molecules in each 6 × 6 block (row-major, (w/6) × ⌊h/6⌋ blocks), for density readouts.
@group(0) @binding(6) var<storage, read_write> blocks: array<u32>;

@compute @workgroup_size(256)
fn count_blocks(@builtin(global_invocation_id) gid: vec3<u32>) {
    let bw = P.w / 6u;
    let b = gid.x;
    if (b >= bw * (P.h / 6u)) {
        return;
    }
    let x0 = 6u * (b % bw);
    let y0 = 6u * (b / bw);
    var m = 0u;
    for (var y = y0; y < y0 + 6u; y++) {
        for (var x = x0; x < x0 + 6u; x++) {
            m += at(x, y);
        }
    }
    blocks[b] = m;
}
