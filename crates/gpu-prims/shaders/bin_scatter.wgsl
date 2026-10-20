// Counting sort, pass 2: scatter item indices to their key's segment. `cursor` must start at
// zero and ends equal to the per-key counts. Order within a key is unspecified.

struct Params { n: u32, _p0: u32, _p1: u32, _p2: u32 }

@group(0) @binding(0) var<storage, read> keys: array<u32>;
@group(0) @binding(1) var<storage, read> starts: array<u32>;
@group(0) @binding(2) var<storage, read_write> cursor: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> sorted: array<u32>;
@group(0) @binding(4) var<uniform> P: Params;

@compute @workgroup_size(256)
fn bin_scatter(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x < P.n) {
        let k = keys[gid.x];
        sorted[starts[k] + atomicAdd(&cursor[k], 1u)] = gid.x;
    }
}
