// gpu-int-f32 in 3D, with a cell list: integer positions and velocities, f32 pair forces
// converted to integer kicks per pair. Requires fixed64.wgsl and threefry.wgsl. CPU twin:
// md-ref IntEngine3<f32>. The 2D kernels (md.wgsl and friends) are separate, so 3D costs 2D
// nothing.
//
// Positions are three 64-bit fixed-point coordinates (lo, hi words) in units of 2^-32 σ;
// velocities and kicks are the i32 displacement per step in the same units.

// Mirrors engine3::Params3.
struct Params3 {
    n: u32,
    ncx: u32,
    ncy: u32,
    ncz: u32,
    ncell: u32,
    // Bit a set if axis a is periodic.
    periodic: u32,
    nspecies: u32,
    // Bit s set if species s is pinned.
    pinned: u32,
    box_x: vec2<u32>,
    box_y: vec2<u32>,
    box_z: vec2<u32>,
    half_x: vec2<u32>,
    half_y: vec2<u32>,
    half_z: vec2<u32>,
    seed_lo: u32,
    seed_hi: u32,
    // Δt·2³² (σ/τ → position units per step), and Δt.
    vstep: f32,
    dt: f32,
    // Bit 0: there are soft walls.
    features: u32,
    // The heat bath acts on species with their bit set here.
    thermo_species: u32,
    // Neighbour lists: capacity per particle (0: cell list every step).
    cap: u32,
    _p0: u32,
    // Cells per σ on each axis (w unused).
    cell_scale: vec4<f32>,
    // Neighbour lists: (squared list radius (rc + skin)², squared half skin, 0, 0).
    list: vec4<f32>,
    // Integer gravity kick per step (w unused).
    gravity: vec4<i32>,
    // Per species: force → integer kick factor Δt²·2³²/m, and mass.
    kick_c: vec4<f32>,
    mass: vec4<f32>,
    // Andersen heat bath: (temperature, probability per particle per step, 0, 0), and √(kT/m)
    // per species in position units per step.
    thermo: vec4<f32>,
    thermo_sd: vec4<f32>,
    // Per wall (2·axis + side): 0 hard, 1 soft, 2 thermal.
    wall_kind: array<vec4<u32>, 2>,
    // Per wall and species: √(kT/m) (σ/τ) for heat-bath walls.
    wall_sd: array<vec4<f32>, 6>,
    // Per species pair a·4 + b: (rc², 4ε, σ², energy shift).
    pairs: array<vec4<f32>, 16>,
    // Per species pair a·4 + b: (its cutoff + skin)², the radius its neighbour lists keep it within.
    pair_list_r2: array<vec4<f32>, 4>,
}

struct Pos3 {
    x: vec2<u32>,
    y: vec2<u32>,
    z: vec2<u32>,
}

struct I3 {
    x: i32,
    y: i32,
    z: i32,
}

// Commands from the host (kick a region, change species), in one uniform buffer.
struct Cmd3 {
    lo: vec4<f32>,
    hi: vec4<f32>,
    add: vec4<i32>,
    scale: f32,
    count: u32,
    to_species: u32,
    _p: u32,
}

