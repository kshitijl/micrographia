# Status

What exists, what it has been checked against, and what comes next. Numbers are from an Apple
M1 Pro (Metal) unless stated. Run everything with `cargo xtask all`.

## Pages

| Page | Question | Checked in the browser test |
|---|---|---|
| `reversal` | Why can't you un-stir cream from coffee? | Forward + reverse restores every bit; a one-bit nudge spreads to all 8,000 molecules |
| `corner` | Could all the air rush into one corner? | Share of all-left boxes within 10% of 2⁻ᴺ (768 boxes) |
| `boltzmann` | Why is one molecule's energy exponential? | Single-molecule energies within 1% of e^(−E/kT); 2D Maxwell speeds within 1.3%; groups of 100 within 6% of the gamma law |
| `heavy` | Does a heavy molecule have less energy? | Light and 16× heavy species within 5% of equal kinetic energy; σ = 6, m = 400 balls converge more slowly |
| `heatflow` | Can heat flow from cold to hot? | ~20% of 1–2 τ windows run backwards; fitted ln P(Q)/P(−Q) slope within 35% of 1/T_c − 1/T_h (measured 0.367 vs 0.333) |
| `oilwater` | Why don't oil and water mix? | Weak cross-attraction separates (segregation > 0.3 after 200 τ); equal attraction stays mixed |
| `osmosis` | Where does the osmotic push come from? | Membrane push within 25% of van 't Hoff (measured 0.041 vs c kT = 0.039) |
| `compress` | Why does a bike pump get hot, a spray can cold? | Compression heats along T·A ≈ const (2%) |

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
- **CPU twins** (crate `md-ref`): `IntEngine<F>` (same integer scheme, f32 or f64 forces) and
  `FloatEngine<F>` (ordinary float MD). The GPU's kicks agree with the f32 twin bit for bit
  for ~70% of particles (the rest differ by a few units: different f32 rounding/fusion) and
  with the f64 twin to ~8×10⁻⁷ rms.
- **Browser:** the same WGSL runs through Chrome's compiler. On this Mac the state hash after
  1000 steps (`f26cd2e88ad2801e`) is identical natively (naga) and in Chrome (Tint).

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

## Next

- Bonds (polymers): rubber band as an entropic force; then lipids and soap.
- Liquid–vapour coexistence (LJ) with the free piston: boiling point vs pressure, the pressure
  cooker, the temperature plateau while boiling.
- Carnot engine and heat pump: piston cycles between baths, efficiency vs speed.
- Maxwell's demon / Feynman ratchet (needs a rigid body).
- All-integer forces (`gpu-int-int`, M1b), cross-device golden hashes.
- 3D.
