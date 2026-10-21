# Status

Micrographia: pages on the Rahman engine.

What exists, what it has been checked against, and what comes next. Numbers are from an Apple
M1 Pro (Metal) unless stated. Run everything with `cargo xtask all`.

## Pages

| Page | Question | Checked in the browser test |
|---|---|---|
| `reversal` | Why can't you un-stir cream from coffee? | Forward + reverse restores every bit; a one-bit nudge spreads to all 8,000 molecules |
| `corner` | Could all the air rush into one corner? | Tiny molecules (0.25 σ): all-left share 0.85–1.1 of 2⁻ᴺ (0.89); 1 σ molecules: 0.46 of 2⁻ᴺ but 0.98 of the excluded-area prediction |
| `boltzmann` | Where does the Boltzmann distribution come from? | 48 boxes behind diathermal walls: from equal energies, entropy rises from −2.4 to within 0.01 of its ceiling and energies come within 2.4% of e^(−E/kT); one hot box (kT 19.8) cools to 1.8; total energy holds to 0.01–0.3% |
| `heavy` | Does a heavy molecule have less energy? | Light and 16× heavy species within 5% of equal kinetic energy; σ = 6, m = 400 balls converge more slowly |
| `heatflow` | Can heat flow from cold to hot? | ~20% of 1–2 τ windows run backwards; fitted ln P(Q)/P(−Q) slope within 35% of 1/T_c − 1/T_h (measured 0.367 vs 0.333) |
| `oilwater` | Why don't oil and water mix? | Weak cross-attraction separates (segregation > 0.3 after 200 τ); equal attraction stays mixed |
| `osmosis` | Where does the osmotic push come from? | Membrane push within 25% of van 't Hoff (measured 0.041 vs c kT = 0.039) |
| `compress` | Why does a bike pump get hot, a spray can cold? | Compression heats along T·A ≈ const (2%) |
| `rubber` | Why does a rubber band pull harder when warm? | Chain tension at T = 2 is 1.5–2.8× that at T = 1; fast stretching warms the chains by >3% |
| `atmosphere` | Why does air thin out with height? | Each species' barometric slope within 15% of −mg/kT; top and bottom temperatures within 8% |
| `carnot` | Why is the Carnot engine the most efficient? | Slow cycles reach 70–105% of 1 − T_c/T_h (0.31 vs 0.33); fast cycles do worse |
| `boiling` | Why does the boiling point rise with pressure? | Vapour pressure rises >1.8× from kT = 0.40 to 0.45; Clausius–Clapeyron latent heat 1.5–6 ε (~4) |
| `lipids` | How do membranes assemble themselves? | >85% of lipids in aggregates from a random start |
| `balloon` | Why does a helium balloon go flat faster? | Leak-rate ratio within −30%/+15% of Graham's √(mass ratio) (2.67 vs 2.69) |
| `smell` | How long would a smell take to cross a room? | Still-air drift within 4 standard errors of zero; a draught beats diffusion |
| `fep` | How do computers predict which drug binds best? | TI and FEP agree within 0.15 kT and with direct counting within 0.5 kT (−1.47, −1.47, −1.60) |
| `saline` | Why are IV drips saline? | Model cell area in water > saline > brine (280, 254, 176 σ²) |
| `arrhenius` | Why do reactions double in speed every 10 °C? | Escape rates fall monotonically from kT = 1.6 to 0.8; Arrhenius slope 4.3–6.5 ε (4.9 in Chrome and WebKit; native 5.4) |
| `buffer` | How does a buffer hold blood's pH steady? | [H][A]/[HA] constant within 1.8× up to equivalence; at 24 protons per box <25% as many free as in water (2.5 vs 24); Henderson–Hasselbalch from K within 0.15 |
| `mask` | How can a mask stop particles smaller than its holes? | Particles 3–27× smaller than the gaps caught on >20% of passes (~40%); the largest ≥0.2 more often (0.76) |
| `dna` | How does PCR copy DNA; why do strands melt? | 8-bead strands melt ≥0.12 kT/ε above 4-bead ones (0.94 vs 0.73); single beads <20% paired at every temperature |
| `speeds` (2D and 3D) | Why is the most likely energy zero but the most likely speed not? What is a gas molecule's average energy? | Average energy 1.00 kT (2D) and 1.50 kT (3D); most common energy 0.05 (2D) and 0.43 (3D, ½kT = 0.48); most common speed 1.03 and 1.40 (√(kT/m) = 1.00, √(2kT/m) = 1.38) |
| `freezing` (3D) | Why does salt lower the melting point? | At kT 0.53 the pure liquid freezes onto the crystal (92% of the solvent crystalline); with 600 salt atoms poured in, it stops at 51% |
| `enthalpy` | Why enthalpy? What is special about G? | Heated alike, slowly: ΔT per unit heat per molecule 0.99 under a locked lid, 0.46 under a free weighted one (2D ideal: 1 and 0.5) |
| `crystal` (3D) | What happens when a crystal melts; why does a cooled liquid wait? | Cold FCC crystal stays >85% crystalline (96%, kT 0.33); 1.6 ε per atom of heat leaves kT stalled below 0.7 (0.62) with the crystal melting (35% left) |
| `coexistence` | Why do liquid and vapour coexist; what happens at the critical point? | Lattice gas (Kawasaki MC on the GPU): liquid and vapour densities within 0.008 of Onsager's at kT = 0.45 and 0.5 (0.976/0.024 vs 0.978/0.022; 0.958/0.042 vs 0.956/0.044). Lattice Boltzmann (Shan–Chen): a pool on a 1.1 T_c plate grows vapour under it within 4,000 steps (bottom 1.09 T_c, ρ 0.016 under liquid at 0.30). Gibbs ensemble: vapour 0.026 and liquid 0.737 at kT 0.40 against MD's 0.023 and 0.742 |

