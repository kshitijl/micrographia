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
    /// A fixed-point separation of any length (for bonds), as the GPU converts it.
    fn sep_long(d: i64) -> Self;
    fn floor(self) -> Self;
    fn sqrt(self) -> Self;
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
    fn sep_long(d: i64) -> Self {
        fixed::sep_long_f32(d)
    }
    fn floor(self) -> Self {
        f32::floor(self)
    }
    fn sqrt(self) -> Self {
        f32::sqrt(self)
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
    fn sep_long(d: i64) -> Self {
        fixed::to_f64(d)
    }
    fn floor(self) -> Self {
        f64::floor(self)
    }
    fn sqrt(self) -> Self {
        f64::sqrt(self)
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

/// Bond constants in the engine's float type: (kind 0 harmonic / 1 FENE, k, r0).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BondConsts<F> {
    pub fene: bool,
    pub k: F,
    pub r0: F,
}

impl<F: Real> BondConsts<F> {
    pub fn new(b: &md_core::BondType) -> Self {
        match *b {
            md_core::BondType::Harmonic { k, r0 } => Self { fene: false, k: F::from_f64(k), r0: F::from_f64(r0) },
            md_core::BondType::Fene { k, r0 } => Self { fene: true, k: F::from_f64(k), r0: F::from_f64(r0) },
        }
    }

    /// (−U'(r)/r, U) at squared separation r2. Same operations as `bonds.wgsl`.
    #[inline(always)]
    pub fn eval(&self, r2: F) -> (F, F) {
        let one = F::from_f64(1.0);
        let half = F::from_f64(0.5);
        if self.fene {
            let mut x = r2 / (self.r0 * self.r0);
            let cap = F::from_f64(0.999999);
            if x > cap {
                x = cap;
            }
            let u = -half * self.k * self.r0 * self.r0 * F::from_f64((one - x).to_f64().ln());
            (-self.k / (one - x), u)
        } else if self.r0 == F::from_f64(0.0) {
            // A zero-length spring (tethers): −k exactly, with no 0/0 at r = 0.
            (-self.k, half * self.k * r2)
        } else {
            let r = r2.sqrt();
            let dr = r - self.r0;
            (-self.k * dr / r, half * self.k * dr * dr)
        }
    }
}
