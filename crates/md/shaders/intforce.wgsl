// All-integer pair forces. Requires fixed64.wgsl, params.wgsl and a module-scope table
// `itab: array<i32>`.

// All-integer pair kick (md_core::table::IntTables::kick, bit for bit): separations in 2^-20 σ
// rounded on magnitudes, r² exact then rounded to 2^-24 σ², a linearly interpolated table of
// −U'(r)/r · c, and the kick |T|·|q| >> 22 with its sign applied last. Needs the module-scope
// table `itab`. Returns false beyond the cutoff.
fn int_q(d: vec2<u32>) -> i32 {
    let neg = is_neg64(d);
    let a = select(d, neg64(d), neg);
    // (|d| + 2^11) >> 12, valid for |d| < 2^35 (8 σ).
    let r = add64(a, vec2<u32>(2048u, 0u));
    let m = i32((r.x >> 12u) | (r.y << 20u));
    return select(m, -m, neg);
}

fn add_pair_int(acc: ptr<function, Acc>, pi: vec4<u32>, pj: vec4<u32>, si: u32, sj: u32) {
    if ((P.features & 2u) == 0u && !same_compartment(pi, pj)) {
        return;
    }
    var dx = sub64(pj.xy, pi.xy);
    var dy = sub64(pj.zw, pi.zw);
    if ((P.periodic & 1u) != 0u) {
        dx = min_image64(dx, P.box_x, P.half_x);
    }
    if ((P.periodic & 2u) != 0u) {
        dy = min_image64(dy, P.box_y, P.half_y);
    }
    let hx = bitcast<i32>(dx.y);
    let hy = bitcast<i32>(dy.y);
    if (hx < -8 || hx > 7 || hy < -8 || hy > 7) {
        return;
    }
    let qx = int_q(dx);
    let qy = int_q(dy);
    let r2w = add64(add64(mul_wide_u32(u32(abs(qx)), u32(abs(qx))), mul_wide_u32(u32(abs(qy)), u32(abs(qy)))), vec2<u32>(32768u, 0u));
    let r2 = shr64(r2w, 16u).x;
    let pair = si * 4u + sj;
    if (r2 >= P.itab_rc2[pair / 4u][pair % 4u]) {
        return;
    }
    let base = (si * P.nspecies + sj) * P.itab_len;
    let idx = r2 >> 12u;
    let frac = r2 & 4095u;
    let t0 = itab[base + idx];
    let t1 = itab[base + idx + 1u];
    // t0 + ((t1 − t0) · frac) >> 12, with the product in 64 bits (floor, as in Rust).
    let diff = t1 - t0;
    let p = mul_wide_u32(u32(abs(diff)), frac);
    var corr = shr64(p, 12u).x;
    var t: i32;
    if (diff < 0) {
        // floor of a negative: −ceil(|x|).
        let exact = (p.x & 4095u) == 0u;
        t = t0 - i32(corr) - select(1, 0, exact);
    } else {
        t = t0 + i32(corr);
    }
    let at = u32(abs(t));
    let kx = i32(shr64(mul_wide_u32(at, u32(abs(qx))), 22u).x);
    let ky = i32(shr64(mul_wide_u32(at, u32(abs(qy))), 22u).x);
    // Positive T pushes a away from b: kick along −q.
    let sx = (t < 0) != (qx < 0);
    let sy = (t < 0) != (qy < 0);
    (*acc).k += vec2<i32>(select(-kx, kx, sx), select(-ky, ky, sy));
    // Energy, for observation only (f32).
    let f = vec2<f32>(sep_f32(dx), sep_f32(dy));
    let pc = P.pairs[pair];
    let fr2 = dot(f, f);
    let s2 = pc.z / fr2;
    let s6 = s2 * s2 * s2;
    (*acc).u += 0.5 * (pc.y * s6 * (s6 - 1.0) - pc.w);
}
