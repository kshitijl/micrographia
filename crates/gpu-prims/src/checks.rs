//! Self-checks of the GPU primitives against CPU results.
//!
//! The same async functions run under native `cargo test` (naga) and inside the browser via
//! the wasm crate (the browser's shader compiler), so both toolchains are covered.

use crate::fixed64;
use crate::prims::{BinPlan, CompactPlan, Params4, Prims, ReducePlan, Scalar, ScanPlan};
use crate::threefry;
use crate::util::{self, Dispatch, WG};

pub type CheckResult = Result<String, String>;

/// A tiny deterministic generator for test data.
pub struct SplitMix(pub u64);

impl SplitMix {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u32) -> u32 {
        (self.next_u64() % n as u64) as u32
    }
}

/// Sizes around the block boundaries, plus one large odd size that needs two scan levels.
pub const SIZES: &[u32] = &[1, 2, 255, 256, 257, 1023, 1024, 1025, 4097, 65_537, (1 << 20) + 3];

pub const NAMES: &[&str] = &["reduce_u32", "reduce_f32", "scan", "bin", "bin_one_key", "compact", "atomic64", "threefry"];

pub async fn run(name: &str, prims: &Prims) -> CheckResult {
    match name {
        "reduce_u32" => reduce_u32(prims).await,
        "reduce_f32" => reduce_f32(prims).await,
        "scan" => scan(prims).await,
        "bin" => bin(prims, false).await,
        "bin_one_key" => bin(prims, true).await,
        "compact" => compact(prims).await,
        "atomic64" => atomic64(prims).await,
        "threefry" => threefry_check(prims).await,
        _ => Err(format!("unknown check {name}")),
    }
}

fn submit(prims: &Prims, f: impl FnOnce(&mut wgpu::ComputePass<'_>)) {
    let gpu = &prims.gpu;
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_compute_pass(&Default::default());
        f(&mut pass);
    }
    gpu.queue.submit([enc.finish()]);
}

pub async fn reduce_u32(prims: &Prims) -> CheckResult {
    let mut rng = SplitMix(1);
    for &n in SIZES {
        let data: Vec<u32> = (0..n).map(|_| rng.next_u64() as u32).collect();
        let input = util::storage_init(&prims.gpu, "in", &data);
        let plan = ReducePlan::new(prims, Scalar::U32, &input, n);
        submit(prims, |p| plan.encode(p));
        let got = util::read::<u32>(&prims.gpu, plan.result(), 1).await[0];
        let want = data.iter().fold(0u32, |a, &b| a.wrapping_add(b));
        if got != want {
            return Err(format!("n={n}: got {got}, want {want}"));
        }
    }
    Ok(format!("{} sizes up to {}", SIZES.len(), SIZES.last().unwrap()))
}

pub async fn reduce_f32(prims: &Prims) -> CheckResult {
    let mut rng = SplitMix(2);
    let mut worst = 0.0f64;
    for &n in SIZES {
        let data: Vec<f32> = (0..n).map(|_| (rng.next_u64() >> 40) as f32 / (1u64 << 24) as f32).collect();
        let input = util::storage_init(&prims.gpu, "in", &data);
        let plan = ReducePlan::new(prims, Scalar::F32, &input, n);
        submit(prims, |p| plan.encode(p));
        let got = util::read::<f32>(&prims.gpu, plan.result(), 1).await[0] as f64;
        let want: f64 = data.iter().map(|&x| x as f64).sum();
        let rel = ((got - want) / want).abs();
        worst = worst.max(rel);
        if rel > 1e-5 {
            return Err(format!("n={n}: got {got}, want {want} (rel {rel:e})"));
        }
    }
    Ok(format!("worst relative error {worst:.1e}"))
}

pub async fn scan(prims: &Prims) -> CheckResult {
    let mut rng = SplitMix(3);
    for &n in SIZES {
        let data: Vec<u32> = (0..n).map(|_| rng.below(17)).collect();
        let input = util::storage_init(&prims.gpu, "in", &data);
        let output = util::storage(&prims.gpu, "out", n as u64 * 4);
        let plan = ScanPlan::new(prims, &input, &output, n);
        submit(prims, |p| plan.encode(p));
        let got = util::read::<u32>(&prims.gpu, &output, n as usize).await;
        let mut run = 0u32;
        for i in 0..n as usize {
            if got[i] != run {
                return Err(format!("n={n}: index {i}: got {}, want {run}", got[i]));
            }
            run = run.wrapping_add(data[i]);
        }
    }
    Ok(format!("exclusive scan, {} sizes", SIZES.len()))
}

