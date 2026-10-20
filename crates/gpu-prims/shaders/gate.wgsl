// Conditional execution without a host round trip: write indirect dispatch arguments that
// are the base workgroup counts if ctl[flag] is set and zero otherwise, then clear the flag
// and count the opening in ctl[counter].

struct Params { count: u32, flag: u32, counter: u32, _p0: u32 }

@group(0) @binding(0) var<storage, read_write> ctl: array<u32>;
@group(0) @binding(1) var<storage, read> base: array<u32>;
@group(0) @binding(2) var<storage, read_write> args: array<u32>;
@group(0) @binding(3) var<uniform> P: Params;

@compute @workgroup_size(1)
fn gate() {
    let open = ctl[P.flag] != 0u;
    for (var k = 0u; k < P.count; k++) {
        args[3u * k] = select(0u, base[k], open);
        args[3u * k + 1u] = 1u;
        args[3u * k + 2u] = 1u;
    }
    if (open) {
        ctl[P.counter] += 1u;
    }
    ctl[P.flag] = 0u;
}
