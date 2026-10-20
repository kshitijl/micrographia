// gpu-int-f32 with a cell list: integer positions and velocities, f32 pair forces converted
// to integer kicks per pair. Requires fixed64.wgsl, threefry.wgsl, params.wgsl and
// boundary.wgsl. CPU twin: md-ref IntEngine<f32>.
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
@group(0) @binding(11) var<storage, read_write> ctl: array<atomic<u32>>;
@group(0) @binding(12) var<storage, read_write> wall_f: array<vec2<f32>>;

@compute @workgroup_size(256)
fn cells(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let c = cell_coords(pos[i]);
    keys[i] = c.y * P.ncx + c.x;
}

// Gather: each thread sums the integer kicks on its particle from the 3×3 neighbouring cells
// (and soft walls). Each pair's kick is rounded to an integer before summing, so the order of
// neighbours does not affect the result.
@compute @workgroup_size(256)
fn force(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let pi = pos[i];
    let si = species[i];
    let c = P.kick_c[si];
    let cc = cell_coords(pi);
    var acc = Acc(vec2<i32>(0, 0), 0.0);
    for (var oy = 0u; oy < 3u; oy++) {
        let cy = (cc.y + P.ncy + oy - 1u) % P.ncy;
        for (var ox = 0u; ox < 3u; ox++) {
            let cx = (cc.x + P.ncx + ox - 1u) % P.ncx;
            let cell = cy * P.ncx + cx;
            let start = cell_start[cell];
            let end = start + cell_count[cell];
            for (var k = start; k < end; k++) {
                let j = cell_items[k];
                if (j != i) {
                    add_pair(&acc, pi, pos[j], si, species[j], c);
                }
            }
        }
    }
    wall_f[i] = add_soft_walls(&acc, pi, c);
    kicks[i] = acc.k;
    pe[i] = acc.u;
}

@compute @workgroup_size(256)
fn integrate(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i < P.n) {
        advance(i);
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
    vel[i] = -(vel[i] + kicks[i] + P.gravity);
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
