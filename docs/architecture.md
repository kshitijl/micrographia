# Architecture (proposal)

Status: draft, 2026-10-19. The rationale and citations for each choice are in
[`research.md`](research.md); section and catalog numbers below (e.g., "catalog #2") refer to
it.

## Goals

1. **A real WebGPU particle simulation engine** as the core of the project. It should be GPU
   resident, run 2D and 3D, and ship with instruments that measure thermodynamics (heat, work,
   entropy bookkeeping), not just display particles. It should be usable on its own by others.
2. **Smaller engines** for questions that particles answer poorly or that need different
   physics: hard-particle Monte Carlo, lattice models, reaction kinetics.
3. **Question-led pages** built on scenarios and instruments, each answering one question from
   [`questions.md`](../questions.md) and, where possible, a documented student misconception
   (research §15).
4. **Quantitative validation** as a first-class product: a headless suite that compares
   simulations with published data (research §11). Selected results become pages.

Non-goals (for now): long-range electrostatics (PME/Ewald), all-atom force fields, multi-GPU,
and supporting browsers without WebGPU beyond a notice.

## Layers

```
pages            question-led pages + validation pages
                 (TypeScript UI, narrative, plots)
---------------------------------------------------------------------------------
scenarios        declarative setups + protocols (serde/JSON), shared by pages AND the
                 validation suite. Example: Carnot = a schedule of piston motion and
                 wall-mode switches.
instruments      observables built on GPU reductions: T, P (wall impulse and virial),
                 heat ledger per wall, ΔS_system / ΔS_bath / ΔS_total, slab profiles,
                 histograms, MSD, PV diagrams
---------------------------------------------------------------------------------
engines          md        particle MD (flagship), 2D/3D, batched replicas
                 hpmc      hard-particle MC (depletion, 2D melting)
                 lattice   Ising / lattice gas / lipid lattice
                 kinetics  reactions (buffers, Arrhenius)
                 md-ref    CPU references: bit-exact twin + f64 ideal integrator
                 (plain-TypeScript "counting" explorables need no engine)
---------------------------------------------------------------------------------
gpu-prims        WGSL + Rust host wrappers: scan, sort, segmented reduce, compaction,
                 load-balancing search, two-word fixed-point accumulation, Threefry
```

### Which engine answers which question

| Engine | Questions |
|---|---|
| md (LJ/WCA, walls, pistons, gravity, membranes) | boiling point vs pressure; triple point; temperature constant during melting/boiling; air holds more water when warm (vapor pressure); average KE; Maxwell vs Boltzmann; pressure falling with height; cold at altitude and flat cloud bases (convection vs an isothermal equilibrium column); Carnot; dS = dQ/T and the sneeze analogy; osmosis (mechanism, no attraction needed); saline IV; salt lowering the melting point; solubility; Brownian motion (1905 track) |
| hpmc | depletion force; 2D hard-disk melting (hexatic phase); entropy-only osmosis |
| lattice | lipid bilayer self-assembly (temperature, acidity); Ising (engine test) |
| kinetics | blood buffers; cooking faster at higher T (Arrhenius) |
| counting/analytic (no engine) | deriving the Boltzmann distribution; is S(E) always increasing (two-level systems, negative T); why Gibbs free energy matters; enthalpy vs energy vs heat |
| compartment model (+ md zoom-in) | kidney concentrating urine (countercurrent multiplier) |

## Language and repository layout

**Engine core in Rust with `wgpu`; kernels in WGSL; pages in TypeScript.** (research §16)

- `wgpu` compiles one host codebase to **WebAssembly** for the browser and to **native**
  (Metal/Vulkan/DX12) for fast headless validation, benchmarks and CI.
- Rust suits the CPU references: explicit wrapping integer ops, IEEE f32, and no implicit
  multiply-add fusion.
- WGSL stays the shader language. rust-gpu is not a dependable path to the web yet.
- Rendering also lives in Rust/wgpu: instanced discs or spheres drawn straight from simulation
  buffers, with no readback. TypeScript handles UI and plots from small asynchronous readbacks.
  Sharing one `GPUDevice` between wasm wgpu and JavaScript is awkward, so keep GPU work on one
  side.

