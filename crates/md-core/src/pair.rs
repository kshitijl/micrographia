//! Truncated and shifted Lennard-Jones pair potential (WCA when the cutoff is 2^(1/6) σ).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pair {
    pub eps: f64,
    pub sigma: f64,
    pub rc: f64,
}

impl Pair {
    /// Purely repulsive Weeks–Chandler–Andersen: LJ cut at its minimum and shifted up by ε.
    pub fn wca() -> Self {
        Self { eps: 1.0, sigma: 1.0, rc: 2f64.powf(1.0 / 6.0) }
    }

    pub fn lj(rc: f64) -> Self {
        Self { eps: 1.0, sigma: 1.0, rc }
    }

    /// No interaction at all: a zero cutoff, so the pair never enters the force (an ε = 0 pair
    /// with a cutoff would still evaluate 0 × ∞ = NaN for coincident particles, e.g. a tethered
    /// particle on its anchor).
    pub fn none() -> Self {
        Self { eps: 0.0, sigma: 1.0, rc: 0.0 }
    }

    /// Unshifted LJ energy at the cutoff; subtracted so the energy is continuous there.
    pub fn shift(&self) -> f64 {
        if self.eps == 0.0 || self.rc == 0.0 {
            return 0.0;
        }
        let s6 = (self.sigma / self.rc).powi(6);
        4.0 * self.eps * s6 * (s6 - 1.0)
    }

    pub fn energy(&self, r2: f64) -> f64 {
        if r2 >= self.rc * self.rc {
            return 0.0;
        }
        let s6 = (self.sigma * self.sigma / r2).powi(3);
        4.0 * self.eps * s6 * (s6 - 1.0) - self.shift()
    }

    /// −U'(r)/r, so the force on i from j is `f_over_r * (r_i − r_j)`.
    pub fn f_over_r(&self, r2: f64) -> f64 {
        if r2 >= self.rc * self.rc {
            return 0.0;
        }
        let s6 = (self.sigma * self.sigma / r2).powi(3);
        24.0 * self.eps * s6 * (2.0 * s6 - 1.0) / r2
    }

    /// The f32 constants the GPU kernel uses: (rc², 4ε, σ², shift).
    pub fn gpu_constants(&self) -> (f32, f32, f32, f32) {
        ((self.rc * self.rc) as f32, (4.0 * self.eps) as f32, (self.sigma * self.sigma) as f32, self.shift() as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wca_is_continuous_and_repulsive() {
        let p = Pair::wca();
        let rc2 = p.rc * p.rc;
        assert!(p.energy(rc2 * (1.0 - 1e-9)).abs() < 1e-6);
        assert!(p.f_over_r(rc2 * (1.0 - 1e-9)).abs() < 1e-6);
        assert!((p.energy(1.0) - 1.0).abs() < 1e-12);
        for i in 1..100 {
            assert!(p.f_over_r(0.8 + 0.004 * i as f64) >= 0.0);
        }
    }

    #[test]
    fn force_is_minus_gradient() {
        let p = Pair::lj(2.5);
        for &r in &[0.95f64, 1.1, 1.5, 2.2] {
            let h = 1e-6;
            let du = (p.energy((r + h) * (r + h)) - p.energy((r - h) * (r - h))) / (2.0 * h);
            assert!((p.f_over_r(r * r) * r + du).abs() < 1e-6, "r={r}");
        }
    }
}
