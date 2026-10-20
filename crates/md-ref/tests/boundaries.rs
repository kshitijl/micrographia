//! Walls and gravity in the CPU integer engine: exactness and physics.

use md_core::{fixed, init, Pair, State, System, Wall};
use md_ref::IntEngine;

fn gas_in_walls(n: usize, density: f64, aspect: f64, seed: u64) -> (System, State) {
    let sys = System::new(init::box_for(n, density, aspect), init::DT, Pair::wca(), vec![1.0]).with_walls(1, Wall::Hard, Wall::Hard);
    let s = init::fluid(&sys, n, 1.0, seed);
    (sys, s)
}

#[test]
fn hard_walls_and_gravity_are_exactly_reversible() {
    let (sys, s0) = gas_in_walls(400, 0.3, 1.0, 1);
    let sys = sys.with_walls(0, Wall::Hard, Wall::Hard).with_gravity([0.0, -0.5]);
    let mut e = IntEngine::<f32>::new(sys, s0.clone());
    e.step(2000);
    assert!(e.ledger.hits.iter().sum::<u64>() > 100, "particles should hit the walls");
    e.reverse();
    e.step(2000);
    e.reverse();
    assert_eq!(e.state, s0);
}

#[test]
fn soft_walls_and_gravity_are_exactly_reversible() {
    let (sys, s0) = gas_in_walls(400, 0.3, 1.0, 1);
    let sys = sys.with_walls(0, Wall::Soft, Wall::Soft).with_walls(1, Wall::Soft, Wall::Soft).with_gravity([0.0, -0.5]);
    let mut e = IntEngine::<f32>::new(sys, s0.clone());
    e.step(2000);
    assert!(e.ledger.impulse.iter().sum::<f64>() > 1.0, "particles should push on the walls");
    e.reverse();
    e.step(2000);
    e.reverse();
    assert_eq!(e.state, s0);
}

#[test]
fn energy_with_gravity_and_soft_walls_is_conserved() {
    let (sys, s0) = gas_in_walls(400, 0.3, 1.0, 2);
    // m g H / T ≈ 2: a stratified gas, not one crushed against the floor.
    let g = 2.0 / sys.box_f64()[1];
    let sys = sys.with_walls(1, Wall::Soft, Wall::Soft).with_gravity([0.0, -g]);
    let mut e = IntEngine::<f64>::new(sys, s0);
    e.step(200);
    let e0 = e.energy().total();
    let mut worst = 0.0f64;
    for _ in 0..20 {
        e.step(100);
        worst = worst.max((e.energy().total() - e0).abs() / 400.0);
    }
    eprintln!("energy per particle with gravity and soft walls: max deviation {worst:.2e}");
    assert!(worst < 2e-3);
}

/// Density falls as exp(−m g y / T) in a dilute gas under gravity. The particles are small
/// (σ = 0.4) so the gas is nearly ideal: for σ = 1 at this density the second virial
/// coefficient flattens the profile by ~15% (dP/dy = −ρ m g with P = ρT(1 + B₂ρ)).
#[test]
fn barometric_profile() {
    let n = 600;
    let small = Pair { eps: 1.0, sigma: 0.4, rc: 0.4 * 2f64.powf(1.0 / 6.0) };
    let sys = System::new(init::box_for(n, 0.05, 4.0), init::DT, small, vec![1.0]).with_walls(1, Wall::Soft, Wall::Soft);
    let s0 = init::fluid(&sys, n, 1.0, 3);
    let ly = sys.box_f64()[1];
    let g = 2.0 / ly; // m g L / T ≈ 2 at T = 1
    let mut e = IntEngine::<f32>::new(sys.with_gravity([0.0, -g]), s0);
    e.step(40000);
    let bins = 10;
    let mut hist = vec![0f64; bins];
    let mut t = 0.0;
    let samples = 400;
    for _ in 0..samples {
        e.step(50);
        for p in &e.state.pos {
            hist[((fixed::to_f64(p[1]) / ly * bins as f64) as usize).min(bins - 1)] += 1.0;
        }
        let v = e.velocities();
        t += v.iter().map(|v| 0.5 * (v[0] * v[0] + v[1] * v[1])).sum::<f64>() / n as f64 / samples as f64;
    }
    // Fit ln(density) against height, away from the soft walls' contact layers.
    let pts: Vec<(f64, f64)> = (1..bins - 1).map(|b| ((b as f64 + 0.5) * ly / bins as f64, hist[b].ln())).collect();
    let np = pts.len() as f64;
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / np, pts.iter().map(|p| p.1).sum::<f64>() / np);
    let slope = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / pts.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>();
    let want = -g / t;
    eprintln!("barometric: fitted ln ρ slope {slope:.4} per σ, expected −m g / T = {want:.4} (T = {t:.3})");
    assert!((slope / want - 1.0).abs() < 0.1);
}

