//! Native GPU tests of the primitives (the same checks also run in the browser).

use gpu_prims::{checks, Gpu, Prims};

fn prims() -> Prims {
    let gpu = pollster::block_on(Gpu::new()).expect("a GPU adapter is required for these tests");
    Prims::new(&gpu)
}

fn check(name: &str) {
    let p = prims();
    match pollster::block_on(checks::run(name, &p)) {
        Ok(msg) => eprintln!("{name}: {msg}"),
        Err(e) => panic!("{name} failed on {}: {e}", p.gpu.describe()),
    }
}

#[test]
fn reduce_u32() {
    check("reduce_u32");
}
#[test]
fn reduce_f32() {
    check("reduce_f32");
}
#[test]
fn scan() {
    check("scan");
}
#[test]
fn bin() {
    check("bin");
}
#[test]
fn bin_one_key() {
    check("bin_one_key");
}
#[test]
fn compact() {
    check("compact");
}
#[test]
fn atomic64() {
    check("atomic64");
}
#[test]
fn threefry() {
    check("threefry");
}
#[test]
fn gate() {
    check("gate");
}
