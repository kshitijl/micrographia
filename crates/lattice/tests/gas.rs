//! The GPU lattice gas against its CPU twin, and against Onsager's exact coexistence curve.

use gpu_prims::Gpu;
use lattice::gas::{coexistence, critical_temperature, GasConditions, GasConfig, GasRef, LatticeGas};
use pollster::block_on;

fn gpu() -> Gpu {
    block_on(Gpu::new()).expect("a GPU adapter is required")
}

#[test]
fn shaders_validate() {
    for (name, src) in lattice::shader_sources() {
        let module = naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&src)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    }
}

/// A liquid slab across the middle of the box (rows h/4..3h/4 full), the rest empty.
fn slab(w: u32, h: u32) -> Vec<u32> {
    (0..w * h).map(|i| ((i / w) >= h / 4 && (i / w) < 3 * h / 4) as u32).collect()
}

#[test]
fn gpu_matches_the_cpu_twin_bit_for_bit() {
    let g = gpu();
    for (config, cond) in [
        (GasConfig { w: 48, h: 36, periodic_y: true, wet_floor: false, wet_ceiling: false, seed: 5 }, GasConditions { t: 0.5, g: 0.0, hot_rows: 0, t_hot: 0.0 }),
        (GasConfig { w: 60, h: 40, periodic_y: false, wet_floor: true, wet_ceiling: false, seed: 9 }, GasConditions { t: 0.45, g: 0.05, hot_rows: 4, t_hot: 1.2 }),
    ] {
        let start = slab(config.w, config.h);
        let mut cpu = GasRef::new(config, cond, start.clone());
        let mut gpu = LatticeGas::new(&g, config, cond, &start);
        cpu.sweep(60);
        gpu.sweep(60);
        let occ = block_on(gpu.read_occ());
        let moved = occ.iter().zip(&start).filter(|(a, b)| a != b).count();
        eprintln!("{}×{}: {moved} sites changed in 60 sweeps; GPU and CPU agree: {}", config.w, config.h, occ == cpu.occ);
        assert!(moved > 20);
        assert_eq!(occ, cpu.occ);
        assert_eq!(occ.iter().sum::<u32>(), start.iter().sum::<u32>(), "molecules are conserved");
    }
}

#[test]
fn coexistence_matches_onsager() {
    let g = gpu();
    let (w, h) = (192u32, 192u32);
    eprintln!("kT_c = {:.4}", critical_temperature());
    for t in [0.40, 0.45, 0.50] {
        let config = GasConfig { w, h, periodic_y: true, wet_floor: false, wet_ceiling: false, seed: 3 };
        let mut gas = LatticeGas::new(&g, config, GasConditions { t, g: 0.0, hot_rows: 0, t_hot: 0.0 }, &slab(w, h));
        gas.sweep(4000);
        // Average the densities in the middle of the liquid slab and of the vapour.
        let (mut liquid, mut vapour, mut samples) = (0.0, 0.0, 0);
        for _ in 0..40 {
            gas.sweep(100);
            let r = block_on(gas.read_rows());
            let band = |y0: u32, y1: u32| (y0..y1).map(|y| r.molecules[y as usize] as f64).sum::<f64>() / ((y1 - y0) * w) as f64;
            liquid += band(3 * h / 8, 5 * h / 8);
            vapour += 0.5 * (band(0, h / 8) + band(7 * h / 8, h));
            samples += 1;
        }
        let (lq, vp) = (liquid / samples as f64, vapour / samples as f64);
        let (v_exact, l_exact) = coexistence(t).unwrap();
        eprintln!("kT = {t}: vapour {vp:.4} (Onsager {v_exact:.4}), liquid {lq:.4} (Onsager {l_exact:.4})");
        assert!((lq - l_exact).abs() < 0.01 + 0.1 * (1.0 - l_exact));
        assert!((vp - v_exact).abs() < 0.01 + 0.1 * v_exact);
    }
}

#[test]
#[ignore = "speed: prints sweeps per second for big lattices"]
fn speed() {
    let g = gpu();
    for side in [264u32, 516, 1020, 2040] {
        let config = GasConfig { w: side, h: side, periodic_y: true, wet_floor: false, wet_ceiling: false, seed: 1 };
        let mut gas = LatticeGas::new(&g, config, GasConditions { t: 0.55, g: 0.0, hot_rows: 0, t_hot: 0.0 }, &slab(side, side));
        gas.sweep(20);
        g.wait();
        let t0 = std::time::Instant::now();
        let n = if side > 1500 { 200 } else { 1000 };
        gas.sweep(n);
        g.wait();
        let s = t0.elapsed().as_secs_f64();
        eprintln!("{side}²: {:.0} sweeps/s, {:.2} ns per site-sweep", n as f64 / s, s / n as f64 / (side * side) as f64 * 1e9);
    }
}