@group(0) @binding(0) var<uniform> P: Params3;
@group(0) @binding(1) var<storage, read_write> pos: array<Pos3>;
@group(0) @binding(2) var<storage, read_write> vel: array<I3>;
@group(0) @binding(3) var<storage, read_write> species: array<u32>;
@group(0) @binding(4) var<storage, read_write> keys: array<u32>;
@group(0) @binding(5) var<storage, read> cell_start: array<u32>;
@group(0) @binding(6) var<storage, read> cell_count: array<u32>;
@group(0) @binding(7) var<storage, read> cell_items: array<u32>;
@group(0) @binding(8) var<storage, read_write> kicks: array<I3>;
// Per particle: potential energy, and the force from soft walls along x, y and z.
@group(0) @binding(9) var<storage, read_write> aux: array<vec4<f32>>;
@group(0) @binding(10) var<storage, read_write> ids: array<u32>;
@group(0) @binding(11) var<storage, read_write> ctl: array<atomic<u32>>;
@group(0) @binding(12) var<uniform> C: Cmd3;
@group(0) @binding(13) var<storage, read> set_list: array<u32>;
@group(0) @binding(14) var<storage, read> perm: array<u32>;
@group(0) @binding(15) var<storage, read_write> tmp_pos: array<Pos3>;
@group(0) @binding(16) var<storage, read_write> tmp_vel: array<I3>;
@group(0) @binding(17) var<storage, read_write> tmp_species: array<u32>;
@group(0) @binding(18) var<storage, read_write> tmp_ids: array<u32>;
@group(0) @binding(19) var<storage, read_write> ke: array<f32>;
// Neighbour lists (column-major: entry k of particle i at k·n + i), their lengths, and where each
// particle was when they were built.
@group(0) @binding(20) var<storage, read_write> nbr: array<u32>;
@group(0) @binding(21) var<storage, read_write> nbr_count: array<u32>;
@group(0) @binding(22) var<storage, read_write> ref_pos: array<Pos3>;

const WALL_SOFT: u32 = 1u;
const WALL_THERMAL: u32 = 2u;
const SOFT_RC: f32 = 1.122462;
const CTL_REBUILD: u32 = 0u;
const CTL_OVERFLOW: u32 = 1u;
const CTL_CLOCK: u32 = 2u;
// Ledger: per wall w (0..5, then the bath as 6): energy, impulse, hits, 2 words each.
const CTL_LEDGER: u32 = 16u;
const LEDGER_SCALE: f32 = 16777216.0;

fn is_pinned(s: u32) -> bool {
    return (P.pinned & (1u << s)) != 0u;
}

fn v3(v: I3) -> vec3<i32> {
    return vec3<i32>(v.x, v.y, v.z);
}

fn i3(v: vec3<i32>) -> I3 {
    return I3(v.x, v.y, v.z);
}

// A coordinate in σ, for cell assignment only (cells are wider than the cutoff by a margin).
fn coord_f32(x: vec2<u32>) -> f32 {
    return f32(x.y) + ldexp(f32(x.x), -32);
}

