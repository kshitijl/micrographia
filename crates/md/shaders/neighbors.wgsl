// Verlet neighbour lists with a skin. Requires fixed64.wgsl, threefry.wgsl, params.wgsl and
// boundary.wgsl.
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
@group(0) @binding(9) var<storage, read_write> ctl: array<atomic<u32>>;
@group(0) @binding(10) var<storage, read_write> kicks: array<vec2<i32>>;
@group(0) @binding(11) var<storage, read_write> pe: array<f32>;
@group(0) @binding(12) var<storage, read> ids: array<u32>;
@group(0) @binding(13) var<storage, read_write> wall_f: array<vec2<f32>>;

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
    var acc = Acc(vec2<i32>(0, 0), 0.0);
    let m = nbr_count[i];
    for (var k = 0u; k < m; k++) {
        let j = nbr[k * P.n + i];
        add_pair(&acc, pi, pos[j], si, species[j], c);
    }
    wall_f[i] = add_soft_walls(&acc, pi, c);
    kicks[i] = acc.k;
    pe[i] = acc.u;
}

// Leapfrog step and boundaries, then flag a rebuild if this particle has moved half the skin
// since the list was built (wall mirrors count as moves, so this stays conservative).
@compute @workgroup_size(256)
fn integrate_list(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let q = advance(i);
    let d = sep2(ref_pos[i], q);
    if (dot(d, d) >= P.half_skin2) {
        atomicMax(&ctl[CTL_REBUILD], 1u);
    }
}
