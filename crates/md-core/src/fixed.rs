//! Fixed-point positions and per-step displacements.
//!
//! A position is an `i64` in units of 2⁻³² σ: the high word is the integer part in σ and the
//! low word the fraction. A velocity is stored as the `i32` displacement per timestep in the
//! same units, so the drift `x += v` is an exact integer add and the only rounding in a step
//! is the conversion of the force kick to an integer.

/// Fractional bits of a position.
pub const FRAC_BITS: u32 = 32;
/// One σ in position units.
pub const ONE: i64 = 1 << FRAC_BITS;
/// σ per position unit.
pub const UNIT: f64 = 1.0 / ONE as f64;

/// Pair separations are converted to float through units of 2⁻²⁸ σ (dropping 4 low bits),
/// which fits an `i32` for |d| < 8 σ. Cutoffs must be below 8 σ.
pub const SEP_SHIFT: u32 = 4;
pub const SEP_LIMIT: i64 = 8 * ONE;
pub const SEP_UNIT_F32: f32 = 1.0 / (1u64 << (FRAC_BITS - SEP_SHIFT)) as f32;

pub fn to_fixed(x: f64) -> i64 {
    (x * ONE as f64).round() as i64
}

pub fn to_f64(x: i64) -> f64 {
    x as f64 * UNIT
}

/// Minimum-image separation `b - a` in a periodic box of length `l`, with `a` and `b` in
/// `[0, l)`. Exact.
pub fn min_image(a: i64, b: i64, l: i64) -> i64 {
    let mut d = b - a;
    let half = l / 2;
    if d >= half {
        d -= l;
    } else if d < -half {
        d += l;
    }
    d
}

/// Wrap `x` into `[0, l)`, assuming it is at most one box length outside.
pub fn wrap(x: i64, l: i64) -> i64 {
    if x < 0 {
        x + l
    } else if x >= l {
        x - l
    } else {
        x
    }
}

/// Separation as f32 exactly as the GPU computes it, or `None` beyond 8 σ. Antisymmetric
/// bit for bit: `sep_f32(-d) == -sep_f32(d)`, which keeps Newton's third law exact.
pub fn sep_f32(d: i64) -> Option<f32> {
    if !(-SEP_LIMIT..SEP_LIMIT).contains(&d) {
        return None;
    }
    let mag = (d.unsigned_abs() >> SEP_SHIFT) as u32 as f32 * SEP_UNIT_F32;
    Some(if d < 0 { -mag } else { mag })
}

/// Like [`sep_f32`], but for any separation: within 8 σ the same; beyond, the magnitude's whole σ
/// plus its fraction in f32, then the sign (still antisymmetric). Exactly the GPU's `sep_long`.
pub fn sep_long_f32(d: i64) -> f32 {
    if let Some(s) = sep_f32(d) {
        return s;
    }
    let m = d.unsigned_abs();
    let mag = (m >> 32) as u32 as f32 + (m as u32) as f32 * (1.0 / 4294967296.0);
    if d < 0 { -mag } else { mag }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_image_and_wrap() {
        let l = 10 * ONE + 12345;
        for &(a, b) in &[(0, l - 1), (l - 1, 0), (3, 3), (0, l / 2), (l / 2, 0), (1, l / 2 + 2)] {
            let d = min_image(a, b, l);
            assert!(d >= -(l - l / 2) && d < l / 2, "d={d}");
            assert_eq!(wrap(a + d, l), b);
        }
        assert_eq!(wrap(-1, l), l - 1);
        assert_eq!(wrap(l, l), 0);
    }

    #[test]
    fn separation_is_antisymmetric() {
        for &d in &[1i64, 15, 16, 17, ONE, ONE + 7, 3 * ONE / 2, SEP_LIMIT - 1, 123_456_789] {
            assert_eq!(sep_f32(-d).unwrap(), -sep_f32(d).unwrap());
        }
        assert!(sep_f32(SEP_LIMIT).is_none());
        assert!(sep_f32(-SEP_LIMIT).is_some());
        assert!((sep_f32(ONE + ONE / 4).unwrap() - 1.25).abs() < 1e-7);
    }
}
