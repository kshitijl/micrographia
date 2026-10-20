//! Compiled pipelines for the primitives, and "plans" that bind them to particular buffers.
//!
//! A plan allocates its scratch buffers and bind groups once; `encode` records its dispatches
//! into a compute pass, so a whole MD step can live in one pass with no host round trips.

use crate::util::{self, Dispatch, WG};
use crate::Gpu;
use bytemuck::{Pod, Zeroable};

/// Elements handled by one workgroup in reduce and scan.
pub const BLOCK: u32 = 1024;

pub const REDUCE_WGSL: &str = include_str!("../shaders/reduce.wgsl");
pub const SCAN_WGSL: &str = include_str!("../shaders/scan.wgsl");
pub const FILL_WGSL: &str = include_str!("../shaders/fill.wgsl");
pub const BIN_COUNT_WGSL: &str = include_str!("../shaders/bin_count.wgsl");
pub const BIN_SCATTER_WGSL: &str = include_str!("../shaders/bin_scatter.wgsl");
pub const COMPACT_WGSL: &str = include_str!("../shaders/compact.wgsl");
pub const FIXED64_WGSL: &str = include_str!("../shaders/fixed64.wgsl");
pub const THREEFRY_WGSL: &str = include_str!("../shaders/threefry.wgsl");
pub const CHECK_WGSL: &str = include_str!("../shaders/check_kernels.wgsl");
pub const GATE_WGSL: &str = include_str!("../shaders/gate.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Params4 {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
}

impl Params4 {
    pub fn n(n: u32) -> Self {
        Self { a: n, b: 0, c: 0, d: 0 }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scalar {
    U32,
    F32,
}

impl Scalar {
    fn wgsl(self) -> &'static str {
        match self {
            Scalar::U32 => "u32",
            Scalar::F32 => "f32",
        }
    }
}

/// Every shader source in this crate after templating, for offline validation.
pub fn shader_sources() -> Vec<(&'static str, String)> {
    vec![
        ("reduce<u32>", util::template(REDUCE_WGSL, &[("T", "u32")])),
        ("reduce<f32>", util::template(REDUCE_WGSL, &[("T", "f32")])),
        ("scan", SCAN_WGSL.to_string()),
        ("fill", FILL_WGSL.to_string()),
        ("bin_count", BIN_COUNT_WGSL.to_string()),
        ("bin_scatter", BIN_SCATTER_WGSL.to_string()),
        ("compact", COMPACT_WGSL.to_string()),
        ("gate", GATE_WGSL.to_string()),
        ("check", check_source()),
    ]
}

pub fn check_source() -> String {
    format!("{FIXED64_WGSL}\n{THREEFRY_WGSL}\n{CHECK_WGSL}")
}

/// Compiled pipelines, shared by all plans on one device.
#[derive(Clone)]
pub struct Prims {
    pub gpu: Gpu,
    reduce_u32: wgpu::ComputePipeline,
    reduce_f32: wgpu::ComputePipeline,
    scan: wgpu::ComputePipeline,
    fill: wgpu::ComputePipeline,
    bin_count: wgpu::ComputePipeline,
    bin_scatter: wgpu::ComputePipeline,
    compact: wgpu::ComputePipeline,
    gate: wgpu::ComputePipeline,
    /// Bound as `offsets` when a scan level has no block offsets.
    dummy: wgpu::Buffer,
}

impl Prims {
    pub fn new(gpu: &Gpu) -> Self {
        let p = |label: &str, src: &str, entry: &str| {
            let m = util::shader(gpu, label, src);
            util::pipeline(gpu, &m, entry)
        };
        Self {
            gpu: gpu.clone(),
            reduce_u32: p("reduce_u32", &util::template(REDUCE_WGSL, &[("T", "u32")]), "reduce"),
            reduce_f32: p("reduce_f32", &util::template(REDUCE_WGSL, &[("T", "f32")]), "reduce"),
            scan: p("scan", SCAN_WGSL, "scan_blocks"),
            fill: p("fill", FILL_WGSL, "fill"),
            bin_count: p("bin_count", BIN_COUNT_WGSL, "bin_count"),
            bin_scatter: p("bin_scatter", BIN_SCATTER_WGSL, "bin_scatter"),
            compact: p("compact", COMPACT_WGSL, "compact"),
            gate: p("gate", GATE_WGSL, "gate"),
            dummy: util::storage(gpu, "dummy", 16),
        }
    }

