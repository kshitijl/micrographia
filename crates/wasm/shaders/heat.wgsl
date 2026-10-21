// A running average of each molecule's squared speed (in the engine's integer velocity units),
// stored by particle id so it survives the engine's reordering of storage. The renderer can
// colour by it: a molecule's recent energy rather than its instantaneous, noisy one.

struct HParams {
    n: u32,
    // Weight of the newest reading (1 resets the average).
    alpha: f32,
    _p0: u32,
    _p1: u32,
}

@group(0) @binding(0) var<uniform> H: HParams;
@group(0) @binding(3) var<storage, read> ids: array<u32>;
@group(0) @binding(5) var<storage, read> vel: array<vec2<i32>>;
@group(0) @binding(6) var<storage, read_write> heat: array<f32>;

@compute @workgroup_size(256)
fn smooth_heat(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= H.n) {
        return;
    }
    let v = vec2<f32>(vel[i]);
    let id = ids[i];
    heat[id] = mix(heat[id], dot(v, v), H.alpha);
}
