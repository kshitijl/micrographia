// 64-bit two's-complement integers stored as vec2<u32>(lo, hi). Integer arithmetic wraps in
// WGSL, so these are exact and identical on every device.

fn i64_from_i32(v: i32) -> vec2<u32> {
    return vec2<u32>(bitcast<u32>(v), select(0u, 0xffffffffu, v < 0));
}

fn add64(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
    let lo = a.x + b.x;
    return vec2<u32>(lo, a.y + b.y + select(0u, 1u, lo < a.x));
}

fn neg64(a: vec2<u32>) -> vec2<u32> {
    let lo = ~a.x + 1u;
    return vec2<u32>(lo, ~a.y + select(0u, 1u, lo == 0u));
}

fn sub64(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
    return add64(a, neg64(b));
}

fn is_neg64(a: vec2<u32>) -> bool {
    return bitcast<i32>(a.y) < 0;
}

// Signed a < b.
fn lt64(a: vec2<u32>, b: vec2<u32>) -> bool {
    let ah = bitcast<i32>(a.y);
    let bh = bitcast<i32>(b.y);
    return ah < bh || (ah == bh && a.x < b.x);
}
