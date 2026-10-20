//! CPU twins of the two-word 64-bit integers in `shaders/fixed64.wgsl`.

/// Split an i64 into the `(lo, hi)` words used on the GPU.
pub fn split(v: i64) -> [u32; 2] {
    [v as u32, (v >> 32) as u32]
}

pub fn join(w: [u32; 2]) -> i64 {
    ((w[1] as u64) << 32 | w[0] as u64) as i64
}

/// Two-word accumulation exactly as the GPU does it: add the low words, detect the carry, and
/// add the high words plus carry.
pub fn add_words(acc: [u32; 2], v: [u32; 2]) -> [u32; 2] {
    let lo = acc[0].wrapping_add(v[0]);
    let carry = (lo < acc[0]) as u32;
    [lo, acc[1].wrapping_add(v[1]).wrapping_add(carry)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_join_and_carry() {
        let vals = [0i64, 1, -1, i64::MAX, i64::MIN, 0xffff_ffff, -0x1_0000_0000, 123456789012345];
        for &a in &vals {
            assert_eq!(join(split(a)), a);
            for &b in &vals {
                assert_eq!(join(add_words(split(a), split(b))), a.wrapping_add(b));
            }
        }
    }
}
