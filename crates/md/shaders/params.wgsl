// Shared by every MD kernel module: the parameter block (mirrors engine::Params, always at
// binding 0), the exact fixed-point pair geometry, and pair and wall forces.
// Requires fixed64.wgsl.

struct Params {
    n: u32,
    ncx: u32,
    ncy: u32,
    ncell: u32,
    box_x: vec2<u32>,
    box_y: vec2<u32>,
    half_x: vec2<u32>,
    half_y: vec2<u32>,
    box_f: vec2<f32>,
    cell_scale: vec2<f32>,
    flip_index: u32,
    flip_word: u32,
    flip_mask: u32,
    // Neighbour lists: capacity per particle, squared list radius (rc + skin)², squared half
    // skin, and bits per axis of the Morton cell index.
    cap: u32,
    list_r2: f32,
    half_skin2: f32,
    mbits: u32,
    // Bit a set if axis a is periodic.
    periodic: u32,
    nspecies: u32,
    seed_lo: u32,
    seed_hi: u32,
    _p0: u32,
    // Integer gravity kick per step, and Δt·2³² (σ/τ → position units per step).
    gravity: vec2<i32>,
    vstep: f32,
    dt: f32,
    // Per species: force → integer kick factor Δt²·2³²/m, and mass.
    kick_c: vec4<f32>,
    mass: vec4<f32>,
    // Per wall (2·axis + side): 0 hard, 1 soft, 2 thermal; and the bath temperature.
    wall_kind: vec4<u32>,
    wall_t: vec4<f32>,
    // Per species pair a·4 + b: (rc², 4ε, σ², energy shift).
    pairs: array<vec4<f32>, 16>,
}

@group(0) @binding(0) var<uniform> P: Params;

const WALL_HARD: u32 = 0u;
const WALL_SOFT: u32 = 1u;
const WALL_THERMAL: u32 = 2u;
const SOFT_RC: f32 = 1.122462;

// Control words (one u32 storage buffer, `ctl`): neighbour-list flags, the step clock for
// stochastic boundaries, and the wall ledger.
const CTL_REBUILD: u32 = 0u;
const CTL_OVERFLOW: u32 = 1u;
const CTL_CLOCK: u32 = 2u;
const CTL_REBUILDS: u32 = 3u;
// Ledger, per wall w: energy (2 words), impulse (2 words), hits (2 words), from word 8 + 6w.
const CTL_LEDGER: u32 = 8u;
const CTL_WORDS: u32 = 32u;
const LEDGER_SCALE: f32 = 16777216.0;

// A coordinate in σ, for cell assignment only (any rounding here is harmless: cells are wider
// than the cutoff by a margin).
fn coord_f32(x: vec2<u32>) -> f32 {
    return f32(x.y) + ldexp(f32(x.x), -32);
}

fn spread_bits(v: u32) -> u32 {
    var x = v & 0x0000ffffu;
    x = (x | (x << 8u)) & 0x00ff00ffu;
    x = (x | (x << 4u)) & 0x0f0f0f0fu;
    x = (x | (x << 2u)) & 0x33333333u;
    x = (x | (x << 1u)) & 0x55555555u;
    return x;
}

// Morton (Z-order) index of a cell.
fn morton(cx: u32, cy: u32) -> u32 {
    return spread_bits(cx) | (spread_bits(cy) << 1u);
}

fn cell_coords(p: vec4<u32>) -> vec2<u32> {
    let cx = min(u32(coord_f32(p.xy) * P.cell_scale.x), P.ncx - 1u);
    let cy = min(u32(coord_f32(p.zw) * P.cell_scale.y), P.ncy - 1u);
    return vec2<u32>(cx, cy);
}

// Exact minimum image of a separation d in [-L, L).
fn min_image64(d: vec2<u32>, l: vec2<u32>, half: vec2<u32>) -> vec2<u32> {
    if (!lt64(d, half)) {
        return sub64(d, l);
    }
    if (lt64(d, neg64(half))) {
        return add64(d, l);
    }
    return d;
}

