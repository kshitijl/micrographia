// gpu-int-f32: integer positions and velocities, f32 pair forces converted to integer kicks
// per pair. Requires fixed64.wgsl and params.wgsl. CPU twin: md-ref IntEngine<f32>.
//
// Positions are vec4<u32>(x.lo, x.hi, y.lo, y.hi) in units of 2^-32 σ; velocities are the
// i32 displacement per step in the same units.

@group(0) @binding(1) var<storage, read_write> pos: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read_write> vel: array<vec2<i32>>;
@group(0) @binding(3) var<storage, read> species: array<u32>;
@group(0) @binding(4) var<storage, read_write> keys: array<u32>;
@group(0) @binding(5) var<storage, read> cell_start: array<u32>;
@group(0) @binding(6) var<storage, read> cell_count: array<u32>;
@group(0) @binding(7) var<storage, read> cell_items: array<u32>;
@group(0) @binding(8) var<storage, read_write> kicks: array<vec2<i32>>;
@group(0) @binding(9) var<storage, read_write> pe: array<f32>;
@group(0) @binding(10) var<storage, read> ids: array<u32>;

@compute @workgroup_size(256)
fn cells(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let c = cell_coords(pos[i]);
    keys[i] = c.y * P.ncx + c.x;
}

// Gather: each thread sums the integer kicks on its particle from the 3×3 neighbouring cells.
// Each pair's kick is rounded to an integer before summing, so the order of neighbours does
// not affect the result.
@compute @workgroup_size(256)
fn force(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let pi = pos[i];
    let c = P.kick_c[species[i]];
    let cc = cell_coords(pi);
    var kx = 0i;
    var ky = 0i;
    var u_sum = 0.0;
    for (var oy = 0u; oy < 3u; oy++) {
        let cy = (cc.y + P.ncy + oy - 1u) % P.ncy;
        for (var ox = 0u; ox < 3u; ox++) {
            let cx = (cc.x + P.ncx + ox - 1u) % P.ncx;
            let cell = cy * P.ncx + cx;
            let start = cell_start[cell];
            let end = start + cell_count[cell];
            for (var k = start; k < end; k++) {
                let j = cell_items[k];
                if (j == i) {
                    continue;
                }
                let pj = pos[j];
                let fx = sep_f32(min_image64(sub64(pj.xy, pi.xy), P.box_x, P.half_x));
                let fy = sep_f32(min_image64(sub64(pj.zw, pi.zw), P.box_y, P.half_y));
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
        }
    }
    kicks[i] = vec2<i32>(kx, ky);
    pe[i] = 0.5 * u_sum;
}

// Leapfrog: v += kick; x += v (exact integer drift, periodic wrap).
@compute @workgroup_size(256)
fn integrate(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let v = vel[i] + kicks[i];
    vel[i] = v;
    let p = pos[i];
    pos[i] = vec4<u32>(
        wrap64(add64(p.xy, i64_from_i32(v.x)), P.box_x),
        wrap64(add64(p.zw, i64_from_i32(v.y)), P.box_y),
    );
}

// Time reversal: v ← −(v + kick(x)).
@compute @workgroup_size(256)
fn reverse(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    vel[i] = -(vel[i] + kicks[i]);
}

// XOR one bit into one word of the position of the particle whose id is P.flip_index.
@compute @workgroup_size(256)
fn flip(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i < P.n && ids[i] == P.flip_index) {
        var p = pos[i];
        p[P.flip_word] ^= P.flip_mask;
        pos[i] = p;
    }
}
