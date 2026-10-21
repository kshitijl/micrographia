# Performance: where we are, what limits us, and the plan

Written 2026-10-20, in answer to questions from the page review:
- How fast and how big can the MD go?
- Are we winning from the GPU?
- When would the CPU be better, and does it use SIMD?
- What bounds the GPU code?
- Is WebGPU costing us compared with CUDA?

**This document is a plan.** The numbers marked "measured" come from `cargo xtask bench`
(docs/status.md, Performance), natively on the development Mac (Apple GPU, Metal through wgpu).
The rest are estimates, marked as such, to be replaced by measurements. Nothing here has been
built yet. The user asked for it to be written down and done later.

---

## Next steps, first (the user's priorities, 2026-10-20)

Before the rest of this plan:

1. **Decouple the simulation from rendering and host–device traffic.** The simulation runs
   underneath, as fast as the GPU allows; the page samples the GPU state now and then to animate
   (drawing straight from GPU buffers needs no readback) and to read out. The UI offers both:
   slow motion, at a chosen τ per second, or full simulation speed. Then bigger systems and many
   more timesteps, to see real boiling and vapour.
   *Done (`SimHost`, "Full speed"):* two batches in flight, each ~10 ms of GPU work; rendering
   every frame from GPU buffers; readouts five times a second with a short pause of the pump.
   The pot at 6,000 molecules went from 3,400 to 8,000 steps/s, which is now the launch limit
   (§1); at 48,000, 2,000 → 4,000. Faster still at small N needs fewer dispatches per step
   (fusing kernels). Pages whose `afterSteps` does something per call rather than per τ (a
   heater dosing a fixed amount per readout) act less often per τ at full speed; heaters should be
   written per τ, as the latent-heat flask's is.
2. **3D in the GPU engine**, for the demos that need it (Stokes–Einstein Brownian motion, the 3D
   dissertation page, real melting and crystallisation), with no 2D performance regression:
   dimension as a compile-time choice in the shaders, not a runtime branch in the hot loops.
   *Done (first version):* `GpuEngine3` with its own kernels (`md3.wgsl`), so 2D is unchanged:
   the 2D benchmark at the commit before and with 3D, run alternately twice, agreed within 2%
   (4k: 9.7–10.1k steps/s; 16k: 8.7–8.9k; 64k: 6.4–6.5k; 256k: 2.8k). 3D cell list, density
   0.8: LJ 1,810 steps/s at 4k, 1,090 at 16k, 351 at 64k, 99 at 256k (40–140 ns per
   particle-step); WCA 6,810 at 4k, 1,280 at 64k, 344 at 256k. In the browser the 8,788-atom
   crystal page runs ~2,100 steps/s at full speed. A 3D cell list scans 27 cells of side ≥ rc, ~4.6×
   the volume of a Verlet sphere (rc + skin). *Verlet lists in 3D (done):* LJ 4,770 steps/s at 4k
   (2.6×), 2,130 at 16k, 613 at 64k, 175 at 256k (22–52 ns per particle-step); WCA 8,420 at 4k,
   3,050 at 64k (2.4×), 909 at 256k. A smaller skin is faster (0.3: +5%; 0.8: −30%): the force over
   ~80 neighbours, not the rebuild, dominates, and at 64k it reads ~170 MB per step, ~100 GB/s,
   close to the copy bandwidth. Halving that would take half lists with integer atomic kicks
   (still exact, since integer sums do not depend on order). In the browser the crystal page's
   engine runs 2,100 steps/s (was 1,400).
3. **Phase transitions, three ways, all on the GPU:** a lattice gas (checkerboard or Kawasaki
   Monte Carlo), Gibbs-ensemble Monte Carlo, and Shan–Chen lattice Boltzmann. They will be
   compared for explaining and understanding.
4. **Higher particle counts everywhere**, or a control for them on every page: default about
   5,000–20,000.
5. **Interactive changes instead of resets** (CLAUDE.md): for example drop soap molecules in by
   hand.