// Separation in σ through units of 2^-28 σ; antisymmetric bit for bit. Beyond 8 σ returns a
// huge value so the pair fails the cutoff test.
fn sep_f32(d: vec2<u32>) -> f32 {
    let hi = bitcast<i32>(d.y);
    if (hi < -8 || hi > 7) {
        return 1e30;
    }
    let neg = hi < 0;
    let a = select(d, neg64(d), neg);
    let m = ldexp(f32((a.y << 28u) | (a.x >> 4u)), -28);
    return select(m, -m, neg);
}

// Separation b − a in σ (minimum image on periodic axes).
fn sep2(a: vec4<u32>, b: vec4<u32>) -> vec2<f32> {
    var dx = sub64(b.xy, a.xy);
    var dy = sub64(b.zw, a.zw);
    if ((P.periodic & 1u) != 0u) {
        dx = min_image64(dx, P.box_x, P.half_x);
    }
    if ((P.periodic & 2u) != 0u) {
        dy = min_image64(dy, P.box_y, P.half_y);
    }
    return vec2<f32>(sep_f32(dx), sep_f32(dy));
}

fn wrap64(x: vec2<u32>, l: vec2<u32>) -> vec2<u32> {
    if (is_neg64(x)) {
        return add64(x, l);
    }
    if (!lt64(x, l)) {
        return sub64(x, l);
    }
    return x;
}

// Accumulates the integer kick and potential energy on one particle.
struct Acc {
    k: vec2<i32>,
    u: f32,
}

// Add the pair (i, j) with separation f = r_j − r_i (σ) to `acc`. Same operations, in the same
// order, as md-ref's PairConsts::eval and IntEngine::forces.
fn add_pair(acc: ptr<function, Acc>, f: vec2<f32>, si: u32, sj: u32, c: f32) {
    let pc = P.pairs[si * 4u + sj];
    let fx = f.x;
    let fy = f.y;
    let r2 = fx * fx + fy * fy;
    if (r2 < pc.x) {
        let s2 = pc.z / r2;
        let s6 = s2 * s2 * s2;
        let fr = 6.0 * pc.y * s6 * (2.0 * s6 - 1.0) / r2;
        let a = -fr * c;
        (*acc).k += vec2<i32>(i32(round(a * fx)), i32(round(a * fy)));
        (*acc).u += 0.5 * (pc.y * s6 * (s6 - 1.0) - pc.w);
    }
}

// Soft (WCA) walls: add their kicks to `acc` and return the force they exert on the particle
// (σ/ε units; +x is the lower x wall pushing, −x the upper one).
fn add_soft_walls(acc: ptr<function, Acc>, p: vec4<u32>, c: f32) -> vec2<f32> {
    var wf = vec2<f32>(0.0, 0.0);
    for (var a = 0u; a < 2u; a++) {
        if ((P.periodic & (1u << a)) != 0u) {
            continue;
        }
        let x = select(p.zw, p.xy, a == 0u);
        let l = select(P.box_y, P.box_x, a == 0u);
        for (var side = 0u; side < 2u; side++) {
            if (P.wall_kind[2u * a + side] != WALL_SOFT) {
                continue;
            }
            let d = sep_f32(select(x, sub64(l, x), side == 1u));
            if (d < SOFT_RC) {
                let s2 = 1.0 / (d * d);
                let s6 = s2 * s2 * s2;
                let f = 24.0 * s6 * (2.0 * s6 - 1.0) / d;
                var k = i32(round(f * c));
                var fs = f;
                if (side == 1u) {
                    k = -k;
                    fs = -f;
                }
                if (a == 0u) {
                    (*acc).k.x += k;
                    wf.x += fs;
                } else {
                    (*acc).k.y += k;
                    wf.y += fs;
                }
                (*acc).u += 4.0 * s6 * (s6 - 1.0) + 1.0;
            }
        }
    }
    return wf;
}