    /// A dispatch that sets `n` u32 elements of `buf` to `value`.
    pub fn fill(&self, buf: &wgpu::Buffer, n: u32, value: u32) -> Dispatch {
        let params = util::uniform_init(&self.gpu, "fill", &Params4 { a: n, b: value, c: 0, d: 0 });
        Dispatch::new(&self.gpu, &self.fill, &[buf, &params], util::groups(n, WG))
    }
}

/// Sum of `n` elements; the result is element 0 of `result()`.
pub struct ReducePlan {
    dispatches: Vec<Dispatch>,
    result: wgpu::Buffer,
}

impl ReducePlan {
    pub fn new(prims: &Prims, scalar: Scalar, input: &wgpu::Buffer, n: u32) -> Self {
        let gpu = &prims.gpu;
        let pipeline = match scalar {
            Scalar::U32 => &prims.reduce_u32,
            Scalar::F32 => &prims.reduce_f32,
        };
        let mut dispatches = Vec::new();
        let mut src = input.clone();
        let mut len = n;
        loop {
            let g = util::groups(len, BLOCK);
            let dst = util::storage(gpu, &format!("reduce<{}>", scalar.wgsl()), g as u64 * 4);
            let params = util::uniform_init(gpu, "reduce", &Params4::n(len));
            dispatches.push(Dispatch::new(gpu, pipeline, &[&src, &dst, &params], g));
            src = dst;
            len = g;
            if g == 1 {
                break;
            }
        }
        Self { dispatches, result: src }
    }

    pub fn encode(&self, pass: &mut wgpu::ComputePass<'_>) {
        for d in &self.dispatches {
            d.encode(pass);
        }
    }

    pub fn result(&self) -> &wgpu::Buffer {
        &self.result
    }
}

/// Exclusive prefix sum of `n` u32 values from `input` into `output` (distinct buffers).
pub struct ScanPlan {
    dispatches: Vec<Dispatch>,
}

impl ScanPlan {
    pub fn new(prims: &Prims, input: &wgpu::Buffer, output: &wgpu::Buffer, n: u32) -> Self {
        let mut dispatches = Vec::new();
        Self::build(prims, input, output, n, &mut dispatches);
        Self { dispatches }
    }

    fn build(prims: &Prims, input: &wgpu::Buffer, output: &wgpu::Buffer, n: u32, out: &mut Vec<Dispatch>) {
        let gpu = &prims.gpu;
        let g = util::groups(n, BLOCK);
        if g == 1 {
            let params = util::uniform_init(gpu, "scan", &Params4 { a: n, b: 0, c: 0, d: 0 });
            out.push(Dispatch::new(gpu, &prims.scan, &[input, output, &prims.dummy, &params], 1));
            return;
        }
        // Reduce each block to its total, scan the totals, then scan each block with its offset.
        let totals = util::storage(gpu, "scan totals", g as u64 * 4);
        let reduce_params = util::uniform_init(gpu, "scan reduce", &Params4::n(n));
        out.push(Dispatch::new(gpu, &prims.reduce_u32, &[input, &totals, &reduce_params], g));
        let offsets = util::storage(gpu, "scan offsets", g as u64 * 4);
        Self::build(prims, &totals, &offsets, g, out);
        let params = util::uniform_init(gpu, "scan", &Params4 { a: n, b: 1, c: 0, d: 0 });
        out.push(Dispatch::new(gpu, &prims.scan, &[input, output, &offsets, &params], g));
    }

    pub fn encode(&self, pass: &mut wgpu::ComputePass<'_>) {
        for d in &self.dispatches {
            d.encode(pass);
        }
    }