#[test]
fn thermal_walls_set_the_temperature() {
    let n = 150;
    let sys = System::new(init::box_for(n, 0.1, 1.0), init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Thermal(1.5), Wall::Thermal(1.5))
        .with_walls(1, Wall::Thermal(1.5), Wall::Thermal(1.5));
    let s0 = init::fluid(&sys, n, 0.5, 4);
    let mut e = IntEngine::<f32>::new(sys, s0);
    e.step(40000);
    let mut t = 0.0;
    for _ in 0..200 {
        e.step(100);
        t += e.velocities().iter().map(|v| 0.5 * (v[0] * v[0] + v[1] * v[1])).sum::<f64>() / n as f64 / 200.0;
    }
    let heat: f64 = e.ledger.energy.iter().sum();
    eprintln!("start T = 0.5, walls at 1.5: final T = {t:.3}; heat taken from the walls {heat:.1} ε");
    assert!((t - 1.5).abs() < 0.06, "T = {t}");
    assert!(heat > 0.5 * n as f64);
}

/// A hot wall and a cold wall: in the steady state the heat in equals the heat out.
#[test]
fn steady_heat_flow_between_two_baths() {
    let n = 150;
    let sys = System::new(init::box_for(n, 0.1, 1.0), init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Thermal(2.0), Wall::Thermal(1.0))
        .with_walls(1, Wall::Hard, Wall::Hard);
    let s0 = init::fluid(&sys, n, 1.5, 6);
    let mut e = IntEngine::<f32>::new(sys, s0);
    e.step(40000);
    let before = e.ledger;
    e.step(100000);
    let hot = e.ledger.energy[0] - before.energy[0];
    let cold = e.ledger.energy[1] - before.energy[1];
    eprintln!("over 500 τ: heat in from the hot wall {hot:.1} ε, out to the cold wall {:.1} ε", -cold);
    assert!(hot > 50.0, "heat should flow from hot to cold");
    assert!(((hot + cold) / hot).abs() < 0.1, "steady state: in ≈ out");
}

/// The momentum given to a hard wall per time per length is the pressure; for a dilute gas
/// it is close to ρT.
#[test]
fn hard_wall_pressure_of_a_dilute_gas() {
    let n = 1000;
    let density = 0.03;
    let (sys, s0) = gas_in_walls(n, density, 1.0, 5);
    let lx = sys.box_f64()[0];
    let dt = sys.dt;
    let mut e = IntEngine::<f32>::new(sys, s0);
    e.step(2000);
    let before = e.ledger;
    let steps = 20000;
    let mut t = 0.0;
    for _ in 0..20 {
        e.step(steps / 20);
        t += e.velocities().iter().map(|v| 0.5 * (v[0] * v[0] + v[1] * v[1])).sum::<f64>() / n as f64 / 20.0;
    }
    let impulse = (e.ledger.impulse[2] - before.impulse[2]) + (e.ledger.impulse[3] - before.impulse[3]);
    let p = impulse / (2.0 * lx * steps as f64 * dt);
    eprintln!("wall pressure {p:.5}, ideal ρT = {:.5}", density * t);
    assert!((p / (density * t) - 1.0).abs() < 0.1);
}
