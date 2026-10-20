// Kick, drift and boundaries for one particle. Requires fixed64.wgsl, threefry.wgsl and
// params.wgsl, and module-scope `pos`, `vel`, `kicks`, `species`, `ids`, `ctl` and `wall_f`.
// CPU twin: md-ref IntEngine::step and collide.

// Add a signed fixed-point value to ledger field `field` (0 energy, 1 impulse, 2 hits) of
// wall w, as a two-word 64-bit atomic sum (exact and order independent).
fn ledger_add(w: u32, field: u32, value: i32) {
    let base = CTL_LEDGER + 6u * w + 2u * field;
    let v = i64_from_i32(value);
    let old = atomicAdd(&ctl[base], v.x);
    atomicAdd(&ctl[base + 1u], v.y + select(0u, 1u, old + v.x < old));
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

// A particle crossed wall `side` of `axis`; returns its new velocity.
fn collide(i: u32, axis: u32, side: u32, v: vec2<i32>) -> vec2<i32> {
    let w = 2u * axis + side;
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
    ledger_add(w, 0u, i32(round(0.5 * m * (dot(after, after) - dot(before, before)) * LEDGER_SCALE)));
    ledger_add(w, 1u, i32(round(m * abs(before[axis] - after[axis]) * LEDGER_SCALE)));
    ledger_add(w, 2u, 1);
    return out;
}

// Reflect a coordinate that left [0, l) about −½ or l − ½ (exact involutions); side 2 if it
// stayed inside.
struct Mirrored {
    x: vec2<u32>,
    side: u32,
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

// Leapfrog: v += kick + gravity; x += v; then periodic wrap or wall collision per axis.
// Returns the new position (also stored).
fn advance(i: u32) -> vec4<u32> {
    var v = vel[i] + kicks[i] + P.gravity;
    let p = pos[i];
    var x = add64(p.xy, i64_from_i32(v.x));
    var y = add64(p.zw, i64_from_i32(v.y));
    let wf = wall_f[i];
    if (wf.x != 0.0) {
        ledger_add(select(1u, 0u, wf.x > 0.0), 1u, i32(round(abs(wf.x) * P.dt * LEDGER_SCALE)));
    }
    if (wf.y != 0.0) {
        ledger_add(select(3u, 2u, wf.y > 0.0), 1u, i32(round(abs(wf.y) * P.dt * LEDGER_SCALE)));
    }
    if ((P.periodic & 1u) != 0u) {
        x = wrap64(x, P.box_x);
    } else {
        let m = mirror64(x, P.box_x);
        x = m.x;
        if (m.side < 2u) {
            v = collide(i, 0u, m.side, v);
        }
    }
    if ((P.periodic & 2u) != 0u) {
        y = wrap64(y, P.box_y);
    } else {
        let m = mirror64(y, P.box_y);
        y = m.x;
        if (m.side < 2u) {
            v = collide(i, 1u, m.side, v);
        }
    }
    vel[i] = v;
    let q = vec4<u32>(x, y);
    pos[i] = q;
    return q;
}