pub async fn bin(prims: &Prims, one_key: bool) -> CheckResult {
    let mut rng = SplitMix(4);
    for &(n, nkeys) in &[(1u32, 1u32), (1000, 7), (100_000, 50_000), (300_001, 1 << 18)] {
        let keys: Vec<u32> = (0..n).map(|_| if one_key { nkeys - 1 } else { rng.below(nkeys) }).collect();
        let kbuf = util::storage_init(&prims.gpu, "keys", &keys);
        let plan = BinPlan::new(prims, &kbuf, n, nkeys);
        submit(prims, |p| plan.encode(p));
        let counts = util::read::<u32>(&prims.gpu, &plan.counts, nkeys as usize).await;
        let starts = util::read::<u32>(&prims.gpu, &plan.starts, nkeys as usize).await;
        let sorted = util::read::<u32>(&prims.gpu, &plan.sorted, n as usize).await;
        let mut want_counts = vec![0u32; nkeys as usize];
        for &k in &keys {
            want_counts[k as usize] += 1;
        }
        if counts != want_counts {
            return Err(format!("n={n}: counts differ"));
        }
        let mut seen = vec![false; n as usize];
        let mut start = 0u32;
        for k in 0..nkeys as usize {
            if starts[k] != start {
                return Err(format!("n={n}: start of key {k} is {}, want {start}", starts[k]));
            }
            for s in start..start + counts[k] {
                let item = sorted[s as usize] as usize;
                if item >= n as usize || seen[item] || keys[item] as usize != k {
                    return Err(format!("n={n}: bad item {item} in segment of key {k}"));
                }
                seen[item] = true;
            }
            start += counts[k];
        }
    }
    Ok(if one_key { "all items share one key".into() } else { "random keys, up to 2^18 bins".into() })
}

pub async fn compact(prims: &Prims) -> CheckResult {
    let mut rng = SplitMix(5);
    for &n in SIZES {
        let flags: Vec<u32> = (0..n).map(|_| (rng.below(3) == 0) as u32).collect();
        let fbuf = util::storage_init(&prims.gpu, "flags", &flags);
        let plan = CompactPlan::new(prims, &fbuf, n);
        submit(prims, |p| plan.encode(p));
        let want: Vec<u32> = (0..n).filter(|&i| flags[i as usize] != 0).collect();
        let count = util::read::<u32>(&prims.gpu, &plan.count, 1).await[0];
        if count as usize != want.len() {
            return Err(format!("n={n}: count {count}, want {}", want.len()));
        }
        let got = util::read::<u32>(&prims.gpu, &plan.out, n as usize).await;
        if got[..want.len()] != want[..] {
            return Err(format!("n={n}: compacted indices differ"));
        }
    }
    Ok(format!("{} sizes", SIZES.len()))
}

struct CheckKernels {
    atomic64: wgpu::ComputePipeline,
    threefry: wgpu::ComputePipeline,
}

impl CheckKernels {
    fn new(prims: &Prims) -> Self {
        let m = util::shader(&prims.gpu, "check", &crate::prims::check_source());
        Self {
            atomic64: util::pipeline(&prims.gpu, &m, "check_atomic64"),
            threefry: util::pipeline(&prims.gpu, &m, "check_threefry"),
        }
    }
}

pub async fn atomic64(prims: &Prims) -> CheckResult {
    let gpu = &prims.gpu;
    let k = CheckKernels::new(prims);
    let mut rng = SplitMix(6);
    let (n, m) = (200_003u32, 7u32);
    let vals: Vec<i64> = (0..n).map(|_| rng.next_u64() as i64 >> rng.below(40)).collect();
    let words: Vec<[u32; 2]> = vals.iter().map(|&v| fixed64::split(v)).collect();
    let vbuf = util::storage_init(gpu, "values", &words);
    let acc = util::storage(gpu, "acc", m as u64 * 8);
    let out = util::storage(gpu, "out", n as u64 * 8);
    let params = util::uniform_init(gpu, "p", &Params4 { a: n, b: m, c: 0, d: 0 });
    let d = Dispatch::new(gpu, &k.atomic64, &[&vbuf, &acc, &out, &params], util::groups(n, WG));
    util::run(gpu, &[&d]);
    let got = util::read::<[u32; 2]>(gpu, &acc, m as usize).await;
    for slot in 0..m as usize {
        let want = vals.iter().skip(slot).step_by(m as usize).fold(0i64, |a, &b| a.wrapping_add(b));
        if fixed64::join(got[slot]) != want {
            return Err(format!("slot {slot}: got {}, want {want}", fixed64::join(got[slot])));
        }
    }
    let arith = util::read::<[u32; 2]>(gpu, &out, n as usize).await;
    if let Some(i) = arith.iter().position(|w| fixed64::join(*w) != 7) {
        return Err(format!("add64/sub64/neg64 identity failed at {i}: {}", fixed64::join(arith[i])));
    }
    Ok(format!("{n} signed 64-bit atomic adds into {m} slots; add/sub/neg identities"))
}

pub async fn threefry_check(prims: &Prims) -> CheckResult {
    let gpu = &prims.gpu;
    let k = CheckKernels::new(prims);
    let mut rng = SplitMix(7);
    let n = 65_536u32;
    let key = [0x243f_6a88u32, 0x85a3_08d3];
    let ctrs: Vec<[u32; 2]> = (0..n).map(|_| [rng.next_u64() as u32, rng.next_u64() as u32]).collect();
    let cbuf = util::storage_init(gpu, "ctrs", &ctrs);
    let out = util::storage(gpu, "out", n as u64 * 8);
    let params = util::uniform_init(gpu, "p", &Params4 { a: n, b: 1, c: key[0], d: key[1] });
    let d = Dispatch::new_at(gpu, &k.threefry, &[(0, &cbuf), (2, &out), (3, &params)], util::groups(n, WG));
    util::run(gpu, &[&d]);
    let got = util::read::<[u32; 2]>(gpu, &out, n as usize).await;
    for i in 0..n as usize {
        let want = threefry::threefry2x32(ctrs[i], key);
        if got[i] != want {
            return Err(format!("counter {:x?}: got {:x?}, want {want:x?}", ctrs[i], got[i]));
        }
    }
    Ok(format!("{n} draws match the CPU twin bit for bit"))
}
