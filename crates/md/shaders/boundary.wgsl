// Kick, drift and boundaries for one particle. Requires fixed64.wgsl, threefry.wgsl and
// params.wgsl, and module-scope `pos`, `vel`, `kicks`, `species`, `ids`, `ctl` and `aux`
// (per particle: potential energy, soft-wall force x and y, piston force).
// CPU twin: md-ref IntEngine::step and collide.

// Add a signed fixed-point value to ledger field `field` (0 energy, 1 impulse, 2 hits) of
// wall w, compartment c, as a two-word 64-bit atomic sum (exact and order independent).
fn ledger_add(w: u32, c: u32, field: u32, value: i32) {
    let base = CTL_LEDGER + 6u * (w * MAX_COMP + c) + 2u * field;
    let v = i64_from_i32(value);
    let old = atomicAdd(&ctl[base], v.x);
    atomicAdd(&ctl[base + 1u], v.y + select(0u, 1u, old + v.x < old));
}

fn atomic_add64(k: u32, value: i32) {
    let v = i64_from_i32(value);
    let old = atomicAdd(&ctl[k], v.x);
    atomicAdd(&ctl[k + 1u], v.y + select(0u, 1u, old + v.x < old));
}

fn load64(k: u32) -> vec2<u32> {
    return vec2<u32>(atomicLoad(&ctl[k]), atomicLoad(&ctl[k + 1u]));
}

fn store64(k: u32, v: vec2<u32>) {
    atomicStore(&ctl[k], v.x);
    atomicStore(&ctl[k + 1u], v.y);
}

fn piston_pos() -> vec2<u32> {
    return load64(CTL_PISTON_POS);
}

// The piston's push on a particle at p (a soft wall above it): adds the kick and energy, and
// adds the reaction to the piston's force sum. Returns the force on the piston (ε/σ).
fn add_piston(acc: ptr<function, Acc>, p: vec4<u32>, c: f32) -> f32 {
    if (P.piston_mode == 0u) {
        return 0.0;
    }
    let d = sep_f32(sub64(piston_pos(), p.zw));
    if (d >= SOFT_RC) {
        return 0.0;
    }
    let s2 = 1.0 / (d * d);
    let s6 = s2 * s2 * s2;
    let f = 24.0 * s6 * (2.0 * s6 - 1.0) / d;
    (*acc).k.y -= i32(round(f * c));
    (*acc).u += 4.0 * s6 * (s6 - 1.0) + 1.0;
    atomic_add64(CTL_PISTON_FORCE, i32(round(f * 65536.0)));
    return f;
}

fn unit_open(x: u32) -> f32 {
    return (f32(x >> 8u) + 0.5) * (1.0 / 16777216.0);
}

// A heat bath's outgoing velocity (σ/τ): Maxwell flux normal component pointing inward, and a
// Gaussian tangential component. Counter-based on (particle id, clock), like
// md_core::walls::thermal_velocity (which uses f64, so values agree only approximately).
fn thermal_draw(id: u32, clock: u32, t: f32, m: f32, inward: f32) -> vec2<f32> {
    let key = vec2<u32>(P.seed_lo, P.seed_hi ^ 0x57414c4cu);
    let r = threefry2x32(vec2<u32>(id, clock), key);
    let r2 = threefry2x32(vec2<u32>(id, clock), vec2<u32>(key.x, key.y ^ 1u));
    let sd = sqrt(t / m);
    let vn = inward * sd * sqrt(-2.0 * log(unit_open(r.x)));
    let vt = sd * sqrt(-2.0 * log(unit_open(r2.x))) * cos(6.2831853 * unit_open(r2.y));
    return vec2<f32>(vn, vt);
}

// The ledger slot of a particle at position p at a wall of `axis`: its compartment along the
// other axis.
fn ledger_slot(p: vec4<u32>, axis: u32) -> u32 {
    return select(comp1(p.xy, 0u), comp1(p.zw, 1u), axis == 0u);
}

// A particle (at old position p) crossed wall `side` of `axis`; returns its new velocity.
fn collide(i: u32, p: vec4<u32>, axis: u32, side: u32, v: vec2<i32>) -> vec2<i32> {
    let w = 2u * axis + side;
    let c = ledger_slot(p, axis);
    let m = P.mass[species[i]];
    var out = v;
    if (P.wall_kind[w] == WALL_THERMAL) {
        let inward = select(-1.0, 1.0, side == 0u);
        let d = thermal_draw(ids[i], atomicLoad(&ctl[CTL_CLOCK]), P.wall_t[w], m, inward);
        let vn = i32(round(d.x * P.vstep));
        let vt = i32(round(d.y * P.vstep));
        out = select(vec2<i32>(vt, vn), vec2<i32>(vn, vt), axis == 0u);
    } else {
        // Hard (and soft, if a particle ever reaches its plane): specular.
        if (axis == 0u) {
            out.x = -v.x;
        } else {
            out.y = -v.y;
        }
    }
    let before = vec2<f32>(v) / P.vstep;
    let after = vec2<f32>(out) / P.vstep;
    ledger_add(w, c, 0u, i32(round(0.5 * m * (dot(after, after) - dot(before, before)) * LEDGER_SCALE)));
    ledger_add(w, c, 1u, i32(round(m * abs(before[axis] - after[axis]) * LEDGER_SCALE)));
    ledger_add(w, c, 2u, 1);
    return out;
}

