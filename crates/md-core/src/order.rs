//! Telling crystal from liquid in 3D, molecule by molecule: Steinhardt's bond-orientational
//! order q6, as ten Wolde, Ruiz-Montero and Frenkel use it to find crystal nuclei (J. Chem. Phys.
//! 104, 9932, 1996). Each molecule's neighbours (within `r_cut`) define a 13-component vector
//! q6m; two neighbours are "solid-bonded" if their normalised vectors are aligned (dot > 0.7),
//! as they are in a crystal, where every molecule sees the same arrangement around it. In a
//! liquid the arrangements differ and few bonds pass. A molecule with 7 or more solid bonds is
//! crystalline.

use crate::fixed;

/// Solid bonds of each molecule. `pos` in position units, `box_len` and `periodic` per axis.
pub fn solid_bonds(pos: &[[i64; 3]], box_len: [i64; 3], periodic: [bool; 3], r_cut: f64) -> Vec<u8> {
    let n = pos.len();
    let b = box_len.map(fixed::to_f64);
    let p: Vec<[f64; 3]> = pos.iter().map(|q| q.map(fixed::to_f64)).collect();
    let cells = b.map(|l| ((l / r_cut).floor() as usize).max(1));
    let cell_of = |x: &[f64; 3]| -> [usize; 3] { std::array::from_fn(|a| ((x[a] / b[a] * cells[a] as f64) as usize).min(cells[a] - 1)) };
    let index = |c: [usize; 3]| (c[2] * cells[1] + c[1]) * cells[0] + c[0];
    let mut heads = vec![usize::MAX; cells[0] * cells[1] * cells[2]];
    let mut next = vec![usize::MAX; n];
    for i in 0..n {
        let k = index(cell_of(&p[i]));
        next[i] = heads[k];
        heads[k] = i;
    }
    let sep = |i: usize, j: usize| -> [f64; 3] {
        std::array::from_fn(|a| {
            let mut d = p[j][a] - p[i][a];
            if periodic[a] {
                d -= b[a] * (d / b[a]).round();
            }
            d
        })
    };
    // Neighbours of each molecule (scanning the 27 cells around it; with fewer than 3 cells on
    // an axis, the scan visits a cell more than once, so skip repeats).
    let rc2 = r_cut * r_cut;
    let mut nbrs: Vec<Vec<usize>> = vec![vec![]; n];
    for i in 0..n {
        let c = cell_of(&p[i]);
        let mut seen = [usize::MAX; 27];
        let mut s = 0;
        for dz in [-1i64, 0, 1] {
            for dy in [-1i64, 0, 1] {
                for dx in [-1i64, 0, 1] {
                    let d = [dx, dy, dz];
                    let cc: Option<[usize; 3]> = (0..3)
                        .map(|a| {
                            let v = c[a] as i64 + d[a];
                            let m = cells[a] as i64;
                            if periodic[a] {
                                Some(v.rem_euclid(m) as usize)
                            } else if (0..m).contains(&v) {
                                Some(v as usize)
                            } else {
                                None
                            }
                        })
                        .collect::<Option<Vec<usize>>>()
                        .map(|v| [v[0], v[1], v[2]]);
                    let Some(cc) = cc else { continue };
                    let k = index(cc);
                    if seen[..s].contains(&k) {
                        continue;
                    }
                    seen[s] = k;
                    s += 1;
                    let mut j = heads[k];
                    while j != usize::MAX {
                        if j != i {
                            let d = sep(i, j);
                            if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < rc2 {
                                nbrs[i].push(j);
                            }
                        }
                        j = next[j];
                    }
                }
            }
        }
    }
    // q6m for m = 0..6 (m < 0 follow by symmetry), normalised.
    let q: Vec<[(f64, f64); 7]> = (0..n)
        .map(|i| {
            let mut acc = [(0.0, 0.0); 7];
            for &j in &nbrs[i] {
                let y = y6(sep(i, j));
                for m in 0..7 {
                    acc[m].0 += y[m].0;
                    acc[m].1 += y[m].1;
                }
            }
            let norm = (acc[0].0 * acc[0].0 + acc[0].1 * acc[0].1 + 2.0 * (1..7).map(|m| acc[m].0 * acc[m].0 + acc[m].1 * acc[m].1).sum::<f64>()).sqrt();
            if norm > 0.0 {
                for a in acc.iter_mut() {
                    a.0 /= norm;
                    a.1 /= norm;
                }
            }
            acc
        })
        .collect();
    (0..n)
        .map(|i| {
            let mut count = 0u8;
            for &j in &nbrs[i] {
                let (a, b) = (&q[i], &q[j]);
                let d = a[0].0 * b[0].0 + a[0].1 * b[0].1 + 2.0 * (1..7).map(|m| a[m].0 * b[m].0 + a[m].1 * b[m].1).sum::<f64>();
                if d > 0.7 {
                    count = count.saturating_add(1);
                }
            }
            count
        })
        .collect()
}

/// Spherical harmonics Y_6^m (m = 0..6) of the direction of `d`, as (re, im).
fn y6(d: [f64; 3]) -> [(f64, f64); 7] {
    let r = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let x = d[2] / r;
    let phi = d[1].atan2(d[0]);
    let s = (1.0 - x * x).max(0.0).sqrt();
    let mut out = [(0.0, 0.0); 7];
    for m in 0..=6usize {
        // Associated Legendre P_6^m(x) by the standard upward recurrence.
        let mut pmm = 1.0;
        for k in 0..m {
            pmm *= -((2 * k + 1) as f64) * s;
        }
        let plm = if m == 6 {
            pmm
        } else {
            let mut p0 = pmm;
            let mut p1 = x * (2 * m + 1) as f64 * pmm;
            for l in m + 2..=6 {
                let p2 = ((2 * l - 1) as f64 * x * p1 - (l + m - 1) as f64 * p0) / (l - m) as f64;
                p0 = p1;
                p1 = p2;
            }
            p1
        };
        // Normalisation √((2l+1)/4π · (l−m)!/(l+m)!).
        let fact = |k: usize| (1..=k).map(|v| v as f64).product::<f64>();
        let norm = (13.0 / (4.0 * std::f64::consts::PI) * fact(6 - m) / fact(6 + m)).sqrt();
        let a = norm * plm;
        out[m] = (a * (m as f64 * phi).cos(), a * (m as f64 * phi).sin());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::three;

    #[test]
    fn crystal_is_solid_and_random_gas_is_not() {
        let n = 4 * 6 * 6 * 6;
        let b = three::cube_for(n, 1.0);
        let crystal = three::fcc(n, b, three::fcc_cells(n, b), 0.1, 1);
        let s = solid_bonds(&crystal, b, [true; 3], 1.4);
        let solid = s.iter().filter(|&&c| c >= 7).count();
        assert!(solid as f64 > 0.95 * n as f64, "{solid}/{n} crystalline in a slightly jittered crystal");
        // Random positions at the same density: essentially none.
        let random: Vec<[i64; 3]> = (0..n as u32).map(|i| std::array::from_fn(|a| (gpu_prims::threefry::uniform(9, 7, 3 * i + a as u32) * b[a] as f64) as i64)).collect();
        let s = solid_bonds(&random, b, [true; 3], 1.4);
        let solid = s.iter().filter(|&&c| c >= 7).count();
        assert!((solid as f64) < 0.02 * n as f64, "{solid}/{n} crystalline in a random arrangement");
    }
}
