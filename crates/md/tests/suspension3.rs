//! Einstein's viscosity of a suspension (the `suspension3` scenario): a shear wave
//! v_x = U sin(2π z / L) decays as e^(−η k² t / ρ), so the decay rate gives the viscosity η, and
//! grains raise it as η (1 + 2.5 φ) for a small volume fraction φ.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine3;
use md_core::fixed;
use pollster::block_on;
use scenarios::Options;

/// The viscosity, from `stirs` shear waves each followed for `t` τ: (η, φ by the grains'
/// hydrodynamic radius (size + 1)/2).
pub fn viscosity(p: &Prims, n: f64, grains: f64, size: f64, stirs: usize) -> (f64, f64) {
    let sc = scenarios::three::build3("suspension3", &Options::default().with("n", n).with("grains", grains).with("size", size)).unwrap();
    let mut e = GpuEngine3::new(p, sc.sys.clone(), &sc.state);
    let l = sc.sys.box_f64();
    let masses = sc.sys.masses.clone();
    let k = 2.0 * std::f64::consts::PI / l[2];
    let total_mass: f64 = sc.state.species.iter().map(|&s| masses[s as usize]).sum();
    let rho_m = total_mass / (l[0] * l[1] * l[2]);
    let u = 0.5;
    e.step(4000);
    let mut rates = vec![];
    for _ in 0..stirs {
        // Stir: sixteen slabs in z, each shoved by U sin(2π z / L) at its middle.
        for b in 0..16 {
            let (z0, z1) = (b as f64 * l[2] / 16.0, (b + 1) as f64 * l[2] / 16.0);
            let add = u * (k * 0.5 * (z0 + z1)).sin();
            e.kick_region([-1.0, -1.0, z0], [l[0] + 1.0, l[1] + 1.0, z1], 1.0, [add, 0.0, 0.0]);
        }
        // The wave's momentum amplitude, every 0.25 τ, while it is clear of the noise.
        let mut pts = vec![];
        for q in 0..60 {
            let s = block_on(e.read_state());
            let mut a = 0.0;
            for i in 0..s.species.len() {
                let m = masses[s.species[i] as usize];
                a += m * sc.sys.velocity(s.vel[i][0]) * (k * fixed::to_f64(s.pos[i][2])).sin();
            }
            let a = 2.0 * a / total_mass;
            if a > 0.05 {
                pts.push((q as f64 * 0.25, a.ln()));
            }
            e.step(50);
        }
        let n = pts.len() as f64;
        let (sx, sy) = (pts.iter().map(|p| p.0).sum::<f64>(), pts.iter().map(|p| p.1).sum::<f64>());
        let (sxx, sxy) = (pts.iter().map(|p| p.0 * p.0).sum::<f64>(), pts.iter().map(|p| p.0 * p.1).sum::<f64>());
        rates.push(-(n * sxy - sx * sy) / (n * sxx - sx * sx));
        e.step(4000); // let the heat of the stir spread; the next stir starts afresh
    }
    eprintln!("  rates: {}", rates.iter().map(|r| format!("{r:.4}")).collect::<Vec<_>>().join(" "));
    let rate = rates.iter().sum::<f64>() / rates.len() as f64;
    let phi = grains * std::f64::consts::PI / 6.0 * (size + 1.0).powi(3) / (l[0] * l[1] * l[2]);
    (rate * rho_m / (k * k), phi)
}