---

## 1. What we know (measured)

2D, density 0.7, natively on the development Mac:

| N | WCA steps/s | LJ (rc 2.5) steps/s | LJ ns per particle-step |
|---|---|---|---|
| 4,000 | 11k (cell list: 15k) | 10k | 24 |
| 64,000 | 7.2k | 4.9k | 3.2 |
| 256,000 | 3.4k | 1.8k | 2.2 |
| 1,000,000 | 858 | 471 | 2.1 |

- Copy bandwidth on this device: ~167 GB/s.
- About 10 dispatches per step, with 256 steps encoded per command submission.

In simulated time, at Δt = 0.005 τ, one LJ step is 0.005 τ:

| N | τ per wall-clock second (engine flat out) |
|---|---|
| 4,000 | ~50 |
| 64,000 | ~25 |
| 256,000 | ~9 |
| 1,000,000 | ~2.4 |

**Pages run far slower than the engine.** A page steps `stepsPerFrame` (20–1000, set by hand per
page) once per animation frame, and then usually awaits a readback (positions, kinetic
energies, histograms) before the next frame. At 100 steps per frame and 60 frames per second
that is 6,000 steps/s, about 30 τ/s, whatever the engine could do. Worse, the GPU idles while
JavaScript reads back and draws. So the first speed-ups are in the page loop, not the engine
(§4.1).

## 2. A simplified performance model of the GPU engine

Per step, the engine runs a fixed sequence of dispatches:
- integrate (kick, drift, walls);
- check whether the neighbour lists need rebuilding;
- sometimes a rebuild: bin count, scan, scatter, list build;
- every 100 steps a Morton reorder (gather and scatter);
- pair forces;
- bonds, if any;
- piston and ledger work, if any.

Model the time per step as

    t_step ≈ n_dispatch · t_dispatch + max(bytes / BW, flops / F) + t_rebuild / steps_between_rebuilds

with three regimes:

1. **Launch (dispatch) bound, small N.**
   - At N = 4,000, 1/11k s = 90 µs per step over ~10 dispatches, about 9 µs per dispatch,
     independent of N.
   - Below roughly 20,000–60,000 particles nearly all the time is this fixed cost. The cell list
     beats the Verlet list at 4,000 (15k against 11k steps/s) because it needs fewer dispatches.
   - **Estimate:** a 1,000-particle page cannot go much above ~10–15k steps/s on this machine,
     however simple the step.
2. **Memory-bandwidth bound, large N** (the expected case for pair-list MD;
   docs/architecture.md, Performance).
   - Per particle per step, the force kernel reads its neighbour list (~18 entries × 4 bytes at
     LJ rc 2.5 + skin, ρ 0.7) and the neighbours' positions (16 bytes each, partly from cache
     thanks to Morton ordering). Integration adds position, velocity and kick traffic, roughly
     50 bytes.
   - Uncached that is ~400 bytes per particle-step: at 167 GB/s, ~2.4 ns. We measure 2.1 ns at
     1M, so **the large-N engine is near the bandwidth roofline** of this device. The cache is
     already doing some of the work.
3. **Compute bound.** Not reached by plain LJ in f32. It would matter for the all-integer
   force path (64-bit products done with 32-bit operations), many-body potentials or long
   tables.

Two smaller terms:
- **Rebuild cost:** amortised over 10–30 steps by the 0.4 σ skin. Hot or fast systems rebuild
  more often.
- **Latency of readbacks:** each `await` of GPU data costs a round trip (map, copy, wake
  JavaScript) of order 1 ms in browsers, plus the pipeline drains. That is what §1's page loop
  suffers from.

**What the model says to do:**
- For page-sized systems (1k–20k particles), cut dispatches and readbacks, not FLOPs.
- For big systems, cut bytes: half neighbour lists with symmetric force writes, if atomics
  allow; 32-bit relative positions inside cells; fused kernels.

## 3. CPU or GPU?

