//! Smoluchowski's trapdoor: does a one-way door pump gas?

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// (molecules on the left, on the right, gas kT) now.
fn sides(e: &GpuEngine) -> (f64, f64, f64) {
    let s = block_on(e.read_state());
    let xm = fixed::to_f64(e.sys.box_len[0]) / 2.0;
    let (mut l, mut r, mut ke) = (0.0, 0.0, 0.0);
    for i in 0..s.n() {
        if s.species[i] == 0 {
            if fixed::to_f64(s.pos[i][0]) < xm { l += 1.0 } else { r += 1.0 }
            let v = s.vel[i].map(|x| e.sys.velocity(x));
            ke += 0.5 * (v[0] * v[0] + v[1] * v[1]);
        }
    }
    (l, r, ke / (l + r))
}

#[test]
#[ignore]
fn trapdoor() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for door_t in [1.0, 0.05] {
        let o = Options::default().with("door_t", door_t).with("gas_t", 1.0).with("spring", 0.2).with("door_mass", 4.0).with("door", 3.5).with("pore", 3.0).with("density", 0.2);
        let sc = build("demon", &o).unwrap();
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        e.step(100000);
        let (mut l, mut r, mut t, mut n) = (0.0, 0.0, 0.0, 0.0);
        let mut line = String::new();
        for k in 0..100 {
            e.step(4000);
            let (a, b, c) = sides(&e);
            l += a;
            r += b;
            t += c;
            n += 1.0;
            if k % 20 == 19 {
                line += &format!(" {:.0}/{:.0}", l / n, r / n);
            }
        }
        eprintln!("door kT {door_t}: running mean left/right{line}; ratio {:.3}, gas kT {:.2}", r / l, t / n);
    }
}
