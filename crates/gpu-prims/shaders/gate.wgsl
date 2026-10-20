// Conditional execution without a host round trip: write indirect dispatch arguments that
// are the base workgroup counts if flags[0] is set and zero otherwise, then clear the flag.

struct Params { count: u32, _p0: u32, _p1: u32, _p2: u32 }

@group(0) @binding(0) var<storage, read_write> flags: array<u32>;
@group(0) @binding(1) var<storage, read> base: array<u32>;
@group(0) @binding(2) var<storage, read_write> args: array<u32>;
// stats[0] counts how many times the gate opened.
@group(0) @binding(3) var<storage, read_write> stats: array<u32>;
@group(0) @binding(4) var<uniform> P: Params;

@compute @workgroup_size(1)
fn gate() {
    let open = flags[0] != 0u;
    for (var k = 0u; k < P.count; k++) {
        args[3u * k] = select(0u, base[k], open);
        args[3u * k + 1u] = 1u;
        args[3u * k + 2u] = 1u;
    }
    if (open) {
        stats[0] += 1u;
    }
    flags[0] = 0u;
}
