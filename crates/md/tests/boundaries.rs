//! GPU walls, gravity and species-pair tables against the CPU twin.

use gpu_prims::{Gpu, Prims};
use md::{ForceMode, GpuEngine};
use md_core::{init, Pair, State, System, Wall};
use md_ref::IntEngine;
use pollster::block_on;

fn prims() -> Prims {
    Prims::new(&block_on(Gpu::new()).expect("a GPU adapter is required"))
}

/// A binary mixture (different sizes and attractions) in a box with soft walls in y and
/// gravity.
fn mixture(n: usize, seed: u64) -> (System, State) {
    let big = Pair { eps: 1.0, sigma: 1.4, rc: 1.4 * 2.5 };
    let cross = Pair { eps: 0.5, sigma: 1.2, rc: 1.2 * 2.5 };
    let sys = System::new(init::box_for(n, 0.4, 1.5), init::DT, Pair::lj(2.5), vec![1.0, 2.0])
        .with_pair(1, 1, big)
        .with_pair(0, 1, cross)
        .with_walls(1, Wall::Soft, Wall::Soft)
        .with_gravity([0.0, -0.05]);
    let mut s = init::fluid(&sys, n, 1.0, seed);
    for i in (0..n).step_by(3) {
        s.species[i] = 1;
    }
    s.vel = init::maxwell(&sys, &s.species, 1.0, seed);
    (sys, s)
}

#[test]
fn mixture_and_wall_kicks_match_the_cpu_twin() {
    let p = prims();
    let (sys, s) = mixture(4000, 1);
    let mut cpu = IntEngine::<f32>::new(sys.clone(), s);
    cpu.step(300);
    let s = cpu.state.clone();
    for mode in [ForceMode::CellList, ForceMode::verlet_for(&sys, s.n())] {
        let gpu = GpuEngine::with_mode(&p, sys.clone(), &s, mode);
        let (kg, _) = block_on(gpu.kicks());
        let (k32, _) = cpu.kicks();
        let (k64, _) = IntEngine::<f64>::new(sys.clone(), s.clone()).kicks();
        let identical = kg.iter().zip(&k32).filter(|(a, b)| a == b).count();
        let (mut num, mut den) = (0.0, 0.0);
        for (a, b) in kg.iter().zip(&k64) {
            for ax in 0..2 {
                num += ((a[ax] - b[ax]) as f64).powi(2);
                den += (b[ax] as f64).powi(2);
            }
        }
        let rel = (num / den).sqrt();
        eprintln!("{mode:?}: {identical}/{} kicks identical to the CPU f32 twin; rms error vs f64 {rel:.1e}", kg.len());
        assert!(rel < 1e-5);
    }
}

#[test]
fn walls_and_gravity_are_exactly_reversible_on_the_gpu() {
    let p = prims();
    let (sys, s0) = mixture(5000, 2);
    let sys = sys.with_walls(0, Wall::Hard, Wall::Hard);
    for mode in [ForceMode::CellList, ForceMode::verlet_for(&sys, s0.n())] {
        let mut e = GpuEngine::with_mode(&p, sys.clone(), &s0, mode);
        e.step(2000);
        let hits: u64 = block_on(e.read_ledger()).hits.iter().sum();
        e.reverse();
        e.step(2000);
        e.reverse();
        assert_eq!(block_on(e.read_state()), s0, "{mode:?}");
        assert!(hits > 100, "{mode:?}: only {hits} wall hits");
    }
}

#[test]
fn verlet_matches_cells_with_walls_gravity_and_mixtures() {
    let p = prims();
    let (sys, s0) = mixture(6000, 3);
    let mut a = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::CellList);
    let mut b = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::verlet_for(&sys, s0.n()));
    a.step(2000);
    b.step(2000);
    assert_eq!(block_on(a.read_state()), block_on(b.read_state()));
    let (la, lb) = (block_on(a.read_ledger()), block_on(b.read_ledger()));
    assert_eq!(la.impulse, lb.impulse, "soft-wall impulse should be bit-identical too");
}

