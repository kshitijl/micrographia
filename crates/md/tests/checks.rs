//! The browser self-checks, run natively.

#[test]
fn browser_checks_pass_natively() {
    let gpu = pollster::block_on(gpu_prims::Gpu::new()).expect("GPU");
    let prims = gpu_prims::Prims::new(&gpu);
    for name in md::checks::NAMES {
        match pollster::block_on(md::checks::run(name, &prims)) {
            Ok(m) => eprintln!("{name}: {m}"),
            Err(e) => panic!("{name}: {e}"),
        }
    }
}
