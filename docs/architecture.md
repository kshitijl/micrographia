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
  validate/                headless validation suite + benchmarks (native wgpu)
  wasm/                    wasm-bindgen API for the web
web/
  pages/                   one directory per question page
  lib/                     plotting, controls, shared UI
docs/
```

WGSL needs 2D/3D variants and per-scenario specializations. Use `override` constants where
possible and a small build-time templating step otherwise. Evaluate WESL (WGSL extended with
imports and conditionals) [memory] before writing our own.

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

Plan: build Tier 1 first, with the force kernel written behind an interface. Then benchmark an
integer force path. If it costs under ~2× on the target GPUs, make it the default so the CPU
twin can be bit-exact.

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

## Correctness strategy
1. **Bit-exact twin** (`md-ref`): equal state hashes after N steps. Tier 1 compares GPU runs on
   the same device (and reversal); Tier 2 compares CPU and GPU.
2. **f64 ideal integrator:** measures integrator and precision error (energy drift vs Δt;
   observables vs the fixed-point engine).
3. **Force-kernel unit tests** against NIST SRSW sample configurations (reference energies and
   virials).
4. **Statistical validation suite** (`validate`), run natively with headless `wgpu`:
   Maxwell–Boltzmann, equipartition, barometric profile, B₂(T), NIST coexistence and
   Clausius–Clapeyron, Smit–Frenkel 2D, Engel et al. hard-disk pressure (βP(2σ)² = 9.1706 at
   η = 0.698), van 't Hoff, Izumida–Okuda Carnot, Yeh–Hummer-corrected diffusion.
5. **Browser parity tests:** Playwright with headless Chrome, plus manual Safari and Firefox
   checks, comparing against native `wgpu`.

## Performance strategy
- Everything GPU resident; many steps per command buffer; asynchronous readback of small
  summaries only.
- Autotune workgroup size and items per thread at startup using `timestamp-query`, falling back
  to defaults (catalog #14, HOOMD `Autotuner`).
- Respect the default limits (research §13): 16 KB workgroup storage, 256 invocations per
  workgroup, 128 MiB per storage binding. Request higher limits when the adapter allows.
- Three device classes: discrete GPU, integrated laptop GPU, phone.

## Milestones (thin vertical slices first)
- **M0, gpu-prims:** reduce-then-scan, counting sort, segmented reduce, two-word atomics,
  Threefry. Tests run natively and in the browser.
- **M1, minimal 2D MD:** WCA/LJ in NVE with a cell list, skin list and fixed-point state, plus
  the CPU twin and f64 reference. **First page: exact reversal (Loschmidt) with the one-bit
  perturbation.** First benchmark on the three device classes; choose the bit budgets.
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

## Open decisions
- Bit budgets for positions, velocities and forces; whether the Tier 2 integer force path
  becomes the default.
- Force strategy (a)/(b)/(c), and whether to pick per scenario.
- Shader templating: WESL vs a custom build step.
- Page framework (static site generator, or plain TypeScript and Vite).
- How pages embed engines: one wasm bundle, or per-engine bundles for size.