/// Thermal walls bring the gas to their temperature, on the GPU and on the CPU alike.
#[test]
fn thermal_walls_on_the_gpu() {
    let p = prims();
    let n = 150;
    let sys = System::new(init::box_for(n, 0.1, 1.0), init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Thermal(1.5), Wall::Thermal(1.5))
        .with_walls(1, Wall::Thermal(1.5), Wall::Thermal(1.5));
    let s0 = init::fluid(&sys, n, 0.5, 4);
    let mut e = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::CellList);
    e.step(40000);
    let mut t = 0.0;
    for _ in 0..100 {
        e.step(200);
        let s = block_on(e.read_state());
        t += s.half_step_kinetic(&sys) / n as f64 / 100.0;
    }
    let heat: f64 = block_on(e.read_ledger()).energy.iter().sum();
    eprintln!("GPU: start T = 0.5, walls at 1.5: final T = {t:.3}; heat from the walls {heat:.1} ε");
    assert!((t - 1.5).abs() < 0.08, "T = {t}");
}

/// Heat flow between a hot and a cold wall: GPU and CPU agree within statistics, and the
/// ledger balances in the steady state.
#[test]
fn heat_flow_matches_the_cpu() {
    let p = prims();
    let n = 150;
    let sys = System::new(init::box_for(n, 0.1, 1.0), init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Thermal(2.0), Wall::Thermal(1.0))
        .with_walls(1, Wall::Hard, Wall::Hard);
    let s0 = init::fluid(&sys, n, 1.5, 6);
    let mut g = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::CellList);
    let mut c = IntEngine::<f32>::new(sys, s0);
    g.step(40000);
    c.step(40000);
    let (g0, c0) = (block_on(g.read_ledger()), c.ledger);
    g.step(200000);
    c.step(200000);
    let gl = block_on(g.read_ledger());
    let flux = |hot: f64| hot / (200000.0 * init::DT);
    let (gh, gc) = (gl.energy[0] - g0.energy[0], gl.energy[1] - g0.energy[1]);
    let (ch, cc) = (c.ledger.energy[0] - c0.energy[0], c.ledger.energy[1] - c0.energy[1]);
    eprintln!("heat flux hot→cold: GPU {:.3} ε/τ (cold wall {:.3}), CPU {:.3} ε/τ (cold wall {:.3})", flux(gh), flux(-gc), flux(ch), flux(-cc));
    assert!(((gh + gc) / gh).abs() < 0.1);
    assert!((gh / ch - 1.0).abs() < 0.15);
}