fn cell_of(p: Pos3) -> vec3<u32> {
    return vec3<u32>(
        min(u32(coord_f32(p.x) * P.cell_scale.x), P.ncx - 1u),
        min(u32(coord_f32(p.y) * P.cell_scale.y), P.ncy - 1u),
        min(u32(coord_f32(p.z) * P.cell_scale.z), P.ncz - 1u),
    );
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
// huge value so the pair fails the cutoff test. (As params.wgsl's sep_f32.)
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

fn sep3(a: Pos3, b: Pos3) -> vec3<f32> {
    var dx = sub64(b.x, a.x);
    var dy = sub64(b.y, a.y);
    var dz = sub64(b.z, a.z);
    if ((P.periodic & 1u) != 0u) {
        dx = min_image64(dx, P.box_x, P.half_x);
    }
    if ((P.periodic & 2u) != 0u) {
        dy = min_image64(dy, P.box_y, P.half_y);
    }
    if ((P.periodic & 4u) != 0u) {
        dz = min_image64(dz, P.box_z, P.half_z);
    }
    return vec3<f32>(sep_f32(dx), sep_f32(dy), sep_f32(dz));
}

fn axis_of(p: Pos3, a: u32) -> vec2<u32> {
    if (a == 0u) {
        return p.x;
    }
    if (a == 1u) {
        return p.y;
    }
    return p.z;
}

fn box_of(a: u32) -> vec2<u32> {
    if (a == 0u) {
        return P.box_x;
    }
    if (a == 1u) {
        return P.box_y;
    }
    return P.box_z;
}

@compute @workgroup_size(256)
fn cells(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let c = cell_of(pos[i]);
    keys[i] = (c.z * P.ncy + c.y) * P.ncx + c.x;
}

struct Acc {
    k: vec3<i32>,
    u: f32,
}

// Add the pair (i, j) to `acc`. Same operations, in the same order, as md-ref's
// IntEngine3::forces.
fn add_pair(acc: ptr<function, Acc>, pi: Pos3, pj: Pos3, si: u32, sj: u32, c: f32) {
    let f = sep3(pi, pj);
    let pc = P.pairs[si * 4u + sj];
    let r2 = f.x * f.x + f.y * f.y + f.z * f.z;
    if (r2 < pc.x) {
        let s2 = pc.z / r2;
        let s6 = s2 * s2 * s2;
        let fr = 6.0 * pc.y * s6 * (2.0 * s6 - 1.0) / r2;
        let a = -fr * c;
        (*acc).k += vec3<i32>(i32(round(a * f.x)), i32(round(a * f.y)), i32(round(a * f.z)));
        (*acc).u += 0.5 * (pc.y * s6 * (s6 - 1.0) - pc.w);
    }
}

// Soft (WCA) walls on the distance of the centre from each wall: adds their kicks and energy,
// and returns their force on the particle (per axis).
fn add_soft_walls(acc: ptr<function, Acc>, pi: Pos3, c: f32) -> vec3<f32> {
    var wf = vec3<f32>(0.0, 0.0, 0.0);
    if ((P.features & 1u) == 0u) {
        return wf;
    }
    for (var a = 0u; a < 3u; a++) {
        if ((P.periodic & (1u << a)) != 0u) {
            continue;
        }
        let x = axis_of(pi, a);
        for (var side = 0u; side < 2u; side++) {
            if (P.wall_kind[(2u * a + side) / 4u][(2u * a + side) % 4u] != WALL_SOFT) {
                continue;
            }
            let d = sep_f32(select(x, sub64(box_of(a), x), side == 1u));
            if (d < SOFT_RC) {
                let s2 = 1.0 / (d * d);
                let s6 = s2 * s2 * s2;
                let f = 24.0 * s6 * (2.0 * s6 - 1.0) / d;
                let kw = i32(round(f * c));
                (*acc).k[a] += select(kw, -kw, side == 1u);
                wf[a] += select(f, -f, side == 1u);
                (*acc).u += 4.0 * s6 * (s6 - 1.0) + 1.0;
            }
        }
    }
    return wf;
}

// Gather: each thread sums the integer kicks on its particle from the 3×3×3 neighbouring cells
// (and soft walls).
@compute @workgroup_size(256)
fn force(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let pi = pos[i];
    let si = species[i];
    let c = P.kick_c[si];
    let cc = cell_of(pi);
    var acc = Acc(vec3<i32>(0, 0, 0), 0.0);
    for (var oz = 0u; oz < 3u; oz++) {
        let cz = (cc.z + P.ncz + oz - 1u) % P.ncz;
        for (var oy = 0u; oy < 3u; oy++) {
            let cy = (cc.y + P.ncy + oy - 1u) % P.ncy;
            for (var ox = 0u; ox < 3u; ox++) {
                let cx = (cc.x + P.ncx + ox - 1u) % P.ncx;
                let cell = (cz * P.ncy + cy) * P.ncx + cx;
                let start = cell_start[cell];
                let end = start + cell_count[cell];
                for (var q = start; q < end; q++) {
                    let j = cell_items[q];
                    if (j != i) {
                        add_pair(&acc, pi, pos[j], si, species[j], c);
                    }
                }
            }
        }
    }
    let wf = add_soft_walls(&acc, pi, c);
    kicks[i] = i3(acc.k);
    aux[i] = vec4<f32>(acc.u, wf);
}

fn ledger_add(w: u32, field: u32, value: i32) {
    let base = CTL_LEDGER + 6u * w + 2u * field;
    let v = i64_from_i32(value);
    let old = atomicAdd(&ctl[base], v.x);
    atomicAdd(&ctl[base + 1u], v.y + select(0u, 1u, old + v.x < old));
}

fn unit_open(x: u32) -> f32 {
    return (f32(x >> 8u) + 0.5) * (1.0 / 16777216.0);
}

// Andersen heat bath: with probability P.thermo.y, a fresh Maxwell velocity (same draws as
// md_core::three::andersen, in f32).
fn andersen(i: u32, s: u32, v: vec3<i32>) -> vec3<i32> {
    if (P.thermo.y <= 0.0 || is_pinned(s) || (P.thermo_species & (1u << s)) == 0u) {
        return v;
    }
    let id = ids[i];
    let clock = atomicLoad(&ctl[CTL_CLOCK]);
    let key = vec2<u32>(P.seed_lo, P.seed_hi ^ 0x5448524du);
    let r = threefry2x32(vec2<u32>(id, clock), key);
    if (unit_open(r.x) >= P.thermo.y) {
        return v;
    }
    let r2 = threefry2x32(vec2<u32>(id, clock), vec2<u32>(key.x, key.y ^ 1u));
    let r3 = threefry2x32(vec2<u32>(id, clock), vec2<u32>(key.x, key.y ^ 2u));
    let sd = P.thermo_sd[s];
    let rad = sqrt(-2.0 * log(unit_open(r.y)));
    let a = 6.2831853 * unit_open(r2.x);
    let rad3 = sqrt(-2.0 * log(unit_open(r3.x)));
    let a3 = 6.2831853 * unit_open(r3.y);
    let out = vec3<i32>(i32(round(sd * rad * cos(a))), i32(round(sd * rad * sin(a))), i32(round(sd * rad3 * cos(a3))));
    let m = P.mass[s];
    let vb = vec3<f32>(v) / P.vstep;
    let va = vec3<f32>(out) / P.vstep;
    ledger_add(6u, 0u, i32(round(0.5 * m * (dot(va, va) - dot(vb, vb)) * LEDGER_SCALE)));
    ledger_add(6u, 2u, 1);
    return out;
}

// A particle crossed wall `side` of axis `a`: its new velocity, booked in the ledger.
fn collide(i: u32, s: u32, a: u32, side: u32, v: vec3<i32>) -> vec3<i32> {
    let w = 2u * a + side;
    var out = v;
    if (P.wall_kind[w / 4u][w % 4u] == WALL_THERMAL) {
        // Normal component from the Maxwell flux, pointing inward; two Gaussian tangential ones
        // (as md_core::three::thermal_velocity).
        let key = vec2<u32>(P.seed_lo, P.seed_hi ^ 0x57414c4cu);
        let clock = atomicLoad(&ctl[CTL_CLOCK]);
        let r = threefry2x32(vec2<u32>(ids[i], clock), key);
        let r2 = threefry2x32(vec2<u32>(ids[i], clock), vec2<u32>(key.x, key.y ^ 1u));
        let sd = P.wall_sd[w][s];
        let inward = select(-1.0, 1.0, side == 0u);
        let vn = inward * sd * sqrt(-2.0 * log(unit_open(r.x)));
        let rad = sd * sqrt(-2.0 * log(unit_open(r2.x)));
        let ang = 6.2831853 * unit_open(r2.y);
        out[a] = i32(round(vn * P.vstep));
        out[(a + 1u) % 3u] = i32(round(rad * cos(ang) * P.vstep));
        out[(a + 2u) % 3u] = i32(round(rad * sin(ang) * P.vstep));
    } else {
        // Hard (and soft, if a particle ever reaches its plane): specular.
        out[a] = -v[a];
    }
    let m = P.mass[s];
    let before = vec3<f32>(v) / P.vstep;
    let after = vec3<f32>(out) / P.vstep;
    ledger_add(w, 0u, i32(round(0.5 * m * (dot(after, after) - dot(before, before)) * LEDGER_SCALE)));
    ledger_add(w, 1u, i32(round(m * abs(before[a] - after[a]) * LEDGER_SCALE)));
    ledger_add(w, 2u, 1);
    return out;
}

// Wrap (periodic) or mirror (walls: exact involutions about −½ and L − ½) one coordinate.
fn bound(i: u32, s: u32, a: u32, x: vec2<u32>, v: ptr<function, vec3<i32>>) -> vec2<u32> {
    let l = box_of(a);
    if ((P.periodic & (1u << a)) != 0u) {
        if (is_neg64(x)) {
            return add64(x, l);
        }
        if (!lt64(x, l)) {
            return sub64(x, l);
        }
        return x;
    }
    if (is_neg64(x)) {
        *v = collide(i, s, a, 0u, *v);
        return vec2<u32>(~x.x, ~x.y);
    }
    if (!lt64(x, l)) {
        *v = collide(i, s, a, 1u, *v);
        return sub64(sub64(add64(l, l), vec2<u32>(1u, 0u)), x);
    }
    return x;
}

// Leapfrog: v += kick + gravity (then the bath); x += v; then wrap or walls on each axis.
// Returns the new position (also stored).
fn advance(i: u32) -> Pos3 {
    let s = species[i];
    var g = P.gravity.xyz;
    if (P.pinned != 0u && is_pinned(s)) {
        g = vec3<i32>(0, 0, 0);
    }
    var v = andersen(i, s, v3(vel[i]) + v3(kicks[i]) + g);
    let p = pos[i];
    if ((P.features & 1u) != 0u) {
        // The soft walls' push, booked as impulse on the wall pushing.
        let wf = aux[i].yzw;
        for (var a = 0u; a < 3u; a++) {
            if (wf[a] != 0.0) {
                ledger_add(2u * a + select(1u, 0u, wf[a] > 0.0), 1u, i32(round(abs(wf[a]) * P.dt * LEDGER_SCALE)));
            }
        }
    }
    let x = bound(i, s, 0u, add64(p.x, i64_from_i32(v.x)), &v);
    let y = bound(i, s, 1u, add64(p.y, i64_from_i32(v.y)), &v);
    let z = bound(i, s, 2u, add64(p.z, i64_from_i32(v.z)), &v);
    vel[i] = i3(v);
    let q = Pos3(x, y, z);
    pos[i] = q;
    return q;
}

@compute @workgroup_size(256)
fn integrate(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i < P.n) {
        advance(i);
    }
}

