//! The 3D engine (`GpuEngine3`) against its CPU twin, and its exactness properties.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine3;
use md_core::three::{self, State3, System3};
use md_core::{init, Pair, Wall};
use md_ref::IntEngine3;
use pollster::block_on;

fn prims() -> Prims {
    Prims::new(&block_on(Gpu::new()).expect("a GPU adapter is required"))
}

/// An LJ liquid melted from an FCC lattice.
fn liquid(n: usize, density: f64, t: f64, walls: bool) -> (System3, State3) {
    let mut sys = System3::new(three::cube_for(n, density), init::DT, Pair::lj(2.5), vec![1.0]);
    if walls {
        sys = sys.with_walls(2, Wall::Hard, Wall::Hard);
    }
    let cells = three::fcc_cells(n, sys.box_len);
    let mut pos = three::fcc(n, sys.box_len, cells, 0.05, 7);
    if walls {
        // Keep clear of the hard walls' planes (fcc sites sit a quarter cell in).
        for p in pos.iter_mut() {
            p[2] = p[2].clamp(md_core::ONE / 2, sys.box_len[2] - md_core::ONE / 2);
        }
    }
    let species = vec![0; n];
    let vel = three::maxwell(&sys, &species, t, 3);
    (sys, State3 { pos, vel, species })
}

#[test]
fn kicks_match_the_cpu_twin() {
    let p = prims();
    let (sys, s) = liquid(4000, 0.8, 1.0, false);
    let mut cpu = IntEngine3::<f32>::new(sys.clone(), s);
    cpu.step(100);
    let gpu = GpuEngine3::new(&p, sys.clone(), &cpu.state);
    let (kg, peg) = block_on(gpu.kicks());
    let (kc, pec) = cpu.kicks();
    let (k64, _) = IntEngine3::<f64>::new(sys, cpu.state.clone()).kicks();
    let mut identical = 0;
    let mut max_diff = 0;
    let (mut num, mut den) = (0.0, 0.0);
    for i in 0..kg.len() {
        if kg[i] == kc[i] {
            identical += 1;
        }
        for a in 0..3 {
            max_diff = max_diff.max((kg[i][a] - kc[i][a]).abs());
            num += ((kg[i][a] - k64[i][a]) as f64).powi(2);
            den += (k64[i][a] as f64).powi(2);
        }
    }
    let rel64 = (num / den).sqrt();
    eprintln!("3D kicks: {identical}/{} identical to the CPU f32 twin, max diff {max_diff} units; rms rel. diff vs f64 {rel64:.2e}; PE {peg:.4} vs {pec:.4}", kg.len());
    // Each particle sums ~4 times as many pairs as in 2D, so more of them differ by a unit or two
    // from the twin (fused multiply-adds on the GPU); all agree with f64 to f32 precision.
    assert!(rel64 < 1e-5);
    assert!(identical as f64 > 0.15 * kg.len() as f64);
    assert!(max_diff <= 16);
    assert!(((peg - pec) / pec).abs() < 1e-5);
}

#[test]
fn trajectories_match_the_cpu_twin() {
    let p = prims();
    let (sys, s) = liquid(2000, 0.8, 1.0, true);
    let mut cpu = IntEngine3::<f32>::new(sys.clone(), s.clone());
    let mut gpu = GpuEngine3::new(&p, sys, &s);
    cpu.step(50);
    gpu.step(50);
    let g = block_on(gpu.read_state());
    let max = (0..g.n()).flat_map(|i| (0..3).map(move |a| (i, a))).map(|(i, a)| md_core::fixed::to_f64((g.pos[i][a] - cpu.state.pos[i][a]).abs())).fold(0.0, f64::max);
    eprintln!("3D after 50 steps: positions within {max:.1e} σ of the CPU twin's");
    // One-unit differences in kicks (fused multiply-adds) grow chaotically, but after 50 steps
    // they are still far below anything visible.
    assert!(max < 1e-5);
}

#[test]
fn reversal_restores_the_start_bit_for_bit() {
    let p = prims();
    let (sys, s) = liquid(4000, 0.8, 1.0, true);
    let mut gpu = GpuEngine3::new(&p, sys, &s);
    gpu.step(1000);
    gpu.reverse();
    gpu.step(1000);
    gpu.reverse();
    let back = block_on(gpu.read_state());
    assert_eq!(back.hash(), s.hash(), "3D run reversed did not return to its start");
}

#[test]
fn energy_is_conserved() {
    let p = prims();
    let (sys, s) = liquid(8000, 0.8, 1.0, false);
    let n = s.n() as f64;
    let mut gpu = GpuEngine3::new(&p, sys.clone(), &s);
    let mut energies = vec![];
    for _ in 0..10 {
        gpu.step(500);
        let st = block_on(gpu.read_state());
        let cpu = IntEngine3::<f64>::new(sys.clone(), st);
        energies.push(cpu.energy().total() / n);
    }
    let (lo, hi) = energies[2..].iter().fold((f64::MAX, f64::MIN), |(a, b), &e| (a.min(e), b.max(e)));
    eprintln!("3D LJ liquid, energy per particle over 5000 steps: {:?}", energies.iter().map(|e| format!("{e:.4}")).collect::<Vec<_>>());
    assert!(hi - lo < 2e-3, "energy per particle drifted by {}", hi - lo);
}

#[test]
fn bath_and_hot_walls_set_the_temperature() {
    let p = prims();
    // The Andersen bath.
    let (sys, s) = liquid(4000, 0.5, 0.8, false);
    let mut gpu = GpuEngine3::new(&p, sys.with_thermostat(1.6, 2.0), &s);
    gpu.step(4000);
    let t = block_on(gpu.read_kinetic_total()) / (1.5 * 4000.0);
    eprintln!("3D Andersen bath at 1.6: kT = {t:.3}");
    assert!((t - 1.6).abs() < 0.06);
    // Heat-bath walls on z, and a gas between them.
    // A thin gas (mean free path ~5 σ in a 34 σ box), so heat crosses it in tens of τ.
    let (sys, s) = liquid(2000, 0.05, 0.5, true);
    let sys = sys.with_walls(2, Wall::Thermal(2.0), Wall::Thermal(2.0));
    let mut gpu = GpuEngine3::new(&p, sys, &s);
    gpu.step(60000);
    let t = block_on(gpu.read_kinetic_total()) / (1.5 * 2000.0);
    let l = block_on(gpu.read_ledger());
    eprintln!("3D heat-bath walls at 2.0: kT = {t:.3}; energy from walls {:.1} ε over {} hits", l.energy[4] + l.energy[5], l.hits[4] + l.hits[5]);
    assert!((t - 2.0).abs() < 0.1);
}

#[test]
fn verlet_lists_and_cell_list_give_the_same_trajectory() {
    let p = prims();
    let (sys, s) = liquid(6000, 0.8, 1.0, true);
    let mut cells = GpuEngine3::with_mode(&p, sys.clone(), &s, md::ForceMode::CellList);
    let mut lists = GpuEngine3::new(&p, sys, &s);
    cells.step(2000);
    lists.step(2000);
    let (a, b) = (block_on(cells.read_state()), block_on(lists.read_state()));
    let (rebuilds, overflow) = block_on(lists.buf.read_list_stats());
    eprintln!("3D: {rebuilds} list rebuilds in 2000 steps");
    assert!(!overflow);
    assert!(rebuilds > 10);
    // Each pair's kick is rounded to an integer before summing, so the order in which neighbours
    // are visited does not matter: the trajectories agree bit for bit.
    assert_eq!(a.hash(), b.hash());
}