Plus `selftest`: the GPU primitive and engine checks, run under the browser's shader compiler.

## Engine

- **`gpu-int-f32`** (crate `md`): 2D, integer positions (64-bit, 2⁻³² σ) and velocities
  (displacement per step), f32 pair forces turned into integer kicks per pair. Exactly
  reversible, independent of storage order, exact momentum conservation for equal masses.
- **Force paths:** cell list every step, or Verlet lists with a skin whose rebuild is decided on
  the GPU (indirect dispatch, no host round trip). Chosen by size (Verlet from 24k particles).
  The two paths give bit-identical trajectories. Storage is periodically reordered along a
  Morton curve; particle ids keep results independent of it.
- **Physics features:** species-pair LJ/WCA tables; gravity; walls per axis (hard, soft WCA,
  thermal bath); compartments (many independent replicas in one run); a semi-permeable
  membrane; a piston (fixed speed or free under a load); an Andersen bulk thermostat; a ledger
  of energy, impulse and hits per wall and compartment, in exact fixed point.
- **3D** (`GpuEngine3`, `md3.wgsl`; `md_core::three`): the same integer leapfrog in 3D, with a
  cell list over 3×3×3 cells, LJ/WCA pairs, hard, soft or heat-bath walls per axis, gravity,
  an Andersen bath, pinned species, region kicks and exact reversal. Its own kernels and buffers,
  so 2D is untouched (an A/B benchmark against the commit before showed no change). CPU twin
  `IntEngine3<F>`. Verlet lists (GPU-gated rebuild) or a cell list, bit-identical. Speed with
  lists: LJ (rc 2.5) at density 0.8: 4,800 steps/s at 4k, 610 at 64k; WCA: 8,400 at 4k, 3,000
  at 64k (M1 Pro, native). Renderer: lit
  sphere impostors with exact depth, an orbit camera and a cut-away. Crystal test
  (`md_core::order`): Steinhardt q6 bonds (ten Wolde–Frenkel).
- **CPU twins** (crate `md-ref`): `IntEngine<F>` (same integer scheme, f32 or f64 forces) and
  `FloatEngine<F>` (ordinary float MD). The GPU's kicks agree with the f32 twin bit for bit
  for ~70% of particles (the rest differ by a few units: different f32 rounding/fusion) and
  with the f64 twin to ~8×10⁻⁷ rms.
- **Browser:** the same WGSL runs through Chrome's compiler. On this Mac the state hash after
  1000 steps (`f26cd2e88ad2801e`) is identical natively (naga) and in Chrome (Tint). The f32
  pages also reproduce bit for bit between Chrome and WebKit, after one fix: the heat baths
  computed √(kT/m) in the shader, and Chrome's and WebKit's compiled shaders disagreed on it for
  kT = 1.6 or 2.25 (not for 1 or 4). The scales are now computed on the CPU in f64 and passed as
  uniforms.

## Physics checks (native tests)

