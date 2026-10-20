// Instruments read on the GPU: a coarse density histogram (all particles and one marked
// species) and per-particle kinetic energy. Requires fixed64.wgsl.

struct ObsParams {
    n: u32,
    gx: u32,
    gy: u32,
    marked: u32,
    // gx / Lx and gy / Ly.
    scale: vec2<f32>,
    // Δt-to-velocity factor 2^-32 / Δt, so v = f32(v_int) * vel_scale.
    vel_scale: f32,
    _pad: f32,
    mass: vec4<f32>,
}

@group(0) @binding(0) var<uniform> P: ObsParams;
@group(0) @binding(1) var<storage, read> pos: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read> species: array<u32>;
@group(0) @binding(3) var<storage, read_write> hist: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read> vel: array<vec2<i32>>;
@group(0) @binding(5) var<storage, read_write> ke: array<f32>;

fn coord(x: vec2<u32>) -> f32 {
    return f32(x.y) + ldexp(f32(x.x), -32);
}

// hist[0 .. gx*gy) counts all particles per bin; hist[gx*gy ..) counts the marked species.
@compute @workgroup_size(256)
fn histogram(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let p = pos[i];
    let bx = min(u32(coord(p.xy) * P.scale.x), P.gx - 1u);
    let by = min(u32(coord(p.zw) * P.scale.y), P.gy - 1u);
    let b = by * P.gx + bx;
    atomicAdd(&hist[b], 1u);
    if (species[i] == P.marked) {
        atomicAdd(&hist[P.gx * P.gy + b], 1u);
    }
}

// Kinetic energy from the half-step velocity (O(Δt²) from the on-step value; for display).
@compute @workgroup_size(256)
fn kinetic(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let v = vec2<f32>(vel[i]) * P.vel_scale;
    ke[i] = 0.5 * P.mass[species[i]] * dot(v, v);
}