**Today:** the CPU twins (`md-ref`) were reference engines, written for bit-exact agreement, not
speed: scalar Rust, single-threaded, no explicit SIMD. Pages always use the GPU.

*Done (2026-10-21): a fast pair path in the 2D CPU engine* (`md-ref/src/fast.rs`), used whenever
every pair may interact (no insulating compartments). Bit-exact with the reference loop, and so
with the GPU (tests: `md-ref/tests/fast_path.rs`, and the existing GPU–CPU comparisons):
- a cell list by counting sort; a half shell of neighbour cells, so each pair is computed once
  (its force factor depends only on r², so it is the same from both ends; each end's kick is
  rounded with its own mass coefficient, exactly as the reference rounds it);
- per cell, the neighbourhood gathered into contiguous buffers (shifted across periodic edges),
  padded to whole vectors; per particle, a branch-free loop over it (masked to the entries after
  it), which LLVM vectorises (NEON here: `fdiv.4s`, `frintx.4s`, `fcvtzs.4s`);
- the separation's conversion to float written without a branch (rounding d/16 towards zero,
  then a signed conversion), bit-identical to `sep_f32` (tested over 2M values), so that it
  vectorises too.

Measured, one thread, density 0.7 (`cargo xtask bench --cpu`), ns per particle-step:

| N | LJ fast | LJ reference | WCA fast | WCA reference |
|---|---|---|---|---|
| 1,000 | 100 (10.0k steps/s) | 550 | 94 (10.7k steps/s) | 140 |
| 16,000 | 100 | 499 | 94 | 140 |
| 64,000 | 100 | 461 | – | – |

Against the GPU's 9.7k steps/s at N = 1,000 (launch-bound), the CPU now matches it on one core
(natively). Where the time goes (macOS `sample`): the force kernel ~40% (two f32 divisions per
pair, kept for bit-exactness: a reciprocal would round differently), converting separations ~20%,
gathering neighbourhoods ~15%, scattering kicks ~10%, integration ~12 ns per particle. Bigger
cells do not help (tried: 1.5–6 particles per cell, 93–122 ns). **Next**, for 2× or more:
Verlet lists (about 9 pairs per particle instead of ~28 padded candidates), cluster-pair tiles
(4 × 4 particles per SIMD tile, as GROMACS's nbnxm), and threads.

**Estimates, to be measured:**
- A good CPU implementation of 2D LJ (cell lists, SIMD over neighbours, all cores) should reach
  ~5–20 ns per particle-step on a laptop with 8 cores and 4-wide f32 SIMD. At N = 1,000 that is
  5–20 µs per step: 50k–200k steps/s, beating the GPU's ~10–15k dispatch-bound ceiling by an
  order of magnitude.
- The crossover is probably somewhere around N ~ 5,000–30,000. Above it the GPU's bandwidth
  wins; below it the CPU's lack of launch overhead does.

**In the browser:**
- **SIMD:** WebAssembly SIMD (128-bit, `simd128`) is supported in Chrome, Firefox and Safari.
  Rust targets it with `-C target-feature=+simd128` and `core::arch::wasm32` intrinsics, or
  portable SIMD.
- **Threads:** need `SharedArrayBuffer`, which needs the page served cross-origin isolated (the
  COOP and COEP headers), plus a threads build (`wasm-bindgen-rayon`, nightly `build-std`).
  Static hosting can set those headers.
- Single-threaded SIMD alone is perhaps a 2–4× gain over our scalar code [estimate].

**Determinism:** a fast CPU engine must still match the GPU bit for bit on the integer path
(research.md §13, architecture.md "Determinism tiers"). Integer positions and fixed-point kicks
make SIMD reordering harmless for the integer forces; the f32 path would need a fixed
summation order per neighbour cell.

**Plan:** the engine choice becomes part of `DiscSim::create`.
- Run a short calibration: a few hundred steps of a small and a large system on each engine,
  cached per device in local storage.
- Pick by N, density, features and the measured crossover.
- Pages never choose; the "About this simulation" section says which engine ran.

## 4. Ways to go faster, in order of payoff

### 4.1 The page loop (no engine changes)

- **Pipeline the frames.** Submit the next batch of steps *before* awaiting the previous frame's
  readback, so the GPU never idles. Use double-buffered readback buffers.
- **Steps per frame from a time budget, not a constant.** Measure the GPU time of a batch and pick
  `stepsPerFrame` to fill ~12 ms of each 16 ms frame. Pages ask for a speed in τ per second; the
  host turns it into steps.
- **Read back less, less often.** Most readouts don't need every frame:
  - put cluster counts, profiles and histograms on GPU reductions (gpu-prims already has
    reduce and scan);
  - fetch every n-th frame;
  - never read all positions to the CPU just to count.

  The salt brush and the U-tube energy books both read everything every frame.
- **Expected gain:** 3–10× in simulated time per second on most pages [estimate]. It is the
  cheapest big win.

### 4.2 Fewer, fatter dispatches (small N)

- Fuse integrate and neighbour-check, and fold bonds into the force kernel.
- For tiny systems, one workgroup can run many steps alone. A single dispatch loops K steps with
  workgroup barriers, no global sync needed while N ≤ the workgroup's capacity (256 threads,
  several particles each).
  - This is the "many small replicas" pattern (architecture.md) and could take a
    1,000-particle page from ~10k to ~100k+ steps/s.
  - It needs the 16 KB workgroup-storage limit respected, or global memory with care.

