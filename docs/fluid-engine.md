# A fluid engine next to Rahman? (report)

Written 2026-10-20, after the atmosphere page got a throwaway JavaScript convection panel
(`web/pages/atmosphere/convection.ts`). The questions:

1. Should Micrographia have a GPU fluid-dynamics engine, in Rust/wgpu like Rahman?
2. Which of our questions need one?
3. Where, if anywhere, should molecules and continuum be merged?

**Short answer.**
- Yes, eventually. Make it a **lattice Boltzmann engine (D2Q9, later D3Q19)** in fixed point.
  It is the continuum method closest to Rahman in spirit:
  - local rules on the GPU;
  - integer state, so bit-reproducible across devices;
  - built from "particles" (distributions of molecular velocities), so the story from
    molecules to fluid can be told honestly.
- Do not build a full domain-decomposition hybrid (MD region embedded in a continuum). It is a
  research project, and none of our questions needs it.
- The merges worth having are cheap:
  - (a) Rahman *measures* the viscosity, the conductivity and the equation of state, and the
    fluid engine *uses* them;
  - (b) side-by-side "same flow, two scales" panels, to show hydrodynamics emerging from
    molecules;
  - (c) Brownian particles carried by a computed flow (smoke, pollen, scent, aerosol through a
    mask).
- Before any of that, add **DPD** (dissipative particle dynamics) to Rahman as an option. It is a
  small change and gives particle simulations real hydrodynamics at a fraction of the cost.
- Keep the JavaScript panel until the engine exists. It already gives the right answer.

## 1. What the throwaway taught us

The panel is a 2D slice of dry air, 20 km × 10 km, on a 128 × 64 grid:
- vorticity and stream function, semi-Lagrangian advection and an exact FFT/tridiagonal
  Poisson solve;
- potential temperature θ carried by the flow;
- pressure from hydrostatic balance with the average temperature at each height;
- T = θ (p/p₀)^(R/cp).

It costs about 1.5 ms per step on the CPU. Measured, in `web/tests/atmosphere.spec.ts`:

| run | result |
|---|---|
| still air, 2 h | stays still (max wind 3·10⁻⁵ m/s); isothermal, as in the MD column |
| sunshine, 4 h | a stirred layer 2.3 km deep, temperature falling at 9.2 K/km (dry adiabat g/cp = 9.8) |
| sunshine, 10 h | 4.4 km deep, 7.7–8.2 K/km |
| night, 2 h | the ground air cools and a 600 m inversion forms; nothing moves |

So the answer to "why is it colder up high" can be shown with a continuum model in an
afternoon's code. The trouble is all in the details, and those details are what a real engine
has to get right:

- **Eddy diffusion of θ stirred still air.** Diffusing θ itself mixes a stable column towards
  the adiabat with no wind at all. The fix was to diffuse only θ's departure from the starting
  profile. A real engine needs a turbulence closure that switches off in stable air
  (Smagorinsky, or a stability-dependent one), or enough resolution to need none.
- **2D flows pile energy into giant eddies.** In 2D, energy cascades to *large* scales, the
  inverse cascade. Winds reached 100 m/s until I added a slow linear drag. That is a property of
  2D, not a bug, but it means 2D convection pages need a drag and a caveat. Most real
  convection questions want 3D eventually.
- **The lid reflects gravity waves.** Plumes launch waves into the stable air above. With a rigid
  lid they bounce back and fill the sky, so the panel needs an absorbing sponge under the lid.
  Standard in weather models; easy to forget.
- **Boussinesq over a scale height is a stretch.** The motion is treated as incompressible while
  the column's density halves. It gives the right lapse rate, because that comes from the
  thermodynamics, but the plume dynamics aloft are off. An anelastic engine
  (∇·(ρ̄u) = 0) fixes this at almost no cost.
