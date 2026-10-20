//! All-integer pair forces (the `gpu-int-int` / `cpu-int-int` configurations).
//!
//! Every step of the force calculation is integer arithmetic, so results are bit-identical on
//! every device, browser and CPU:
//!
//! 1. Separations in units of 2⁻²⁰ σ (`d >> 12` of the 2⁻³² σ positions), |d| < 8 σ.
//! 2. r² exactly in 64 bits, then `>> 16` into units of 2⁻²⁴ σ².
//! 3. A table in r², per ordered species pair (a, b), of `T(r²) = −U'(r)/r · c_a`, the force per
//!    unit separation times species a's kick factor, in units of 2⁻² position units per step per
//!    σ, linearly interpolated in integers.
//! 4. The kick magnitude `|T| · |d| >> 22`, with the sign applied afterwards, so the kick on b
//!    from a is exactly minus the kick on a from b (for equal masses).
//!
//! The table is built once on the CPU in f64 and rounded, so every device uses the same
//! integers. Quantization errors are about one position unit per kick.

use crate::system::System;

/// Separations are in units of 2⁻²⁰ σ.
pub const SEP_BITS: u32 = 20;
/// r² is in units of 2⁻²⁴ σ² (the exact 2⁻⁴⁰ σ² square shifted right by 16).
pub const R2_BITS: u32 = 24;
/// Table values are in units of 2⁻² (position units per step) per σ of separation.
pub const T_BITS: u32 = 2;
/// The kick is `T · d >> KICK_SHIFT` (2 + 20 bits of scale).
pub const KICK_SHIFT: u32 = T_BITS + SEP_BITS;
/// log2 of the r² spacing between table entries (in r² units): 2^12 / 2^24 σ² ≈ 2.4×10⁻⁴ σ²,
/// which keeps linear interpolation within ~10⁻⁶ of the exact force even on the steep core.
pub const STEP_BITS: u32 = 12;

/// Integer force tables for every ordered species pair.
#[derive(Clone, Debug, PartialEq)]
pub struct IntTables {
    /// Number of species (tables are indexed `a * ns + b`).
    pub ns: usize,
    /// Entries per table (the same for all pairs; padded past each cutoff with zeros).
    pub len: usize,
    /// All tables, concatenated.
    pub data: Vec<i32>,
    /// Squared cutoff of each pair in r² units.
    pub rc2: Vec<u32>,
}

impl IntTables {
    pub fn new(sys: &System) -> Self {
        let ns = sys.species();
        let rc2_max = sys.rc_max() * sys.rc_max();
        let len = ((rc2_max * (1u64 << R2_BITS) as f64) as u64 >> STEP_BITS) as usize + 2;
        let mut data = vec![0i32; ns * ns * len];
        let mut rc2 = vec![0u32; ns * ns];
        for a in 0..ns {
            let c = sys.kick_coeff(a as u32);
            for b in 0..ns {
                let p = sys.pair(a as u32, b as u32);
                rc2[a * ns + b] = (p.rc * p.rc * (1u64 << R2_BITS) as f64) as u32;
                for k in 0..len {
                    let r2 = (k as u64 * (1u64 << STEP_BITS)) as f64 / (1u64 << R2_BITS) as f64;
                    // Below r² = 0.36 (r = 0.6 σ) the force is clamped: molecules never get that
                    // close at sensible temperatures, and the table must fit in i32.
                    let r2 = r2.max(0.36);
                    // Continue the force smoothly past the cutoff (by up to one entry), so that
                    // interpolation just inside the cutoff does not mix in the jump to zero; the
                    // cutoff itself is applied separately.
                    let continued = crate::Pair { rc: f64::INFINITY, ..*p };
                    let t = if r2 < p.rc * p.rc + 2.0 * (1u64 << STEP_BITS) as f64 / (1u64 << R2_BITS) as f64 { continued.f_over_r(r2) * c * (1u64 << T_BITS) as f64 } else { 0.0 };
                    data[(a * ns + b) * len + k] = t.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
                }
            }
        }
        Self { ns, len, data, rc2 }
    }

    /// The integer kick on particle a (species `sa`) from b (species `sb`) at fixed-point
    /// separation d = r_b − r_a (2⁻³² σ units), or `None` if beyond the cutoff or 8 σ.
    #[inline]
    pub fn kick(&self, sa: u32, sb: u32, d: [i64; 2]) -> Option<[i32; 2]> {
        let limit = 8i64 << 32;
        if d.iter().any(|&x| !(-limit..limit).contains(&x)) {
            return None;
        }
        // Separations in 2⁻²⁰ σ, rounded to nearest on magnitudes (so the result is antisymmetric
        // and unbiased; truncation made the steep core ~10⁻⁵ too strong).
        let q = d.map(|x| {
            let m = ((x.unsigned_abs() + (1 << (31 - SEP_BITS))) >> (32 - SEP_BITS)) as i64;
            if x < 0 { -m } else { m }
        });
        let r2 = (((q[0] * q[0] + q[1] * q[1]) as u64 + (1 << (2 * SEP_BITS - R2_BITS - 1))) >> (2 * SEP_BITS - R2_BITS)) as u32;
        let pair = sa as usize * self.ns + sb as usize;
        if r2 >= self.rc2[pair] {
            return None;
        }
        let idx = (r2 >> STEP_BITS) as usize;
        let frac = (r2 & ((1 << STEP_BITS) - 1)) as i64;
        let base = pair * self.len;
        let (t0, t1) = (self.data[base + idx] as i64, self.data[base + idx + 1] as i64);
        let t = t0 + (((t1 - t0) * frac) >> STEP_BITS);
        // kick = −T · q >> KICK_SHIFT, on magnitudes, then signed.
        let k = |qa: i64| {
            let m = ((t.unsigned_abs() as u128 * qa.unsigned_abs() as u128) >> KICK_SHIFT) as i64;
            let neg = (t < 0) != (qa < 0);
            // The force on a points away from b for positive T (repulsion): −q direction.
            (if neg { m } else { -m }) as i32
        };
        Some([k(q[0]), k(q[1])])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init, Pair};

    #[test]
    fn integer_kicks_are_antisymmetric_and_accurate() {
        let sys = System::new(init::box_for(100, 0.5, 1.0), init::DT, Pair::lj(2.5), vec![1.0]);
        let t = IntTables::new(&sys);
        let mut worst = 0.0f64;
        for k in 0..2000 {
            let r = 0.8 + 1.69 * k as f64 / 2000.0;
            let a = 0.3 + k as f64 * 0.001;
            let d = [crate::fixed::to_fixed(r * a.cos()), crate::fixed::to_fixed(r * a.sin())];
            let ki = t.kick(0, 0, d).unwrap();
            let kj = t.kick(0, 0, [-d[0], -d[1]]).unwrap();
            assert_eq!(ki, [-kj[0], -kj[1]], "antisymmetry at r = {r}");
            // Compare with the exact kick in f64.
            let f = sys.pair(0, 0).f_over_r(r * r) * sys.kick_coeff(0);
            let exact = [-f * r * a.cos(), -f * r * a.sin()];
            for ax in 0..2 {
                // Error in units of (20 position units + 10⁻⁵ of the kick): a few units where the
                // force crosses zero, ~10⁻⁶ relative on the steep core.
                let err = (ki[ax] as f64 - exact[ax]).abs();
                worst = worst.max(err / (20.0 + 1e-5 * exact[ax].abs()));
            }
        }
        assert!(worst < 1.0, "kick error {worst:.2} × (20 + 10⁻⁵ |kick|)");
    }
}