#[test]
#[ignore = "an experiment: the suspension's viscosity against its grains"]
fn suspension_viscosity() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let size: f64 = std::env::var("SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(3.0);
    let n: f64 = std::env::var("N").ok().and_then(|s| s.parse().ok()).unwrap_or(16000.0);
    let (eta0, _) = viscosity(&p, n, 0.0, size, 6);
    let grains: Vec<f64> = std::env::var("GRAINS").map(|s| s.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or(vec![27.0, 64.0]);
    for g in grains {
        let (eta, phi) = viscosity(&p, n, g.round(), size, 6);
        eprintln!("suspension: n {n}, {} grains of {size} σ, φ {phi:.3}: η {eta:.3} against {eta0:.3} pure: ratio {:.3} (Einstein 1 + 2.5φ = {:.3})", g.round(), eta / eta0, 1.0 + 2.5 * phi);
    }
}

/// The viscosity by the periodic-force method: a steady push f sin(kz) on every particle (as a
/// kick per slab every 10 steps) drives a steady flow u sin(kz) with u = f ρ / (η k²) per unit
/// mass; a thermostat every 100 steps rescales only the velocities relative to that flow (each
/// slab's own mean), so the flow is not damped. Returns (η, φ).
pub fn viscosity_steady(p: &Prims, n: f64, grains: f64, size: f64, f: f64, settle: usize, measure: usize) -> (f64, f64) {
    let clear: f64 = std::env::var("CLEAR").ok().and_then(|s| s.parse().ok()).unwrap_or(1.02);
    let sc = scenarios::three::build3("suspension3", &Options::default().with("n", n).with("grains", grains).with("size", size).with("clear", clear)).unwrap();
    let solvent = sc.state.species.iter().filter(|&&s| s == 0).count();
    eprintln!("  {solvent} solvent molecules");
    let mut e = GpuEngine3::new(p, sc.sys.clone(), &sc.state);
    let l = sc.sys.box_f64();
    let masses = sc.sys.masses.clone();
    let k = 2.0 * std::f64::consts::PI / l[2];
    let total_mass: f64 = sc.state.species.iter().map(|&s| masses[s as usize]).sum();
    let rho_m = total_mass / (l[0] * l[1] * l[2]);
    let dt = sc.sys.dt;
    const SLABS: usize = 24;
    let slab = |b: usize| (b as f64 * l[2] / SLABS as f64, (b + 1) as f64 * l[2] / SLABS as f64);
    let (mut sum, mut count) = (0.0, 0);
    for block in 0..(settle + measure) {
        for _ in 0..10 {
            for b in 0..SLABS {
                let (z0, z1) = slab(b);
                e.kick_region([-1.0, -1.0, z0], [l[0] + 1.0, l[1] + 1.0, z1], 1.0, [f * 10.0 * dt * (k * 0.5 * (z0 + z1)).sin(), 0.0, 0.0]);
            }
            e.step(10);
        }
        // Each slab's flow (mass-weighted mean v_x) and peculiar temperature; the mode amplitude.
        let s = block_on(e.read_state());
        let (mut mv, mut m, mut ke, mut cnt) = ([0.0; SLABS], [0.0; SLABS], [0.0; SLABS], [0usize; SLABS]);
        let mut a = 0.0;
        for i in 0..s.species.len() {
            let mi = masses[s.species[i] as usize];
            let z = fixed::to_f64(s.pos[i][2]);
            let b = ((z / l[2] * SLABS as f64) as usize).min(SLABS - 1);
            let v = s.vel[i].map(|x| sc.sys.velocity(x));
            mv[b] += mi * v[0];
            m[b] += mi;
            ke[b] += 0.5 * mi * (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
            cnt[b] += 1;
            a += mi * v[0] * (k * z).sin();
        }
        let a = 2.0 * a / total_mass;
        for b in 0..SLABS {
            let u = mv[b] / m[b];
            let t = (ke[b] - 0.5 * m[b] * u * u) / (1.5 * cnt[b] as f64);
            let sc_ = (1.0 / t).sqrt();
            let (z0, z1) = slab(b);
            e.kick_region([-1.0, -1.0, z0], [l[0] + 1.0, l[1] + 1.0, z1], sc_, [(1.0 - sc_) * u, 0.0, 0.0]);
        }
        if block >= settle {
            sum += a;
            count += 1;
        }
    }
    let amp = sum / count as f64;
    let phi = grains * std::f64::consts::PI / 6.0 * (size + 1.0).powi(3) / (l[0] * l[1] * l[2]);
    (rho_m * f / (amp * k * k), phi)
}

#[test]
#[ignore = "an experiment: the suspension's viscosity, steady forcing"]
fn suspension_viscosity_steady() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let size: f64 = std::env::var("SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(3.0);
    let n: f64 = std::env::var("N").ok().and_then(|s| s.parse().ok()).unwrap_or(16000.0);
    let f: f64 = std::env::var("F").ok().and_then(|s| s.parse().ok()).unwrap_or(0.01);
    let grains: Vec<f64> = std::env::var("GRAINS").map(|s| s.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or(vec![0.0, 27.0, 64.0]);
    let mut eta0 = f64::NAN;
    for g in grains {
        let (eta, phi) = viscosity_steady(&p, n, g, size, f, 100, 400);
        if g == 0.0 {
            eta0 = eta;
        }
        eprintln!("steady: n {n}, {g} grains of {size} σ, φ {phi:.3}: η {eta:.3}, ratio {:.3} (Einstein 1 + 2.5φ = {:.3})", eta / eta0, 1.0 + 2.5 * phi);
    }
}
