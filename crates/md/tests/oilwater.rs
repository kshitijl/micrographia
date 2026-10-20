//! Oil and water under gravity: does the lighter oil collect on top, and how fast?

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

#[test]
#[ignore]
fn oil_rises() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for (g, rate, m) in [(0.03, 0.02, 0.5), (0.05, 0.02, 0.5)] {
        let o = Options::default().with("n", 8000.0).with("cross", 0.3).with("temperature", 0.7).with("gravity", g).with("oil_mass", m).with("bath_rate", rate).with("seed", 6.0);
        let sc = build("oilwater", &o).unwrap();
        let h = fixed::to_f64(sc.sys.box_len[1]);
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let mut line = String::new();
        for k in 0..6 {
            e.step(40000);
            let s = block_on(e.read_state());
            let (mut top, mut all) = (0.0, 0.0);
            for i in 0..s.n() {
                if s.species[i] == 1 {
                    all += 1.0;
                    if fixed::to_f64(s.pos[i][1]) > h / 2.0 {
                        top += 1.0;
                    }
                }
            }
            line += &format!(" {}τ:{:.2}", (k + 1) * 200, top / all);
        }
        eprintln!("g {g}, bath rate {rate}, oil mass {m}: oil share in the top half{line}");
    }
}