### 4.3 Bytes (large N)

- Store relative positions inside cells as 32-bit values (or f16 offsets) for the force pass.
- Half neighbour lists; tighter skins with cheaper rebuild checks.
- Subgroup operations where available: WebGPU now has a subgroups feature in Chrome [verify
  version]. Use them for reductions and for sharing neighbour data.

### 4.4 Bigger science per FLOP

Choose the method that suits the question (§6): lattice models, hard-particle Monte Carlo
(HPMC), lattice Boltzmann. MD is not always the right tool for a phase transition.

## 5. Is WebGPU leaving performance on the table, compared with CUDA?

**Where WebGPU costs us** (research.md §13):
- **Dispatch and submission overhead.** Validation in the browser, no CUDA Graphs. Mitigated by
  batching (we encode 256 steps per submission) and by 4.2.
- **No subgroups in baseline WebGPU.** Now an optional feature [verify]; HOOMD and GROMACS lean
  on warp-level operations.
- **No 64-bit atomics, no float atomics in workgroup memory.** We use fixed-point integer
  atomics anyway (by design, for determinism), so this costs us little.
- **No forward-progress guarantee.** No persistent kernels and no decoupled look-back scans;
  scans take extra passes.
- **The browser's readback path** (§2). Avoidable by design (§4.1).
- **Limits:** 256 invocations per workgroup, 16 KB workgroup storage, 128 MiB per binding by
  default. Higher limits can be requested.

