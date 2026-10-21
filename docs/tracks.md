# Two tracks: Einstein's 1905, and the evidence for atoms

Written 2026-10-20. Proposals for two sequences of pages:
- **the 1905 track**: one page per piece of Einstein's 1905 work;
- **the "evidence for atoms" track**: one page per line of evidence in Jean Perrin's *Atoms*.

Brownian motion belongs to both. Each proposal covers:
- the question the page asks;
- what the visitor does;
- what the simulation is and which engine it needs;
- what it measures;
- which existing pages it can reuse;
- the caveats.

**Sources.**
- **[Atoms]** J. Perrin, *Atoms*, translated by D. Ll. Hammick (Constable, London, 1916), from *Les
  Atomes* (1913). Cited by Perrin's section numbers (§), which are the same in every edition, and
  by page where useful. The copy read was an OCR text; its §120 table is garbled, and the values
  quoted below were read from the sections themselves.
- **[AJP]** R. Newburgh, J. Peidle and W. Rueckner, "Einstein, Perrin, and the reality of atoms:
  1905 revisited", *Am. J. Phys.* 74, 478–481 (2006), doi:10.1119/1.2188962.
- Einstein's 1905 papers, as cited in [AJP]: light quanta, *Ann. Phys.* 17, 132 (1905); Brownian
  motion, 17, 549 (1905); electrodynamics of moving bodies, 17, 891 (1905); the dissertation,
  Zürich (1905). From memory, not checked here: the dissertation was printed in *Ann. Phys.* 19,
  289 (1906), with a correction in 34, 591 (1911), and E = mc² is in 18, 639 (1905). [memory]

---

## A unifying device: count the molecules of the simulation

Perrin's argument rests on agreement, not on any one measurement. Thirteen unrelated phenomena,
each turned into a value of Avogadro's number N, all give 60–75 × 10²² ([Atoms] §120). So "the
real existence of the molecule is given a probability bordering on certainty".

Our simulations know their own N exactly, which makes an honest game possible:
- Give each page a "laboratory" in macroscopic units: a mass unit (the "gramme-molecule" of the
  simulated substance), a length scale seen through a "microscope", a temperature scale. Hide the
  molecular count.
- The visitor measures something macroscopic: a sedimentation profile, a viscosity, a
  scattering brightness, a drift of charge.
- The page turns the measurement into "how many molecules there must be per gramme-molecule".
- At the end it reveals the true count.

A final page collects the visitor's numbers into **their own Perrin table**. Perrin's closing
remark then lands. Eliminate N between the thirteen equations and you get twelve relations
between visible things. Measuring furnace light predicts how fast micron-sized spheres diffuse
in water ([Atoms] §120, p. 207).

---

## Track A: Einstein's 1905

### A1. The dissertation: "A new determination of molecular dimensions" (April 1905)

**What Einstein did** ([AJP] §I–II, Appendix A).
- He treated dissolved sugar molecules as spheres in a viscous continuum and derived two
  relations.
- First: the solution's viscosity rises with the fraction φ of volume the spheres occupy,
  η* = η(1 + 2.5φ). The 2.5 is the 1911 corrected value; his first calculation erred and gave N
  about three times too low (research.md §9).
- Second: the spheres diffuse with D = RT/(6πηaN) (eq. A1).
- Measured viscosity and diffusion of sugar solutions then give both the size a and N. Perrin
  recounts it in [Atoms] §77, p. 127–128.