- **Time scales are not interactive.** A real sunny day grows a 1–2 km mixed layer over hours.
  The panel uses sunshine 7× real and radiation towards the starting temperature on a 12 h scale.
  Every atmosphere page will need such dials, and must say so.

## 2. Which questions need a continuum engine

Rahman covers a box some hundreds of molecules across. Anything about flow on scales of
centimetres to kilometres, or about *where heat goes* in a moving fluid, needs a continuum
engine. From our pages and the list in `architecture.md`:

| Question | Needs | Why MD alone can't |
|---|---|---|
| Why is it colder up high; why are cloud bases flat; why do mountains have rain shadows | convection with stratification, then moisture (latent heat) | the lapse rate is a property of convecting air over km; the MD column is (correctly) isothermal |
| Smog and inversions; why does fog sit in valleys | stable stratification, night cooling | same |
| Sea breeze, chimney draught, the stack effect, hot-air balloons at full scale | buoyancy-driven flow around geometry | scale |
| Why stirring cools coffee faster than waiting; Rayleigh–Bénard cells | convection versus conduction | MD shows conduction honestly (heatflow page) but convection needs ~10⁴ molecular lengths |
| Smell across a room: diffusion versus draughts | advection–diffusion at room scale | the smell page already shows diffusion is hopeless at 1600σ; a room is 10¹⁰σ |
| How masks work: flow through fibres, why small particles are caught by diffusion and large ones by impaction | low-Re flow through porous geometry, with Brownian particles | MD gives the mechanism of one fibre; the filtration curve needs flow fields |
| Kármán vortex streets, why flags flutter, why wires sing | moderate-Re flow past obstacles | scale |
| Taylor's "unmixing" (dye in glycerine turned back) and the scallop theorem | Stokes flow, reversible in time | a beautiful companion to the reversal page: kinematic reversibility of a continuum, unlike molecular chaos |
| Sweating and wind chill | boundary layers with heat and vapour transport | scale |
| Boiling at kettle scale, droplets, splashing | multiphase flow | MD boils a few thousand molecules; bubbles rising need continuum or SPH |

Questions that stay with MD: anything whose answer is about molecules, such as entropy,
equilibrium distributions, osmosis mechanism, rubber elasticity, phase coexistence, vapour
pressure, reversibility and the arrow of time. The fluid engine is for the questions whose answer
is about *flow*.

## 3. Methods compared

| Method | What it is | For us | Against us |
|---|---|---|---|
| **Lattice Boltzmann (LBM)** | Distributions f_i of particle velocities on a lattice; stream to neighbours, relax towards a local Maxwellian | Purely local stream + collide: ideal for WGSL compute, no global solve. Arbitrary geometry by bounce-back from a painted mask (masks, chimneys, obstacles). Thermal (a second distribution, or a passive scalar) and Boussinesq buoyancy are standard. Multiphase (Shan–Chen) gives droplets and boiling. Integer/fixed-point versions exist: bit-exact across devices, matching our Tier 2 determinism. The molecular story is honest: a coarse Boltzmann equation, which Micrographia already explains. | Weakly compressible, low Mach only (no shocks; the bike pump stays MD). Large density variation over a scale height is awkward; use Boussinesq plus an anelastic correction, or a thermal compressible lattice. Memory: 9 values per cell in 2D (~38 MB for 1024×512 at double buffering). |
| Incompressible Navier–Stokes on a grid (projection, "stable fluids") | Advect, then solve a Poisson equation for pressure | Cheapest per cell; what the throwaway does; anelastic is a small step. | The Poisson solve is global: multigrid on WebGPU is work, and Jacobi needs hundreds of iterations. Determinism needs fixed iteration counts and care with reductions. |
| SPH / MLS-MPM | Particles carrying fluid properties | Free surfaces (pouring, splashing, waves). Shares Rahman's neighbour search and binning almost line for line. | Noisy pressure, tuning-heavy. Research notes: SPH topped out around 30k particles on integrated graphics in WebGPU-Ocean, which switched to MPM. |
| DPD | Coarse-grained particles with soft repulsion plus pairwise friction and noise | Hydrodynamics *in Rahman*: momentum is conserved pairwise, so flow emerges. A pair force plus a counter-based random number (we already have Threefry in gpu-prims). Polymers, membranes and osmosis at 10× the scale. | A thermostat by construction (canonical, not microcanonical; energy-conserving DPD exists but is fiddlier). Not reversible. |
| MPCD/SRD | Point particles stream, then rotate velocities in cells | A cheap solvent for colloids and polymers (listed in research.md). | Same thermostat caveat as DPD; less intuitive to explain. |
| DSMC | Stochastic collisions of simulated molecules, for rarefied gases | Shock structure, the upper atmosphere, Knudsen flow; a "zoomed-out MD". | Only for dilute gases. A niche for us. |
| Lattice gas (FHP) | Boolean particles on a hexagonal lattice with exact collision rules | **Exactly reversible and integer**: Navier–Stokes emerging from a reversible automaton is pure Micrographia, and a direct sequel to the reversal page. | Very noisy; needs coarse-graining over many cells. A page, not an engine. |