```
Cargo.toml                 workspace
crates/
  gpu-prims/               scan (reduce-then-scan baseline; Decoupled Fallback fast path),
                           radix/counting sort, segmented reduce, compaction, LBS,
                           fixed-point 64-bit accumulation, Threefry; plus tests
  md/                      state layout, integrator, pair tables, cell list + skin list,
                           boundaries, fields, replicas
  md-ref/                  CPU bit-exact twin + f64 ideal reference
  hpmc/  lattice/  kinetics/
  instruments/             reductions → observables, ledgers, profiles
  scenarios/               scenario + protocol schema (serde), presets
  validate/                headless physics validation suite (native wgpu)
  bench/                   benchmark harness; JSON results per commit and device
  wasm/                    wasm-bindgen API for the web
web/
  pages/                   one directory per question page
  lib/                     plotting, controls, shared UI
docs/
```

WGSL needs 2D/3D variants and per-scenario specializations. Use `override` constants where
possible and a small build-time templating step otherwise. Evaluate WESL (WGSL extended with
imports and conditionals) [memory] before writing our own.

## Toolchain and build

| | Native (dev machine, CI, benchmarks) | Browser |
|---|---|---|
| Rust | `cargo build` / `cargo test` | `wasm32-unknown-unknown` + `wasm-bindgen` JS glue |
| GPU API | `wgpu` → Metal / Vulkan / DX12 | `wgpu` → the browser's WebGPU |
| WGSL compiled by | **naga** → MSL / SPIR-V / HLSL, then the driver | the **browser's** compiler (Tint in Chrome) → MSL / HLSL / SPIR-V, then the driver |
| Pages | — | TypeScript + Vite import the wasm package |

- WGSL source ships inside the wasm (`include_str!`). Shaders are compiled **at runtime** when
  pipelines are created (use `createComputePipelineAsync`; Chrome caches compiled pipelines on
  disk). At build time we only **validate** shaders, with naga, and in CI also with Tint if
  practical.
- **Two different shader compilers.** naga and Tint may reassociate and fuse f32 operations
  differently, so `gpu-int-f32` hashes are not expected to match between native and browser
  (consistent with Tier 1). `gpu-int-int` hashes must match everywhere, which makes the
  all-integer path a check that the compilers agree.
- **All readbacks are asynchronous.** The browser cannot block on a buffer map, so the core API
  is async from the start; native code drives the same futures with `pollster`.
- **No f64 in WGSL.** GPU float forces are f32; the f32/f64 switch lives in the CPU engines.
  Double-float emulation on the GPU is a possible experiment, not a planned mode.
- With the WebGPU backend, `wgpu` passes WGSL straight to the browser. Confirm that naga is not
  bundled into the wasm when only WGSL is used, to keep the download small [verify].
- Threads in wasm need cross-origin isolation (COOP/COEP headers). Avoid depending on them; the
  GPU does the work, and the CPU engines in the browser are only for small demos.

## Numerics

### State representation
- **Positions:** fixed-point integers in units `u = L / 2^N` per axis, stored as **64 bits (two
  u32)**. Choose N so that u ≈ 10⁻⁹ σ (e.g., N = 40 for L = 1000 σ). Periodic wrap is a mask;
  minimum image is a sign-extended difference mod 2^N.
- **Velocities:** i32 **displacement per step** in units of u. With u ≈ 10⁻⁹ σ, a thermal step
  (~0.005 σ) is ~5×10⁶ units and the 2³¹ limit is ~2 σ per step. Relative resolution ~10⁻⁷,
  comparable to f32.
- **Drift is exact:** `x ← x + v` with integer add-with-carry and no rounding.
- **Kick:** `v ← v + round(Δt² F(x) / (m u))` is the **only rounding**, and it is a pure
  function of positions.
- **Reversal:** negate v at a half step and the integer leapfrog retraces its path exactly.
  Integer drift needs no symmetric-rounding rule; this replaces the "symmetric rounding" sketch
  in earlier notes.
