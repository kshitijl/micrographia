// Exclusive prefix sum of u32 over blocks of 1024 elements, plus an optional per-block offset
// (the scanned block totals from the reduce pass). Reduce-then-scan needs no forward-progress
// guarantee between workgroups.

struct Params { n: u32, use_offsets: u32, _p0: u32, _p1: u32 }

@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<storage, read_write> dst: array<u32>;
@group(0) @binding(2) var<storage, read> offsets: array<u32>;
@group(0) @binding(3) var<uniform> P: Params;

const WG: u32 = 256u;
const ITEMS: u32 = 4u;
var<workgroup> sh: array<u32, 1024>;
var<workgroup> ts: array<u32, 256>;

@compute @workgroup_size(256)
fn scan_blocks(@builtin(local_invocation_id) lid3: vec3<u32>, @builtin(workgroup_id) wid3: vec3<u32>) {
    let lid = lid3.x;
    let base = wid3.x * WG * ITEMS;
    for (var k = 0u; k < ITEMS; k++) {
        let i = base + k * WG + lid;
        var v = 0u;
        if (i < P.n) {
            v = src[i];
        }
        sh[k * WG + lid] = v;
    }
    workgroupBarrier();
    var t = 0u;
    for (var k = 0u; k < ITEMS; k++) {
        t += sh[lid * ITEMS + k];
    }
    ts[lid] = t;
    workgroupBarrier();
    for (var d = 1u; d < WG; d <<= 1u) {
        var v = ts[lid];
        if (lid >= d) {
            v += ts[lid - d];
        }
        workgroupBarrier();
        ts[lid] = v;
        workgroupBarrier();
    }
    var run = ts[lid] - t;
    if (P.use_offsets != 0u) {
        run += offsets[wid3.x];
    }
    for (var k = 0u; k < ITEMS; k++) {
        let x = sh[lid * ITEMS + k];
        sh[lid * ITEMS + k] = run;
        run += x;
    }
    workgroupBarrier();
    for (var k = 0u; k < ITEMS; k++) {
        let i = base + k * WG + lid;
        if (i < P.n) {
            dst[i] = sh[k * WG + lid];
        }
    }
}