**Recommendation: LBM.** It fits the GPU, fits our determinism story, handles the geometry most
of our questions need (painted walls, fibres, chimneys), and does thermal convection and
multiphase with known, published schemes. It cannot do shocks; MD does those already.

## 4. Where to merge molecules and continuum

### Worth doing

1. **Calibration (one-way, offline).** Rahman measures, the fluid engine uses:
   - viscosity (Green–Kubo, or the slope of a sheared box);
   - thermal conductivity (the heatflow page already measures it);
   - the equation of state p(n, T), and later the surface tension and the latent heat.

   The fluid page then says "this water's viscosity was measured on the molecules page". This
   costs nothing at run time and closes the loop honestly.
2. **The same flow at two scales, side by side.** Poiseuille flow in a channel, or Couette shear:
   - in Rahman: a few thousand molecules, a body force, the velocity profile emerging as a noisy
     parabola;
   - in LBM: the same channel with the measured viscosity, a clean parabola.

   Overlay the two. This is the "is hydrodynamics real?" page, and no coupling is needed, only
   matched units.
3. **Particles carried by a flow (one-way coupling).** Langevin particles: Stokes drag towards the
   local fluid velocity plus thermal kicks. They are what you see:
   - smoke in the convection panel;
   - scent in a room's draught;
   - aerosol caught (or not) by mask fibres.

   This is a few lines on top of any fluid engine, and it gives the mask page its real answer:
   the filtration curve, with its most-penetrating particle size.

