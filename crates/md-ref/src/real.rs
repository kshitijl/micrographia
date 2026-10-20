use md_core::fixed;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

/// The float type of a CPU engine: f32 or f64.
pub trait Real:
    Copy
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + std::fmt::Debug
    + 'static
{
    const NAME: &'static str;
    fn from_f64(x: f64) -> Self;
    fn to_f64(self) -> f64;
    /// Round half to even, like WGSL `round`.
    fn round_even(self) -> Self;
    /// Saturating conversion, like WGSL `i32(f)`.
    fn to_i32(self) -> i32;
    /// A fixed-point pair separation as a float in σ, or `None` if it is beyond 8 σ.
    fn sep(d: i64) -> Option<Self>;
    fn floor(self) -> Self;
}

impl Real for f32 {
    const NAME: &'static str = "f32";
    fn from_f64(x: f64) -> Self {
        x as f32
    }
    fn to_f64(self) -> f64 {
        self as f64
    }
    fn round_even(self) -> Self {
        self.round_ties_even()
    }
    fn to_i32(self) -> i32 {
        self as i32
    }
    /// Exactly the GPU's conversion.
    fn sep(d: i64) -> Option<Self> {
        fixed::sep_f32(d)
    }
    fn floor(self) -> Self {
        f32::floor(self)
    }
}

impl Real for f64 {
    const NAME: &'static str = "f64";
    fn from_f64(x: f64) -> Self {
        x
    }
    fn to_f64(self) -> f64 {
        self
    }
    fn round_even(self) -> Self {
        self.round_ties_even()
    }
    fn to_i32(self) -> i32 {
        self as i32
    }
    /// Exact (every separation below 8 σ is representable in f64).
    fn sep(d: i64) -> Option<Self> {
        if (-fixed::SEP_LIMIT..fixed::SEP_LIMIT).contains(&d) { Some(fixed::to_f64(d)) } else { None }
    }
    fn floor(self) -> Self {
        f64::floor(self)
    }
}

/// Pair constants in the engine's float type.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PairConsts<F> {
    pub rc2: F,
    pub eps4: F,
    pub sigma2: F,
    pub shift: F,
}

impl<F: Real> PairConsts<F> {
    pub fn new(p: &md_core::Pair) -> Self {
        if F::NAME == "f32" {
            // Round the f64 constants once, exactly as the GPU receives them.
            let (rc2, eps4, sigma2, shift) = p.gpu_constants();
            Self { rc2: F::from_f64(rc2 as f64), eps4: F::from_f64(eps4 as f64), sigma2: F::from_f64(sigma2 as f64), shift: F::from_f64(shift as f64) }
        } else {
            Self { rc2: F::from_f64(p.rc * p.rc), eps4: F::from_f64(4.0 * p.eps), sigma2: F::from_f64(p.sigma * p.sigma), shift: F::from_f64(p.shift()) }
        }
    }

    /// (−U'(r)/r, U(r)) at separation (fx, fy), or `None` beyond the cutoff. Same operations
    /// in the same order as `force.wgsl`.
    #[inline(always)]
    pub fn eval(&self, fx: F, fy: F) -> Option<(F, F)> {
        let r2 = fx * fx + fy * fy;
        if !(r2 < self.rc2) {
            return None;
        }
        let one = F::from_f64(1.0);
        let s2 = self.sigma2 / r2;
        let s6 = s2 * s2 * s2;
        let fr = F::from_f64(6.0) * self.eps4 * s6 * (F::from_f64(2.0) * s6 - one) / r2;
        let u = self.eps4 * s6 * (s6 - one) - self.shift;
        Some((fr, u))
    }
}
