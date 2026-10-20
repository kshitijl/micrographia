//! CPU twin of `shaders/threefry.wgsl`.

const ROT: [u32; 8] = [13, 15, 26, 6, 17, 29, 16, 24];
const PARITY: u32 = 0x1BD1_1BDA;

/// Threefry-2x32 with `rounds` rounds (the GPU version always uses 20).
pub fn threefry2x32_rounds(ctr: [u32; 2], key: [u32; 2], rounds: u32) -> [u32; 2] {
    let ks = [key[0], key[1], PARITY ^ key[0] ^ key[1]];
    let mut x0 = ctr[0].wrapping_add(ks[0]);
    let mut x1 = ctr[1].wrapping_add(ks[1]);
    for r in 0..rounds {
        x0 = x0.wrapping_add(x1);
        x1 = x1.rotate_left(ROT[(r % 8) as usize]);
        x1 ^= x0;
        if r & 3 == 3 {
            let s = (r >> 2) + 1;
            x0 = x0.wrapping_add(ks[(s % 3) as usize]);
            x1 = x1.wrapping_add(ks[((s + 1) % 3) as usize]).wrapping_add(s);
        }
    }
    [x0, x1]
}

pub fn threefry2x32(ctr: [u32; 2], key: [u32; 2]) -> [u32; 2] {
    threefry2x32_rounds(ctr, key, 20)
}

/// A uniform double in [0, 1) from 53 random bits.
pub fn to_unit_f64(r: [u32; 2]) -> f64 {
    let bits = ((r[0] as u64) << 21) ^ (r[1] as u64 >> 11);
    (bits & ((1u64 << 53) - 1)) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// A counter-based stream: draw `i` of stream `stream` under `seed`.
pub fn uniform(seed: u64, stream: u32, i: u32) -> f64 {
    to_unit_f64(threefry2x32([i, stream], [seed as u32, (seed >> 32) as u32]))
}

/// A standard normal deviate (Box–Muller on two counter-based uniforms).
pub fn normal(seed: u64, stream: u32, i: u32) -> f64 {
    let u1 = 1.0 - uniform(seed, stream, 2 * i);
    let u2 = uniform(seed, stream, 2 * i + 1);
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known-answer vectors from Random123 `tests/kat_vectors` (counter, key, expected).
    const KAT: &[(u32, [u32; 2], [u32; 2], [u32; 2])] = &[
        (13, [0x00000000, 0x00000000], [0x00000000, 0x00000000], [0x9d1c5ec6, 0x8bd50731]),
        (13, [0xffffffff, 0xffffffff], [0xffffffff, 0xffffffff], [0xfd36d048, 0x2d17272c]),
        (13, [0x243f6a88, 0x85a308d3], [0x13198a2e, 0x03707344], [0xba3e4725, 0xf27d669e]),
        (20, [0x00000000, 0x00000000], [0x00000000, 0x00000000], [0x6b200159, 0x99ba4efe]),
        (20, [0xffffffff, 0xffffffff], [0xffffffff, 0xffffffff], [0x1cb996fc, 0xbb002be7]),
        (20, [0x243f6a88, 0x85a308d3], [0x13198a2e, 0x03707344], [0xc4923a9c, 0x483df7a0]),
        (32, [0x00000000, 0x00000000], [0x00000000, 0x00000000], [0xcee3d47e, 0xa23dfd5c]),
        (32, [0xffffffff, 0xffffffff], [0xffffffff, 0xffffffff], [0x6e2fe0d0, 0xb1b76f82]),
        (32, [0x243f6a88, 0x85a308d3], [0x13198a2e, 0x03707344], [0xe2827716, 0xc3c05cdf]),
    ];

    #[test]
    fn random123_known_answers() {
        for &(rounds, ctr, key, want) in KAT {
            assert_eq!(threefry2x32_rounds(ctr, key, rounds), want, "rounds={rounds} ctr={ctr:x?}");
        }
    }

    #[test]
    fn normal_moments() {
        let n = 200_000;
        let xs: Vec<f64> = (0..n).map(|i| normal(42, 7, i)).collect();
        let mean = xs.iter().sum::<f64>() / n as f64;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        let kurt = xs.iter().map(|x| (x - mean).powi(4)).sum::<f64>() / n as f64 / var.powi(2);
        assert!(mean.abs() < 0.01, "mean {mean}");
        assert!((var - 1.0).abs() < 0.01, "var {var}");
        assert!((kurt - 3.0).abs() < 0.05, "kurtosis {kurt}");
    }
}