**Where it doesn't:** bandwidth-bound kernels run on the same silicon at the same bandwidth. Our
1M-particle LJ is already near the device's roofline, so CUDA could not make that kernel much
faster on this hardware. The realistic gap for well-tuned bandwidth-bound MD is perhaps
1.2–2× [estimate; the architecture doc's goal is "within ~1.5–2× of HOOMD on the same NVIDIA
GPU"]. Most of it comes from the launch overhead and the missing subgroups, both shrinking.

**For recording explainer videos on a desktop, CUDA is not needed.** The engine is Rust on wgpu,
and it already runs natively: all the native tests run on Metal. On a Windows or Linux desktop
with an NVIDIA GPU the same code runs on Vulkan or DirectX 12, with no browser overhead and
higher limits. The missing piece is a native recorder:
- an `xtask record <page-or-scenario>` that drives a scenario headless at any resolution and any
  number of steps per frame;
- it renders each frame offscreen with the same renderer and pipes the frames to `ffmpeg`.

Video becomes independent of real-time speed: a 10⁶-particle boiling movie at 4K can take as
long as it takes. Everything stays one code base, and the browser stays the place anyone can run
it. A CUDA port would duplicate the engine and give up the cross-browser determinism.

## 6. Faster or better methods for phase transitions (question 21 of the review)

The liquid–gas transition is a hard case for 2D MD:
- the vapour is dilute (slow evaporation, few molecules to count);
- the liquid range of 2D LJ is narrow;
- interfaces are noisy, and coexistence needs big boxes and long runs.

Alternatives, each good at a different part of the story:

| Method | Good for | Speed |
|---|---|---|
| **Lattice gas** (the Ising model in particle language), Monte Carlo on the GPU (checkerboard Metropolis, or Kawasaki exchange to conserve particles) | coexistence, the critical point (exact in 2D: Onsager), critical opalescence, nucleation, coarsening, the shape of the coexistence curve | 10⁶–10⁷ sites at interactive rates, ~10⁹ updates/s on a GPU [estimate]; already in the plan as engine `lattice` (architecture.md) |
| **Gibbs-ensemble Monte Carlo** (two boxes exchanging volume and particles, no interface; [FS] §6.6) | the coexistence densities and the vapour pressure directly | fast; one number per temperature |
| **Lattice Boltzmann with a pseudopotential (Shan–Chen)** | boiling with bubbles, condensation, droplets, contact angles, at scales MD can't reach | very fast on the GPU (fluid-engine.md) |
| **Phase-field / Cahn–Hilliard** (a continuum order parameter) | spinodal decomposition, coarsening, the same physics as the oil–water page at large scale | very fast; pure diffusion of a field |
| **Event-driven or hard-particle MC** | melting of hard disks (the hexatic phase), entropy-driven transitions | engine `hpmc` in the plan |

**Recommendation:** keep MD for "what the molecules do" (escaping a surface, latent heat as broken
bonds, superheating). Add a **GPU lattice gas** for the phase diagram, the critical point and
nucleation, where it is orders of magnitude faster and exactly solvable as a check. Add
**Shan–Chen lattice Boltzmann** later for boiling with real bubbles. The lattice gas is the
cheapest of these and doubles as the engine test the architecture already wanted (Ising).

## 7. The work, when we do it

1. **Benchmarks, properly:**
   - a browser benchmark page, alongside the native `xtask bench`, covering steps/s and ns per
     particle-step for N = 500 … 10⁶ with WCA, LJ, bonds and walls;
   - per-kernel GPU timestamps (timestamp queries; Chrome quantises them, so use the Dawn toggle
     in development);
   - results logged per device to `bench/results.jsonl`;
   - also run on an integrated laptop GPU, a Windows NVIDIA desktop and a phone.
2. **The performance model:** fit t_dispatch, effective bandwidth and rebuild cost per device
   from (1), and publish it in this document. Page authors then read off how big and how fast a
   page can be.
3. **The page loop (§4.1):** pipelining, time-budgeted steps per frame, GPU-side readouts. Measure
   τ/s before and after on every page.
4. **A fast CPU engine:** cell lists, `simd128`, threads where cross-origin isolation allows; the
   integer path bit-exact with the GPU. Then the automatic CPU/GPU choice (§3).
5. **Small-N single-workgroup stepping (§4.2)** and **bandwidth work for large N (§4.3)**, guided
   by (2).
6. **The native recorder (§5).**
7. **The GPU lattice gas (§6)** for phase transitions.

**What this decides.** Which phenomena are within reach in a browser:
- phase coexistence and nucleation need ~10⁵ particles for ~10⁴ τ;
- self-assembly needs ~10⁴–10⁵ for ~10⁴ τ;
- 2D melting needs ~10⁵–10⁶ hard disks;
- proteins (coarse-grained) need ~10³–10⁴ beads for ~10⁶ τ.

At today's ~25 τ/s for 64k particles, 10⁴ τ is about 7 minutes. That is borderline, and worth
the work above.