| Check | Result |
|---|---|
| Exact reversal (CPU f32 and f64, GPU both force modes, with walls, gravity, membrane, moving piston, compartments) | bit-identical |
| Energy drift vs Δt (WCA, f64) | ratio of fluctuations at Δt and Δt/2 = 4.14 (expected 4) |
| Lyapunov exponent at the reversal page's state | 3.2 per τ |
| Maxwell–Boltzmann relaxation from equal speeds | kurtosis 1.5 → 2.95 (Gaussian: 3) |
| Equipartition, masses 1 and 10 | within 4% |
| Barometric profile (nearly ideal gas) | slope within 6% of −mg/kT |
| Thermal walls set the temperature (CPU, GPU) | 1.510, 1.516 for T = 1.5 |
| Steady heat flow between baths (GPU vs CPU) | 1.05 vs 1.10 ε/τ; in = out within 2% |
| Hard-wall pressure of a dilute gas | +6% over ρkT (second virial) |
| Andersen thermostat (CPU, GPU) | 0.807, 0.800 for T = 0.8 |
| Osmotic pressure, 5% solute (GPU) | 0.86 × c kT (non-ideal: the solute is depleted next to the membrane) |
| Free piston vs bottom-wall pressure (NVE) | within 1% |
| Slow adiabatic compression, nearly ideal gas | T·A within 3%; work done = gain in kinetic energy |

Things we learned along the way, all physical and now in the test comments: the second virial
coefficient flattens barometric profiles and bends adiabats by ~10% at densities ~0.05 for σ = 1
(so ideal-gas tests use σ = 0.4); excluded volume narrows the left/right count distribution in
small boxes; a bulk heat bath makes the gas's centre of mass diffuse, so wall forces agree only
on long averages.

## Performance

`cargo xtask bench` (2D, density 0.7, shuffled start; results logged to `bench/results.jsonl`):

| N | WCA steps/s | LJ (rc 2.5) steps/s | LJ ns per particle-step |
|---|---|---|---|
| 4,000 | 11k (cell list: 15k) | 10k | 24 |
| 64,000 | 7.2k | 4.9k | 3.2 |
| 256,000 | 3.4k | 1.8k | 2.2 |
| 1,000,000 | 858 | 471 | 2.1 |

Copy bandwidth on this device: ~167 GB/s. History: the first version (cell list every step,
no reordering) ran 1M shuffled LJ at 54 steps/s; Morton reordering brought it to 222, Verlet
lists to 470. Small systems are limited by dispatch overhead (~10 dispatches per step).
Comparisons against HOOMD/LAMMPS/OpenMM on the same hardware are not done yet (they need 3D or
a CUDA machine).

## Since the first status report

