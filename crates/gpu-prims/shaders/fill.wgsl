// Set `n` u32 elements to a value.

struct Params { n: u32, value: u32, _p0: u32, _p1: u32 }

@group(0) @binding(0) var<storage, read_write> buf: array<u32>;
@group(0) @binding(1) var<uniform> P: Params;

@compute @workgroup_size(256)
fn fill(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x < P.n) {
        buf[gid.x] = P.value;
    }
}
