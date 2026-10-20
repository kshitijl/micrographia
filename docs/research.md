# Research notes

Background research for a set of browser-based, GPU-powered (WebGPU) statistical mechanics
simulations, organized around the questions in [`../questions.md`](../questions.md).

Compiled 2026-10-19.

**Provenance tags** (how solid each claim is):

- **[read]**: read the primary source (paper full text, source code, or README) directly.
- **[abstract]**: seen only the abstract, docs summary, or secondary description.
- **[survey]**: from an earlier web survey (a prior chat session), not re-verified here.
- **[memory]**: from background knowledge, not checked against a source in this pass. Verify before relying on it.

---

## 1. Project framing and the gap

The individual pieces exist many times over: Maxwell-distribution boxes, Ising models,
Lennard-Jones demos. No project found combines all of these:

1. **GPU-scale MD with real thermodynamic instruments**: thermal walls that keep a heat ledger,
   pistons, entropy bookkeeping, Carnot cycles. Existing tools have either the instruments or the
   scale, never both.
2. **Organization by question** ("why does boiling point rise with pressure?"), not by model
   ("Ising, Lennard-Jones, gas").
3. **Quantitative validation** against Clausius–Clapeyron, van 't Hoff, Π = ckT, published
   equations of state, and finite-time Carnot results.
4. **Honest labeling** of 2D artifacts, scaled-up parameters, and contested points.
5. **A history thread** (e.g., a "1905" Einstein track).