- More pages (23 in all): rubber band (entropic spring), atmosphere (barometric, isothermal
  column), Carnot engine and heat pump (0.31 vs the 0.33 limit when slow), boiling (vapour
  pressure, Clausius–Clapeyron, latent heat ~4 ε), lipid self-assembly, helium balloon
  (Graham's law, 2.67 vs 2.69), smell (diffusion vs draught), drug binding by FEP, a saline IV
  drip and a model cell, reaction rates (Arrhenius), buffers (mass action), a mask filter, DNA melting.
- Engine: bonds and pinned species, membrane pores, piston, flow-relative Andersen bath with an
  exact energy ledger, a fan (the bath restricted to some species in an x slab, with a drift), M1b all-integer forces (`Arith::Int`): GPU and CPU bit-identical, golden
  hash `3aaeff680a2f0278` reproduced natively, in Chrome and in WebKit.
- The whole browser suite passes in Chrome and in Playwright's WebKit.

## Drug binding and FEP

The `fep` page measures a relative binding free energy two ways. Direct counting uses 48 boxes,
each with a pocket and ligand A or B, and counts time bound against time far away. The alchemical
calculation runs 11 λ windows with the ligand tethered in the pocket and in bulk solvent, and
combines the two legs with TI and exponential-averaging FEP. In native tests the cycle gives
−1.19 (bound leg) − 0.28 (free leg) = −1.47 kT, against −1.21 to −1.27 kT from direct counting
(three seeds). The browser test gave TI −1.47, FEP −1.47 and direct −1.60 kT. "Unbound" has to
mean far from the pocket: ligands stuck to the pocket's outside otherwise blur the comparison
(see `crates/md/tests/binding.rs`).

## Reaction rates

The `arrhenius` page counts escapes from 768 pockets (8 σ boxes) at kT = 1.6, 1.25, 1.0 and 0.8,
sweeping one run from hot to cold. Natively: k = 0.043, 0.018, 0.0065, 0.0014 per τ bound, so
Eₐ ≈ 5.4 ε from the end points; the local slope steepens from 4.9 to 6.1 ε towards the cold end
(the prefactor depends on temperature). Two things had to be learned (`crates/md/tests/binding.rs`):

- From a start with every ligand free, the rate per unit time bound comes out high and settles
  only as the occupancy reaches equilibrium, ~500 τ at kT = 0.8. It is not ligands bouncing
  straight back out (requiring 5–10 τ in the pocket changes little): some slower relaxation in the
  bound state. Each temperature gets its own equilibration.
- The time bound has more scatter than Poisson counting suggests, because episodes are long. Many
  small boxes beat a few long runs; the solvent density per box also changes the rate (by ~2×
  between 8 σ and 16 σ boxes), as Kramers' theory of friction would have it.

## Buffers

The `buffer` page titrates in one run: 12 columns of 32 σ boxes get 0, 8, …, 88 protons each; six
rows also hold 40 anions per box, three are plain water. A proton binds in a short, deep LJ well
(σ 0.5, cut at 1 σ, ε = 8); repulsions keep one proton per anion and one anion per proton. The
measured K = [H][A]/[HA] is 0.0025 σ⁻² within ±20% up to the equivalence point while free protons
vary twelvefold, then drifts down by ~30% as free protons crowd and a few anions take two
(`crates/md/tests/buffer.rs`). The first design (a wider well, weak repulsions) let protons
bridge anion pairs and anions hold two protons, and K drifted 3.5×. Chrome reproduces the native
run bit for bit.

The boiling plateau at fixed pressure (pressure cooker) was tried and set aside for now: see
`crates/md/tests/cooker.rs`.

## Masks

The `mask` page blows air round a periodic channel with frictionless walls, using the fan,
through two staggered rows of pinned fibres (4 σ across, 8 σ gaps). Aerosols stick to a fibre on
contact through a short LJ well. A sieve with these gaps would catch none of them; per pass the
filter catches ~40% of particles 7–27× smaller than the gaps and ~3/4 of the largest. The
small-particle (diffusion) side of the most-penetrating-size curve is not resolved: even the
smallest aerosols are as heavy as an air molecule, and the conservative well lets fast light
particles bounce off. Stuck particles clog the filter, and the air slows from 0.65 to 0.45 σ/τ.

## DNA melting

The `dna` page holds one pair of bead strands per box (A beads attract only B beads; springs
along each strand and between next-nearest beads), in bands of 1, 4 and 8 beads. Swept from hot
to cold, 4-bead strands melt at kT ≈ 0.73 and 8-bead strands at 0.94, while single bead pairs
never hold (2–9% paired). Pairing is only partly cooperative: near the melting temperature
strands are mostly apart or mostly zipped with frayed ends, without real DNA's sharp
all-or-none melting (which needs base stacking). Stiffer strands melt higher but no more sharply
(`crates/md/tests/dna.rs`).

## Next

Priority: the original list of questions in `questions.md` (the first section), most of which
need phase transitions that the 2D Lennard-Jones fluid shows poorly (its liquid exists only for
kT ≈ 0.41–0.46, and 2D melting goes through a hexatic phase).

1. **3D Rahman.** The same integer scheme in 3D (positions, cell and Verlet lists, walls, piston,
   baths, compartments, bonds), with its CPU twin and the same reversal and cross-browser checks,
   and a 3D renderer (perspective, or slices).
2. **Phase transitions must be visible**, in 3D (LJ triple point ~0.69, critical ~1.31 kT/ε) and
   with a water-like model later:
   - temperature holding steady while boiling and while melting (heating at constant power);
   - boiling point vs pressure (the pressure cooker; Leh) and vapour pressure;
   - melting and freezing, supercooling and nucleation;
   - salt lowering the melting point (and barely the boiling point); dissolving and solubility;
   - the triple point.
3. Then the rest of the original list: colder at altitude and flat cloud bases, Gibbs free
   energy vs enthalpy, dQ = T dS, entropy rising with energy, Einstein 1905 (Brownian motion),
   kidneys concentrating urine.
4. Performance on phones (not measured yet).

Later: the trapdoor (Maxwell's demon, `demon` scenario, in progress), lattice and kinetics
engines (protein folding, haemoglobin, catalysis), comparisons with HOOMD/LAMMPS/OpenMM.
