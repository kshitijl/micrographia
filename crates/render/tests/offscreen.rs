//! Offscreen rendering (for the recorder): a frame read back has the right size, the
//! background where there is nothing, and particles' colours where they are.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::{init, Pair, System};
use pollster::block_on;

#[test]
fn an_offscreen_frame_shows_the_particles() {
    let gpu = block_on(Gpu::new()).unwrap();
    let p = Prims::new(&gpu);
    let sys = System::new(init::box_for(400, 0.3, 1.0), init::DT, Pair::wca(), vec![1.0]);
    let st = init::fluid(&sys, 400, 1.0, 1);
    let e = GpuEngine::new(&p, sys, &st);
    let (w, h) = (320, 240);
    let mut r = render::Renderer::offscreen(&gpu, &e, &[0.5], w, h);
    r.params.colors[0] = [1.0, 0.0, 0.0, 1.0];
    r.background = wgpu::Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    r.render();
    let px = block_on(r.read_frame());
    assert_eq!(px.len(), (4 * w * h) as usize);
    let (mut red, mut blue) = (0, 0);
    for q in px.chunks_exact(4) {
        if q[0] > 200 && q[2] < 50 {
            red += 1;
        } else if q[2] > 200 && q[0] < 50 {
            blue += 1;
        }
    }
    eprintln!("offscreen: {red} particle pixels, {blue} background pixels of {}", w * h);
    assert!(red > 1000 && blue > 1000);
}