**Page: "How big is a sugar molecule? Weighing molecules with a viscometer."**
- *Simulation:* Rahman MD. Large disks (the solute) in a small-disk solvent.
  - Measure the suspension's viscosity against the solute fraction, by reverse NEMD (momentum
    swaps; exactly energy- and momentum-conserving: research.md §18.4) or by a sheared box.
  - Measure the solute's diffusion coefficient.
  - The visitor fits the slope (Einstein's 2.5 in 3D; 2 for disks in 2D [memory]), then solves
    the two equations for the solute's size and for N.
- *Interaction:* add solute and watch the "honey" thicken. Drag a solute disk through the fluid
  and feel the drag.
- *Caveats.* In 2D a disk has no Stokes drag; the friction depends on the box size (Stokes'
  paradox, research.md §4). Its diffusion coefficient grows like ln t ([BY] §3.4,
  research.md §18.4). So either:
  - do this page in 3D (the honest choice), or
  - measure the friction directly in the same box by dragging, and say that 2D needs it.

### A2. Brownian motion (May 1905). Also in track B (B10–B12)

**What Einstein did** ([AJP] Appendix A; [Atoms] §68–70).
- A suspended grain is a giant solute molecule. It feels the van 't Hoff osmotic pressure and
  Stokes friction, and it diffuses: ⟨x²⟩ = 2Dt with D = RT/(N·6πηa).
- Langevin's 1908 derivation reaches the same result from Newton's law with random kicks
  ([AJP] Appendix B).
- Einstein noted it fails at very short times, where the motion is not yet random ([Atoms] §69).

**Page: "Why do pollen grains jiggle, and how did that count atoms?"** This extends the `heavy`
page (a big particle in a gas already gets its kT).
- *Simulation:* Rahman MD, a big grain in a WCA or LJ solvent. The same physics as `heavy` and
  `smell`.
- *Interaction, borrowing from the experiments:*
  1. **Trace it yourself.** As in Perrin's camera lucida ([Atoms] §72, Fig. 8) and the AJP
     students' frame-by-frame tracking, the visitor clicks the grain's position every Δt. The
     page draws the zigzag. Choose a shorter Δt and every segment becomes "a polygonal contour
     relatively just as complicated as the whole figure" (§72). This shows why the "mean speed"
     early observers measured was wrong: about 100,000 times too little energy (§68,
     footnote 3).
  2. **The ½ rule.** ⟨x²⟩ doubles when Δt doubles (§73: 120 s against 30 s), and the
     displacements fall on a Gaussian. This is Chaudesaigues's table in §73, rebuilt live.
  3. **Einstein's relation.** Pull the grain with a small steady force and measure its drift
     speed: the friction. With D from the zigzag, kT = D × friction. Then the page's N.
  4. **Change the temperature and the viscosity**, as Perrin did over a 1:330 range of
     viscosity and a 1:70,000 range of mass ([Atoms] §79).
- *Caveats.* The 2D diffusion coefficient creeps up with time (long-time tails), so report D for
  the time window used. Equipartition of the grain's velocity is exact
  ([AJP] eq. B4).
- *From [AJP]:* their N_A came out 8.2, 6.4 and 5.7 × 10²³ for 0.5, 1.09 and 2.06 µm spheres,
  falling with size, and they were "not certain of the reason" (Table I, §III). A page could
  show how such a trend arises in a simulation: too few frames, wall effects, Δt too long.

### A3. Light quanta (March 1905)

**What Einstein did.** The entropy of dilute (Wien-regime) radiation depends on volume exactly as
an ideal gas's does, as if it were N = E/hν independent particles. That gives (V/V₀)^N for the
chance that all of it is found in a part V of the volume. From this "heuristic point of view" he
explained photoluminescence (Stokes' rule), the photoelectric effect and photoionisation.
research.md §9 covers the caveats: Lamb & Scully's semiclassical model; antibunching as the
stronger evidence.

**Page: "Is light made of grains? The question you can ask of a gas."**
- *Simulation, step 1 (Rahman MD):* an ideal gas of N molecules. Count how often all of them
  happen to be in the left half; plot the frequency against N, falling as 2⁻ᴺ. This is already a
  brainstorm question ("how often does it happen", questions.md).
- *Step 2 (no MD):* Einstein's step. The same formula for radiation, with N = E/hν. A slider for
  the colour (hν) shows red light behaving like fewer, bigger grains than blue for the same energy.
- *Step 3:* the photoelectric threshold, as a counting explorable. A classical wave spreads its
  energy over the whole surface, so emission would take minutes; grains give immediate emission,
  with a maximum kinetic energy hν − W independent of brightness.
- *Links:* the black-body page (B17); the Einstein solid (B18).

### A4. Special relativity (June 1905)

**Page: "Can a gas get hotter without its molecules getting faster?"**
- *Simulation:* a relativistic gas. Momenta p = γmv, hard-disk collisions conserving relativistic
  energy and momentum, and a slow "speed of light" c (a few σ/τ) so the effects show at kT ~ 1.
  Event-driven hard disks are the easiest exact scheme. This is a new, small engine, or a
  relativistic mode of the hard-disk engine (research.md §8).
- *Interaction:* heat it. The speeds pile up just below c, while the energy keeps rising. The
  distribution turns from Maxwell–Boltzmann into Maxwell–Jüttner.
- *Caveats:* how temperature transforms between moving frames is still argued over
  (research.md §9); the page should not claim an answer.

### A5. E = mc² (September 1905)

**Page: "Is a hot cup of tea heavier?"**
- *Simulation:* the relativistic gas of A4 in a box. Push the box with a known force and measure
  its acceleration: its inertia is the rest mass plus the gas's thermal energy divided by c².
- *Interaction:* heat it and weigh it again. With c set small, the visitor sees the inertia grow,
  as heat literally becomes mass.

### A6. A bonus from 1907: the Einstein solid. See B18.

---

## Track B: the evidence for atoms, after Perrin's *Atoms*

Perrin builds his case in three stages:
- chemistry, which gives the ratios of atomic weights but no sizes ([Atoms] Chapter I);
- kinetic theory, the first estimates of sizes (Chapter II);
- then a string of independent counts (Chapters III–VIII).

The first three lines are qualitative but come first historically. The rest each yield N.

### B1. Definite and multiple proportions; Avogadro's hypothesis ([Atoms] §1–13, §18–19)

**Page: "Why do gases combine in whole-number volumes?"** (Gay-Lussac; Avogadro, §13).
- *Simulation:* reactive MD, two-atom molecules A₂ and B₂ in a box at constant pressure (a free
  piston). A binding rule A + B → AB fires on contact above an energy threshold, like the
  buffer page's kinetics.
- *What it shows:* a litre of A₂ and a litre of B₂ give two litres of AB at the same temperature
  and pressure. The whole numbers come from counting.
- *Engine:* needs bond formation during the run (Rahman has fixed bonds and `remove_bond`; adding
  `add_bond` is the missing piece). Medium difficulty.

### B2. Specific heats: Dulong and Petit, and equipartition ([Atoms] §15, §39–45)

**Page: "Why does it take more heat to warm some gases than others?"**
- *Simulation:* Rahman MD, heated in counted amounts (the boiling page's latent-heat panel
  already does this exactly). Compare:
  - monatomic disks: C = 1 k per molecule in 2D;
  - rigid dumbbells (stiff bonds): 1.5 k (2 translations + 1 rotation in 2D);
  - floppy chains: more;
  - a 2D crystal: 2 k per atom, the 2D Dulong–Petit (§15, §44).
- *Then Perrin's "serious difficulty"* (§41–45): real gases and solids are colder-blooded than
  this. Hydrogen below 50 K has the specific heat of a monatomic gas (§45), and solids' specific
  heat goes to zero (§44). Classical MD cannot show it; that is the point. Pass to the quantum
  counting models (B18, B19).

### B3. Raoult's laws and van 't Hoff's osmotic pressure ([Atoms] §25–26, §53)

- *Already built:* `osmosis` (the push, the free membrane, the U-tubes) and `saline`.
- *To add:* "count the solute". From the measured osmotic pressure and the mass of solute
  dissolved, the visitor infers the number of molecules per gramme-molecule. Also the
  freezing-point and boiling-point shifts (questions.md: salt and melting), as Raoult's laws.

### B4. Electrolysis and the faraday: the first hint of an elementary charge ([Atoms] §28–30)

**Page: "Why does every gramme of silver need the same charge?"**
- *Simulation:* charged ions need electrostatics, which Rahman lacks (research.md §5). A lattice
  or kinetics explorable would do: ions arriving at an electrode one by one, each carrying an
  integer number of charges, with the faraday as N times the elementary charge.
- Low priority; B20 is the dramatic version.

### B5. Thin films: oil on water, black soap films ([Atoms] §31–32)

**Page: "How thin can oil spread? Rayleigh's upper bound on a molecule's size."**
- *Simulation:* Rahman MD. Amphiphiles (the `lipids` scenario's heads and tails) placed as a drop
  on the surface of an LJ liquid under its vapour.
- *What happens:* the drop spreads into a monolayer. Area per amphiphile is about one molecule's
  width squared; Devaux measured "very little more than a millimicron" (§32, p. 51).
- *Interaction:* add a known number (the "standard solution" Devaux used) and watch the film's
  edge, then sprinkle talc (tracers) to see it, as Devaux did.

### B6. Molecular speeds and the proof of Avogadro's hypothesis ([Atoms] §33–36)

- *Already built:* `boltzmann` and `heavy` (equal mean energy for unequal masses, §36).
- *To add:* pressure as bombardment (§34: p = 2 × mass flux × normal speed), measured on a wall
  in the simulation and compared with ½ ρ⟨v²⟩ in 2D.

### B7. Effusion through small holes: Graham's law ([Atoms] §37)

- *Already built:* `balloon` (light molecules escape faster through pores).
- *To add:* Perrin's footnote. The radium emanation took 2.65 times as long as oxygen to effuse,
  so its molecular weight is 32 × 2.65² ≈ 225 (§37, p. 62). The visitor measures an "unknown
  gas" the same way.

### B8. The width of spectral lines ([Atoms] §38)

**Page: "Why are a hot gas's spectral lines blurred?"**
- *Simulation:* any Rahman gas. Each molecule "emits" a fixed frequency, Doppler-shifted by its
  velocity along the line of sight. The page shows the line's histogram: a Gaussian of width
  ∝ √(kT/m).
- *Interaction:* heat the gas, change the mass, look along a different axis, mix in a drifting
  "star". Fabry and Buisson's agreement to about 1% (§38, p. 64) is the test; Buisson and Fabry
  even used it to show that hydrogen glows as atoms, not molecules.
- *Cost:* cheap (no new engine). A good first page for track B.

### B9. The viscosity of gases, the mean free path, and van der Waals ([Atoms] §46–49)

**Page: "Why doesn't a gas get stickier when you compress it?"** (Maxwell's surprise: η ≈ ⅓ ρ v̄ L
does not depend on the density, §46–47).
- *Simulation:* Rahman MD, a dilute gas sheared between walls, or reverse NEMD (research.md
  §18.4). Measure η at several densities (flat) and temperatures (rising like √T).
- *Then:* the mean free path, hence the diameter of impact (§47). Then van der Waals' b from how
  the pressure departs from the ideal gas law. The `corner` page already shows excluded area.
  Together these give N (§49: N ≈ 62 × 10²² from argon, ±30%).
- *Interaction:* Perrin's two trains of travellers jumping between them (§46, p. 75): colour
  molecules by the layer they came from and watch momentum mix.

### B10. Brownian emulsions: the vertical distribution ([Atoms] §53–67)

**Page: "Weighing a molecule with a microscope."** This is Perrin's central experiment and the
`atmosphere` page's other half.
- *Simulation:* Rahman MD. Grains (big disks, slightly denser than the solvent) in an LJ or WCA
  liquid solvent, under gravity.
- *Physics:* buoyancy emerges by itself from the solvent's pressure gradient, giving the
  effective weight m(1 − d/D) of §55. The grains settle into an exponential atmosphere a
  hundred million times shorter than air's (§55, §64).
- *Interaction, Perrin's procedure:*
  - count grains at four heights. His real counts were 100, 47, 22.6, 12 at 5, 35, 65 and 95 µm
    for 0.212 µm gamboge (§64);
  - cool or heat it and watch the column sink or swell, Bruhat's verification of Gay-Lussac's
    law for emulsions (§66);
  - spin it in a centrifuge to sort grains by size, Perrin's fractionation (§57).
- *Result:* N from the scale height. His best was 68.2 × 10²² from 17,000 grains (§67). This is
  the strongest candidate for track B's first full page.

### B11. Brownian displacements, rotations and diffusion ([Atoms] §68–78)

- *Displacements:* page A2 (shared).
- *Rotations* (§71, §76). Perrin grew spheres "quite a dozen microns" across, big enough to
  watch them turn, and got N = 65 × 10²². *Simulation:* a grain made of a ring of bonded beads. Track its
  orientation; the mean square angle grows like t with Einstein's coefficient. It also shows
  equipartition between rotation and translation (§71).
- *Diffusion with a sticky wall* (§78, Léon Brillouin's method). Grains that touch the glass
  stick. Counting how many have stuck after time t gives D (N = 69 × 10²²). *Simulation:* a wall
  that turns any grain touching it into a pinned species, with the new `set_species` API. Cheap
  and vivid.

### B12. Density fluctuations ([Atoms] §80)

**Page: "Why does a small enough box of air never hold the same number twice?"**
- *Simulation:* any Rahman gas or liquid, with a movable counting window. Show the histogram of
  counts and its spread √n: 1 in 100 for 6,000 molecules, 10 in 100 for 60 (§80, p. 135).
- *For a liquid:* the spread shrinks with the compressibility (Smoluchowski). The visitor
  compares gas and liquid, and finds the compressibility from the spread.

### B13. Critical opalescence ([Atoms] §81–82)

**Page: "Why does a fluid turn milky at its critical point?"**
- *Simulation:* Rahman's 2D LJ fluid near its critical point (kT ≈ 0.46 for the 2.5 σ cutoff),
  at the critical density. Colour by smoothed local density: huge fluctuating patches. Show the
  scattered-light intensity from the structure factor S(k → 0), which grows as the compressibility
  diverges.
- *Physics:* Keesom's formula and the Leiden measurements gave N = 75 × 10²² (§82, table).
- *Interaction:* slide the temperature through Tc and watch milkiness come and go.
- *Caveats:* 2D critical exponents differ from 3D; finite boxes cut off the divergence.

### B14. The blue of the sky ([Atoms] §83)

**Page: "Why is the sky blue, and how does it count molecules?"**
- *Physics (Rayleigh, then Einstein, §83):* air scatters light because its molecules are
  randomly placed. Perfectly uniform (continuous) air would not scatter at all. The scattered
  intensity is proportional to the number of molecules and to 1/λ⁴, and it is polarised at
  90°. Measuring the sky's brightness against the sun's gives N, about 60 × 10²² in Perrin's
  table (§83).
- *Simulation:* positions from any Rahman gas. In JavaScript compute |Σⱼ exp(i k·rⱼ)|² for the
  scattering vector k of a chosen colour and angle, and compare:
  - a perfect crystal: no scattering except Bragg spots;
  - a liquid: little;
  - a gas: scattering ∝ N.

  Paint the sky by mixing colours with the 1/λ⁴ weight. Show sunset reddening along a long path,
  and the polarisation at 90°.
- *Cost:* cheap (no new engine). Visceral: "the blue sky is the graininess of the air".

### B15. Chemical fluctuations ([Atoms] §84)

**Page: "Equilibrium is busy."** In a small volume, a chemical equilibrium A ⇌ A′ + A″ flickers
as molecules break up and re-form (§84). *Simulation:* the `buffer` or `dna` scenarios, with a
small counting window: the number bound fluctuates like B12, and the mass-action law comes out of
the averages.

### B16. Fluctuations of molecular orientation ([Atoms] §85)

Mauguin's twinkling of liquid crystals between crossed polarisers, "like the swarming of a
luminous ant heap". *Simulation:* rods (stiff bonded chains) at nematic density; a polarisation
image from local orientation. Needs rods and many of them; later.

### B17. Black-body radiation ([Atoms] §86–90)

**Page: "Why does a furnace glow red, then yellow, then white?"**
- *Physics:* a cavity full of light in equilibrium (§86); Stefan's law (§87); Wien's
  displacement (§88); Planck's quanta (§89). Fitting Planck's law to two measurements gives both
  h and N: N = 64 × 10²² (§90).
- *Simulation:* not MD. A counting explorable: the cavity's modes (density ∝ ν² in 3D, ν in 2D)
  sharing energy quanta hν with oscillators in the walls. With quanta, the Planck curve; let the
  quanta shrink to nothing and the ultraviolet catastrophe (Rayleigh–Jeans) appears.
- *Interaction:* the furnace's temperature slider; the glow colour from the spectrum; the peak
  moving.

### B18. The specific heat of solids: the Einstein solid (1907) ([Atoms] §44, §91)

**Page: "Why do solids stop soaking up heat when they get very cold?"**
- *Simulation:* two side by side.
  - A 2D Rahman crystal: classical, 2 k per atom at every temperature.
  - Einstein's oscillators exchanging quanta: a counting model, essentially Schroeder's
    Einstein-solid explorable. Its specific heat falls to zero below hν/k.
- *Interaction:* lower the temperature and watch them part ways; the measured diamond curve
  follows the quantum one.

### B19. Quantised rotation; the atom's mass at its centre ([Atoms] §92–95)

**Page: "Why don't argon atoms spin?"**
- *Physics (§94):* the atom's mass is concentrated in a tiny nucleus, so its moment of inertia is
  tiny. The smallest allowed spin then costs more energy than collisions can give, and monatomic
  gases never pick up rotational energy. Hydrogen molecules stop rotating below about 50 K (§95).
- *Simulation:* a counting model of a quantised rotor, linked to B2's puzzle.

### B20. The atom of electricity ([Atoms] §97–104)

**Page: "Counting the charges on a droplet"** (Millikan, §100–101; Townsend, §98).
- *Simulation:* a droplet doing Brownian motion in air between condenser plates. Use a Langevin
  or MD droplet in a Rahman gas, with a uniform field force q·E on the droplet only; no
  electrostatics engine needed.
- *What the visitor sees:* now and then an "ion" (a small charged molecule) attaches, and the
  droplet's drift speed jumps by equal steps. They measure the steps; the charge is quantised.
- *Townsend's relation* (§98), D/mobility = kT/e, is Einstein's relation again, which links to A2.
- *Values:* Perrin's table has 68 × 10²² (§101 gives Millikan's own 59, with Perrin doubting its
  claimed precision).

### B21. Radioactivity: counting single atoms ([Atoms] §105–118)

**Page: "Why does every atom have the same chance of decaying, whatever its age?"**
- *Perrin's own analogy* (§110). Two small connected globes hold a gas mixture; by chance the
  molecules occasionally separate completely. Over many pairs of globes, these spontaneous
  separations happen exactly like radioactive decays: an exponential, with a half-life, and no
  ageing.
- *Simulation:* Rahman's compartments. Hundreds of tiny two-chamber boxes with a few molecules
  each; flag a "decay" when every molecule is on one side. The `arrhenius` page already measures
  escape times from 768 pockets this way. The histogram of waiting times is exponential, and
  counting events one by one is the scintillation screen (§114) and the Geiger counter (§115).
- *Then the counts give N* (§116–118): helium gas collected from counted α particles gives N = 64
  (§116); radium's disappearance about 71 (§117); the energy released about 60 (§118); and the
  charges produced 62.5 (§114).

### B22. Seeing the track of one atom ([Atoms] §119)

**Page: "The path of a single atom"** (C. T. R. Wilson's cloud chamber).
- *Simulation:* Rahman MD. Shoot one very fast heavy atom through an LJ gas and colour every
  molecule it hits: a straight track, bending near its end as small deflections add up.
- *Rutherford:* give the projectile a tiny hard core inside a soft halo. Rare collisions with
  another core send it off at a large angle, as Perrin describes from Wilson's photographs
  (Fig. 15–16).
- *Interaction:* aim and fire.

### B23. The summary: "Thirteen ways to count atoms" ([Atoms] §120)

The visitor's values from B8–B21 are gathered into their own version of Perrin's table, beside
his:

| Phenomenon | Perrin's N (× 10²²) | Section |
|---|---|---|
| Viscosity of gases (with van der Waals) | 62 | §49 |
| Brownian movement: distribution of grains | 68.3 (68.2 in §67) | §67 |
| Brownian movement: displacements | 68.8 | §79 |
| Brownian movement: rotations | 65 | §76, §79 |
| Brownian movement: diffusion | 69 | §78, §79 |
| Critical opalescence | 75 | §82 |
| The blue of the sky | ~60 (marked uncertain) | §83 |
| Black-body spectrum | 64 | §90 |
| Charged spheres in a gas | 68 (Millikan's own: 59) | §101 |
| Radioactivity: charges produced | 62.5 | §114 |
| Radioactivity: helium produced | 64 | §116 |
| Radioactivity: radium disappearing | ~71 (OCR unclear) | §117 |
| Radioactivity: energy radiated | 60 | §118 |

Perrin's closing caution belongs on the page too (§120, p. 207). We should be able to state the
results without the invisible elements; but using molecular constants to formulate laws that
could not have been found without them would be "cutting the roots that nourish" the plant.

---

## Suggested order

1. **B10, Perrin's column** (the `atmosphere` page's sibling) and **A2, Brownian motion** (from
   `heavy`). These are the heart of both tracks, and they share code.
2. **B14, the blue sky**, and **B8, spectral lines**. Cheap, JavaScript on positions,
   high-impact.
3. **B12, fluctuations**, then **B13, critical opalescence**: one counting window, then the
   critical point.
4. **B21, radioactivity as spontaneous unmixing**: reuses `arrhenius`.
5. **B20, Millikan**: a Langevin droplet with a field.
6. **B2 with B18–B19, specific heats and quanta**: the classical failure, then counting models.
7. **B9, gas viscosity**, and **A1, the dissertation**. These need the reverse-NEMD viscosity
   measurement (useful for the fluid engine too), and A1 wants 3D.
8. **A3–A5**: the relativistic gas is a new small engine; the light-quanta argument is a
   counting page.
9. **B1, B4, B5, B16**: need reactions, charges, interfaces or rods.
