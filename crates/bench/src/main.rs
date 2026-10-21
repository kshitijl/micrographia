//! Native benchmarks. Prints a table and appends one JSON line per case to
//! `bench/results.jsonl` (commit, device, case, results), so performance can be tracked over
//! commits and devices.
//!
//! Usage: `cargo xtask bench [--quick] [--sizes 4000,64000] [--density 0.7] [--pair wca|lj]
//!                           [--no-save] [--cpu] [--3d]`
//!
//! `--3d` benchmarks the 3D engine (cell list) instead, on an LJ or WCA fluid (density 0.8
//! unless given).

use gpu_prims::timing::PassTimer;
use gpu_prims::util::{self, Dispatch, WG};
use gpu_prims::{Gpu, Prims};
use md::{ForceMode, GpuEngine};
use md_core::{init, Pair, System};
use std::io::Write;
use std::time::Instant;

struct Args {
    sizes: Vec<usize>,
    density: f64,
    pair: Pair,
    pair_name: String,
    steps: usize,
    save: bool,
    cpu: bool,
    /// Randomly permute particle storage order (what a long run without re-sorting looks like).
    shuffle: bool,
    /// Cell list every step instead of Verlet lists.
    cells: bool,
    /// All-integer forces (gpu-int-int).
    int: bool,
    skin: Option<f64>,
    three: bool,
    density_given: bool,
}

fn parse() -> Args {
    let mut a = Args {
        sizes: vec![4_000, 16_000, 64_000, 256_000, 1_000_000],
        density: 0.7,
        pair: Pair::wca(),
        pair_name: "wca".into(),
        steps: 2000,
        save: true,
        cpu: false,
        shuffle: false,
        cells: false,
        int: false,
        skin: None,
        three: false,
        density_given: false,
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--quick" => {
                a.sizes = vec![4_000, 64_000];
                a.steps = 500;
            }
            "--sizes" => {
                i += 1;
                a.sizes = argv[i].split(',').map(|s| s.parse().expect("size")).collect();
            }
            "--density" => {
                i += 1;
                a.density = argv[i].parse().expect("density");
                a.density_given = true;
            }
            "--pair" => {
                i += 1;
                a.pair_name = argv[i].clone();
                a.pair = match argv[i].as_str() {
                    "wca" => Pair::wca(),
                    "lj" => Pair::lj(2.5),
                    other => panic!("unknown pair {other}"),
                };
            }
            "--steps" => {
                i += 1;
                a.steps = argv[i].parse().expect("steps");
            }
            "--no-save" => a.save = false,
            "--cpu" => a.cpu = true,
            "--shuffle" => a.shuffle = true,
            "--cells" => a.cells = true,
            "--int" => a.int = true,
            "--3d" => a.three = true,
            "--skin" => {
                i += 1;
                a.skin = Some(argv[i].parse().expect("skin"));
            }
            other => panic!("unknown argument {other}"),
        }
        i += 1;
    }
    a
}

fn commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn json_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Achievable device bandwidth: GB/s for a read+write copy of 64 MiB.
fn copy_bandwidth(gpu: &Gpu) -> f64 {
    let n: u32 = 4 << 20; // vec4<u32> elements = 64 MiB
    let src = util::storage(gpu, "src", n as u64 * 16);
    let dst = util::storage(gpu, "dst", n as u64 * 16);
    let m = util::shader(gpu, "copy", include_str!("../shaders/copy.wgsl"));
    let p = util::pipeline(gpu, &m, "copy");
    let d = Dispatch::new(gpu, &p, &[&src, &dst], n / WG);
    let reps = 50;
    let run = || {
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            for _ in 0..reps {
                d.encode(&mut pass);
            }
        }
        gpu.queue.submit([enc.finish()]);
        gpu.wait();
    };
    run();
    let t = Instant::now();
    run();
    let secs = t.elapsed().as_secs_f64();
    (2.0 * n as f64 * 16.0 * reps as f64) / secs / 1e9
}

