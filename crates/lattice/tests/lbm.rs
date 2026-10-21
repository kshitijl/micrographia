//! The Shan–Chen fluid: its equation of state, and its liquid and vapour against the model's
//! own coexistence prediction.

use gpu_prims::Gpu;
use lattice::lbm::{Eos, LbmConditions, LbmConfig, ShanChen};
use pollster::block_on;

fn gpu() -> Gpu {
    block_on(Gpu::new()).expect("a GPU adapter is required")
}

#[test]
fn critical_point_of_carnahan_starling() {
    let (tc, rc) = Eos::default().critical();
    eprintln!("Carnahan–Starling (a = 1, b = 4): T_c = {tc:.5}, ρ_c = {rc:.5}");
    assert!((tc - 0.09432).abs() < 2e-4);
    assert!((rc - 0.1304).abs() < 1e-3);
    let e = Eos::default();
    for f in [0.7, 0.8, 0.9] {
        let (mv, ml) = e.coexistence_maxwell(f * tc).unwrap();
        let (sv, sl) = e.coexistence_sc(f * tc).unwrap();
        eprintln!("T = {f} T_c: Maxwell {mv:.4} / {ml:.4}; Shan–Chen {sv:.4} / {sl:.4}");
        assert!(sv < mv * 2.0 && sl > 0.5 * ml);
    }
}

/// A flat liquid slab between vapour, in a periodic-x box with neutral walls top and bottom, at
/// fixed uniform temperature: after it settles, the bulk densities should be the model's.
#[test]
fn slab_reaches_shan_chen_coexistence() {
    let g = gpu();
    let e = Eos::default();
    let (tc, _) = e.critical();
    for f in [0.75, 0.85] {
        let t = f * tc;
        let (v, l) = e.coexistence_sc(t).unwrap();
        let (w, h) = (64u32, 200u32);
        let rho: Vec<f32> = (0..w * h).map(|i| if (i / w) >= h / 3 && (i / w) < 2 * h / 3 { l as f32 } else { v as f32 }).collect();
        let temp = vec![t as f32; (w * h) as usize];
        let mut sim = ShanChen::new(&g, LbmConfig { w, h, eos: e }, LbmConditions::default(), &rho, &temp);
        let m0: f64 = rho.iter().enumerate().filter(|(i, _)| (*i as u32 / w) > 0 && (*i as u32 / w) < h - 1).map(|(_, &r)| r as f64).sum();
        sim.step(20000);
        let r = block_on(sim.read_rho());
        let m1: f64 = r.iter().map(|&x| x as f64).sum();
        let row = |y: u32| (0..w).map(|x| r[(y * w + x) as usize] as f64).sum::<f64>() / w as f64;
        let (liquid, vapour) = (row(h / 2), row(h / 6));
        eprintln!("T = {f} T_c: liquid {liquid:.4} (model {l:.4}), vapour {vapour:.4} (model {v:.4}); mass {m0:.2} → {m1:.2}");
        // Mass is conserved up to f32 rounding (a drift of a few 10⁻⁴ over 20,000 steps).
        assert!(((m1 - m0) / m0).abs() < 1e-3, "mass is conserved");
        assert!((liquid - l).abs() < 0.03 * l);
        // The thin vapour is the least accurate (the lattice's discretisation of the force).
        assert!((vapour - v).abs() < 0.35 * v);
    }
}

#[test]
#[ignore = "an experiment: a pool on a hot plate"]
fn pool_on_a_hot_plate() {
    let g = gpu();
    let e = Eos::default();
    let (tc, _) = e.critical();
    let t = 0.8 * tc;
    let (v, l) = e.coexistence_sc(t).unwrap();
    let (w, h) = (256u32, 256u32);
    let rho: Vec<f32> = (0..w * h).map(|i| if (i / w) < 90 { l as f32 } else { v as f32 }).collect();
    let temp = vec![t as f32; (w * h) as usize];
    let hot = std::env::var("HOT").ok().and_then(|s| s.parse().ok()).unwrap_or(1.1);
    let cond = LbmConditions { thermal: true, gravity: 2e-5, t_floor: hot * tc, plate: (64, 192), t_ceiling: t, ..Default::default() };
    let mut sim = ShanChen::new(&g, LbmConfig { w, h, eos: e }, cond, &rho, &temp);
    for k in 0..8 {
        sim.step(10000);
        let (r, tt) = (block_on(sim.read_rho()), block_on(sim.read_temp()));
        let col: Vec<String> = [1u32, 5, 20, 50, 85, 100, 150, 250].iter().map(|&y| format!("{:.2}/{:.3}", tt[(y * w + 128) as usize] / tc as f32, r[(y * w + 128) as usize])).collect();
        let liquid = r.iter().filter(|&&x| x as f64 > 0.5 * (l + v)).count();
        eprintln!("{:>6} steps: liquid {liquid}, T/Tc / ρ up the middle: {}", (k + 1) * 10000, col.join("  "));
    }
}