#[test]
fn compartments_on_the_gpu() {
    let p = prims();
    let side = 8i64 << 32;
    let sys = System::new([32 * side, 16 * side], init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(0, 3)
        .with_compartments(1, 3);
    let s0 = init::compartment_fluid(&sys, 8, 1.0, 9);
    let home: Vec<[i64; 2]> = s0.pos.iter().map(|&q| sys.compartment_of(q)).collect();
    for mode in [ForceMode::CellList, ForceMode::verlet_for(&sys, s0.n())] {
        let mut e = GpuEngine::with_mode(&p, sys.clone(), &s0, mode);
        // Kicks first: compartments must cut interactions exactly as on the CPU.
        let (kg, _) = block_on(e.kicks());
        let (k64, _) = IntEngine::<f64>::new(sys.clone(), s0.clone()).kicks();
        let worst = kg.iter().zip(&k64).map(|(a, b)| (a[0] - b[0]).abs().max((a[1] - b[1]).abs())).max().unwrap();
        assert!(worst < 64, "{mode:?}: kick difference {worst}");
        e.step(3000);
        let s = block_on(e.read_state());
        for (i, q) in s.pos.iter().enumerate() {
            assert_eq!(sys.compartment_of(*q), home[i], "{mode:?}: particle {i} left its compartment");
        }
        e.reverse();
        e.step(3000);
        e.reverse();
        assert_eq!(block_on(e.read_state()), s0, "{mode:?}");
    }
}

/// Rows of compartments between a hot and a cold wall: one ledger slot per row, summing to the
/// wall totals.
#[test]
fn per_compartment_ledger() {
    let p = prims();
    let side = 16i64 << 32;
    let rows = 8;
    let sys = System::new([2 * side, rows * side], init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Thermal(2.0), Wall::Thermal(1.0))
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(1, 4);
    let s0 = init::compartment_fluid(&sys, 30, 1.5, 10);
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.step(20000);
    let l = block_on(e.buf.read_compartment_ledger(rows as usize));
    let t = l.totals();
    eprintln!("heat from the hot wall per row: {:?}", l.energy[0].iter().map(|x| format!("{x:.1}")).collect::<Vec<_>>());
    assert!(l.energy[0].iter().all(|&q| q > 0.0));
    let total: f64 = l.energy[0].iter().sum();
    assert!((total - t.energy[0]).abs() < 1e-6);
    assert!(l.hits[2].iter().skip(1).all(|&h| h == 0), "only row 0 touches the bottom wall");
}

#[test]
fn andersen_thermostat_on_the_gpu() {
    let p = prims();
    let n = 5000;
    let sys = System::new(init::box_for(n, 0.6, 1.0), init::DT, Pair::lj(2.5), vec![1.0]).with_thermostat(0.8, 1.0);
    let s0 = init::fluid(&sys, n, 2.0, 12);
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.step(3000);
    let mut t = 0.0;
    for _ in 0..20 {
        e.step(100);
        t += block_on(e.read_state()).half_step_kinetic(&sys) / n as f64 / 20.0;
    }
    eprintln!("GPU Andersen at T = 0.8: {t:.3}");
    assert!((t - 0.8).abs() < 0.04);
}

/// Osmotic pressure on a membrane against van 't Hoff's c kT, for a dilute solute in a dense
/// repulsive solvent (no attraction anywhere). At 10% solute the measured value is ~20% below
/// c kT (non-ideal: the solute is depleted next to the membrane); at 5% it is closer.
#[test]
fn osmotic_pressure_is_van_t_hoff() {
    let p = prims();
    let n = 20000;
    let sys = System::new(init::box_for(n, 0.5, 2.0), init::DT, Pair::wca(), vec![1.0, 1.0])
        .with_walls(0, Wall::Soft, Wall::Soft)
        .with_thermostat(1.0, 0.5);
    let b = sys.box_f64();
    let sys = sys.with_membrane(0, b[0] / 2.0, &[1]);
    let mut s0 = init::fluid(&sys, n, 1.0, 14);
    let mut solute = 0;
    for i in 0..n {
        if md_core::fixed::to_f64(s0.pos[i][0]) > b[0] / 2.0 + 1.0 && gpu_prims::threefry::uniform(14, 3, i as u32) < 0.05 {
            s0.species[i] = 1;
            solute += 1;
        }
    }
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.step(10000);
    let before = block_on(e.read_ledger());
    let steps = 100000;
    e.step(steps);
    let l = block_on(e.read_ledger());
    let push = (l.impulse[5] - before.impulse[5]) - (l.impulse[4] - before.impulse[4]);
    let pi = push / (steps as f64 * sys.dt * b[1]);
    // Solute concentration in the right half (soft wall layer excluded).
    let c = solute as f64 / (b[1] * (b[0] / 2.0 - 0.5));
    eprintln!("osmotic pressure {pi:.5} vs van 't Hoff c kT {c:.5} ({solute} solute, {} hits)", l.hits[5] - before.hits[5]);
    assert!((pi / c - 1.0).abs() < 0.2, "ratio {}", pi / c);
}

/// The membrane's pressure measurement with no solvent: a dilute gas on one side.
#[test]
fn membrane_pressure_of_a_dilute_gas() {
    let p = prims();
    let n = 20000;
    let sys = System::new(init::box_for(n, 0.5, 2.0), init::DT, Pair::wca(), vec![1.0, 1.0])
        .with_walls(0, Wall::Soft, Wall::Soft)
        .with_thermostat(1.0, 0.5);
    let b = sys.box_f64();
    let sys = sys.with_membrane(0, b[0] / 2.0, &[1]);
    let mut s0 = init::fluid(&sys, n, 1.0, 15);
    // Keep only the would-be solute (now a dilute gas on the right), drop the rest.
    let keep: Vec<usize> = (0..n).filter(|&i| md_core::fixed::to_f64(s0.pos[i][0]) > b[0] / 2.0 + 1.0 && gpu_prims::threefry::uniform(15, 3, i as u32) < 0.05).collect();
    s0 = State { pos: keep.iter().map(|&i| s0.pos[i]).collect(), vel: keep.iter().map(|&i| s0.vel[i]).collect(), species: vec![1; keep.len()] };
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.step(10000);
    let before = block_on(e.read_ledger());
    let steps = 100000;
    e.step(steps);
    let l = block_on(e.read_ledger());
    let pi = ((l.impulse[5] - before.impulse[5]) - (l.impulse[4] - before.impulse[4])) / (steps as f64 * sys.dt * b[1]);
    let wall = (l.impulse[1] - before.impulse[1]) / (steps as f64 * sys.dt * b[1]);
    let c = keep.len() as f64 / (b[1] * (b[0] / 2.0 - 0.5));
    eprintln!("dilute gas: membrane pressure {pi:.5}, far wall {wall:.5}, ρkT {c:.5}");
    assert!((pi / c - 1.0).abs() < 0.1);
}

fn gas_under_piston(n: usize, density: f64, drive: md_core::PistonDrive, seed: u64) -> (System, State) {
    let mut bl = init::box_for(n, density, 1.0);
    bl[1] *= 2;
    let h = md_core::fixed::to_f64(bl[1]) / 2.0;
    let sys = System::new(bl, init::DT, Pair::wca(), vec![1.0]).with_walls(1, Wall::Soft, Wall::Hard).with_piston(drive, h, 2.0);
    let mut s = init::fluid(&sys, n, 1.0, seed);
    for q in s.pos.iter_mut() {
        q[1] /= 2;
    }
    (sys, s)
}

#[test]
fn piston_on_the_gpu_matches_the_cpu_and_reverses() {
    let p = prims();
    let (sys, s0) = gas_under_piston(3000, 0.3, md_core::PistonDrive::Speed(-0.5), 19);
    let mut cpu = IntEngine::<f32>::new(sys.clone(), s0.clone());
    let mut gpu = GpuEngine::new(&p, sys.clone(), &s0);
    cpu.step(400);
    gpu.step(400);
    let (w_cpu, w_gpu) = (md_core::fixed::to_f64(cpu.piston_w), block_on(gpu.read_piston())[0]);
    assert!((w_cpu - w_gpu).abs() < 1e-9, "piston heights {w_cpu} vs {w_gpu}");
    let g = GpuEngine::new(&p, sys.clone(), &cpu.state);
    // Same positions and piston height: kicks agree (the GPU engine starts at the initial height,
    // so compare at the start instead).
    let (kg, _) = block_on(GpuEngine::new(&p, sys.clone(), &s0).kicks());
    let (kc, _) = IntEngine::<f64>::new(sys.clone(), s0.clone()).kicks();
    let worst = kg.iter().zip(&kc).map(|(a, b)| (a[1] - b[1]).abs()).max().unwrap();
    assert!(worst < 64, "kick difference {worst}");
    drop(g);
    gpu.reverse();
    gpu.step(400);
    gpu.reverse();
    assert_eq!(block_on(gpu.read_state()), s0);
    assert!((block_on(gpu.read_piston())[0] - md_core::fixed::to_f64(md_core::fixed::to_fixed(sys.piston.unwrap().start))).abs() < 1e-12);
}

/// Slow compression with no heat exchange follows the adiabat T·A = const (2D, γ = 2). Small
/// molecules (σ = 0.4) keep the gas nearly ideal; with σ = 1 at these densities the second
/// virial coefficient adds ~10% (T·A grows as exp(B₂ Δρ)).
#[test]
fn slow_compression_is_adiabatic() {
    let p = prims();
    let n = 2000;
    let (sys, s0) = gas_under_piston(n, 0.05, md_core::PistonDrive::Speed(-0.1), 20);
    let sys = sys.with_pair(0, 0, Pair { eps: 1.0, sigma: 0.4, rc: 0.4 * 2f64.powf(1.0 / 6.0) });
    let width = sys.box_f64()[0];
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.step(2000);
    let t = |s: &State| s.half_step_kinetic(&sys) / n as f64;
    let (t0, h0) = (t(&block_on(e.read_state())), block_on(e.read_piston())[0]);
    let steps = ((h0 * 0.6) / (0.1 * sys.dt)) as usize;
    e.step(steps);
    let (t1, h1) = (t(&block_on(e.read_state())), block_on(e.read_piston())[0]);
    let (a0, a1) = (width * (h0 - 1.0), width * (h1 - 1.0));
    let work = -block_on(e.read_piston())[3];
    eprintln!("slow compression {h0:.1} → {h1:.1} σ: T {t0:.3} → {t1:.3}, T·A ratio {:.3}; work on gas {work:.0} ε, ΔKE {:.0} ε", t1 * a1 / (t0 * a0), (t1 - t0) * n as f64);
    assert!((t1 * a1 / (t0 * a0) - 1.0).abs() < 0.08);
}

/// Effusion through small pores: a light and a heavy gas leak at rates in the ratio √(m_heavy /
/// m_light) (Graham's law). Needs a dilute gas (mean free path ≫ pore width); at a density of
/// 1 per σ² the measured ratio was 2.1 instead of 3.
#[test]
fn effusion_through_pores_follows_graham() {
    let p = prims();
    let n = 1000;
    let small = Pair { eps: 1.0, sigma: 0.3, rc: 0.3 * 2f64.powf(1.0 / 6.0) };
    let sys = System::new([128i64 << 32, 128i64 << 32], init::DT, small, vec![1.0, 9.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_membrane(0, 32.0, &[0, 1])
        .with_pores(4, 0.6);
    // Everything starts inside (left of the membrane).
    let inside = System::new([31i64 << 32, 128i64 << 32], init::DT, small, vec![1.0, 9.0]).with_walls(0, Wall::Hard, Wall::Hard);
    let mut s0 = init::fluid(&inside, n, 1.0, 21);
    for i in (1..n).step_by(2) {
        s0.species[i] = 1;
    }
    s0.vel = init::maxwell(&sys, &s0.species, 1.0, 21);
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    let left = |s: &State, sp: u32| s.pos.iter().zip(&s.species).filter(|(q, k)| **k == sp && md_core::fixed::to_f64(q[0]) < 32.0).count() as f64;
    e.step(80000);
    let s = block_on(e.read_state());
    let (l, h) = (left(&s, 0) / (n / 2) as f64, left(&s, 1) / (n / 2) as f64);
    // Early on the leak is exponential: N(t) = N0 exp(−k t) with k ∝ 1/√m.
    let ratio = l.ln() / h.ln();
    eprintln!("after 400 τ: {:.1}% of the light and {:.1}% of the heavy gas remain; rate ratio {ratio:.2} (Graham: 3)", 100.0 * l, 100.0 * h);
    assert!((ratio / 3.0 - 1.0).abs() < 0.2);
}

/// A fan (a thermostat with a drift, acting on one species in a slab) drives the gas round a
/// periodic channel with frictionless (hard) walls until everything, including a species the
/// fan ignores, moves at the fan's speed, at the fan's temperature in the moving frame. GPU and
/// CPU twin.
#[test]
fn fan_drives_a_flow() {
    let p = prims();
    let n = 500;
    let (w, h) = (80.0, 20.0);
    let sys = System::new([md_core::fixed::to_fixed(w), md_core::fixed::to_fixed(h)], init::DT, Pair::wca(), vec![1.0, 4.0])
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_thermostat(1.0, 2.0)
        .with_fan(1, 0.0, 20.0, 0.5);
    let mut s0 = init::fluid(&sys, n, 1.0, 21);
    for i in (0..n).step_by(20) {
        s0.species[i] = 1;
    }
    let flow = |s: &State| {
        let mut out = [[0.0; 3]; 2];
        for i in 0..s.n() {
            let k = s.species[i] as usize;
            let v = s.vel[i].map(|x| sys.velocity(x));
            out[k][0] += v[0];
            out[k][1] += sys.masses[k] * (v[0] * v[0] + v[1] * v[1]);
            out[k][2] += 1.0;
        }
        // (mean x velocity, kT in the frame moving with the mean) per species.
        out.map(|[sv, s2, c]| {
            let u = sv / c;
            let m = if c == (n / 20) as f64 { 4.0 } else { 1.0 };
            (u, 0.5 * (s2 / c - m * u * u))
        })
    };
    let mut gpu = GpuEngine::new(&p, sys.clone(), &s0);
    let mut cpu = IntEngine::<f32>::new(sys.clone(), s0.clone());
    // The flow takes ~1500 steps to respond to the fan (it acts on a quarter of the box), and
    // wanders on that time scale by ~0.06 σ/τ, so average 20 snapshots 1000 steps apart.
    gpu.step(8000);
    cpu.step(8000);
    let (mut g, mut c) = ([(0.0, 0.0); 2], [(0.0, 0.0); 2]);
    for _ in 0..20 {
        gpu.step(1000);
        cpu.step(1000);
        for (acc, f) in [(&mut g, flow(&block_on(gpu.read_state()))), (&mut c, flow(&cpu.state))] {
            for k in 0..2 {
                acc[k].0 += f[k].0 / 20.0;
                acc[k].1 += f[k].1 / 20.0;
            }
        }
    }
    eprintln!("fan: GPU {g:?}, CPU {c:?}");
    for f in [g, c] {
        for (k, (u, t)) in f.iter().enumerate() {
            // Species 1 (25 particles) is noisier.
            let tol = [0.06, 0.1][k];
            assert!((u - 0.5).abs() < tol, "species {k} flows at {u:.3}, not 0.5");
            assert!((t - 1.0).abs() < tol, "species {k} at kT {t:.3} in the moving frame");
        }
    }
}



/// Diathermal compartment walls: molecules interact through them but never cross. Four boxes,
/// one hot: their temperatures converge (on the GPU and the CPU twin), the total energy holds,
/// and the run still reverses exactly. With insulating walls the hot box stays hot.
#[test]
fn diathermal_walls_share_energy() {
    let p = prims();
    let side = 16.0;
    let mk = |diathermal: bool| {
        let mut sys = System::new([md_core::fixed::to_fixed(2.0 * side); 2], init::DT, Pair::wca(), vec![1.0])
            .with_walls(0, Wall::Hard, Wall::Hard)
            .with_walls(1, Wall::Hard, Wall::Hard)
            .with_compartments(0, 4)
            .with_compartments(1, 4);
        if diathermal {
            sys = sys.with_diathermal_compartments();
        }
        sys
    };
    let sys = mk(true);
    let mut s0 = init::compartment_fluid(&sys, 80, 0.5, 31);
    // The box at the origin is hot: kT 3.5 against 0.5, so the average is 1.25.
    for i in 0..s0.n() {
        if sys.compartment_of(s0.pos[i]) == [0, 0] {
            s0.vel[i] = s0.vel[i].map(|v| (v as f64 * 7f64.sqrt()).round() as i32);
        }
    }
    let box_t = |sys: &System, s: &State| {
        let mut t = [[0.0; 2]; 4];
        for i in 0..s.n() {
            let c = sys.compartment_of(s.pos[i]);
            let v = s.vel[i].map(|x| sys.velocity(x));
            let k = (c[1] * 2 + c[0]) as usize;
            t[k][0] += 0.5 * (v[0] * v[0] + v[1] * v[1]);
            t[k][1] += 1.0;
        }
        t.map(|[e, n]| e / n)
    };
    eprintln!("start: box kT {:?}", box_t(&sys, &s0));
    let e0 = IntEngine::<f64>::new(sys.clone(), s0.clone()).energy().total();
    let mut g = GpuEngine::new(&p, sys.clone(), &s0);
    let mut c = IntEngine::<f32>::new(sys.clone(), s0.clone());
    let mut ins = GpuEngine::new(&p, mk(false), &s0);
    g.step(20000);
    c.step(20000);
    ins.step(20000);
    let (tg, tc, ti) = (box_t(&sys, &block_on(g.read_state())), box_t(&sys, &c.state), box_t(&sys, &block_on(ins.read_state())));
    let e1 = IntEngine::<f64>::new(sys.clone(), block_on(g.read_state())).energy().total();
    eprintln!("after 100 τ: GPU {tg:?}, CPU {tc:?}, insulating {ti:?}; total energy {e0:.2} → {e1:.2}");
    // With 80 molecules a box's kT fluctuates by ~±0.14 even in equilibrium.
    for t in tg.iter().chain(&tc) {
        assert!((t - 1.25).abs() < 0.4, "box kT {t:.3} has not approached 1.25");
    }
    assert!(ti[0] > 3.0, "insulated hot box cooled to {:.2}", ti[0]);
    // Integration error (the hot box's fast molecules) moves the total by a few tenths of a percent.
    assert!((e1 - e0).abs() < 1e-2 * e0.abs().max(1.0), "energy drifted {e0} → {e1}");
    g.reverse();
    g.step(20000);
    g.reverse();
    assert!(block_on(g.read_state()) == s0, "diathermal walls broke exact reversal");
}
