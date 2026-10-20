// Stream compaction: write the indices i with flags[i] != 0, in order, and their count.

struct Params { n: u32, _p0: u32, _p1: u32, _p2: u32 }

@group(0) @binding(0) var<storage, read> flags: array<u32>;
@group(0) @binding(1) var<storage, read> scanned: array<u32>;
@group(0) @binding(2) var<storage, read_write> out: array<u32>;
@group(0) @binding(3) var<storage, read_write> count: array<u32>;
@group(0) @binding(4) var<uniform> P: Params;

@compute @workgroup_size(256)
fn compact(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let f = select(0u, 1u, flags[i] != 0u);
    if (f != 0u) {
        out[scanned[i]] = i;
    }
    if (i == P.n - 1u) {
        count[0] = scanned[i] + f;
    }
}