// Reflect a coordinate that left [0, l) about −½ or l − ½ (exact involutions); side 2 if it
// stayed inside.
struct Mirrored {
    x: vec2<u32>,
    side: u32,
}

// Mirror back into compartment k of side 2^s (s ≥ 32) if x left it.
fn mirror_compartment64(x: vec2<u32>, k: u32, s: u32) -> vec2<u32> {
    let lo = vec2<u32>(0u, k << (s - 32u));
    let hi = vec2<u32>(0u, (k + 1u) << (s - 32u));
    let one = vec2<u32>(1u, 0u);
    if (lt64(x, lo)) {
        return sub64(sub64(add64(lo, lo), one), x);
    }
    if (!lt64(x, hi)) {
        return sub64(sub64(add64(hi, hi), one), x);
    }
    return x;
}

fn mirror64(x: vec2<u32>, l: vec2<u32>) -> Mirrored {
    if (is_neg64(x)) {
        return Mirrored(vec2<u32>(~x.x, ~x.y), 0u);
    }
    if (!lt64(x, l)) {
        return Mirrored(sub64(sub64(add64(l, l), vec2<u32>(1u, 0u)), x), 1u);
    }
    return Mirrored(x, 2u);
}

// Andersen thermostat: with probability P.thermo.y, a new Maxwell velocity at P.thermo.x.
// Counter-based on (particle id, clock), like md_core::walls::andersen.
fn andersen(i: u32, v: vec2<i32>) -> vec2<i32> {
    if (P.thermo.y <= 0.0 || is_pinned(species[i])) {
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
    let m = P.mass[species[i]];
    let sd = sqrt(P.thermo.x / m) * P.vstep;
    let rad = sqrt(-2.0 * log(unit_open(r.y)));
    let a = 6.2831853 * unit_open(r2.x);
    var out = vec2<i32>(i32(round(sd * rad * cos(a))), i32(round(sd * rad * sin(a))));
    // Under a moving piston the gas flows (affinely, v = u y / h); the bath redraws velocities
    // relative to that flow, otherwise it would act as friction on it.
    if (P.piston_mode != 0u) {
        let vp = f32(bitcast<i32>(atomicLoad(&ctl[CTL_PISTON_VEL])));
        let h = coord_f32(piston_pos());
        out.y += i32(round(vp * clamp(coord_f32(pos[i].zw) / h, 0.0, 1.0)));
    }
    // Book the energy the bath gave (slot 6 of the ledger, like a wall).
    let vb = vec2<f32>(v) / P.vstep;
    let va = vec2<f32>(out) / P.vstep;
    ledger_add(6u, 0u, 0u, i32(round(0.5 * m * (dot(va, va) - dot(vb, vb)) * LEDGER_SCALE)));
    ledger_add(6u, 0u, 2u, 1);
    return out;
}

// Is the coordinate `along` the membrane inside a pore? (Pore period 2^s, s ≥ 32.)
fn in_pore(along: vec2<u32>) -> bool {
    let s = P.pores.x;
    if (s == 0u) {
        return false;
    }
    let offset = vec2<u32>(along.x, along.y & ((1u << (s - 32u)) - 1u));
    return lt64(offset, P.pores.yz);
}

// Leapfrog: v += kick + gravity; x += v; then periodic wrap or wall collision per axis.
// Returns the new position (also stored).
fn advance(i: u32) -> vec4<u32> {
    // Only read what this system uses (uniform branches, so no divergence and no extra loads).
    var g = P.gravity;
    if (P.pinned != 0u && is_pinned(species[i])) {
        g = vec2<i32>(0, 0);
    }
    var v = andersen(i, vel[i] + kicks[i] + g);
    let p = pos[i];
    var x = add64(p.xy, i64_from_i32(v.x));
    var y = add64(p.zw, i64_from_i32(v.y));
    // A semi-permeable membrane reflects blocked species (exact mirror about position − ½).
    if (P.membrane.x < 2u && (P.membrane.y & (1u << species[i])) != 0u && !in_pore(select(p.xy, p.zw, P.membrane.x == 0u))) {
        let a = P.membrane.x;
        let w = P.membrane.zw;
        let before = select(p.zw, p.xy, a == 0u);
        var now = select(y, x, a == 0u);
        let was_low = lt64(before, w);
        if (was_low != lt64(now, w)) {
            now = sub64(sub64(add64(w, w), vec2<u32>(1u, 0u)), now);
            let face = select(5u, 4u, was_low);
            let vn = select(v.y, v.x, a == 0u);
            let m = P.mass[species[i]];
            ledger_add(face, ledger_slot(p, a), 1u, i32(round(2.0 * m * abs(f32(vn) / P.vstep) * LEDGER_SCALE)));
            ledger_add(face, ledger_slot(p, a), 2u, 1);
            if (a == 0u) {
                x = now;
                v.x = -v.x;
            } else {
                y = now;
                v.y = -v.y;
            }
        }
    }
    if ((P.features & 1u) != 0u) {
        let wf = aux[i].yz;
        if (wf.x != 0.0) {
            ledger_add(select(1u, 0u, wf.x > 0.0), ledger_slot(p, 0u), 1u, i32(round(abs(wf.x) * P.dt * LEDGER_SCALE)));
        }
        if (wf.y != 0.0) {
            ledger_add(select(3u, 2u, wf.y > 0.0), ledger_slot(p, 1u), 1u, i32(round(abs(wf.y) * P.dt * LEDGER_SCALE)));
        }
    }
    if ((P.periodic & 1u) != 0u) {
        x = wrap64(x, P.box_x);
    } else {
        let m = mirror64(x, P.box_x);
        x = m.x;
        if (m.side < 2u) {
            v = collide(i, p, 0u, m.side, v);
        } else if (P.comp_shift.x != 0u) {
            let k = comp1(p.xy, 0u);
            if (comp1(x, 0u) != k) {
                x = mirror_compartment64(x, k, P.comp_shift.x);
                v.x = -v.x;
            }
        }
    }
    if (P.piston_mode != 0u) {
        // A particle that reaches the piston's plane (only at very high speed) reflects.
        let w = piston_pos();
        if (!lt64(y, w)) {
            y = sub64(sub64(add64(w, w), vec2<u32>(1u, 0u)), y);
            v.y = -v.y;
        }
    }
    if ((P.periodic & 2u) != 0u) {
        y = wrap64(y, P.box_y);
    } else {
        let m = mirror64(y, P.box_y);
        y = m.x;
        if (m.side < 2u) {
            v = collide(i, p, 1u, m.side, v);
        } else if (P.comp_shift.y != 0u) {
            let k = comp1(p.zw, 1u);
            if (comp1(y, 1u) != k) {
                y = mirror_compartment64(y, k, P.comp_shift.y);
                v.y = -v.y;
            }
        }
    }
    vel[i] = v;
    let q = vec4<u32>(x, y);
    pos[i] = q;
    return q;
}

// Gas force on the piston at the last force evaluation (ε/σ).
fn piston_gas_force() -> f32 {
    let f = load64(CTL_PISTON_FORCE);
    return f32(bitcast<i32>(f.y)) * 65536.0 + f32(f.x) / 65536.0;
}

// The piston's leapfrog (one thread, after the particles moved): kick (free pistons), drift,
// stops, and the work done by the gas.
fn piston_step() {
    var v = bitcast<i32>(atomicLoad(&ctl[CTL_PISTON_VEL]));
    let gas = piston_gas_force();
    if (P.piston_mode == 2u) {
        v += i32(round((gas - P.piston_fext) * P.piston_kc));
    }
    let old = piston_pos();
    var w = add64(old, i64_from_i32(v));
    if (lt64(w, P.piston_min)) {
        w = P.piston_min;
        v = 0;
    }
    if (lt64(P.piston_max, w)) {
        w = P.piston_max;
        v = 0;
    }
    store64(CTL_PISTON_POS, w);
    atomicStore(&ctl[CTL_PISTON_VEL], bitcast<u32>(v));
    let dw = sep_f32(sub64(w, old));
    atomic_add64(CTL_PISTON_WORK, i32(round(gas * dw * LEDGER_SCALE)));
}

// Time reversal for the piston: v ← −(v + kick).
fn piston_reverse() {
    var v = bitcast<i32>(atomicLoad(&ctl[CTL_PISTON_VEL]));
    if (P.piston_mode == 2u) {
        v += i32(round((piston_gas_force() - P.piston_fext) * P.piston_kc));
    }
    atomicStore(&ctl[CTL_PISTON_VEL], bitcast<u32>(-v));
}