// ---- Verlet lists (as neighbors.wgsl in 2D). Storage is in Morton order of cells when a list
// is built (the reorder runs first), so the particles of Morton cell c occupy the slots
// [cell_start[c], cell_start[c] + cell_count[c]). A pair is listed if it is within rc + skin; the
// list stays valid until some particle has moved half the skin, which `integrate_list` detects
// exactly. Forces are then the same function of positions as with the cell list. ----

@compute @workgroup_size(256)
fn build_list(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let pi = pos[i];
    let si = species[i];
    let cc = cell_of(pi);
    var count = 0u;
    for (var oz = 0u; oz < 3u; oz++) {
        let cz = (cc.z + P.ncz + oz - 1u) % P.ncz;
        for (var oy = 0u; oy < 3u; oy++) {
            let cy = (cc.y + P.ncy + oy - 1u) % P.ncy;
            for (var ox = 0u; ox < 3u; ox++) {
                let cx = (cc.x + P.ncx + ox - 1u) % P.ncx;
                let key = spread3(cx) | (spread3(cy) << 1u) | (spread3(cz) << 2u);
                let start = cell_start[key];
                let end = start + cell_count[key];
                for (var j = start; j < end; j++) {
                    if (j == i) {
                        continue;
                    }
                    let f = sep3(pi, pos[j]);
                    let k = si * 4u + species[j];
                    if (dot(f, f) < P.pair_list_r2[k / 4u][k % 4u]) {
                        if (count < P.cap) {
                            nbr[count * P.n + i] = j;
                        }
                        count++;
                    }
                }
            }
        }
    }
    if (count > P.cap) {
        atomicMax(&ctl[CTL_OVERFLOW], 1u);
        count = P.cap;
    }
    nbr_count[i] = count;
    ref_pos[i] = pi;
}

