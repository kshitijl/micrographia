// Verlet neighbour lists with a skin. Requires fixed64.wgsl and params.wgsl.
//
// Storage is in Morton order of cells when the list is built (reorder.wgsl runs first), so the
// particles of Morton cell c occupy the contiguous slots [starts[c], starts[c] + counts[c]).
// A pair is listed if it is within rc + skin at build time; the list stays valid until some
// particle has moved half the skin, which the integrate kernel detects exactly. Every pair
// within rc is therefore always listed, and forces are the same pure function of positions as
// with a cell list.

@group(0) @binding(1) var<storage, read_write> pos: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read_write> vel: array<vec2<i32>>;
@group(0) @binding(3) var<storage, read> species: array<u32>;
@group(0) @binding(4) var<storage, read> starts: array<u32>;
@group(0) @binding(5) var<storage, read> counts: array<u32>;
@group(0) @binding(6) var<storage, read_write> nbr: array<u32>;
@group(0) @binding(7) var<storage, read_write> nbr_count: array<u32>;
@group(0) @binding(8) var<storage, read_write> ref_pos: array<vec4<u32>>;
// flags[0]: rebuild needed; flags[1]: list overflowed (sticky).
@group(0) @binding(9) var<storage, read_write> flags: array<atomic<u32>>;
@group(0) @binding(10) var<storage, read_write> kicks: array<vec2<i32>>;
@group(0) @binding(11) var<storage, read_write> pe: array<f32>;

fn sep2(a: vec4<u32>, b: vec4<u32>) -> vec2<f32> {
    let fx = sep_f32(min_image64(sub64(b.xy, a.xy), P.box_x, P.half_x));
    let fy = sep_f32(min_image64(sub64(b.zw, a.zw), P.box_y, P.half_y));
    return vec2<f32>(fx, fy);
}

@compute @workgroup_size(256)
fn build_list(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let pi = pos[i];
    let cc = cell_coords(pi);
    var count = 0u;
    for (var oy = 0u; oy < 3u; oy++) {
        let cy = (cc.y + P.ncy + oy - 1u) % P.ncy;
        for (var ox = 0u; ox < 3u; ox++) {
            let cx = (cc.x + P.ncx + ox - 1u) % P.ncx;
            let key = morton(cx, cy);
            let start = starts[key];
            let end = start + counts[key];
            for (var j = start; j < end; j++) {
                if (j == i) {
                    continue;
                }
                let f = sep2(pi, pos[j]);
                if (dot(f, f) < P.list_r2) {
                    if (count < P.cap) {
                        nbr[count * P.n + i] = j;
                    }
                    count++;
                }
            }
        }
    }
    if (count > P.cap) {
        atomicMax(&flags[1], 1u);
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
    let c = P.kick_c[species[i]];
    var kx = 0i;
    var ky = 0i;
    var u_sum = 0.0;
    let m = nbr_count[i];
    for (var k = 0u; k < m; k++) {
        let j = nbr[k * P.n + i];
        let f = sep2(pi, pos[j]);
        let fx = f.x;
        let fy = f.y;
        let r2 = fx * fx + fy * fy;
        if (r2 < P.rc2) {
            let s2 = P.sigma2 / r2;
            let s6 = s2 * s2 * s2;
            let fr = 6.0 * P.eps4 * s6 * (2.0 * s6 - 1.0) / r2;
            let a = -fr * c;
            kx += i32(round(a * fx));
            ky += i32(round(a * fy));
            u_sum += P.eps4 * s6 * (s6 - 1.0) - P.shift;
        }
    }
    kicks[i] = vec2<i32>(kx, ky);
    pe[i] = 0.5 * u_sum;
}

// Leapfrog step, then flag a rebuild if this particle has moved half the skin since the list
// was built.
@compute @workgroup_size(256)
fn integrate_list(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let v = vel[i] + kicks[i];
    vel[i] = v;
    let p = pos[i];
    let q = vec4<u32>(
        wrap64(add64(p.xy, i64_from_i32(v.x)), P.box_x),
        wrap64(add64(p.zw, i64_from_i32(v.y)), P.box_y),
    );
    pos[i] = q;
    let d = sep2(ref_pos[i], q);
    if (dot(d, d) >= P.half_skin2) {
        atomicMax(&flags[0], 1u);
    }
}