struct CaseResult {
    n: usize,
    steps_per_s: f64,
    ns_per_particle_step: f64,
    phase_ns: Vec<(&'static str, f64)>,
    rebuilds: u32,
}

fn md_case(prims: &Prims, args: &Args, n: usize) -> CaseResult {
    let gpu = &prims.gpu;
    let sys = System::new(init::box_for(n, args.density, 1.0), init::DT, args.pair, vec![1.0]);
    let mut s0 = init::fluid(&sys, n, 1.0, 1);
    if args.shuffle {
        let mut rng = gpu_prims::checks::SplitMix(99);
        let mut perm: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            perm.swap(i, rng.below(i as u32 + 1) as usize);
        }
        s0 = s0.permuted(&perm);
    }
    let mode = match (args.cells, args.skin) {
        (true, _) => ForceMode::CellList,
        (false, None) => ForceMode::verlet_for(&sys, n),
        (false, Some(skin)) => ForceMode::verlet_with_skin(&sys, n, skin),
    };
    let arith = if args.int { md::Arith::Int } else { md::Arith::F32 };
    let mut e = GpuEngine::with_options(prims, sys, &s0, mode, arith);
    // Warm up (and leave the lattice start behind).
    e.step(300);
    gpu.wait();
    let t = Instant::now();
    e.step(args.steps);
    gpu.wait();
    let secs = t.elapsed().as_secs_f64();
    let steps_per_s = args.steps as f64 / secs;

    // Per-phase GPU time: each phase in its own timed pass.
    let mut phase_ns = Vec::new();
    let timed_steps = 20u32;
    let names = e.phase_names();
    if let Some(mut timer) = PassTimer::new(gpu, timed_steps * names.len() as u32) {
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        for _ in 0..timed_steps {
            for (k, name) in names.iter().enumerate() {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(name), timestamp_writes: Some(timer.next()) });
                e.encode_phase(&mut pass, k);
            }
        }
        timer.resolve(&mut enc);
        gpu.queue.submit([enc.finish()]);
        let ns = pollster::block_on(timer.read_ns());
        for (k, name) in names.iter().enumerate() {
            // Mean, not median: list rebuilds happen only on some steps.
            let mean = (0..timed_steps as usize).map(|s| ns[s * names.len() + k]).sum::<f64>() / timed_steps as f64;
            phase_ns.push((*name, mean));
        }
    }
    let (rebuilds, overflow) = pollster::block_on(e.buf.read_list_stats());
    assert!(!overflow, "neighbour list overflowed");
    CaseResult { n, steps_per_s, ns_per_particle_step: 1e9 / (steps_per_s * n as f64), phase_ns, rebuilds }
}

/// The 3D engine on a fluid melted from an FCC lattice.
fn md3_case(prims: &Prims, args: &Args, n: usize) -> CaseResult {
    use md_core::three;
    let gpu = &prims.gpu;
    let sys = three::System3::new(three::cube_for(n, args.density), init::DT, args.pair, vec![1.0]);
    let species = vec![0; n];
    let s0 = three::State3 { pos: three::fcc(n, sys.box_len, three::fcc_cells(n, sys.box_len), 0.05, 1), vel: three::maxwell(&sys, &species, 1.0, 1), species };
    let mode = match (args.cells, args.skin) {
        (true, _) => ForceMode::CellList,
        (false, None) => md::engine3::verlet3(&sys, n),
        (false, Some(skin)) => md::engine3::verlet3_with_skin(&sys, n, skin),
    };
    let mut e = md::GpuEngine3::with_mode(prims, sys, &s0, mode);
    e.step(300);
    gpu.wait();
    let t = Instant::now();
    e.step(args.steps);
    gpu.wait();
    let steps_per_s = args.steps as f64 / t.elapsed().as_secs_f64();
    let (rebuilds, overflow) = pollster::block_on(e.buf.read_list_stats());
    assert!(!overflow, "neighbour list overflowed");
    CaseResult { n, steps_per_s, ns_per_particle_step: 1e9 / (steps_per_s * n as f64), phase_ns: vec![], rebuilds }
}

