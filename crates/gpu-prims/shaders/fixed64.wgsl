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

// Full 64-bit product of two u32 (16-bit limbs; WGSL has no 64-bit integers).
fn mul_wide_u32(a: u32, b: u32) -> vec2<u32> {
    let a0 = a & 0xffffu;
    let a1 = a >> 16u;
    let b0 = b & 0xffffu;
    let b1 = b >> 16u;
    let p00 = a0 * b0;
    let p01 = a0 * b1;
    let p10 = a1 * b0;
    let p11 = a1 * b1;
    let mid = p01 + p10;
    let mid_carry = select(0u, 1u, mid < p01);
    let lo = p00 + (mid << 16u);
    let lo_carry = select(0u, 1u, lo < p00);
    return vec2<u32>(lo, p11 + (mid >> 16u) + (mid_carry << 16u) + lo_carry);
}

// Logical shift right of a 64-bit value by s (0 < s < 32).
fn shr64(x: vec2<u32>, s: u32) -> vec2<u32> {
    return vec2<u32>((x.x >> s) | (x.y << (32u - s)), x.y >> s);
}