@compute @workgroup_size(256)
fn force_list(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let pi = pos[i];
    let si = species[i];
    let c = P.kick_c[si];
    var acc = Acc(vec3<i32>(0, 0, 0), 0.0);
    let m = nbr_count[i];
    for (var k = 0u; k < m; k++) {
        let j = nbr[k * P.n + i];
        add_pair(&acc, pi, pos[j], si, species[j], c);
    }
    let wf = add_soft_walls(&acc, pi, c);
    kicks[i] = i3(acc.k);
    aux[i] = vec4<f32>(acc.u, wf);
}

// Leapfrog and boundaries, then flag a rebuild if this particle has moved half the skin since
// the list was built (wall mirrors count as moves, so this stays conservative).
@compute @workgroup_size(256)
fn integrate_list(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let q = advance(i);
    let d = sep3(ref_pos[i], q);
    if (dot(d, d) >= P.list.y) {
        atomicMax(&ctl[CTL_REBUILD], 1u);
    }
}

// Advance the clock used by stochastic boundaries (once per step, after integrate).
@compute @workgroup_size(1)
fn tick() {
    atomicAdd(&ctl[CTL_CLOCK], 1u);
}

// Time reversal: v ← −(v + kick(x) + g).
@compute @workgroup_size(256)
fn reverse(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    var g = P.gravity.xyz;
    if (P.pinned != 0u && is_pinned(species[i])) {
        g = vec3<i32>(0, 0, 0);
    }
    vel[i] = i3(-(v3(vel[i]) + v3(kicks[i]) + g));
}

