// Kernels used only by the self-tests of fixed64 and threefry.

struct Params { n: u32, m: u32, key0: u32, key1: u32 }

@group(0) @binding(0) var<storage, read> values: array<vec2<u32>>;
@group(0) @binding(1) var<storage, read_write> acc: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> out: array<vec2<u32>>;
@group(0) @binding(3) var<uniform> P: Params;

// Two-word 64-bit atomic accumulation: add values[i] into slot i % m, detecting the carry out
// of the low word. The final totals are independent of the order in which threads arrive.
@compute @workgroup_size(256)
fn check_atomic64(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let v = values[i];
    let k = i % P.m;
    let old = atomicAdd(&acc[2u * k], v.x);
    atomicAdd(&acc[2u * k + 1u], v.y + select(0u, 1u, old + v.x < old));
    out[i] = add64(sub64(v, i64_from_i32(-7)), neg64(v));
}

@compute @workgroup_size(256)
fn check_threefry(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    out[i] = threefry2x32(values[i], vec2<u32>(P.key0, P.key1));
}