    pub fn dispatches(&self) -> Vec<Dispatch> {
        self.dispatches.clone()
    }
}

/// Counting sort of item indices by key (unstable: order within a key is unspecified).
///
/// After `encode`: `counts[k]` is the number of items with key k, `starts[k]` the offset of
/// key k's segment in `sorted`, and `sorted` holds item indices grouped by key.
pub struct BinPlan {
    pub counts: wgpu::Buffer,
    pub starts: wgpu::Buffer,
    pub sorted: wgpu::Buffer,
    clear: Dispatch,
    count: Dispatch,
    scan: ScanPlan,
    scatter: Dispatch,
}

impl BinPlan {
    pub fn new(prims: &Prims, keys: &wgpu::Buffer, n: u32, nkeys: u32) -> Self {
        let gpu = &prims.gpu;
        let counts = util::storage(gpu, "bin counts", nkeys as u64 * 4);
        let starts = util::storage(gpu, "bin starts", nkeys as u64 * 4);
        let sorted = util::storage(gpu, "bin sorted", n as u64 * 4);
        let params = util::uniform_init(gpu, "bin", &Params4::n(n));
        let groups = util::groups(n, WG);
        Self {
            clear: prims.fill(&counts, nkeys, 0),
            count: Dispatch::new(gpu, &prims.bin_count, &[keys, &counts, &params], groups),
            scan: ScanPlan::new(prims, &counts, &starts, nkeys),
            scatter: Dispatch::new(gpu, &prims.bin_scatter, &[keys, &starts, &counts, &sorted, &params], groups),
            counts,
            starts,
            sorted,
        }
    }

    pub fn encode(&self, pass: &mut wgpu::ComputePass<'_>) {
        self.clear.encode(pass);
        self.count.encode(pass);
        self.scan.encode(pass);
        // The counts become the scatter cursors, and end up as the counts again.
        self.clear.encode(pass);
        self.scatter.encode(pass);
    }

    /// The dispatches of `encode`, in order.
    pub fn dispatches(&self) -> Vec<Dispatch> {
        let mut v = vec![self.clear.clone(), self.count.clone()];
        v.extend(self.scan.dispatches());
        v.push(self.clear.clone());
        v.push(self.scatter.clone());
        v
    }
}

/// A list of dispatches that runs only if `flags[0]` is nonzero when the gate executes (the
/// gate then clears it). Decided on the GPU through indirect dispatch, so a simulation loop
/// never waits for the host. `stats[0]` counts the times the gate opened.
pub struct Gated {
    gate: Dispatch,
    args: wgpu::Buffer,
    list: Vec<Dispatch>,
}

impl Gated {
    pub fn new(prims: &Prims, flags: &wgpu::Buffer, stats: &wgpu::Buffer, list: Vec<Dispatch>) -> Self {
        let gpu = &prims.gpu;
        let base: Vec<u32> = list.iter().map(|d| d.groups).collect();
        let base_buf = util::storage_init(gpu, "gate base", &base);
        let args = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gate args"),
            size: (list.len().max(1) * 12) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT,
            mapped_at_creation: false,
        });
        let params = util::uniform_init(gpu, "gate", &Params4::n(list.len() as u32));
        let gate = Dispatch::new(gpu, &prims.gate, &[flags, &base_buf, &args, stats, &params], 1);
        Self { gate, args, list }
    }

    pub fn encode(&self, pass: &mut wgpu::ComputePass<'_>) {
        self.gate.encode(pass);
        for (k, d) in self.list.iter().enumerate() {
            pass.set_pipeline(&d.pipeline);
            pass.set_bind_group(0, &d.bind_group, &[]);
            pass.dispatch_workgroups_indirect(&self.args, 12 * k as u64);
        }
    }
}

/// Indices of the flags equal to 1 (flags must be 0 or 1), in increasing order, and their count.
pub struct CompactPlan {
    pub out: wgpu::Buffer,
    pub count: wgpu::Buffer,
    scan: ScanPlan,
    compact: Dispatch,
}

impl CompactPlan {
    pub fn new(prims: &Prims, flags: &wgpu::Buffer, n: u32) -> Self {
        let gpu = &prims.gpu;
        let scanned = util::storage(gpu, "compact scanned", n as u64 * 4);
        let out = util::storage(gpu, "compact out", n as u64 * 4);
        let count = util::storage(gpu, "compact count", 4);
        let params = util::uniform_init(gpu, "compact", &Params4::n(n));
        Self {
            scan: ScanPlan::new(prims, flags, &scanned, n),
            compact: Dispatch::new(gpu, &prims.compact, &[flags, &scanned, &out, &count, &params], util::groups(n, WG)),
            out,
            count,
        }
    }

    pub fn encode(&self, pass: &mut wgpu::ComputePass<'_>) {
        self.scan.encode(pass);
        self.compact.encode(pass);
    }
}