4. **Solute particles in a continuum solvent (two-way coupling). An idea for later.** The
   solvent is a fluid on a grid (lattice Boltzmann with thermal noise), and the solute molecules
   are particles. Each particle feels drag towards the local fluid velocity, F = γ (u − v), plus
   random kicks, and the equal and opposite force goes back into the fluid where the particle is,
   so momentum is conserved. This exists and is well established:
   - Ahlrichs and Dünweg (1999) coupled point particles (polymer beads) to lattice Boltzmann this
     way, and it is a standard feature of codes like ESPResSo;
   - immersed-boundary and force-coupling methods, and fluctuating hydrodynamics with particles
     (Donev and co-workers), are cousins.

   **Osmosis in it.** The membrane would be a force that acts on solute particles only (like
   Rahman's species-selective membrane), with the fluid passing freely. The membrane pushes the
   solute back; the solute, through drag, pushes the fluid; a pressure difference builds in the
   fluid, and in a U-tube under gravity the solution side rises until ρ g Δh = c kT. This is the
   bombardment picture of osmosis with nothing else in it. (I have not checked the literature for
   osmosis done exactly this way; it is a natural test of such a code, and diffusio-osmosis is
   studied with these models.)

   **Why it fits.**
   - It answers the red-cell question cleanly. Measured on the saline page, the cell's membrane
     tension is nearly the same with the water made invisible (5.3 against 5.9 with real water).
     The push is the solute's; the solvent supplies volume and flow, not force.
   - It is cheap: thousands of solute particles in a fluid of 10⁵–10⁶ cells, so U-tubes and cells
     hundreds of σ across, with less noise than an all-molecule liquid.
   - It shows where the energy goes: the drag dissipates, and the noise feeds it back.

   **Against.** It is canonical by construction: the drag and the noise are a thermostat
   (fluctuation–dissipation), not the microcanonical ensemble we prefer, and the page must say so.
   It needs the LBM engine of §5 first; the coupling itself is perhaps 100 lines.

### Not worth doing (for now)

- **Concurrent hybrid coupling**: an MD region embedded in a continuum, exchanging mass, momentum
  and energy fluxes across an overlap (O'Connell–Thompson, Flekkøy, Delgado-Buscalioni). It is the
  "proper" merge for contact lines, membranes and shock fronts. It is also notoriously fiddly:
  - fluctuations at the interface;
  - the mass flux through it;
  - energy conservation;
  - and with our determinism goals, much harder.

  The one page that might want it is the osmosis U-tube (MD at the membrane, continuum in the
  arms). The honest version of that page is a big MD box, which Rahman can now nearly do.
  Revisit only if a page is impossible without it.
- **MD equations of state at run time** (calling Rahman from the fluid engine per cell): no.
  Tabulate offline (merge 1).

## 5. If we build it: shape of the crate

- `crates/fluid` (wgpu, also compiled to wasm), on `gpu-prims` for reductions and Threefry:
  - D2Q9 BGK first; then TRT/MRT collision (stabler at low viscosity);
  - a thermal scalar with Boussinesq buoyancy;
  - bounce-back walls from a mask texture;
  - later D3Q19, Shan–Chen multiphase, and an anelastic base state for atmospheres.
- **State in fixed point**, as in Rahman: distributions as i32 with a fixed scale. The collision is
  a few multiply-adds per value, so an all-integer collision is affordable, and that gives Tier 2
  (bit-identical everywhere) from day one, not as a later plan item.
- **The same page contract as Rahman.** These follow from `CLAUDE.md`:
  - `describe()` and `energy_note()`. The ensemble here is "driven, open": sunshine in,
    radiation out. LBM conserves mass and momentum exactly; total energy (kinetic + internal +
    potential) is shown and its budget stated.
  - Interaction: drag to stir, paint walls, drop warm or cold blobs, place an obstacle and drag
    it, release a puff of smoke. The panel's drag-to-stir and warm bubble are the prototype.
  - Side profiles of density, pressure and temperature, as on the atmosphere page.
- **Size**: 1024 × 512 D2Q9 at a few thousand steps per second on a laptop GPU. Several times the
  throwaway's grid, at frame rate.
- **Tests**:
  - Poiseuille profile and its viscosity;
  - decay of a shear wave;
  - Rayleigh–Bénard onset near Ra ≈ 1708 (rigid plates);
  - the dry lapse rate in a convecting layer;
  - a still stratified column staying still (the bug the throwaway hit).

## 6. Suggested order

1. Now: keep the JS panel; it answers the question.
2. Small and soon: DPD as a Rahman option (pair force + Threefry noise, a "canonical, momentum
   conserving" ensemble note). That gives particle hydrodynamics for big membranes and polymers,
   and the two-scales page in its particle half.
3. The LBM crate with the five tests above. Then replace the atmosphere panel with it (anelastic,
   higher resolution, later moisture for flat cloud bases).
4. New pages on it:
   - the mask filtration curve (with merge 3);
   - smell across a room;
   - coffee: stirring versus waiting;
   - Kármán streets;
   - Taylor's unmixing, next to the reversal page.
5. Only if a page demands it: hybrid MD/continuum coupling.