fn main() {
    let mut args = parse();
    if args.three && !args.density_given {
        args.density = 0.8;
    }
    let gpu = pollster::block_on(Gpu::new()).expect("GPU");
    let prims = Prims::new(&gpu);
    let device = gpu.describe();
    let commit = commit();
    let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    println!("device: {device}   commit: {commit}");
    let bw = copy_bandwidth(&gpu);
    println!("copy bandwidth: {bw:.0} GB/s (read + write)");
    let mode_name = if args.cells { "cells" } else { "verlet" };
    println!(
        "\n{}, {} {} fluid, density {}, {} initial storage order, {}",
        if args.int { "gpu-int-int" } else { "gpu-int-f32" },
        if args.three { "3D" } else { "2D" },
        args.pair_name,
        args.density,
        if args.shuffle { "shuffled" } else { "lattice" },
        if args.cells { "cell list every step" } else { "Verlet lists (skin 0.4)" }
    );
    let mut lines = Vec::new();
    let mut header = false;
    for &n in &args.sizes {
        let r = if args.three { md3_case(&prims, &args, n) } else { md_case(&prims, &args, n) };
        if !header {
            let cols: String = r.phase_ns.iter().map(|(k, _)| format!(" {:>10}", k)).collect();
            println!("{:>9} {:>11} {:>12} {:>9}{cols}", "N", "steps/s", "ns/(N·step)", "rebuilds");
            header = true;
        }
        let cols: String = r.phase_ns.iter().map(|(_, v)| format!(" {:>9.1}µ", v / 1e3)).collect();
        println!("{:>9} {:>11.0} {:>12.2} {:>9}{cols}", r.n, r.steps_per_s, r.ns_per_particle_step, r.rebuilds);
        let phases = r.phase_ns.iter().map(|(k, v)| format!("{}:{:.0}", json_str(k), v)).collect::<Vec<_>>().join(",");
        lines.push(format!(
            "{{\"time\":{when},\"commit\":{},\"device\":{},\"engine\":{},\"mode\":{},\"order\":{},\"dim\":{},\"pair\":{},\"density\":{},\"n\":{},\
             \"steps_per_s\":{:.1},\"ns_per_particle_step\":{:.3},\"phase_ns\":{{{phases}}},\"copy_gbps\":{bw:.1}}}",
            json_str(&commit),
            json_str(&device),
            json_str(if args.int { "gpu-int-int" } else { "gpu-int-f32" }),
            json_str(mode_name),
            json_str(if args.shuffle { "shuffled" } else { "lattice" }),
            if args.three { 3 } else { 2 },
            json_str(&args.pair_name),
            args.density,
            r.n,
            r.steps_per_s,
            r.ns_per_particle_step
        ));
    }
    if args.cpu {
        // The CPU engine, one thread: its fast pair path, and the reference loop (bit-identical).
        println!("\ncpu-int-float<f32>, one thread, {} fluid, density {}", args.pair_name, args.density);
        println!("{:>9} {:>12} {:>12} {:>12} {:>12}", "N", "fast st/s", "ns/(N·step)", "ref st/s", "ns/(N·step)");
        for &n in &args.sizes {
            let sys = System::new(init::box_for(n, args.density, 1.0), init::DT, args.pair, vec![1.0]);
            let run = |fast: bool| {
                let mut e = md_ref::IntEngine::<f32>::new(sys.clone(), init::fluid(&sys, n, 1.0, 1));
                e.fast = fast;
                e.step(5);
                let steps = (2_000_000 / n).clamp(5, 2000);
                let t = Instant::now();
                e.step(steps);
                steps as f64 / t.elapsed().as_secs_f64()
            };
            let f = run(true);
            let r = if n <= 64_000 { run(false) } else { f64::NAN };
            println!("{n:>9} {f:>12.0} {:>12.1} {r:>12.0} {:>12.1}", 1e9 / (f * n as f64), 1e9 / (r * n as f64));
        }
    }
    if args.save {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/results.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
        for l in lines {
            writeln!(f, "{l}").unwrap();
        }
        println!("\nappended to bench/results.jsonl");
    }
}
