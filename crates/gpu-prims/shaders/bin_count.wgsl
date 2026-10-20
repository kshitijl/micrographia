// Counting sort, pass 1: histogram of keys.

struct Params { n: u32, _p0: u32, _p1: u32, _p2: u32 }

@group(0) @binding(0) var<storage, read> keys: array<u32>;
@group(0) @binding(1) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(2) var<uniform> P: Params;

@compute @workgroup_size(256)
fn bin_count(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x < P.n) {
        atomicAdd(&counts[keys[gid.x]], 1u);
    }
}