Guidance from the survey: treat the Maxwell box and the Ising model as *engine tests*, not
content. Use GPU scale only where it buys something: clean interfaces, rare events, smooth
statistics. Publishing venues for this kind of work are the *American Journal of Physics*
(where Schroeder's MD paper and the Moore–Schroeder Einstein-solid paper appeared) and the
*Journal of Chemical Education*. [survey]

Risks: maintenance burden, phone performance, uneven WebGPU support (Firefox on Linux, Chrome
on older Linux GPUs). [survey]

---

## 2. Existing interactive teaching collections [survey]

URLs in this table were reconstructed from memory, not copied from search results; check them before linking.

| Project | What it is | Notes |
|---|---|---|
| [Dan Schroeder's simulations](https://physics.weber.edu/schroeder/software/) (Weber State) | HTML5 apps. [Interactive MD](https://physics.weber.edu/schroeder/md/interactivemd.html): up to ~2500 interacting particles in 2D, Verlet integration, LJ truncated at 3 molecular diameters. Also a 2D Ising model (Metropolis). | Best quality on the list. Useful CPU baseline for validation. |
| [Gould & Tobochnik STP programs](https://www.compadre.org/stp/) | Programs for *Statistical and Thermal Physics* (2nd ed., 2021): particles in a box, random walks, Einstein solids, Bose/Fermi gases, LJ fluids, Ising/XY, percolation. | Ported from Java, clunky. |
| [PhET](https://phet.colorado.edu/) (U. Colorado) | Gas Properties, States of Matter (adjustable LJ potential graph). | Pitched at a lower level. |
| [Complexity Explorables](https://www.complexity-explorables.org/) (Dirk Brockmann) | Critical phenomena, synchronization, evolutionary dynamics. | Stat-mech ideas in other fields. |
| [Falstad gas simulation](https://www.falstad.com/gas/) | Preset setups, live velocity histogram. | |
| [MinuteLabs Brownian motion](https://labs.minutelabs.io/Brownian-Motion/) | Brownian motion demo. | |
| [Entropy Explained, With Sheep](https://aatishb.com/entropy/) (Aatish Bhatia) | Einstein solids as sheep shuffling between farms. | |
| [Damon Binder's simulations](https://www.damonbinder.com/simulations) | Ising, XY, Heisenberg, lattice gas, oil-and-water, Potts, hard-core gas, LJ, salts. | Organized by model. |
| [MinervaLab](https://github.com/MinervaLab) | Jupyter notebooks on Binder: van der Waals isotherms, phase transitions. | |
| nanoHUB (e.g., RASPA tools) | Research MD/MC codes behind a free login; runs kept short. | |
| Molecular Workbench (Concord Consortium) | Classic MD teaching tool. | |
| WebAssembly + WebGL2 Ising app (Thomas C. Lang) | Millions of spins with temperature and field controls. | GPU Ising in the browser is essentially done. |
| Many single-page gas demos | e.g., a "Boltzmann ideal gas" page with 1,200 argon atoms (says it was built by an AI model); "The Particle Box" with 130 particles. | Basic Maxwell/gas-law demos are a commodity. One hobby MD project says its own pressure calculation is wrong. |

**Moore & Schroeder, "A different approach to introducing statistical mechanics"** (*Am. J.
Phys.*): two Einstein solids exchanging energy, microstates counted numerically. Entropy vs
energy motivates T = (∂S/∂U)⁻¹. It was originally a spreadsheet exercise. [survey]

---

## 3. Existing WebGPU / browser simulators

Searched 2026-10-19. **No mature, general WebGPU MD engine was found** (neighbor lists,
thermostats, measurement, 10⁵–10⁶ particles). A search can't prove absence.

| Project | Tech | Scale / notes | Takeaways |
|---|---|---|---|
| [whitead/symdjs](https://github.com/whitead/symdjs) ([live](https://whitead.github.io/symdjs/)), Andrew White | WebGPU, TypeScript | Symmetric MD of 2D LJ crystals under all 17 plane groups (Cox & White, *JCTC* 2022, 18, 4077, [doi](https://doi.org/10.1021/acs.jctc.2c00401)). Only the asymmetric unit is integrated; the other particles are ghosts. One workgroup per system, K steps per dispatch. 1 star, updated Oct 2026. [read README] | **Float64 CPU reference engine plus WebGPU-vs-CPU parity tests in headless Chrome (Playwright)**. CPU and GPU share a counter-based RNG and agree to float32 precision. Fractional positions. NPT via Monte Carlo moves in lattice-parameter space with a Jacobian term and Hastings correction. Dimension `D` is a parameter. |
| [scttfrdmn/webgpu-compute-exploration](https://github.com/scttfrdmn/webgpu-compute-exploration) | WebGPU, JS | 10 compute demos. MD demo: 100–5000 LJ atoms, velocity Verlet, periodic boundaries, all-pairs forces. [abstract] | Tech demo only. |
| [matsuoka-601/WebGPU-Ocean](https://github.com/matsuoka-601/WebGPU-Ocean) ([demo](https://webgpu-ocean.netlify.app/)) | WebGPU, TS | MLS-MPM fluid: ~100k particles on integrated graphics, ~300k on decent GPUs. **SPH mode topped out around 30k particles on integrated graphics, bottlenecked by the neighbor search.** SPH uses Hoetzlein's fixed-radius neighbor method. 560 stars. [read README] | **Fixed-point `atomicAdd`** for float scatter (WGSL atomics are 32-bit integers only), learned from EA's [pbmpm](https://github.com/electronicarts/pbmpm) (Chris Lewin, "A Position Based Material Point Method", SIGGRAPH 2024). A sobering data point on WebGPU neighbor-search cost. |
| [matsuoka-601/WaterBall](https://github.com/matsuoka-601/WaterBall) | WebGPU | Fluid on a sphere. | |
| [Particle Life in WebGPU](https://lisyarus.github.io/blog/posts/particle-life-simulation-in-browser-using-webgpu.html) (lisyarus blog) | WebGPU | 2D particles with finite-range forces. [read] | Clear write-up of GPU binning in three phases: (1) count per bin with atomics, (2) parallel prefix sum for bin offsets, (3) scatter particles into a sorted array. This is the standard cell-list build. |
| [andeplane/atomify](https://github.com/andeplane/atomify) | LAMMPS → WebAssembly, WebGL | Real LAMMPS MD in the browser, on the CPU. Actively maintained. Related: `andeplane/lammps.js`, `MendelsResearchGroup/lammps.js`. | Real MD but CPU-bound. No question-led content. |
| Fluid demos (Codrops articles, MLS-MPM three.js forum post) | WebGPU | A three.js forum author says SPH hit performance limits around 100k particles and switched to MPM. [abstract] | |
| Hoetzlein, ["Fast Fixed-Radius Nearest Neighbors: Interactive Million-Particle Fluids"](https://ramakarl.com/pdfs/2014_Hoetzlein_FastFixedRadius_Neighbors.pdf) (GTC 2014) | CUDA | Counting-sort grid neighbor search. [abstract] | The standard reference for the count/scan/scatter grid. |

**Performance expectation (revised):** the earlier estimate of 10⁵–10⁶ particles in 2D was never
benchmarked. Liquid LJ has about 20 neighbors per particle within the cutoff in 2D and 60+ in 3D,
so neighbor search dominates. A tentative target is 10⁵–10⁶ in 2D on a discrete GPU and 10⁴–10⁵
on laptops and phones. **This needs an early benchmark.**

---

## 4. Choice of physical model

The plan is one engine with a table of short-range pair potentials by species pair, choosing
the minimal model per question.

### Hard spheres / hard disks
- No attraction, so **no liquid–gas transition**: no condensation, boiling, latent heat of
  vaporization, triple point, or vapor pressure. The only transition is fluid → solid, driven
  purely by entropy. [memory]
- P/(nkT) depends only on packing fraction; temperature just rescales time. [memory]
- Ideal for: osmosis (it proves no attraction is needed), the depletion force, Brownian
  motion, ideal-gas/Maxwell behavior, pistons, Carnot.
- True hard spheres need event-driven MD (a sequential event queue, awkward on GPUs) or
  Monte Carlo (see §8). In MD, the practical substitute is **WCA** (Weeks–Chandler–Andersen:
  the LJ potential cut at its minimum and shifted, purely repulsive), which runs on the same
  integrator.

### Lennard-Jones
- The minimal model with gas, liquid, solid, triple point, critical point and two-phase
  coexistence.
- The phase diagram and equation of state are well published, including in 2D, which makes
  quantitative validation possible. (Johnson–Zollweg–Gubbins EOS for 3D LJ. [memory])
- Binary LJ mixtures (solute excluded from the crystal) cover freezing-point depression and
  some solubility questions.

### All-atom force fields (CHARMM, AMBER): rejected
- They need bonds, angles, dihedrals, partial charges and PME long-range electrostatics
  (FFT-heavy, painful without f64 in WGSL), with timesteps of about 1–2 fs. That is roughly
  10³–10⁴× more cost per unit of physical time.
- **They get the water physics wrong anyway.** Melting points of ice Ih for common water
  models:
  - TIP3P: ~146 K [memory]
  - SPC/E: ~215 K [memory]
  - TIP4P/2005: 250 K [read: Blazquez & Vega 2022]
  - TIP4P/Ice: ~272 K [memory]
  - TIP5P: 274 K [read: Blazquez & Vega 2022]

  The 2005 citation given earlier (*J. Chem. Phys.* 122, 234511) was wrong. The original is
  probably Vega, Sanz & Abascal, *J. Chem. Phys.* 122, 114507 (2005) [memory]. The verified
  current summary is Blazquez & Vega, "Melting points of water models: Current situation," *J.
  Chem. Phys.* 156, 216101 (2022), [doi:10.1063/5.0093815](https://doi.org/10.1063/5.0093815)
  (also TIP3P-FB 216 K, TIP4P-FB 242 K, TIP4P-D 247 K). They conclude that no model reproduces
  both the melting point and the temperature of maximum density.
- The detail obscures the mechanism, which works against question-led teaching.

### Coarse-grained middle ground
- **mW water** (Molinero & Moore, *J. Phys. Chem. B* 113, 4008, 2009): a single site with a
  Stillinger–Weber three-body term favoring tetrahedral bonding. It has the density maximum
  and floating ice, and costs about 100× less than atomistic water. Verified numbers are in §11.
  Candidate model for the ice, salt and triple-point questions. There is follow-up work on
  coarse-grained ions with mW.
- **MARTINI** (4-to-1 coarse graining) or simpler bead-spring lipids for bilayer
  self-assembly. A lattice model may be enough to start.
- **Ions:** screened Coulomb (Debye–Hückel/Yukawa) keeps everything short-range, with no PME.
- **Buffers:** reactive particles with binding and unbinding rules, not a force field.

### 2D vs 3D
Both will be needed. 2D gives about 10× more particles and is easy to watch. 3D is required
where 2D is qualitatively wrong:
- **Stokes' paradox:** 2D fluids have no well-defined drag, and the diffusion coefficient
  creeps up logarithmically over time, so Stokes–Einstein D = kT/(6πηa) fails. [survey]
- **2D melting** has an intermediate **hexatic** phase (see §8). [survey]
- Quantitative mW water.

---

## 5. D. E. Shaw Research: Desmond, Desmond/GPU, Anton, Random123

### Availability
- **Desmond source code is not publicly released.** The
  [Desmond license](https://www.deshawresearch.com/downloads/download_desmond.cgi/Desmond_License.txt)
  is royalty-free for non-commercial research by academic and non-profit entities. It refers to
  "any Source Code provided by DESRES to LICENSEE" and forbids distributing modifications.
  Schrödinger sells commercial licenses and maintains an active fork.
  ([Wikipedia](https://en.wikipedia.org/wiki/Desmond_(software)),
  [download page](https://deshawresearch.com/downloads/download_desmond.cgi)) [read license excerpts]
- Builds offered from Desmond_Maestro_2019.1 on are GPU-only (per HPC center docs, e.g.
  [TAMU HPRC](https://hprc.tamu.edu/kb/Software/Desmond)). [abstract]
- The [DEShawResearch GitHub org](https://github.com/DEShawResearch) has no Desmond. Public repos
  include `random123` (counter-based RNGs), `msys` (system preparation and analysis), `viparr`
  (force-field parameterization for Desmond/Anton), `viparr-ffpublic`, `Technical-Reports`, `tss`
  (Times Square Sampling), `fs123`, `DESMILES`, `ark` (config format), `sns-mp2`. [read via GitHub API]
- **No peer-reviewed paper describes the Desmond/GPU algorithms** (nonbonded kernel, pair lists).
  Only performance technical reports exist.

### Desmond/GPU technical reports ([repo](https://github.com/DEShawResearch/Technical-Reports))
All are performance-only, with no algorithm detail. [read]
- Bergdorf, Kim, Rendleman, Shaw. "Desmond/GPU Performance as of November 2014."
  DESRES/TR--2014-01. Version 1.2.2. Claims one GPU ≈ a dozen CPUs.
- Bergdorf, **Baxter**, Rendleman, Shaw. "...as of October 2015." DESRES/TR--2015-01.
- Bergdorf, **Baxter**, Rendleman, Shaw. "...as of November 2016." DESRES/TR--2016-01.
  [PDF](https://www.deshawresearch.com/publications/Desmond-GPU_Performance_November_2016.pdf).
  - CUDA C++. Versions 1.6.2/1.6.4 on Kepler, Maxwell and Pascal; compute capability 3.0–6.1.
  - Benchmarks: DHFR (23,558 atoms), ApoA1 (92,224), FEP/REST p38 systems.
  - 2.5 fs timestep, 4.0 fs with hydrogen mass repartitioning (HMR, H mass 1 → 4 u).
  - Far electrostatics evaluated every 2–3 steps, using PME or the new **u-series**.
  - The CPU only dispatches work.
  - Memory ≈ 235 MB + 2.7 MB per 1000 particles.
  - Example: TITAN X (Pascal) DHFR at 1008 ns/day with u-series and HMR.
  - GeForce cards need "extensive testing, selection, and burn-in".
- Bergdorf, Robinson-Mosher, Guo, Law, Shaw. "...as of April 2021." DESRES/TR--2021-01.
  [PDF](https://www.deshawresearch.com/publications/Desmond-GPU_Performance_April_2021.pdf).
  - Version 1.9.1, using u-series and midtown splines.
  - Benchmarks up to 10× STMV (10.7M atoms) on Pascal through Ampere.
  - Memory ≈ 405 MB + 2.3 MB per 1000 particles.
  - A single CPU core dispatches work.
- Chow, Rendleman, Bowers, Dror, Hughes, Gullingsrud, Sacerdoti, Shaw. "Desmond Performance on a
  Cluster of Multicore Processors." DESRES/TR--2008-01, July 2008. Includes RESPA integrator
  configs.

### SC06 Desmond paper (CPU)
Bowers, Chow, Xu, Dror, Eastwood, Gregersen, Klepeis, Kolossvary, Moraes, Sacerdoti, Salmon,
Shan, Shaw. **"Scalable Algorithms for Molecular Dynamics Simulations on Commodity Clusters."**
SC06, Tampa, Nov 2006. [IEEE 4090217](https://ieeexplore.ieee.org/document/4090217),
[PDF](https://www.cines.fr/wp-content/uploads/2014/01/pap259.pdf),
[abstract](https://cs.stanford.edu/people/rondror/abstracts/bowers_SC_06.htm). [read]

Techniques:
- **Midpoint method** (a neutral-territory decomposition). A pair interacts on the box that
  contains the midpoint of the pair. The import region only reaches R/2 beyond the home box.
  - Import volume, 3D: V_midpoint = b³(πα³/6 + 3πα²/4 + 3α), compared with half-shell
    V_HS = b³(2πα³/3 + 3πα²/2 + 3α), where α = R/b.
  - Generalizes to m-body terms (the box containing the center of the smallest enclosing sphere).
  - Bonded terms, charge spreading and constraints need no extra communication.
- **Infrequent particle migration:** expand the import region by a few tenths of an Å, so
  migration is needed only every few steps. This is the same idea as a Verlet skin.
- **Staged 6-message exchange** (±x, then ±y, then ±z) instead of 26 neighbor messages:
  3 × 33.7 µs versus 328.8 µs on InfiniBand.
- **FFT parallelization** with direct butterfly communication. The forward transform is
  decimation in frequency and the inverse is decimation in time, so bit-reversed order in
  k-space costs nothing.
- **SIMD C++ wrapper classes** (in the style of VPIC's V4). The nonbonded kernel takes 56 flops
  per pair.
- **Numerics:**
  - Single precision with energy drift below 1 K/µs, better than most double-precision codes.
  - **Bitwise time reversibility** (without constraints, with migration every step):
    - **fixed-point arithmetic for coordinate updates**, so integration loses no information;
    - **a consistent ordering of particles and computations**, so forces are unique functions of
      positions and parameters.
  - Always deterministic.
  - **Constraint stabilization:** new position-constraint algorithms avoid correlated
    position/velocity roundoff.
  - **Normalized local coordinates** (each box spans −0.5 to +0.5). This gives extra precision,
    a time-reversible implementation of pressure control, and simpler periodic boundaries.
  - **Interpolation in a transformed variable a·r² + b:** energy and force come from the same
    polynomial coefficients, need no sqrt or rsqrt, and the force is exactly the gradient of a
    potential.
- **RDMA primitives** for an "iterative exchange" pattern (double-buffered, no receiver
  handshake): 12.5 µs vs 111.5 µs for MVAPICH on a ring of 1024 processes.
- Energy drift (ApoA1, 300 K): Desmond 0.07 K/ns vs NAMD 0.9 K/ns at 2.5 fs with constraints.

### Lippert et al. 2007
Lippert, Bowers, Dror, Eastwood, Gregersen, Klepeis, Kolossvary, Shaw. **"A common, avoidable
source of error in molecular dynamics integrators."** *J. Chem. Phys.* 126, 046101 (2007).
[doi:10.1063/1.2431176](https://doi.org/10.1063/1.2431176),
[PDF](https://www.deshawresearch.com/publications/A%20common,%20avoidable%20source%20of%20error%20in%20molecular%20dynamics%20integrators.pdf). [read]
- Velocity-explicit integrators (leapfrog, velocity Verlet) beat Verlet in finite precision
  because storing v uses the precision better than storing x_{i−1}. x_i and x_{i−1} share
  leading bits, so the difference carries less information.
- Many codes recompute v_{i+1/2} = (x_{i+1} − x_i)/δt when constraints are active. This degrades
  them to Verlet-level accuracy and is significant in single precision.
- Codes affected at the time: AMBER 9 (sander, pmemd), GROMACS 3.3.1, NAMD 2.6 (SETTLE for
  water), and LINCS's velocity correction. CHARMM 33 used a better alternative.
- Fix: the RATTLE-style discretization, which solves for λ in the velocity update. Desmond uses
  RATTLE with an M-SHAKE variant.

### Anton
- Shaw et al. "Anton, a special-purpose machine for molecular dynamics simulation." ISCA 2007 /
  *CACM* 2008. [CACM](https://cacm.acm.org/research/anton-a-special-purpose-machine-for-molecular-dynamics-simulation). [abstract]
  - **Customized-precision fixed-point arithmetic throughout.** Results are deterministic, time
    reversible, and independent of the number of nodes, because fixed-point addition is
    associative.
  - Lecture notes ([UMD CMSC714](https://www.cs.umd.edu/class/fall2023/cmsc714/Lectures/lect23-moldyn-vmscope.pdf))
    note that MD is chaotic, so bitwise determinism doesn't make single trajectories
    "correct". Its value is reversibility, reproducibility and node-count independence.
- Related patent application: "Numerical techniques for dynamics simulation"
  ([US 20080082303](https://patents.justia.com/patent/20080082303)).
- Hot Chips 33 (2021) Anton 3 talk:
  [slides](https://www.hc33.hotchips.org/assets/program/conference/day2/HC2021.DESRES.AdamButts.v03.pdf). [not read]

### Electrostatics papers (relevant only if we add long-range Coulomb)
- Predescu, Lerer, Lippert, Towles, Grossman, Dirks, Shaw. "The u-series: A separable
  decomposition for electrostatics computation with improved accuracy." *J. Chem. Phys.* 152,
  084113 (2020).
- Predescu, Bergdorf, Shaw. "Midtown splines: An optimal charge assignment for electrostatics
  calculations." *J. Chem. Phys.* 153, 224117 (2020).
- Shan et al. "Gaussian split Ewald" (k-GSE), *J. Chem. Phys.* 2005. [memory]

### Random123
Salmon, Moraes, Dror, Shaw. **"Parallel random numbers: as easy as 1, 2, 3."** SC11.
[PDF](https://users.cs.utah.edu/~hari/teaching/bigdata/random123sc11.pdf),
[docs](https://www.thesalmons.org/john/random123/releases/latest/docs/),
[code](https://github.com/DEShawResearch/random123). [abstract]
- Counter-based PRNGs: output = f(counter, key), with no sequential state. Philox
  (integer-multiply based), Threefry (add/rotate/xor), and AES-based variants. They pass
  TestU01 BigCrush, and Philox beat CURAND on a GPU.
- **For WGSL:** Threefry needs only 32-bit add, rotate and xor. Philox needs a 32×32→64
  multiply-high, which WGSL lacks and would have to be emulated.

### Schrödinger's Desmond GPU, GTC 2024 talk
Jiqun Tu (NVIDIA) and Ellery Russell (Schrödinger, Desmond tech lead). S61156,
"...CUDA Graphs, ...Memory, C++ Coroutines, and More."
[PDF](https://developer-blogs.nvidia.com/wp-content/uploads/2024/10/S61156.pdf). [read parts]
- Desmond is **fully GPU-resident**; the purpose-built GPU port dates from 2014. Data goes to
  the CPU only for file output. Schrödinger averages 8,000–10,000 GPUs in use.
- **Strong-scaling problem:** proteins stay the same size while GPUs grow, so kernel-launch
  latency and small serial kernels dominate (Amdahl).
- Fixes:
  - **CUDA Graphs.** DHFR CPU API overhead dropped from ~520 µs to ~115 µs per step and kernel
    time from ~550 to ~320 µs.
  - A "graph comparison" safety check that periodically re-records the graph and compares it.
  - **Multiple simulations per GPU (MPS)** for throughput.
  - **VDW coefficients loaded into shared memory** once per kernel instead of repeatedly from
    texture.
- A100 results: SOLVENT 5k×12 went from 100 to 304 ns/day, and DHFR from 659 to 843.

### Other references cited in the Desmond reports
- Feenstra, Hess, Berendsen. Hydrogen mass repartitioning. *J. Comput. Chem.* 20, 786 (1999).
- Wang, Berne, Friesner. FEP/REST. *PNAS* 109, 1937 (2012).
- Phillips et al. NAMD, SC02.

---

## 6. moderngpu (Sean Baxter)

- Repo: [moderngpu/moderngpu](https://github.com/moderngpu/moderngpu) (~1.8k stars, last push Jan
  2026). [Wiki](https://github.com/moderngpu/moderngpu/wiki) (2.0 docs). 1.0 docs at
  [moderngpu.github.io](https://moderngpu.github.io/) (originally NVIDIA Research, 2013).
  Citation: Sean Baxter, "moderngpu 2.0", 2016. [read wiki and 1.0 intro, performance and
  load-balance pages]
- Header-only CUDA C++. 2.0 is about 5,000 lines, against cub at ~45k, CUDPP ~26k, and thrust
  >160k.
- Philosophy: **"Software is an asset, code a liability."** Keep code small so it's easy to
  discard.

### Key ideas
- **Two-phase decomposition:**
  1. Partition the problem coarsely so work is *exactly* load-balanced over threads, typically
     with binary searches such as Merge Path.
  2. Each thread runs simple, work-efficient, *sequential* logic.

  This separates scheduling from problem logic. Critique: "conventional wisdom is to lower every
  problem to scan", but partitioning in parallel and then solving sequentially is often simpler.
- **Load-balancing search (LBS):** "kind of scan inverse".
  - Input: an exclusive scan of per-object work counts (the segments descriptor).
  - Output: for each work item, the index of the object that generated it, plus its rank within
    that object.
  - Merge Path partitions the work-item indices together with the scan into equal tiles per
    CTA. Inside a CTA, each thread does a small merge-path search and then VT serial steps.
- **Patterns plus behaviors (2.0):** patterns are specialized with lambdas.
  - `transform`, `cta_launch`, `cta_transform`, `transform_reduce`, `transform_scan`
  - **`transform_lbs`** (the signature pattern), **`lbs_segreduce`**, `transform_segreduce`
  - `transform_compact` (two-pass stream compaction)
  - **`lbs_workcreate`** (dynamic work creation, upsweep and downsweep, exactly load-balanced;
    solves many problems CUDA Dynamic Parallelism was meant for)
  - Also: `merge`, `mergesort`, `segmented_sort` (short-circuiting, faster with more segments),
    `sorted_search` (vectorized: O(n+m) instead of O(n log m)), `bulk_insert`/`bulk_remove`,
    `inner_join`, `interval_expand`, `interval_move`, `spmv`, and multisets (Balanced Path) in
    1.0.
- **Tuple caching:** per-segment values are loaded cooperatively through shared memory once per
  CTA and passed to the lambda. This avoids redundant L2 loads.
- **Deterministic networks:** every pattern defines a deterministic network of operations
  (reductions with a fixed tree).
- **Performance guidance:**
  - Occupancy versus ILP: lower occupancy is fine if ILP hides latency.
  - Register blocking. Index register arrays only with compile-time constants, to avoid
    spilling to local memory.
  - Grain size VT and CTA size NT set the tile; try odd grain sizes; start with 128–256 threads
    per block.
  - **LaunchBox:** per-architecture tuning parameters chosen at compile time and run time.
  - "Tune before profiling."
- Demos in the wiki: sparse matrix–vector multiply via `transform_segreduce`; breadth-first
  search via `transform_lbs`, then `lbs_workcreate`, then bit-packed visited sets with
  `atomicOr`; and "Attu Station, AK", a geographic query that does 25M distance computations
  reduced to 29k results with O(n) temporary storage.

### Mapping to this project
- **Desmond's SC06 paper says load balance "is not a significant issue" because biomolecular
  density is uniform (~0.1 atoms/Å³). Our systems are deliberately non-uniform:** liquid next to
  vapor (a 100–1000× density ratio), gravity columns, concentrated solution against a membrane.
  LBS over pair candidates plus a segmented reduction per particle keeps cost independent of
  where the dense phase is.
- Building the cell list is histogram, scan, scatter (in moderngpu terms, scan plus interval
  move).
- Stream compaction collects particles near walls, pistons and membranes into special lists,
  keeping the main force kernel branch-free.
- Segmented reductions give density, temperature and pressure profiles by slab, and heat
  totals per wall.
- LaunchBox maps to WGSL `override` constants (workgroup size, items per thread), set when the
  pipeline is created and tuned per device.
- WebGPU constraints: the default workgroup storage is 16 KB, and warp-level operations need
  the optional `subgroups` feature. Subgroup size isn't fixed: Apple 32, AMD 32/64, Intel 8–32.
  [memory]

---

## 7. Open-source GPU MD engines

| Engine | License | Stars (Oct 2026) | Best at |
|---|---|---|---|
| [HOOMD-blue](https://github.com/glotzerlab/hoomd-blue) (Glotzer lab) | BSD-3 | ~450 | Soft matter, coarse-grained models, GPU-first, native 2D, hard-particle MC, MPCD |
| [GROMACS](https://github.com/gromacs/gromacs) | LGPL-2.1 | ~970 | Biomolecular MD; among the fastest |
| [OpenMM](https://github.com/openmm/openmm) | MIT/LGPL | ~2.0k | Hackable and Python-driven; one kernel source for CUDA/OpenCL/HIP; runtime-compiled custom forces |
| [LAMMPS](https://github.com/lammps/lammps) | GPL-2.0 | ~3.1k | Broadest features and potentials (KOKKOS and GPU packages) |
| [JAX MD](https://github.com/jax-md/jax-md) | Apache-2.0 | ~1.5k | Differentiable MD in Python |
| AMBER pmemd.cuda | not open source | – | Its SPFP precision model paper is public |

### GROMACS
- Páll & Hess. "A flexible algorithm for calculating pair interactions on SIMD architectures."
  *Comput. Phys. Commun.* (2013). [doi:10.1016/j.cpc.2013.06.003](https://doi.org/10.1016/j.cpc.2013.06.003),
  arXiv:1306.1737. This is the **cluster-pair (NxM) algorithm**, `nbnxm` in the code
  ([module docs](https://manual.gromacs.org/2022/doxygen/html-full/group______module__nbnxm.xhtml)).
  - Spatially adjacent atoms are grouped into fixed-size clusters with bounding boxes, and
    cluster pairs are tested against the cutoff.
  - Whole cluster-pair tiles are computed, accepting some wasted pair evaluations beyond the
    cutoff in exchange for regular, SIMD/warp-friendly work.
  - Exclusions are bitmasks.
  - Cluster sizes (4×4 / 4×8 on CPU, 8×8 with super-clusters on GPU) are from memory; check the
    paper. [abstract + memory]
- Páll, Zhmurov, Bauer, Abraham, Lundborg, Gray, Hess, Lindahl. "Heterogeneous parallelization
  and acceleration of molecular dynamics simulations in GROMACS." *J. Chem. Phys.* 153, 134110
  (2020). [arXiv:2006.09167](https://arxiv.org/pdf/2006.09167). [abstract]
  - **Dual pair list with rolling (dynamic) pruning.** The outer list has a buffer and is rebuilt
    infrequently. Pruning to an inner list with almost no buffer runs every few steps and costs
    under 1% of runtime. On GPUs, part of the list is pruned every other step, overlapping CPU
    work.
  - The full step (update and constraints) is GPU-resident, minimizing CPU↔GPU transfers.
- Kutzner et al. "More bang for your buck: Improved use of GPU nodes for GROMACS 2018."
  [arXiv:1903.05918](https://arxiv.org/pdf/1903.05918): the list is rebuilt every 100–200 steps,
  and the pruned list lasts 5–15 steps (auto-tuned). [abstract]
- **Automatic Verlet buffer sizing** from a target energy-drift tolerance
  (`verlet-buffer-tolerance`). [memory]

### OpenMM
- Eastman & Pande. "Efficient nonbonded interactions for molecular dynamics on a graphics
  processing unit." *J. Comput. Chem.* (2010). [hgpu](https://hgpu.org/?p=1255). [abstract]
- From the source code [read]:
  - **Atoms are reordered along a Hilbert curve**, or a simple zigzag curve for small systems
    (`platforms/common/src/ComputeContext.cpp`, `reorderAtoms()`).
  - **32-atom blocks** (`TILE_SIZE`) with bounding boxes. `findInteractingBlocks.cu` works in two
    stages:
    1. Block against block, using bounding-box distance.
    2. Block against individual atoms.
  - **Sparse tiles are pulled out and computed as single pairs.** The kernel also checks whether
    any atom has moved enough to require a list rebuild.
  - **64-bit fixed-point force accumulation:** `realToFixedPoint`, scale `0x100000000` (2³²),
    `mm_ulong` atomic adds (`platforms/common/src/kernels/integrationUtilities.cc`, `verlet.cc`).
  - "Common" platform kernels are written once and compiled for CUDA, OpenCL and HIP. That's the
    closest existing model for generating WGSL.
  - Custom forces take user-supplied algebraic expressions compiled into kernels at run time.

### AMBER SPFP
- Le Grand, Götz, Walker. **"SPFP: Speed without compromise—A mixed precision model for GPU
  accelerated molecular dynamics simulations."** *Comput. Phys. Commun.* 184, 374–380 (2013).
  [doi:10.1016/j.cpc.2012.09.022](https://doi.org/10.1016/j.cpc.2012.09.022). [abstract]
  - Forces are computed in single precision and accumulated in **fixed-point integers**. The
    default in AMBER's GPU code is "hybrid bitwise reproducible fixed point precision"
    ([AMBER GPU docs](https://ambermd.org/gpus16)). Alternatives: SPDP, SPSP, DPDP.
- Tinker-HP adopted the idea with 34 fractional bits (30 integer bits) for force accumulation
  ([arXiv:2011.01207](https://arxiv.org/pdf/2011.01207)). [abstract]

### HOOMD-blue
- Anderson, Lorenz, Travesset. "General purpose molecular dynamics simulations fully implemented
  on graphics processing units." *J. Comput. Phys.* 227 (2008). The original GPU MD code.
  [memory]
- **Counter-based RNG:** `hoomd/RandomNumbers.h` wraps OpenRAND's Philox4x32. The seed packs a
  class id, the timestep and the user seed; the counter carries the particle tag.
  [read source]
- **Hard-particle Monte Carlo (HPMC):** Anderson, Irrgang, Glotzer. "Scalable Metropolis Monte
  Carlo for simulation of hard shapes." [arXiv:1509.04692](https://arxiv.org/pdf/1509.04692).
  [abstract] (Also known as "HPMC".)
  - The GPU proposes many trial moves in parallel on a **checkerboard**, with a block-level
    queue that redistributes work and avoids divergence.
  - Domain decomposition across GPUs: 16.8M particles on 2048 GPUs.
  - On the CPU, bounding volume hierarchy (BVH) trees instead of cell lists for large size
    disparity.
  - Shapes: polygons, spheropolygons, polyhedra, ellipsoids, spheres, unions, and more.
    **Implicit depletants** connect to the depletion force in the notes.
  - [HOOMD features](https://hoomd-blue.readthedocs.io/en/latest/features.html).
- **LBVH neighbor lists:** Howard, Anderson, Nikoubashman, Glotzer, Panagiotopoulos. "Efficient
  neighbor list calculation for molecular simulation of colloidal systems using graphics
  processing units." *Comput. Phys. Commun.* 203, 45–52 (2016).
  [PDF](https://par.nsf.gov/servlets/purl/10078947),
  [HOOMD Tree docs](https://hoomd-blue.readthedocs.io/en/v7.1.0/hoomd/md/nlist/tree.html). [abstract]
  - Linear BVHs per particle type beat a GPU stenciled cell list for moderate or large size
    disparity, and for dilute or semidilute large particles.
  - HOOMD docs: cell lists are usually best below a 2:1 cutoff asymmetry.
- **MPCD** (multiparticle collision dynamics): a mesoscale solvent with hydrodynamics, much
  cheaper than explicit solvent. A candidate for 3D Brownian motion and Stokes–Einstein, if
  labeled as a coarse model. [memory]
- Particle sorting along a Hilbert curve for memory locality. [memory]

### JAX MD
- **Fixed-capacity neighbor buffers with an overflow flag.** The host reallocates and reruns on
  overflow. This is exactly what WebGPU needs, since shaders can't allocate. [memory]

### LAMMPS
- Broadest potentials and features. KOKKOS and GPU packages. "Porting LAMMPS to GPUs" (Sandia,
  2010). More useful as a reference for potentials than for GPU technique. Atomify (§3) runs it
  in the browser.

---

## 8. Hard disks, 2D melting, and parallel Monte Carlo

- Engel, Anderson, Glotzer, Isobe, Bernard, Krauth. **"Hard-disk equation of state: First-order
  liquid-hexatic transition in two dimensions with three simulation methods."** *Phys. Rev. E*
  87, 042134 (2013). [arXiv:1211.1645](https://arxiv.org/pdf/1211.1645). [abstract]
  - Reproduces the event-chain Monte Carlo equation of state with massively parallel local MC
    on the GPU and with event-driven MD.
  - Confirms a **first-order liquid–hexatic transition**, then a continuous hexatic–solid one.
    This settled a long-running debate about 2D melting.
- Anderson, Jankowski, Grubb, Engel, Glotzer [author list from memory]. "Massively parallel Monte Carlo for many-particle
  simulations on GPUs." *J. Comput. Phys.* (2013).
  [record](https://cris.fau.de/publications/117060064). [abstract]
  - **Checkerboard decomposition that keeps detailed balance.**
  - Over 10⁹ trial moves per second on a Tesla K20, 148× a single Xeon core. Systems up to 10⁶
    hard disks.
- Bernard & Krauth. "Two-step melting in two dimensions: First-order liquid-hexatic
  transition." *PRL* 107, 155704 (2011). Uses event-chain Monte Carlo (Bernard, Krauth & Wilson
  2009). [memory]
- **Implication:** we could show liquid–hexatic coexistence in the browser with the same
  algorithm that confirmed it, and validate against the published equation of state.

---

## 9. Physics points from the earlier survey to flag on pages [survey]

- **Integrator:** velocity Verlet (symplectic, so energy wobbles but doesn't drift), not Euler,
  which pumps in energy.
- **Units:** reduced units (σ = ε = m = k = 1), since WGSL has no f64.
- **Temperature:** from mean kinetic energy; in 2D, ⟨KE⟩ = kT per particle.
- **Pressure:** from wall momentum transfer (intuitive, noisy) or the virial (smoother).
- **Pistons:** a wall moving at speed u reflects a particle as v′ = 2u − v (normal component).
  - Slow piston: reversible adiabatic, PV^γ constant, with γ = 2 for a 2D monatomic gas.
  - Fast piston: irreversible, entropy rises.
  - Instant removal: free expansion, T unchanged for a dilute gas.
- **Thermal walls:** re-emit particles with velocities drawn from the Maxwell–Boltzmann
  distribution at the bath temperature; the energy exchanged equals Q exactly. Avoid
  velocity-rescaling thermostats, which are unphysical and hide where the heat goes.
- **Carnot:** switch walls between thermal and specular, compute W = ∮P dV, compare with
  1 − T_c/T_h. The limit is reached only as the cycle slows. At maximum power the efficiency is
  near 1 − √(T_c/T_h) (Curzon–Ahlborn), which is model-dependent, not universal.
- **Fluctuations** scale as 1/√N.

### "1905 track" [survey]
- **Brownian motion:** mean-squared displacement ⟨x²⟩ = 2Dt. D = kT/γ: the same collisions
  cause both the fluctuation and the friction. The Perrin barometric experiment yields Avogadro's
  number (Nobel Prize 1926). The Stokes–Einstein formula fails in 2D; use 3D with about 10⁵
  solvent particles and a tracer about 5× their size, with finite-size corrections.
- **Thesis (viscosity):** η = η₀(1 + 2.5φ). The original calculation error gave an Avogadro's
  number about 3× too low, corrected in 1911.
- **Light quanta:** the volume dependence of the entropy of radiation matches an ideal gas,
  (V/V₀)^N with N = E/hν. The photoelectric effect alone doesn't prove photons: Lamb & Scully
  1969 showed a semiclassical model reproduces it, and photon antibunching (1977) is stronger
  evidence. Millikan confirmed the equation in 1914–16 while still doubting photons.
- **Special relativity:** relativistic collision rules turn the Maxwell distribution into
  Maxwell–Jüttner. How temperature transforms between moving frames is still disputed.
- **E = mc²:** the inertia of a heated box of relativistic gas grows by ΔE/c².
- **Einstein solid:** from his 1907 specific-heat paper.

---

## 10. Consolidated technique catalog and design implications

| # | Technique | Source(s) | How we'd use it |
|---|---|---|---|
| 1 | **Fixed-point positions** in box-normalized coordinates (u32) | Desmond SC06, Anton | 2³² steps across the box. Integer wraparound gives periodic boundaries for free; nearest image is the signed i32 difference. No f64 needed. |
| 2 | **Fixed-point accumulation** of forces, energies, impulses and heat | Anton, OpenMM, AMBER SPFP, Tinker-HP, pbmpm, WebGPU-Ocean | Order-independent and deterministic. WGSL has only 32-bit atomics, so emulate 64-bit with **two u32 words**: `atomicAdd` the low word, detect carry from the returned old value, `atomicAdd` the carry and high part into the high word. The final sum is exact because addition commutes. |
| 3 | **Bitwise-reversible integration** (integer leapfrog: positions and velocities as integers in the same units, so the drift step is exact and only the kick rounds; see `architecture.md`) | Desmond SC06; Levesque & Verlet, "Molecular dynamics and time reversibility", *J. Stat. Phys.* 72, 519 (1993) [memory] | Exact-reversal **Loschmidt demo**: release a gas, flip velocities, it un-mixes exactly; flip a single bit first and it doesn't (chaos). Doubles as an engine determinism test. Thermal walls break reversibility, which is a lesson in itself. |
| 4 | **Counter-based RNG** keyed by (seed, step, particle id) | Random123; HOOMD (Philox via OpenRAND); symdjs | Reproducible thermal walls and Langevin noise with no RNG state. Use **Threefry** in WGSL (no multiply-high). |
| 5 | **Pair-potential tables in r²** (a·r² + b) | Desmond SC06 | Per-species-pair tables (WCA, Yukawa, LJ variants). No sqrt, force is the exact gradient, consistent energy and force. Fewer cross-vendor transcendental differences. |
| 6 | Store v, not x_{i−1}; never recompute v from position differences | Lippert 2007 | Matters for constraints and rigid water later. |
| 7 | **Neighbor-list buffer (skin) with infrequent rebuild**; dual list with rolling pruning; automatic buffer sizing | Desmond SC06; GROMACS 2018/2020 | Rebuild when any particle has moved half the skin, not every step. Later: outer and inner lists. |
| 8 | **Cell list via count, prefix-sum, scatter** | lisyarus, Hoetzlein 2014, moderngpu | Standard WGSL implementation. |
| 9 | **Hilbert-curve spatial ordering** | OpenMM, HOOMD | Sort cells in Hilbert order so consecutive particles are spatially close; clusters come for free. |
| 10 | **Cluster/tile pair interactions** with bounding boxes, plus single-pair fallback for sparse tiles | GROMACS nbnxm, OpenMM | Benchmark against #11 on liquid–vapor systems. Stage tiles in workgroup memory; use subgroups only when available; tile size as an `override` constant. |
| 11 | **Load-balancing search plus segmented reduce** over pair candidates | moderngpu | Load balance independent of density in strongly non-uniform systems. Deterministic per-particle reduction. |
| 12 | **Stream compaction** for special particles | moderngpu `transform_compact` | Lists of particles near walls, pistons and membranes; branch-free main kernel. |
| 13 | **Segmented reductions** for measurements | moderngpu | Slab profiles (density, T, P), per-wall heat totals. |
| 14 | **Tuning via override constants** | moderngpu LaunchBox | Workgroup size and items per thread tuned per device when the pipeline is created. |
| 15 | **GPU-resident step; many steps per command buffer; rare async readback** | Desmond/GPU (GTC 2024), GROMACS 2020, symdjs | The WebGPU equivalent of CUDA Graphs. |
| 16 | **Replicas batched in one dispatch** | Desmond/GPU (MPS, multiple sims per GPU) | Carnot efficiency vs cycle speed (one replica per speed), ensemble statistics, fluctuation demos. |
| 17 | **Per-species coefficients in shared memory** | Desmond/GPU (GTC 2024) | Load the pair table into workgroup memory once per workgroup. |
| 18 | **Fixed-capacity buffers with overflow flags** | JAX MD | Neighbor and cell buffers preallocated; on overflow the host grows them and reruns. |
| 19 | **Checkerboard parallel hard-particle MC** | Anderson et al. 2013, HPMC | Second engine for entropy-only pages: hard-disk melting and hexatic phase, depletion, osmosis without attraction. |
| 20 | **LBVH or per-species-pair grids** for size disparity | Howard et al. 2016 | Large Brownian tracer in small solvent, sheep and sheepdogs. |
| 21 | **MPCD solvent** | HOOMD | Cheap hydrodynamics for 3D Brownian motion, labeled as coarse. |
| 22 | **Two CPU references plus GPU parity tests** | symdjs (f64 + parity), Anton/Desmond (bit-exact) | (a) a **bit-exact twin** of the fixed-point integrator, where the test is equal state hashes; (b) an **f64 ideal integrator** for integrator error and observables. Cross-platform bit-exactness needs integer force arithmetic, because WGSL allows float reassociation and fusion (see §13). |
| 23 | Midpoint / neutral-territory decomposition; RDMA primitives | Desmond SC06 | **Skip:** these are for multi-node machines. The midpoint idea for m-body terms may matter if we add three-body mW on tiles. |
| 24 | PME / u-series / midtown splines / k-GSE | DESRES | **Skip** unless long-range electrostatics become necessary; use Yukawa/Debye–Hückel instead. |

### Proposed first step
A benchmark: a 2D LJ/WCA engine in WGSL with a count/scan/scatter cell list in Hilbert order,
a skin neighbor list, u32 fixed-point positions, two-word fixed-point sums, Threefry RNG, a
bit-exact CPU twin and an f64 reference. Run it on a desktop GPU, a laptop's integrated GPU and
a phone to find the real particle ceiling before designing pages around a number. Then compare
the force strategies (#10 vs #11) on a liquid–vapor slab.

---

## 11. Validation reference data

Validating against published numbers is what turns demos into a contribution. Most checks
depend on the potential's truncation, so match the convention exactly.

### Checks that are exact by construction (no literature needed)
- Maxwell–Boltzmann speed and velocity distributions; equipartition (⟨KE⟩ = kT per particle in
  2D).
- Ideal-gas law, and the barometric profile ρ(z) ∝ exp(−mgz/kT) for a dilute gas in gravity.
- Low-density LJ pressure against the second virial coefficient B₂(T), computable by quadrature.
- Einstein relation D = kT/γ (drag a tracer to measure γ, and separately measure D from MSD).
- Energy conservation and drift (NVE), and bit-exact reversal.
- Self-consistency: the Clausius–Clapeyron slope of our own measured ln p_sat vs 1/T should match
  our own measured ΔH_vap; freezing-point depression should match RT²x/ΔH_fus using our own
  measured ΔH_fus.

### NIST Standard Reference Simulation Website (SRD 173)
[LJ benchmark index](https://mmlapps.nist.gov/srs/LJ_PURE/index.htm),
[dataset record](https://data.nist.gov/od/id/FF429BC178798B3EE0431A570681E858232),
doi:10.18434/mds2-232. Contacts: Daniel Siderius; reference calculations by Krekelberg and Shen.
[read]
- **Sample configurations with reference energies and pair virials** (3σ cutoff with long-range
  corrections, 4σ with corrections, 3σ linear-force-shifted). These are unit tests for our force
  kernels.
- Datasets: NVE MD at T* = 0.85 (T, E, P, **diffusion coefficient**); NVT MC at T* = 0.85 and
  0.90; SAT-TMMC coexistence at T* = 0.70–1.20; EOS-TMMC; an empirical saturation fit (not for
  use near the critical point); surface tension, including truncation effects; and, for the
  linear-force-shifted potential at 2.5σ, SAT-TMMC and vapor EOS at T* = 0.65–0.90.
- **Full-LJ critical point** (Potoff & Panagiotopoulos 1998): T*c = 1.3120(7), ρ*c = 0.316(1),
  p*c = 0.1279(6).
- **Linear-force-shifted 2.5σ critical point:** T*c = 0.937, ρ*c = 0.320, p*c = 0.0820.
- **SAT-TMMC coexistence**, LJ with 3σ cutoff and long-range corrections, V/σ³ = 512
  ([table](https://mmlapps.nist.gov/srs/LJ_PURE/sattmmc.htm)):

| T* | ρ*vap | ρ*liq | p*sat |
|---|---|---|---|
| 0.70 | 1.99600E-03 | 8.43739E-01 | 1.36959E-03 |
| 0.75 | 3.62958E-03 | 8.21911E-01 | 2.63525E-03 |
| 0.80 | 6.09962E-03 | 7.99867E-01 | 4.64539E-03 |
| 0.85 | 9.64024E-03 | 7.76854E-01 | 7.63583E-03 |
| 0.90 | 1.45128E-02 | 7.52806E-01 | 1.18493E-02 |
| 0.95 | 2.10228E-02 | 7.27724E-01 | 1.75374E-02 |
| 1.00 | 2.95686E-02 | 7.01018E-01 | 2.49576E-02 |
| 1.05 | 4.06547E-02 | 6.72152E-01 | 3.43602E-02 |
| 1.10 | 5.50768E-02 | 6.40799E-01 | 4.60204E-02 |
| 1.15 | 7.41176E-02 | 6.05204E-01 | 6.02112E-02 |
| 1.20 | 1.00339E-01 | 5.63158E-01 | 7.72251E-02 |

  The ln p_sat vs 1/T slope of this table gives a reference ΔH_vap. That's a ready-made
  **Clausius–Clapeyron validation page** for "why does the boiling point rise with pressure?"

### Other 3D LJ references
- Truncated-and-shifted at 2.5σ: triple point T* ≈ 0.62 (Gibbs–Duhem integration,
  [arXiv:2104.03028](https://arxiv.org/pdf/2104.03028)) [abstract]; coexistence p* ≈ 0.0267 at
  T* = 0.785 ([arXiv:2101.08135](https://arxiv.org/pdf/2101.08135)) [abstract]. The likely
  primary source for this potential's coexistence curve and critical point is Vrabec et al.
  (Mol. Phys. 2006) [memory; numbers not retrieved].
- Full LJ triple point T* ≈ 0.694 [memory].
- Smit, "Phase diagrams of Lennard-Jones fluids," *J. Chem. Phys.* (1992): the phase diagram
  depends significantly on truncation details
  ([PDF](https://pure.uva.nl/ws/files/2199981/29999_3595309057smi922.pdf)). [abstract]

### 2D LJ references
- Smit & Frenkel, "Vapour–liquid equilibria of the two-dimensional Lennard-Jones fluid," *J.
  Chem. Phys.* 94, 5663 (1991): truncated-and-shifted potential, **T_c = 0.459 ± 0.001,
  ρ_c = 0.35 ± 0.01**. The tail of the potential has a substantial effect, and the 2D
  coexistence curve is very flat near T_c. [abstract; check the cutoff in the paper]
- Li & Pica Ciamarra, "Phase behavior of Lennard-Jones particles in two dimensions," *Phys.
  Rev. E* 102, 062101 (2020), [arXiv:2010.03876](https://arxiv.org/pdf/2010.03876). At high T,
  melting is a continuous solid–hexatic transition followed by a first-order hexatic–fluid
  transition; at low T it is a first-order liquid–solid transition. [abstract]
- [arXiv:2110.01986](https://arxiv.org/pdf/2110.01986) (generalized 2D LJ): a shorter attraction
  range lowers T_c toward the triple point. Its ρ_c ≈ 0.366 and its P_c differs between preprint
  versions. The 2D LJ phase diagram is "still debated". [abstract]
- No clean 2D LJ triple-point temperature was found. Read Li & Pica Ciamarra for one.

### Hard disks (Engel et al. 2013, [arXiv HTML](https://arxiv.org/html/1211.1645v2)) [read]
- Mayer–Wood loop extrema at η ≈ 0.702 and η ≈ 0.714; "the coexistence phase ends at η ≃ 0.716".
  Hexatic at η = 0.718; at η = 0.720 orientational correlations decay almost as a power law. The
  hexatic–solid transition location is not pinned down.
- Pressure test at η = 0.698, N = 256², in units of βP(2σ)²: LMC 9.17046(15), EDMD 9.17076(18),
  ECMC 9.17062(9), MPMC 9.17078(5). Four methods agree to about 10⁻⁴, a sharp validation target.
- Largest N = 1024². Efficiency at N = 512², η = 0.698, in displacements per hour: LMC 6.5×10⁹,
  EDMD 1.7×10⁹, ECMC 1.6×10¹⁰, **GPU massively parallel MC 2.3×10¹²** (overall speed-up vs
  LMC: 1, 10, 70, 320).

### Water (mW)
Molinero & Moore 2009, [arXiv:0809.2811](https://arxiv.org/abs/0809.2811),
doi:10.1021/jp805227c. [abstract]
- Ice Ih T_m = 274.6 ± 1 K and cubic ice 271.5 ± 1 K, so hexagonal ice is more stable, as
  observed.
- Maximum liquid density ≈ 1.003 g/cm³ at a temperature of maximum density ≈ 250 K (experiment:
  277 K).
- Liquid density within 1% of experiment for 250–350 K. Surface tension at 300 K is 66 ± 2
  mJ/m² (experiment 71.6). T_m and the sublimation and vaporization enthalpies are within 2% of
  experiment.
- Also in the [NIST Interatomic Potentials Repository](https://www.ctcms.nist.gov/potentials/entry/2009--Molinero-V-Moore-E-B--water/2009--Molinero-V--water--ipr-1.html).

### Finite-time Carnot
- Izumida & Okuda, "Molecular kinetic analysis of a finite-time Carnot cycle," *EPL* (2008),
  [arXiv:0802.3759](https://arxiv.org/pdf/0802.3759). [abstract]
  - MD of a nearly ideal gas with a piston moving at constant speed u, the only control
    parameter.
  - The efficiency at maximum power **deviates from Curzon–Ahlborn** except as T_c → T_h, and a
    simple kinetic analysis explains why.
  - **This is our Carnot page's validation target and almost its exact protocol.**
- Follow-ups: "Onsager coefficients of a finite-time Carnot cycle," *PRE* 80, 021121 (2009)
  (tight coupling explains reaching CA in linear response); an endoreversible model in *NJP*
  17 (2015).

### Osmosis
- Itano et al., *J. Phys. Soc. Jpn.* (2008): MD of an ideal solution and pure solvent separated
  by a semipermeable membrane made of "phantom molecules" that reflect solute only. van 't Hoff
  holds up to about 0.1 mole fraction, with a non-uniform balance of solute and solvent partial
  pressures. [abstract]
- [arXiv:1111.2705](https://arxiv.org/pdf/1111.2705): a membrane as a smooth confining potential
  on the solute; osmotic pressure from local pressures via the virial. [abstract]
- Yoshida, Marbach & Bocquet (2017),
  [arXiv:1703.02759](https://arxiv.org/pdf/1703.02759): partially permeable membranes and the
  reflection coefficient (Kedem–Katchalsky); van 't Hoff at full semipermeability. [abstract]
- [arXiv:1207.6998](https://arxiv.org/pdf/1207.6998), "Osmosis in a minimal model system": van
  't Hoff holds only at low concentration; deviations signal solute–solute interactions. [abstract]

### Diffusion finite-size correction
- Yeh & Hummer, *J. Phys. Chem. B* (2004): for a cubic periodic box,
  **D_∞ = D_PBC + 2.837 k_BT/(6πηL)**. Tested on water, LJ and hard spheres. Needed for any
  quantitative Stokes–Einstein page. [abstract; volume and page not retrieved]

---

## 12. Source reading: ALCHEMI Toolkit-Ops and HOOMD-blue

### NVIDIA ALCHEMI Toolkit-Ops
[github.com/NVIDIA/nvalchemi-toolkit-ops](https://github.com/NVIDIA/nvalchemi-toolkit-ops),
Apache-2.0, 2025–2026. Written in Warp. [read module and kernel docstrings]
- **Neighbor modules:** `naive` (all pairs), `naive_dual_cutoff`, `cell_list`, `cluster_tile`,
  `rebuild`, each with **batched** variants.
- **Cluster tiles** ("GROMACS-style NxM cluster pair list: Morton-sorted atoms grouped into
  32-atom clusters"):
  1. Morton codes, then sort (fixed 1024- and 2048-element single-block tile sorts for small
     systems).
  2. Gather into SoA arrays.
  3. An axis-aligned bounding box per 32-atom cluster.
  4. Enumerate tile pairs (row group, column group) whose box distance is within the cutoff.
  5. Query the tiles into a per-atom neighbor matrix, or into a COO pair list via a count pass
     then a fill pass.

  float32 only.
- **Batching:** systems are concatenated and addressed by a CSR `batch_ptr`; per-system
  parameters such as the timestep are looked up by `batch_idx`. Buffer capacities come from
  density and cutoff estimates with a safety factor (`estimate_max_tiles_per_group`,
  `estimate_batch_cluster_tile_segments`). Overflow raises `TileBufferOverflow` or
  `NeighborOverflowError`, the same fixed-capacity pattern as JAX MD. **This is the layout to
  copy for our replicas.**
- **Cell list:** stages are `estimate_sizes` → `construct_bin_size` → `count_atoms_per_bin` →
  `bin_atoms` (count, scan, scatter), with atom-centric and pair-centric query kernels.
  - A grid-selection cost model (`_grid_selection.py`): C cells, stencil volume S, occupancy
    q = ⌈N/C⌉, estimated cost C·S·(setup_warps + active_warp_passes·q).
  - It **"assumes approximately uniform occupancy"**, which our liquid–vapor systems violate.
- **Rebuild detection:** for the neighbor list, rebuild when any atom moves beyond the skin from
  its reference position (minimum image); for the cell list, rebuild when any atom changes cell
  coordinates.
- **Integrators:** two-pass velocity Verlet (drift with half kick; user computes forces; final
  half kick), Langevin, Nosé–Hoover chains, NPT/NPH (MTK), velocity rescaling, plus FIRE, FIRE2
  and L-BFGS optimizers and constraints. Batched kernels index per-system parameters; "ptr"
  variants run one thread per small system.
- Elaborate certified triclinic minimum-image logic (QR, Babai nearest plane). We don't need it
  for rectangular boxes.

### HOOMD-blue
[github.com/glotzerlab/hoomd-blue](https://github.com/glotzerlab/hoomd-blue), BSD-3. [read file
layout, headers, and parts of the HPMC GPU integrator]
- Neighbor lists: `NeighborListGPUBinned` (cell list), `NeighborListGPUStencil` (stenciled cells
  for multiple cutoffs), `NeighborListGPUTree` (LBVH, Howard et al. 2016). Cells: `CellList`,
  `CellListGPU`, `CellListStencil`.
- `SFCPackTuner`: periodically reorders particle data along a space-filling curve on a grid for
  memory locality (Hilbert in 3D [memory for the curve type]).
- `Autotuner`: runtime autotuning of block sizes and similar parameters. We could autotune
  workgroup sizes at startup using `timestamp-query`.
- **HPMC GPU** (`hpmc/IntegratorHPMCMonoGPU.h`). Semantics are inferred from the code structure:
  - The particle update order is shuffled every step, seeded by the timestep.
  - Trial moves are proposed for all particles in parallel; moves that leave the particle's
    cell are rejected (`reject_out_of_cell`).
  - Overlap checks against "expanded cells" iterate until the reject flags converge (a
    `while (!converged)` loop swapping reject in/out buffers). This resolves conflicts as a
    sequential sweep in the shuffled order would.
  - **A random grid shift each sweep** (`hpmc_shift`, `translateOrigin`) keeps cell boundaries
    from biasing the dynamics.
  - This looks like an evolution of the 2013 checkerboard scheme; read the kernels before
    porting.
- RNG: Philox4x32 via OpenRAND, seeded by (class id, timestep, user seed), with the particle
  tag in the counter.

---

## 13. WebGPU compute portability

### WGSL numerics ([WGSL spec](https://gpuweb.github.io/gpuweb/wgsl/)) [read]
- f32 `+ − ×` are correctly rounded; `/` is within 2.5 ULP; transcendentals have looser bounds
  (e.g., `atan` 4096 ULP, `cos` absolute error 2⁻¹¹ on [−π, π]).
- **§15.7.5 Reassociation and Fusion: "An implementation may reassociate operations. An
  implementation may fuse operations if the transformed expression is at least as accurate as
  the original formulation."** So f32 results are **not** guaranteed bit-identical across
  browsers, GPUs, or between GPU and CPU, even for plain `a*b + c`.
- Dawn on macOS 15+/iOS 18+ emits `#pragma METAL fp math_mode(relaxed|safe)` from a
  `useStrictMath` flag, and a follow-up commit says fast math defaults to true
  ([commit](https://dawn.googlesource.com/dawn/+/58c1c685a8dab745a8c22d231b7b1cd68bfc977a%5E%21),
  [fix](https://dawn.googlesource.com/dawn/+/13298c36dfff8b19f8867867ebaeb680849a4b80)). Apple:
  `fastMathEnabled = true` means `MTLMathMode.fast`
  ([docs](https://developer.apple.com/documentation/metal/mtlcompileoptions/mathmode.md)).
- **Integers:** i32/u32 are two's complement with defined wrapping on overflow. **Float→int
  conversion is fully defined** (clamps to range, rounds toward zero; NaN is indeterminate).
  Integer paths are therefore bit-exact everywhere.
- **Atomics:** only `atomic<i32>` and `atomic<u32>`. `atomic<vec2<u32>>` (64-bit) exists only for
  **min/max** via the `atomic_vec2u_min_max` extension, with no 64-bit add. So two-word
  fixed-point accumulation (catalog #2) is still required.

### Default limits ([WebGPU spec](https://gpuweb.github.io/gpuweb/)) [read]

| Limit | Default |
|---|---|
| `maxComputeWorkgroupStorageSize` | 16384 bytes |
| `maxComputeInvocationsPerWorkgroup` | 256 |
| `maxComputeWorkgroupSizeX` | 256 |
| `maxComputeWorkgroupsPerDimension` | 65535 |
| `maxStorageBufferBindingSize` | 134217728 (128 MiB) |
| `maxBufferSize` | 268435456 (256 MiB) |
| `maxStorageBuffersPerShaderStage` | 8 |

Adapters often allow more if requested. At 32 B of state per particle, 128 MiB is about 4M
particles per binding.

Optional features: `timestamp-query` (benchmarks, autotuning), `subgroups`, `shader-f16`.

### Subgroups
Supported in **Chrome and Edge 144+ only**; not in Safari or Firefox per
[caniuse](https://caniuse.com/wf-webgpu-subgroups).
- Chromium has intents to ship for
  [`subgroup_id`](https://groups.google.com/a/chromium.org/g/blink-dev/c/SV75BHCUJz0/m/pUsyjxvoBQAJ)
  and [subgroup size control](https://groups.google.com/a/chromium.org/g/Blink-dev/c/Lx06y9Rdzvs)
  (June 2026; positive signals from Gecko and WebKit).
- **The engine must not require subgroups;** they can only be an optional fast path.

### Forward progress and scans
- WebGPU gives no guarantee that one workgroup will eventually run while another spins waiting
  for it, and Apple GPUs lack that guarantee in practice. So the single-pass chained scan with
  **decoupled look-back** (Merrill & Garland,
  [NVR-2016-002](https://research.nvidia.com/publication/2016-03_single-pass-parallel-prefix-scan-decoupled-look-back),
  used in CUB) can starve or deadlock.
- **Smith, Levien & Owens, "Decoupled Fallback: A Portable Single-Pass GPU Scan," SPAA 2025**
  (Google / UC Davis; [paper source and artifact](https://github.com/b0nes164/Decoupled-Fallback-Paper),
  [eScholarship](https://escholarship.org/uc/item/0bk9z4bt)). [read abstract]
  - Work-stealing fallback for hardware without forward-progress guarantees.
  - A tile-state representation that needs **no 64-bit atomics or memory barriers** for
    correctness; a subgroup-size-agnostic intra-workgroup implementation.
  - Near-memcpy inclusive scan on M1 Max and M3 under Dawn, approaching the expected 50% speedup
    over reduce-then-scan; robust when workgroups are blocked up to 50% of the time.
  - The artifact requires subgroups and timestamp queries.
- Related code by Thomas Smith (b0nes164):
  [GPUPrefixSums](https://github.com/b0nes164/GPUPrefixSums) (CUDA/D3D12/Unity/WGPU, about 300
  stars), [GPUSorting](https://github.com/b0nes164/GPUSorting) (OneSweep and segmented sort,
  about 480 stars), [ShaderOneSweep](https://github.com/b0nes164/ShaderOneSweep).
- **Plan:** a portable baseline with **reduce-then-scan** (no subgroups, no forward-progress
  assumption); Decoupled Fallback as the fast path where subgroups exist.

### Consequences for bit-exactness (see `architecture.md`)
- **Tier 1, device-local determinism and exact reversal.** The same compiled shader with the
  same inputs gives the same outputs. If forces are a pure function of positions (fixed-point,
  order-independent accumulation), **exact reversal works on any single device, even with f32
  force math.**
- **Tier 2, cross-platform bit-exactness** (GPU ↔ CPU twin, browser ↔ browser, Mac ↔ PC).
  Because WGSL permits fusion and reassociation, every operation that affects the trajectory must
  be **integer**: integer displacement → integer r² → table index plus fixed-point interpolation
  → fixed-point force. This is the Anton approach; the cost on GPUs needs benchmarking.

---

## 14. Recent NVIDIA work relevant to the design

- **[Warp](https://github.com/NVIDIA/warp)** (Python GPU simulation framework, Apache-2.0).
  - [v1.16.0](https://github.com/NVIDIA/warp/discussions/1716): **grouped `HashGrid` queries**,
    so independent environments share one spatial grid and skip other groups' candidates. That's
    our replica-batching problem. `reserve(..., with_groups=True)` allows graph capture without a
    warm-up build.
  - Queries return cell candidates, so kernels still test the actual distance.
  - Tile primitives since 1.5 (linear algebra via MathDx); NumPy-style tile slicing in 1.16;
    in-place rebuild of fixed-capacity NanoVDB volumes.
  - [Changelog](https://nvidia.github.io/warp/latest/project/changelog.html),
    [tiles docs](https://nvidia.github.io/warp/latest/user_guide/programming_model/tiles.html).
    GPU runs need CUDA, so it's not usable on a Mac.
- **ALCHEMI** ([overview](https://developer.nvidia.com/cuda/cuda-x-libraries/alchemi),
  [Toolkit blog](https://developer.nvidia.com/blog/building-custom-atomistic-simulation-workflows-for-chemistry-and-materials-science-with-nvidia-alchemi-toolkit/),
  [Toolkit-Ops blog](https://developer.nvidia.com/blog/accelerating-ai-powered-chemistry-and-materials-science-simulations-with-nvidia-alchemi-toolkit-ops),
  [batched MD NIM](https://docs.nvidia.com/nim/alchemi/alchemi-bmd/latest/overview.html)).
  PyTorch-native atomistic workflows with a GPU-native **batched** dynamics engine
  (`nvalchemi-toolkit` on PyPI). Toolkit-Ops is the Warp kernel layer (§12). Vendor-reported:
  about 33× speedup for batched small systems (Orbital). Ewald, PME and DSF electrostatics; the
  MTK barostat.
- **CUB/CCCL:** decoupled look-back scan (above); **Onesweep** LSD radix sort (Adinets &
  Merrill, [arXiv:2206.01784](https://arxiv.org/abs/2206.01784v1), 2022; about 2n global
  reads/writes per digit pass; [GTC 2020 talk](https://developer.download.nvidia.com/video/gputechconf/gtc/2020/presentations/s21572-a-faster-radix-sort-implementation.pdf);
  a variant ships in Intel oneDPL). It needs a forward-progress-safe scan before use on WebGPU.
- **CUDA Tile / cuTile Python** ([docs](https://docs.nvidia.com/cuda/cutile-python/),
  [blog](https://developer.nvidia.com/blog/simplify-gpu-programming-with-nvidia-cuda-tile-in-python),
  [CUDA Tile](https://developer.nvidia.com/cuda/tile)): a tile-based model with the Tile IR
  compiler stack, announced March 2025 and released with CUDA 13.1 (PyPI `cuda-tile`). It
  targets Blackwell and Ampere/Ada. Conceptually aligned with cluster-pair tiles; not usable
  from WebGPU.
- **Matthias Müller (NVIDIA), *Ten Minute Physics***: self-contained HTML/JavaScript physics demos
  (hard and soft bodies, cloth, fluids; three.js for 3D). Video 11 covers spatial hashing. Co-author
  of Teschner et al., "Optimized Spatial Hashing for Collision Detection of Deformable Objects"
  (2003). A model for how small and teachable a demo page can be.
  ([forum](https://discourse.threejs.org/t/ten-minute-physics-youtube-channel/35450),
  [Gigazine](https://gigazine.net/gsc_news/en/20221212-ten-minute-physics/))
- Hoetzlein's fixed-radius nearest-neighbor grid (§3) and the Schrödinger/NVIDIA Desmond GTC
  talk (§5) also belong here.

---

## 15. Page design and physics education research

### Interactive-simulation design (PhET)
- Wieman, Adams & Perkins, "PhET: Simulations That Enhance Learning," *Science* 322, 682–683
  (2008), [doi:10.1126/science.1161948](https://doi.org/10.1126/science.1161948). [abstract]
- Adams et al., "A Study of Educational Simulations Part I: Engagement and Learning" and "Part
  II: Interface Design," *J. Interactive Learning Research* 19 (2008). Based on 200+ (later
  275+) think-aloud student interviews.
  ([ComPADRE](https://www.compadre.org/portal/items/detail.cfm?ID=14284)) [abstract]
- Wieman, Perkins & Adams, *Am. J. Phys.* 76 (2008): design philosophy, development and testing
  process. [abstract]

### Documented student difficulties in thermodynamics
Each of these is a target for a page.
- **Entropy of system vs surroundings vs universe:** students struggle to tell them apart, and
  many treat the entropy of the universe as **conserved** (Christensen, Meltzer & Ogilvie,
  introductory course). Upper-level students make the same confusion (Bucy et al.).
  → **The heat-ledger instrument shows ΔS_system, ΔS_bath and ΔS_total side by side.**
- **Carnot and the second law:** upper-division students "do not clearly articulate the
  connection between the Carnot cycle and the second law" after lecture (Smith, Christensen,
  Mountcastle & Thompson, *PRST-PER* 2015). → The Carnot page.
- **First law:** up to 80% of introductory students could not apply it after instruction, partly
  from mixing up state functions and process quantities (Meltzer 2004/2008). → Pages on "heat vs
  energy vs enthalpy" and path dependence.
- **Brundage, Meltzer & Singh (2024),
  [arXiv:2408.00944](https://arxiv.org/pdf/2408.00944):** a validated 78-item survey given to more
  than 1000 students. Associating entropy with energy leads to "conservation of entropy" errors,
  and students widely fail to apply ΔS = Q_rev/T. → **A possible pre/post evaluation
  instrument for our pages.**
- Loverude, Kautz & Heron, *Am. J. Phys.* 70 (2002), first law and adiabatic compression [memory].
- Sources: [DOAJ PRST-PER 2015](https://doaj.org/article/f56cfd2786c14bc79c39c353b2c71b6a),
  [Rowan](https://rdw.rowan.edu/csm_facpub/36),
  [UMaine](https://digitalcommons.library.umaine.edu/phy_facpub/3),
  [ASU](https://asu.elsevierpure.com/en/publications/investigating-and-addressing-learning-difficulties-in-thermodynam-2).
  [abstracts]

### Craft references to study [memory]
Bartosz Ciechanowski's explainers; Bret Victor, "Explorable Explanations" (2011); Nicky Case;
Ten Minute Physics (§14).

---

## 16. Tooling

- **Rust:** [`wgpu`](https://crates.io/crates/wgpu) 30.0.1 and `naga` 30.0.1 (2026-08-22). The
  WebGPU implementation behind **Firefox** (Mozilla pinned a wgpu release from 2026-03-18 in
  [moz.yaml](https://kdab.searchfox.org/firefox-main/source/gfx/wgpu_bindings/moz.yaml)).
  - Runs natively on Vulkan, Metal, DX12 and GL, and in the browser via WebAssembly on top of
    the browser's WebGPU. [wgpu-native](https://gittrend.io/repo/gfx-rs/wgpu-native) offers a C
    API.
  - Native `wgpu` translates WGSL through naga, while Chrome uses Dawn/Tint, so the two shader
    compilers can differ. Test both.
- **[rust-gpu](https://github.com/Rust-GPU/rust-gpu)** (Rust as a shader language → SPIR-V;
  about 3.4k stars, active, "🚧"). Not a dependable path to the web yet; write WGSL.
- **Node/JS:** npm [`webgpu`](https://www.npmjs.com/package/webgpu) 0.6.2 (2026-10-02,
  `dawn-gpu/node-webgpu`), Dawn for Node, for headless WebGPU in Node.
  - Dawn's own `dawn.node` is a source build and runs the WebGPU conformance tests; SwiftShader
    provides a software Vulkan device for CI (`VK_ICD_FILENAMES`)
    ([README](https://dawn.googlesource.com/dawn/+/refs/heads/chromium/4688/src/dawn_node/README.md)).
  - Deno has built-in WebGPU [memory].
- **TypeGPU** 0.12.8 (2026-10-16): typed WGSL from TypeScript [memory: by Software Mansion].
  Probably unnecessary if the core is Rust.
- **Testing:** Playwright and headless Chrome for WebGPU-vs-CPU parity (as symdjs does); native
  `wgpu` for fast headless validation runs.

---

## 17. Open items to verify

- Exact GROMACS cluster sizes (CPU 4×4/4×8, GPU 8×8 super-clusters): read Páll & Hess 2013.
- TIP3P, SPC/E and TIP4P/Ice melting points and the exact 2005 Vega–Sanz–Abascal citation.
- Levesque & Verlet 1993 bit-reversible algorithm: read for the exact rounding scheme.
- Truncated-and-shifted 2.5σ LJ critical point and coexistence (Vrabec et al. 2006); 2D LJ
  triple point (Li & Pica Ciamarra 2020); the Smit & Frenkel cutoff.
- Yeh & Hummer 2004 volume and page.
- HOOMD HPMC GPU kernels in detail (`IntegratorHPMCMonoGPUMoves.cuh`, the narrow-phase kernel):
  confirm the convergence-loop semantics and detailed balance.
- Whether naga (native `wgpu`) or Tint (Chrome) fuse or reassociate in practice, and whether
  `wgpu` on Metal enables fast math. Empirically: run the same f32 kernel on Chrome, Firefox,
  Safari and native `wgpu`, and compare bits.
- Cost of an all-integer force path vs f32 on Apple, NVIDIA and AMD GPUs (benchmark).
- Whether any newer WebGPU MD engine has appeared (repeat the search periodically).
---

## 18. Bibliography (consolidated)

### DESRES
1. Bowers, K. J., et al. "Scalable Algorithms for Molecular Dynamics Simulations on Commodity Clusters." SC06 (2006). https://ieeexplore.ieee.org/document/4090217
2. Lippert, R. A., et al. "A common, avoidable source of error in molecular dynamics integrators." *J. Chem. Phys.* 126, 046101 (2007). https://doi.org/10.1063/1.2431176
3. Shaw, D. E., et al. "Anton, a special-purpose machine for molecular dynamics simulation." *Commun. ACM* (2008). https://cacm.acm.org/research/anton-a-special-purpose-machine-for-molecular-dynamics-simulation
4. Salmon, J. K., Moraes, M. A., Dror, R. O., Shaw, D. E. "Parallel random numbers: as easy as 1, 2, 3." SC11 (2011). https://users.cs.utah.edu/~hari/teaching/bigdata/random123sc11.pdf
5. Chow, E., et al. "Desmond Performance on a Cluster of Multicore Processors." DESRES/TR--2008-01 (2008).
6. Bergdorf, M., Kim, E. T., Rendleman, C. A., Shaw, D. E. "Desmond/GPU Performance as of November 2014." DESRES/TR--2014-01.
7. Bergdorf, M., Baxter, S., Rendleman, C. A., Shaw, D. E. "Desmond/GPU Performance as of October 2015." DESRES/TR--2015-01.
8. Bergdorf, M., Baxter, S., Rendleman, C. A., Shaw, D. E. "Desmond/GPU Performance as of November 2016." DESRES/TR--2016-01.
9. Bergdorf, M., Robinson-Mosher, A., Guo, X., Law, K.-H., Shaw, D. E. "Desmond/GPU Performance as of April 2021." DESRES/TR--2021-01.
10. Predescu, C., et al. "The u-series: A separable decomposition for electrostatics computation with improved accuracy." *J. Chem. Phys.* 152, 084113 (2020).
11. Predescu, C., Bergdorf, M., Shaw, D. E. "Midtown splines: An optimal charge assignment for electrostatics calculations." *J. Chem. Phys.* 153, 224117 (2020).
12. Tu, J., Russell, E. "S61156" (GTC 2024, Desmond GPU optimization). https://developer-blogs.nvidia.com/wp-content/uploads/2024/10/S61156.pdf

### GPU algorithms and libraries
13. Baxter, S. "moderngpu 2.0" (2016). https://github.com/moderngpu/moderngpu/wiki ; 1.0 docs: https://moderngpu.github.io/
14. Hoetzlein, R. "Fast Fixed-Radius Nearest Neighbors: Interactive Million-Particle Fluids." GTC 2014. https://ramakarl.com/pdfs/2014_Hoetzlein_FastFixedRadius_Neighbors.pdf
15. Lewin, C. "A Position Based Material Point Method." SIGGRAPH 2024. https://github.com/electronicarts/pbmpm

### MD engines
16. Páll, S., Hess, B. "A flexible algorithm for calculating pair interactions on SIMD architectures." *Comput. Phys. Commun.* (2013). https://doi.org/10.1016/j.cpc.2013.06.003
17. Páll, S., et al. "Heterogeneous parallelization and acceleration of molecular dynamics simulations in GROMACS." *J. Chem. Phys.* 153, 134110 (2020). https://arxiv.org/abs/2006.09167
18. Kutzner, C., et al. "More bang for your buck: Improved use of GPU nodes for GROMACS 2018." https://arxiv.org/abs/1903.05918
19. Eastman, P., Pande, V. S. "Efficient nonbonded interactions for molecular dynamics on a graphics processing unit." *J. Comput. Chem.* (2010).
20. Le Grand, S., Götz, A. W., Walker, R. C. "SPFP: Speed without compromise—A mixed precision model for GPU accelerated molecular dynamics simulations." *Comput. Phys. Commun.* 184, 374–380 (2013). https://doi.org/10.1016/j.cpc.2012.09.022
21. Tinker-HP GPU paper. https://arxiv.org/abs/2011.01207
22. Anderson, J. A., Lorenz, C. D., Travesset, A. "General purpose molecular dynamics simulations fully implemented on graphics processing units." *J. Comput. Phys.* 227 (2008).
23. Anderson, J. A., Irrgang, M. E., Glotzer, S. C. "Scalable Metropolis Monte Carlo for simulation of hard shapes." https://arxiv.org/abs/1509.04692
24. Howard, M. P., et al. "Efficient neighbor list calculation for molecular simulation of colloidal systems using graphics processing units." *Comput. Phys. Commun.* 203, 45–52 (2016). https://par.nsf.gov/servlets/purl/10078947

### Hard disks / 2D melting
25. Engel, M., et al. "Hard-disk equation of state: First-order liquid-hexatic transition in two dimensions with three simulation methods." *Phys. Rev. E* 87, 042134 (2013). https://arxiv.org/abs/1211.1645
26. Anderson, J. A., et al. "Massively parallel Monte Carlo for many-particle simulations on GPUs." *J. Comput. Phys.* (2013).
27. Bernard, E. P., Krauth, W. "Two-step melting in two dimensions: First-order liquid-hexatic transition." *Phys. Rev. Lett.* 107, 155704 (2011).

### Models
28. Vega, C., et al. Melting temperatures of common water models. *J. Chem. Phys.* 122, 234511 (2005).
29. Molinero, V., Moore, E. B. "Water modeled as an intermediate element between carbon and silicon" (mW). *J. Phys. Chem. B* 113, 4008 (2009).
30. Cox, S. J., White, A. D. Symmetric molecular dynamics. *J. Chem. Theory Comput.* 18, 4077 (2022). https://doi.org/10.1021/acs.jctc.2c00401
31. Levesque, D., Verlet, L. "Molecular dynamics and time reversibility." *J. Stat. Phys.* 72, 519 (1993).
32. Feenstra, K. A., Hess, B., Berendsen, H. J. C. Hydrogen mass repartitioning. *J. Comput. Chem.* 20, 786 (1999).

### Teaching
33. Moore, T. A., Schroeder, D. V. "A different approach to introducing statistical mechanics." *Am. J. Phys.* (1997) [memory: check year and link]
34. Schroeder, D. V. *An Introduction to Thermal Physics*; Interactive MD: https://physics.weber.edu/schroeder/md/interactivemd.html
35. Gould, H., Tobochnik, J. *Statistical and Thermal Physics*, 2nd ed. (2021).

### Validation data
36. Potoff, J. J., Panagiotopoulos, A. Z. Critical point and phase behavior of the pure fluid and a Lennard-Jones mixture. *J. Chem. Phys.* (1998).
37. NIST Standard Reference Simulation Website, SRD 173. doi:10.18434/mds2-232. https://mmlapps.nist.gov/srs/LJ_PURE/index.htm
38. Smit, B., Frenkel, D. "Vapour-liquid equilibria of the two-dimensional Lennard-Jones fluid." *J. Chem. Phys.* 94, 5663 (1991).
39. Smit, B. "Phase diagrams of Lennard-Jones fluids." *J. Chem. Phys.* (1992).
40. Li, Y.-W., Pica Ciamarra, M. "Phase behavior of Lennard-Jones particles in two dimensions." *Phys. Rev. E* 102, 062101 (2020). https://arxiv.org/abs/2010.03876
41. Blazquez, S., Vega, C. "Melting points of water models: Current situation." *J. Chem. Phys.* 156, 216101 (2022). https://doi.org/10.1063/5.0093815
42. Izumida, Y., Okuda, K. "Molecular kinetic analysis of a finite-time Carnot cycle." *EPL* (2008). https://arxiv.org/abs/0802.3759
43. Izumida, Y., Okuda, K. "Onsager coefficients of a finite-time Carnot cycle." *Phys. Rev. E* 80, 021121 (2009).
44. Itano, T., et al. Nanoscale osmosis MD. *J. Phys. Soc. Jpn.* (2008).
45. Yoshida, H., Marbach, S., Bocquet, L. "Osmotic and diffusio-osmotic flow generation at high solute concentration. II. Molecular dynamics simulations." (2017). https://arxiv.org/abs/1703.02759
46. Yeh, I.-C., Hummer, G. System-size dependence of diffusion coefficients and viscosities from MD with periodic boundary conditions. *J. Phys. Chem. B* (2004).

### WebGPU / GPU primitives
47. W3C. WebGPU Shading Language (WGSL). https://gpuweb.github.io/gpuweb/wgsl/ ; WebGPU. https://gpuweb.github.io/gpuweb/
48. Merrill, D., Garland, M. "Single-pass Parallel Prefix Scan with Decoupled Look-back." NVIDIA NVR-2016-002 (2016).
49. Smith, T., Levien, R., Owens, J. D. "Decoupled Fallback: A Portable Single-Pass GPU Scan." SPAA 2025. https://github.com/b0nes164/Decoupled-Fallback-Paper
50. Adinets, A., Merrill, D. "Onesweep: A Faster Least Significant Digit Radix Sort for GPUs." (2022). https://arxiv.org/abs/2206.01784
51. NVIDIA Warp. https://github.com/NVIDIA/warp
52. NVIDIA ALCHEMI Toolkit-Ops. https://github.com/NVIDIA/nvalchemi-toolkit-ops
53. NVIDIA cuTile Python. https://docs.nvidia.com/cuda/cutile-python/
54. Teschner, M., Heidelberger, B., Müller, M., Pomeranets, D., Gross, M. "Optimized Spatial Hashing for Collision Detection of Deformable Objects." (2003).

### Education research
55. Wieman, C. E., Adams, W. K., Perkins, K. K. "PhET: Simulations That Enhance Learning." *Science* 322, 682–683 (2008).
56. Adams, W. K., et al. "A Study of Educational Simulations Part I/II." *J. Interactive Learning Research* 19 (2008).
57. Christensen, W. M., Meltzer, D. E., Ogilvie, C. A. Student ideas regarding entropy and the second law in an introductory physics course. *Am. J. Phys.* (2009) [memory for venue].
58. Smith, T. I., Christensen, W. M., Mountcastle, D. B., Thompson, J. R. Heat engines and the Carnot cycle, upper division. *Phys. Rev. ST Phys. Educ. Res.* (2015).
59. Meltzer, D. E. Students' reasoning regarding heat, work, and the first law. *Am. J. Phys.* (2004).
60. Brundage, M. J., Meltzer, D. E., Singh, C. Introductory and advanced students' difficulties with entropy and the second law, validated instrument. (2024). https://arxiv.org/abs/2408.00944
61. Loverude, M. E., Kautz, C. H., Heron, P. R. L. Student understanding of the first law of thermodynamics. *Am. J. Phys.* 70 (2002) [memory].