- **Forces and sums:** accumulate in two-word (64-bit) fixed point with `atomicAdd` and carry
  detection (catalog #2). Totals are independent of summation order. Energies, wall impulses
  and heat ledgers are accumulated the same way.
- **Random numbers:** Threefry keyed by (seed, step, particle, stream) (catalog #4).

Bit budgets (N, the fixed-point scale of forces, the 2D vs 3D layout) are open decisions, to be
settled by the first benchmark.

### Determinism tiers (research §13)
WGSL lets implementations reassociate and fuse f32 operations, and Dawn on macOS uses relaxed
math, so:

- **Tier 1, device-local:** same device + same compiled shader + same inputs → same bits.
  Achieved with f32 force math plus fixed-point accumulation. **Enough for the exact-reversal
  demo on any device.**
- **Tier 2, cross-platform:** GPU ↔ CPU twin, browser ↔ browser, Mac ↔ PC. Requires integer
  force evaluation: integer displacement → integer r² → table index plus fixed-point
  interpolation → fixed-point force (the Anton approach).

Plan: **start with integer positions and f32 forces** (exact drift, Tier 1), with the force
kernel behind an interface. A later plan item implements the **all-integer force path** and
benchmarks it against the f32 path.

### Engine configurations
By the end of the project, all of these should exist behind one interface, built in stages:

| Config | Positions/velocities | Forces | Runs on | Purpose |
|---|---|---|---|---|
| `gpu-int-f32` | integer | f32 | GPU | Default engine. Tier 1: bit-exact on one device, exactly reversible |
| `gpu-int-int` | integer | integer | GPU | Tier 2: bit-exact across devices and browsers. Benchmark against `gpu-int-f32` |
| `cpu-int-int` | integer | integer | CPU | Bit-exact oracle for `gpu-int-int` |
| `cpu-int-float` | integer | f32 or f64 | CPU | Same algorithm as `gpu-int-f32`, with a switchable float type |
| `cpu-float` | f32 or f64 | f32 or f64 | CPU | Conventional float MD. The f64 build is the reference for observables and integrator error; the f32 build shows what fixed point buys |

The CPU float type is a generic parameter, so each CPU engine switches between f32 and f64.
WebGPU has no f64. Native `wgpu` exposes `SHADER_F64` on some backends (not Metal), which could
add an f64 GPU build for native validation runs later.

### Cross-checks between configurations
- **Bitwise reproducibility tests**, wherever a configuration has integer positions:
  - Run-to-run: same config, same device, same seed → identical state hash after N steps.
  - Reversal: N steps forward, flip velocities, N steps back → bit-identical initial state.
    Applies to `gpu-int-f32`, `gpu-int-int`, `cpu-int-int` and `cpu-int-float`, in NVE without
    stochastic walls.
  - `gpu-int-int` == `cpu-int-int` hashes, and identical across devices and browsers (golden
    hashes checked into the repo).
  - A permutation test: shuffling the particle storage order doesn't change the hash (catches
    order-dependent accumulation).
- **Force comparison** on identical configurations (NIST SRSW samples and our snapshots): every
  configuration against `cpu-float` f64, reporting max and rms relative force error and energy
  error.
- **Short-trajectory agreement** before chaotic divergence; the measured divergence rate (a
  Lyapunov exponent) is itself a result.
- **Statistical agreement** of observables (T, P, g(r), coexistence densities, D) within error
  bars.
- **Energy drift vs Δt** for each configuration.

### Pair potentials
- Per-species-pair tables in r² (catalog #5): WCA, LJ (truncated and shifted, cutoff a
  parameter that must match the validation data's convention), Yukawa/Debye–Hückel.
- Tables are loaded into workgroup memory once per workgroup (catalog #17).

### Integrator and ensembles
- NVE leapfrog/velocity Verlet as above.
- **Heat baths as walls, not thermostats:** a wall in thermal mode re-emits particles with
  velocities drawn at the bath temperature, and the energy exchanged is booked exactly in that
  wall's ledger.
- Langevin (with the Threefry RNG) is optional, for Brownian dynamics. No velocity rescaling.
- Boundary types, per wall and per species: periodic, specular, thermal(T), piston (moving
  specular wall at speed u), and membrane (pass or reflect by species).
- Fields: gravity, optional external potentials.

## Neighbor search and force strategies
1. **Cell list:** count, scan, scatter (catalog #8), with cells ordered along a Hilbert or Morton
   curve (catalog #9). Particles are permuted into cell order, which keeps forces deterministic
   because accumulation is order-independent.
2. **Skin list:** rebuild when any particle has moved half the skin (catalog #7, as in ALCHEMI's
   rebuild detection). Later: GROMACS-style dual lists with rolling pruning.
3. **Force kernels**, all behind one interface:
   - (a) baseline: per-particle gather over the skin list, no atomics;
   - (b) cluster tiles of 32 consecutive particles with bounding boxes, plus single pairs for
     sparse tiles (GROMACS/OpenMM/ALCHEMI);
   - (c) load-balancing search over pair candidates plus segmented reduction (moderngpu).
   Benchmark on a uniform liquid and on a **liquid–vapor slab**, since ALCHEMI's grid cost model
   explicitly assumes uniform occupancy.
4. **Fixed-capacity buffers** with overflow flags; the host grows them and reruns (catalog #18).
5. **No dependency on subgroups** (Chrome/Edge only); subgroups are only an optional fast path.
   Scans assume no forward-progress guarantee.

## Batched replicas
Concatenate systems with a CSR pointer, and give each system its own parameters (box,
temperature, piston speed, seed), following ALCHEMI. Uses: Carnot efficiency vs piston speed
(one replica per speed), ensemble averages, fluctuation demos, and validation sweeps over
temperature. A single grid shared across replicas with group IDs is an alternative (Warp
`HashGrid` groups).

## Testing

Almost everything runs under `cargo test` without a browser. Every run is seeded and
deterministic, so tests are reproducible, including the statistical ones.

1. **CPU unit and property tests** (no GPU; run everywhere, including plain CI): fixed-point
   arithmetic, periodic wrap and minimum image, two-word carry logic, pair-table interpolation
   error bounds, Threefry against Random123 known-answer vectors, and the CPU engines themselves.
   Use `proptest` for randomized inputs.
2. **GPU primitive tests** (native `wgpu`): scan, sort, reduce and compaction against CPU
   results. Include edge sizes (0, 1, 255, 256, 257, 2^20 + 3) and adversarial inputs (all
   particles in one cell; a liquid–vapor density contrast).
3. **Engine cross-checks:** everything under "Cross-checks between configurations" above, and
   force-kernel tests against NIST SRSW sample configurations (reference energies and virials).
4. **Physics validation** (`validate`; slow, nightly or on demand): Maxwell–Boltzmann,
   equipartition, barometric profile, B₂(T), NIST coexistence and Clausius–Clapeyron,
   Smit–Frenkel 2D, Engel et al. hard-disk pressure (βP(2σ)² = 9.1706 at η = 0.698),
   van 't Hoff, Izumida–Okuda Carnot, Yeh–Hummer-corrected diffusion. Error bars come from block
   averaging; assert agreement within ~4σ. `cpu-float` in f64 is the reference for integrator
   and precision error.
5. **Scenario and parameter tests:** every scenario is a data file. A smoke test runs each one
   briefly and checks invariants: no overflow flags, no NaN, energy drift within bounds,
   temperature in range. Each page's sliders are tested at their extremes and at random
   combinations, so users can't drive a page into a blowup. Scenarios run the engine's stability
   checks at load time: Δt against the fastest vibration period, and the skin against the largest
   per-step displacement.
6. **Browser tests:**
   - `wasm-bindgen-test` runs layers 2–3 **inside headless Chrome**, so the suites run under Tint
     as well as naga.
   - **Playwright** drives the real pages in Chrome, WebKit (Safari) and Firefox: load, run N
     steps, then compare `gpu-int-int` hashes with the native golden hashes (exact), `gpu-int-f32`
     forces with the reference (within tolerance), and a few screenshots (rendering only).
   - CI without a GPU: software Vulkan (lavapipe) on Linux, WARP on Windows, SwiftShader for
     Chrome. Good enough for correctness, not for speed. Whether GitHub's macOS runners expose a
     usable GPU is unverified, so GPU and performance runs will likely be local or on a
     self-hosted runner.

## Performance and benchmarking

**Metrics:** timesteps per second at fixed N, ns per particle-step, pair interactions per second,
and per-kernel time against the **memory-bandwidth roofline**. Pair-list MD is usually bandwidth
bound, so a fraction of peak bandwidth (e.g., 60–70%) is a target that doesn't depend on other
engines.

**Workloads:**
- LAMMPS `bench/in.lj`: 3D LJ liquid, ρ = 0.8442, cutoff 2.5σ, skin 0.3σ, 32k atoms, scaled up
  to millions.
- HOOMD benchmark equivalents.
- Our 2D liquid–vapor slab, where uniform-density cost models break.
- Many small replicas, where dispatch latency dominates.

**Comparison on the same hardware, with matched settings** (N, density, cutoff, skin, precision):
- Mac: OpenMM (OpenCL, or the community Metal plugin) and the LAMMPS GPU package (OpenCL).
- NVIDIA machine (rented is fine): HOOMD, LAMMPS-KOKKOS and OpenMM CUDA, against our engine
  natively on Vulkan and in Chrome.

**Goal:** within ~1.5–2× of HOOMD for plain LJ on the same NVIDIA GPU; equal or better on
non-uniform density and on many small replicas. The structural costs are no subgroups in
baseline WebGPU, a portable scan without forward-progress guarantees, and browser submit
overhead.

**Rules:**
- Everything GPU resident; many timesteps per command submission; no readback mid-run, only
  asynchronous readback of small summaries.
- Decide neighbor-list rebuilds on the GPU. WebGPU has no conditional dispatch, so either the
  kernels always run and exit early, or the host checks an asynchronous flag every K steps.
- Autotune workgroup size and items per thread per device at startup using `timestamp-query`,
  falling back to defaults (catalog #14, HOOMD `Autotuner`).
- Respect the default limits (research §13): 16 KB workgroup storage, 256 invocations per
  workgroup, 128 MiB per storage binding. Request higher limits when the adapter allows.
- Three device classes: discrete GPU, integrated laptop GPU, phone.

**Workflow:**
- Profile natively, where the tools are good: Xcode Metal capture, Nsight, RenderDoc.
- Confirm in the browser with timestamp queries. Chrome quantizes them by default; a Dawn toggle
  disables that for development [verify flag name].
- A browser benchmark page, to collect numbers from phones and Windows laptops.
- **Regression tracking:** `bench` writes JSON per commit and per device. Take the median of
  many runs after warm-up, and flag regressions beyond each device's measured noise. Start with a
  committed results file; add a dashboard later if it earns its keep.

## Milestones (thin vertical slices first)
- **M0, toolchain + gpu-prims:** the workspace skeleton and an end-to-end toolchain (native
  tests, wasm build, a browser smoke test, benchmark JSON). Then reduce-then-scan, counting sort,
  segmented reduce, two-word atomics and Threefry, tested natively and in the browser.
- **M1, minimal 2D MD:** `gpu-int-f32` with WCA/LJ in NVE, a cell list, a skin list and
  integer positions, plus `cpu-int-float` and `cpu-float` (f32/f64). Run-to-run, reversal and
  permutation hash tests. **First page: exact reversal (Loschmidt) with the one-bit
  perturbation.** First benchmark on the three device classes; choose the bit budgets.
- **M1b, all-integer forces:** `gpu-int-int` and `cpu-int-int`; cross-device golden hashes;
  benchmark against `gpu-int-f32`. Decide whether to make integer forces the default.
- **M2, instruments and boundaries:** T, P, heat ledger, thermal walls, piston. **Page: Carnot
  at finite speed**, validated against Izumida–Okuda; plus the dS = dQ/T ledger page.
- **M3, phase coexistence:** LJ liquid–vapor slab and vapor-pressure curve, validated against
  NIST and Smit–Frenkel. **Pages: boiling point vs pressure; temperature plateau during
  boiling.** Benchmark force strategies (a)/(b)/(c) here.
- **M4, membranes and mixtures:** **Pages: where the osmotic push comes from; saline IV;**
  van 't Hoff validation.
- **M5, hpmc:** hard-disk equation of state vs Engel et al.; **pages: depletion force, 2D
  melting.**
- **M6, 3D:** Brownian motion with the Yeh–Hummer correction; the mW water evaluation.

## Later: exploratory directions

### Quantum chemistry of very small systems
Explore whether a few tiny electronic-structure demos fit the project, as a separate engine
(`qchem`) that shares only the GPU primitives. Possible ladder, from easy to ambitious:

1. **Hydrogen atom:** solve the Schrödinger equation on a 3D grid (imaginary-time propagation
   or a sparse eigen-solver) and show orbitals and energies against the exact −13.6 eV / n².
2. **Helium atom:** two electrons, where electron–electron repulsion first matters. Compare
   Hartree–Fock (−2.862 Hartree) with the exact energy (−2.9037 Hartree); the gap is the
   correlation energy. Diffusion or variational Monte Carlo is a natural fit, because walkers
   are particles and the GPU engine is good at particles.
3. **Water molecule:** show the electron density shifting toward the oxygen and away from the
   hydrogens, the partial charges, and the resulting dipole moment (experiment: 1.85 D).
   Hartree–Fock or simple DFT in a small Gaussian basis.
4. **Water hydrogen bond (most ambitious):** the water dimer. Binding energy ~5 kcal/mol at the
   high-level reference; Hartree–Fock gets the sign right but misses part of it. This ties the
   quantum picture to the classical water models used elsewhere.

All [verify] before use; none of this is scheduled.

## Open decisions
- Bit budgets for positions, velocities and forces; whether the Tier 2 integer force path
  becomes the default.
- Force strategy (a)/(b)/(c), and whether to pick per scenario.
- Shader templating: WESL vs a custom build step.
- Page framework (static site generator, or plain TypeScript and Vite).
- How pages embed engines: one wasm bundle, or per-engine bundles for size.
