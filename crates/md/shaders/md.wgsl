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
@group(0) @binding(9) var<storage, read_write> aux: array<vec4<f32>>;
@group(0) @binding(10) var<storage, read> ids: array<u32>;
@group(0) @binding(11) var<storage, read_write> ctl: array<atomic<u32>>;
// Bond partners by particle id: (partner id, bond type) × 4, padded with 0xffffffff.
@group(0) @binding(13) var<storage, read> partners: array<vec4<u32>>;
@group(0) @binding(14) var<storage, read_write> slot_of: array<u32>;

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
    let wf = add_soft_walls(&acc, pi, c);
    let pf = add_piston(&acc, pi, c);
    kicks[i] = acc.k;
    aux[i] = vec4<f32>(acc.u, wf, pf);
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

// Zero the piston's force sum before a force evaluation.
@compute @workgroup_size(1)
fn piston_clear() {
    store64(CTL_PISTON_FORCE, vec2<u32>(0u, 0u));
}

@compute @workgroup_size(1)
fn piston_move() {
    piston_step();
}

@compute @workgroup_size(1)
fn piston_flip() {
    piston_reverse();
}

// Bonds, after the pair forces: add each bond's kick, rounded per bond like pairs (so
// Newton's third law stays exact). Same operations as md-ref's BondConsts::eval.
@compute @workgroup_size(256)
fn bond_force(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let id = ids[i];
    let pi = pos[i];
    let c = P.kick_c[species[i]];
    var k = kicks[i];
    var u = 0.0;
    for (var h = 0u; h < 2u; h++) {
        let e = partners[2u * id + h];
        for (var q = 0u; q < 2u; q++) {
            let other = select(e.z, e.x, q == 0u);
            let ty = select(e.w, e.y, q == 0u);
            if (other == 0xffffffffu) {
                continue;
            }
            let f = sep2(pi, pos[slot_of[other]]);
            let r2 = f.x * f.x + f.y * f.y;
            let bt = P.bond_types[ty];
            var fr = 0.0;
            if (bt.x > 0.5) {
                let x = min(r2 / (bt.z * bt.z), 0.999999);
                fr = -bt.y / (1.0 - x);
                u += 0.5 * (-0.5 * bt.y * bt.z * bt.z * log(1.0 - x));
            } else {
                let r = sqrt(r2);
                let dr = r - bt.z;
                fr = -bt.y * dr / r;
                u += 0.5 * (0.5 * bt.y * dr * dr);
            }
            let a = -fr * c;
            k += vec2<i32>(i32(round(a * f.x)), i32(round(a * f.y)));
        }
    }
    kicks[i] = k;
    aux[i].x += u;
}

// Set the velocity of every particle of species P.flip_index to (P.flip_word, P.flip_mask)
// reinterpreted as i32 displacement per step (a command, using the flip fields as arguments).
@compute @workgroup_size(256)
fn set_species_velocity(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i < P.n && species[i] == P.flip_index) {
        vel[i] = vec2<i32>(bitcast<i32>(P.flip_word), bitcast<i32>(P.flip_mask));
    }
}

// After a spatial reorder: where is each id stored?
@compute @workgroup_size(256)
fn invert_ids(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if (k < P.n) {
        slot_of[ids[k]] = k;
    }
}

// Time reversal: v ← −(v + kick(x) + g).
@compute @workgroup_size(256)
fn reverse(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let g = select(P.gravity, vec2<i32>(0, 0), is_pinned(species[i]));
    vel[i] = -(vel[i] + kicks[i] + g);
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