// For every unpinned particle in the box C.lo–C.hi: velocity × C.scale + C.add.
@compute @workgroup_size(256)
fn kick_region(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n || is_pinned(species[i])) {
        return;
    }
    let p = pos[i];
    let x = vec3<f32>(coord_f32(p.x), coord_f32(p.y), coord_f32(p.z));
    if (any(x < C.lo.xyz) || any(x >= C.hi.xyz)) {
        return;
    }
    vel[i] = i3(vec3<i32>(round(vec3<f32>(v3(vel[i])) * C.scale)) + C.add.xyz);
}

// Set the species of the particles whose ids are in `set_list` (sorted, C.count of them).
@compute @workgroup_size(256)
fn set_species(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n || C.count == 0u) {
        return;
    }
    let id = ids[i];
    var lo = 0u;
    var hi = C.count;
    while (lo < hi) {
        let mid = (lo + hi) / 2u;
        if (set_list[mid] < id) {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    if (lo < C.count && set_list[lo] == id) {
        species[i] = C.to_species;
    }
}

// Kinetic energy of each slot from the half-step velocity (pinned particles count zero).
@compute @workgroup_size(256)
fn kinetic(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let s = species[i];
    let v = vec3<f32>(v3(vel[i])) / P.vstep;
    ke[i] = select(0.5 * P.mass[s] * dot(v, v), 0.0, is_pinned(s));
}

// ---- Spatial reordering into Morton order of cells (locality for the force kernel). ----

fn spread3(v: u32) -> u32 {
    var x = v & 0x3ffu;
    x = (x | (x << 16u)) & 0x030000ffu;
    x = (x | (x << 8u)) & 0x0300f00fu;
    x = (x | (x << 4u)) & 0x030c30c3u;
    x = (x | (x << 2u)) & 0x09249249u;
    return x;
}

@compute @workgroup_size(256)
fn morton_keys(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let c = cell_of(pos[i]);
    keys[i] = spread3(c.x) | (spread3(c.y) << 1u) | (spread3(c.z) << 2u);
}

@compute @workgroup_size(256)
fn gather_motion(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if (k >= P.n) {
        return;
    }
    let i = perm[k];
    tmp_pos[k] = pos[i];
    tmp_vel[k] = vel[i];
}

@compute @workgroup_size(256)
fn gather_meta(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if (k >= P.n) {
        return;
    }
    let i = perm[k];
    tmp_species[k] = species[i];
    tmp_ids[k] = ids[i];
}

@compute @workgroup_size(256)
fn copy_back_motion(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if (k >= P.n) {
        return;
    }
    pos[k] = tmp_pos[k];
    vel[k] = tmp_vel[k];
}

@compute @workgroup_size(256)
fn copy_back_meta(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if (k >= P.n) {
        return;
    }
    species[k] = tmp_species[k];
    ids[k] = tmp_ids[k];
}

// Give every particle of species C.to_species the velocity C.add (position units per step).
@compute @workgroup_size(256)
fn set_species_velocity(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i < P.n && species[i] == C.to_species) {
        vel[i] = i3(C.add.xyz);
    }
}
