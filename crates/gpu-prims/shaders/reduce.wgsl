// Sum of `n` elements of type {{T}}: each workgroup reduces 1024 elements to one partial.
// The tree shape is fixed, so float sums are deterministic on a given device.

struct Params { n: u32, _p0: u32, _p1: u32, _p2: u32 }

@group(0) @binding(0) var<storage, read> src: array<{{T}}>;
@group(0) @binding(1) var<storage, read_write> dst: array<{{T}}>;
@group(0) @binding(2) var<uniform> P: Params;

const WG: u32 = 256u;
const ITEMS: u32 = 4u;
var<workgroup> sh: array<{{T}}, 256>;

@compute @workgroup_size(256)
fn reduce(@builtin(local_invocation_id) lid3: vec3<u32>, @builtin(workgroup_id) wid3: vec3<u32>) {
    let lid = lid3.x;
    let base = wid3.x * WG * ITEMS;
    var acc = {{T}}(0);
    for (var k = 0u; k < ITEMS; k++) {
        let i = base + k * WG + lid;
        if (i < P.n) {
            acc += src[i];
        }
    }
    sh[lid] = acc;
    workgroupBarrier();
    for (var s = WG / 2u; s > 0u; s >>= 1u) {
        if (lid < s) {
            sh[lid] += sh[lid + s];
        }
        workgroupBarrier();
    }
    if (lid == 0u) {
        dst[wid3.x] = sh[0];
    }
}
