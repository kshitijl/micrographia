# Initial review of the pages (October 2026)

A working file, not permanent: Kshitij's feedback on each page as given, what was done about
it, and decisions still being discussed. Check each item off once it is done and confirmed.
Delete this file once everything here has been settled.

## Decisions

- **Names.** The MD engine was first named **Brownie** (Robert Brown; the Kodak Brownie
  camera), then renamed **Rahman** (third round). The project is **Micrographia** (Hooke's 1665 book showing
  the public the microscopic world). Done so far: README, site title, every page's top bar.
  Open: rename the `md` crate to `rahman` (and `md-core`/`md-ref` to match)? Not done yet.
- **Priorities.** The original list of questions comes first. That needs the 3D engine next,
  and phase transitions (boiling, melting, freezing, salt and melting point, triple point,
  temperature holding steady during a phase change) must be visible in it. Written into
  `docs/status.md` under Next.

## Pages

### `reversal`: un-stirring the coffee

Feedback:
1. Pick 5 representative cream molecules, trace their paths and draw them, so the viewer can see
   each one literally retrace its steps on the way back.
2. Replace the checkbox ("When reversing, nudge one cream molecule by 2⁻³² of its diameter") with
   buttons: "nudge representatives", "nudge all cream", "nudge all molecules", so the user nudges
   by hand, then reverses time and sees the retracing go wrong.

Done (to confirm):
- [ ] 1. Five traced cream molecules, spread over the blob (one at its centre, four around it at
  60% of its radius). Each draws its outward path in its own colour and its return path as
  smaller white dots on top, so an exact return shows white dots sitting inside the coloured
  ones. Each traced molecule is also marked with a translucent square.
- [ ] 2. The checkbox is gone. Three buttons, "The 5 traced", "All cream" and "Every molecule",
  each nudge by 2⁻³² σ (one GPU dispatch for a whole species). A nudge can be given any time;
  the next reversal and the arrival message say what was nudged. Text in the notes updated.
- Browser test: drives the buttons (run 8 τ, nudge all cream, reverse) and checks that the
  return fails and that both outward and return paths were recorded.

Open questions:
- Paths are recorded once a molecule has moved 0.2 σ since the last point, so they stay light
  even for long runs. They are never trimmed: after a very long run they get dense.

### `corner`: all the air in one corner

Feedback:
1. "Measured share is 0.65×": measured is always lower than predicted, probably because the
   molecules are hard spheres. Can the model be closer to the prediction (e.g. by changing the
   potential's parameters)?

Done (to confirm):
- [ ] 1. Yes, it was the molecules' size: each denies the others an area b ≈ πd², which makes
  gathering in half the box rarer by about e^(−N(N−1)b/2A). With the old 0.5 σ molecules that
  is 0.80 at 6 per box and 0.54 at 10 (measured 0.81, 0.50); 0.65 is what 9 per box gives, or
  6 per box early on, before many events have been counted.
  The default is now 0.25 σ (nearly ideal, like air): about 0.95 of 2⁻ᴺ (0.92–1.01 over seeds;
  the page's seed shows ~0.9). A new "Molecule size" selector (0.25, 0.5, 1 σ) shows the effect
  on purpose, the page shows both the ideal and the size-corrected odds (b integrated from the
  actual repulsion), and the notes explain the correction. At 1 σ the measurement is 0.46 of
  2⁻ᴺ and 0.98 of the corrected odds.

Open questions:
- The tiny molecules are drawn at 0.5 σ so they can be seen (noted in the caveats). Smaller
  still (0.1 σ) would be even more ideal but needs more cells than the GPU buffers allow at
  this box size, and the gas would mix even more slowly.

### `boltzmann`: where does the Boltzmann distribution come from?

Feedback:
1. The page makes the point hard to see (did everything start at the same energy? it
   equilibrates very fast). "One molecule's energy is distributed exponentially" isn't quite the
   right framing: the point is where the Boltzmann distribution comes from.
2. Want the textbook ensemble: many boxes of gas; molecules interact inside a box and cannot
   leave it, but boxes weakly exchange energy with each other, without building the Boltzmann
   distribution in through a thermostat wall. Brainstorm how.
3. A dropdown of initial distributions: all the same speed; half cold and half hot; mostly
   average with 5% hot; one box full of hot molecules.
4. Watch it equilibrate. Show that the total energy of the whole box never changes (track and
   graph it), while each sub-box approaches the Boltzmann distribution.
5. Show entropy increasing over time.

Coupling options considered:
- (chosen) Diathermal walls: the walls between boxes stop molecules crossing but pass on their
  pushes. Molecules on either side of a wall repel each other through it as if it weren't
  there, so energy flows through the wall by ordinary collisions. Nothing but Newton's laws;
  total energy is conserved exactly. Only the layers next to the walls touch, so each box
  equilibrates internally first, and with its neighbours more slowly (a knob: box size).
- A wall made of beads on springs, belonging to neither box, that conducts heat like a solid.
  More realistic, more machinery, slower.
- Movable partitions (pistons) between boxes: they exchange energy but also volume.
- A thermostat or thermal walls: rejected, since they put the Boltzmann distribution in by hand.

Done (to confirm):
- [ ] 1–2. The page is rebuilt around the ensemble: 48 boxes (16 σ, 64 molecules each) behind
  diathermal walls (a new engine option, on the GPU and the CPU twin, with exact reversal still
  holding). Title: "Where does the Boltzmann distribution come from?". Molecules are coloured by
  speed; click a box to outline it.
- [ ] 3. Start menu: every molecule the same energy; half hot and half cold (mixed in every box);
  5% very hot; one box very hot, the rest cold; left boxes hot, right cold. Every start averages
  exactly kT = 1, and each group starts at exactly one energy (spikes in the histogram).
- [ ] 4. Histogram of all molecules now, or of the outlined box over the last 2 τ (energy, log
  energy or speed), against Boltzmann at the final temperature. A plot of total energy per
  molecule (flat) and the outlined box's kT, and a readout of the total with its % change.
  Measured: each box reaches Boltzmann within 2–5 τ; boxes even out over ~100–300 τ; box
  temperatures keep fluctuating by ~12% (1/√64: the canonical fluctuation, explained in the
  notes).
- [ ] 5. Entropy: Boltzmann's H-function −Σ p ln p of the pooled energy histogram (with the
  Miller–Madow small-sample correction), plotted against its ceiling 1 + ln kT. From equal
  energies it rises from −2.4 to 0.95 (ceiling 0.96) within a few τ.
- Browser tests: from equal energies, entropy reaches within 0.06 of its ceiling, the histogram
  is within 10% of Boltzmann (2.4% measured) and total energy holds to 0.5% (0.01%); with one
  hot box (kT 19.8) it falls below 30% of that (1.8) and the total holds to 1% (0.3%).

Open questions:
- Total energy drifts by a few tenths of a percent over hundreds of τ with the hot starts, even at
  half the usual time step (0.0025 τ, used on this page). Part of it is that a molecule
  reflected at a wall jumps to its mirror position, changing its distance to the molecules
  pushing on it from across the wall. Fine for the page; worth a closer look in the 3D engine.
- Removed from the old page: tracking groups of k molecules (the exponential narrowing into a
  Gaussian) and the red molecule's energy trace. Bring the groups back as a second section?
- The old page's 20,000-molecule gas is replaced by 3,072 molecules; histograms of the pooled
  gas are a little noisier.

## Second round of feedback

### All pages

1. Show a graph of total energy on every page, so you can see it is conserved, and then see a
   big kick or a freeze push it up or down. Also entropy: approaching equipartition should make
   it rise, and kicking a selection of molecules should push it down.
2. In general: give users direct, reach-in interactivity (click around, select things, give
   kicks or nudges, buttons that shake things up). Recorded in `CLAUDE.md`.
3. Each page should say at the bottom what kind of simulation it is: molecular dynamics? Which
   potentials (Lennard-Jones)? Constant energy or a thermostat? Etc.

### `heavy`: light, heavy and bowling-ball molecules

1. Buttons to give some molecules a big kick: e.g. drag a box to select molecules, then press a
   button to kick them or freeze them.
2. A little brown tail is drawn near the bowling balls. What is it?

### `boltzmann`

1. Interactivity: select a bunch of molecules and give them a kick.
2. Question: how exactly does energy transfer if molecules can't cross?

### `reversal`

1. When the retracing is exact, erase the traced paths as the molecules retrace them (but not
   if the retracing is not exact). Some kind of XOR in a buffer?
2. Don't trace only cream: trace 2 cream and 2 coffee molecules.

### `heatflow`: can heat flow from cold to hot?

1. Hard for an untrained viewer to grasp what the simulation is doing: how does heat transfer
   even work here? Before the main simulation, a couple of small interactive sims explaining:
   molecules are coloured by speed (one panel); a hot molecule touching the cold plate gets cold
   (and what is its speed then? does it take a fixed speed from the plate?), and the same for a
   cold molecule touching the hot plate.
2. The write-up says backwards heat flow is "a few fast molecules happened to hit the cold wall
   and slow ones the hot wall". Isn't that the normal direction? Backwards needs, e.g., molecules
   faster than usual hitting the hot plate and heating it further.

### `oilwater`

1. Allow the oil–water attraction to go beyond 1, to see what that does.

### `osmosis`

1. A "membrane push" number isn't fun. Two demos instead, where the motion is visible:
   (a) a movable membrane, pure water on the left and solution on the right: the membrane moves
   all the way left; (b) a U-tube with a fixed membrane at the bottom, pure water left, solution
   right, under gravity: the water rises on the right and stops, and the height difference is
   the van 't Hoff pressure.

### Second round: done (to confirm)

- [ ] All pages: an "About this simulation" section, written by the engine from the system it
  runs (method, units, forces grouped by potential, masses, bonds, fixed species, boundaries,
  boxes, membrane, piston, and whether energy is conserved or a bath/heat-bath walls exchange it).
  A browser test checks every page has one.
- [ ] All pages, energy and entropy graphs: a reusable panel (`web/src/lib/interact.ts`), so far
  on `heavy` (new) and `boltzmann` (its own). Not yet on the other pages.
- [ ] All pages, reach-in tools: drag a box to select, then "Kick (3× faster)", "Freeze",
  "Shove →" (a GPU command on the region). So far on `heavy`, `boltzmann`, the heat-flow intro
  panel and the free-membrane osmosis demo. Not yet elsewhere. Rule recorded in `CLAUDE.md`.
- [ ] `heavy`: tools and the energy/entropy panel. The brown tail is each bowling ball's recent
  path (its Brownian motion); now labelled in the legend and the notes. (Also fixed: the ball
  mass slider snapped to 316 while the label said 400.)
- [ ] `boltzmann`: tools (a click still outlines a box). New notes section "How energy gets
  through a wall that molecules can't": the walls stop molecules but the molecules' repulsion
  reaches through, so a fast molecule near the wall shoves a slow one on the other side.
- [ ] `reversal`: 2 cream and 2 coffee traced. Coming back, the page lands on exactly the step
  counts it recorded going out and compares positions bit for bit (f64): a matching point is
  erased (XOR), a non-matching one stays, with a white dot where the molecule really is. An exact
  return wipes the paths completely; a nudged one leaves them.
- [ ] `heatflow`: two intro panels before the main simulation: (1) colour means speed, with
  kick/freeze tools; (2) a thin gas between a hot and a cold wall, with histograms of the speeds
  of molecules leaving each wall against theory. Answer to the question: a wall does not give a
  fixed speed; each molecule leaves with a fresh random velocity drawn at the wall's temperature
  (the flux-weighted Maxwell distribution), so the two walls' spreads overlap. Measured ⟨v²⟩/3 of
  departures: 2.25 for the 2.5 wall, 0.42 for the 0.4 wall.
- [ ] `heatflow` text: fixed. Backwards means molecules arriving at the hot wall faster than it
  returns them (heating it further) and the cold wall sending molecules back faster than they
  came (cooling it further), as you said.
- [ ] `oilwater`: the attraction slider goes to 2.
- [ ] `osmosis`: (a) a free bead membrane, pure water left and solution right, moves from 48 σ
  to 33 σ in 500 τ (it would go all the way to the wall); (b) an osmometer: two arms of a
  Lennard-Jones liquid under gravity with the membrane between them; the solution arm climbs
  ~10 σ above the pure water. The old membrane-push measurement stays as part 3.

Open questions (second round):
- Osmometer (b) is qualitative: the rise is clear, but it settles at about half to three
  quarters of van 't Hoff's height and the levels jitter by several σ. A 2D liquid 16 σ wide has
  a ragged surface with films creeping up the walls and the membrane. Many variants were tried
  (see `crates/md/tests/osmosis_demos.rs`). A clean version wants a wider tube (slower) or 3D.
- Energy/entropy panel and tools on every remaining page: to do, page by page.

## Third round of feedback

### Decisions

- **Rename the engine to Rahman** (after Aneesur Rahman, who ran the first realistic MD
  simulation, of liquid argon, in 1964). Replaces "Brownie".
- **A public API.** The MD engine will be exposed for others to use, so its API should be good.
  Build the demos on the publicly exposed API as much as possible, not deep in the engine's
  internals. Recorded in `CLAUDE.md`.
- **Visceral feeling.** In general the demos should give the viewer a visceral feeling for the
  forces and effects (see them bend, stretch, burst), not just a number. Recorded in `CLAUDE.md`.
- **Theme: entropy matters more at higher temperature.** If an effect gets stronger as
  temperature rises, that is evidence it is entropically driven. Emphasise this throughout.
  Recorded in `CLAUDE.md`.

### `boltzmann`

1. Hard to see the graphs and the simulation at the same time. Fix the layout.

### `oilwater`

1. The stir button puts the molecules into an ordered lattice. Make it feel like stirring. Even
   better: press stir and the pointer becomes a big spoon you swish around the simulation.
2. A note on salad dressing: vigorous stirring puts it out of equilibrium, and left long enough at
   room temperature it separates.
3. A gravity control, with oil slightly lighter: with no gravity it still separates, with gravity
   the oil goes to the top.

### `saline`: IV drips

1. Not visceral that the cell is pushed and stretched. Colour-code the membrane's stretch? (How
   is the ring of beads made, springs?) Show the springs stretching more.
2. Bursting: several red blood cells; tension builds in their membranes and some burst (a spring
   over a certain force breaks from then on).
3. The opposite direction: the cell loses water. Less visceral than bursting; what happens in real
   life, and how can the viewer really feel it?

### `compress`: bike pump

1. Needs an energy graph. All pages should have one.

### `rubber`

1. Let the viewer feel the force, not read a number: attach the chains at each end to something
   that bends inward visibly, or to springs that stretch inward, or a membrane of beads that
   bends; colour springs by how hard they are stretched.
2. The page explains where the pull comes from and that it grows with temperature (good). Does it
   explain why a rubber band gets hot when stretched? Brainstorm a panel (same or another) for it.

### `atmosphere`: pressure with height

1. Don't start at the equilibrium distribution. A start dropdown: uniform, all at the bottom, all
   at the top.
2. Sliders for gravity strength and temperature, the two main variables. Mass matters only
   through m·g (is that right?), so not both a gravity and a mass slider.
3. A panel with the simpler derivation: slabs, each pushing up the slab above (pressure balance),
   solved with discrete slabs; a slider for the number of slabs, showing it become the classic
   differential equation and its exponential.
4. Why "keep the gas in contact with a heat bath"? Molecules at the top move erratically. Where
   does energy come in, why is it needed, wouldn't the effect show without it? Isn't heating from
   the bottom what gives a temperature gradient, so adding heat uniformly and then finding a
   uniform temperature is a bit of a cheat?

### Third round: done (to confirm)

- [ ] Engine renamed **Rahman** (README, site, docs). Crate names unchanged (still open).
- [ ] `CLAUDE.md`: build pages on the public API; make things visceral; entropy-stronger-when-hotter
  as a theme.
- [ ] `boltzmann`: the three plots sit in a row under the simulation. On every page the
  simulation now stays in view (sticky) while a long panel scrolls.
- [ ] Energy graphs everywhere: every simulation gets the total-energy and entropy plots by
  default; the legend says how energy can change (heat bath, heat-bath walls, piston work).
  Potential energy now includes m g h, so energy under gravity is flat. `compress` and `carnot`
  also plot "total minus the work the piston has done", which stays flat.
- [ ] `oilwater`: "Stir with a spoon": the pointer becomes a spoon; dragging pulls the molecules
  under it along with the spoon (velocities moved halfway to the spoon's each event, swept along
  its path), so it stirs without boiling. "Start again, mixed" replaces the old lattice reset.
  Gravity slider, with oil half as heavy as water (exaggerated, said so): with gravity off it still
  separates into blobs; with g = 0.03, 2/3 of the oil is in the top half after ~1200 τ. The heat
  bath is now gentle (it was killing the flow that lets oil float up). Salad-dressing note.
- [ ] `saline`: four cells. Answer to the question: the ring is beads joined by harmonic springs,
  plus springs between next-nearest beads for bending stiffness. Measured: the springs barely
  stretch more in water than in saline (the swelling unfolds the ring's wrinkles, and thermal
  jiggling stretches springs far more than osmosis does), so colouring by spring strain would show
  noise. Instead each cell's membrane is coloured by how full it is (blue shrunken, white normal,
  red about to burst, grey burst), and a cell bursts when its 10 τ-averaged area passes the
  lysis limit, like a real red cell that tears once swollen to its limit: the membrane opens
  where it is most stretched (three springs removed through a new public `remove_bond` API) and
  the salt leaks out. In pure water all four burst within a few hundred τ; in saline none; in
  brine they shrink to ~60% and crumple. Notes on crenation (stiff spiky cells that can't pass
  capillaries; brain cells tearing vessels in very salty blood).
- [ ] `rubber`: part 1, "feel the pull": each chain's right end is a handle on a soft spring to a
  fixed post, drawn as dots from blue (slack) to red (taut); heat it and the springs stretch further
  (2 σ → 6.7 σ from kT 1 to 2.5), and the pull is k × stretch (the T = 2 / T = 1 ratio is 2.04).
  Part 2, "why stretching warms it": insulated chains, "Stretch fast" and "Release fast" with a
  temperature trace: stretching warms them (1.00 → 1.19), releasing cools them (→ 1.05). Releasing
  has to be slower than the chains can follow, or the ends just push on them and heat them.
- [ ] `atmosphere`: no heat bath (answer: the bath was an Andersen thermostat redrawing velocities
  at random, which is what made top molecules jump about; it set the temperature but isn't needed:
  a sealed column also settles to one uniform temperature, which is the real point). Start menu
  (spread evenly, squeezed against the floor, against the ceiling, already settled); gravity and
  temperature sliders (mass only enters through m g, so no mass slider; two kinds of molecule show
  two scale heights); the temperature slider rescales everyone's speed through the public region
  command. Falling from the ceiling heats the gas (to kT ≈ 4); spreading from the floor cools it.
  New "slabs" panel: N slabs each holding up the ones above, (1 − x/N)^k, becoming the
  exponential as N grows.

Open questions (third round):
- Rename the `md` crates to `rahman`?
- `saline`: bursting is a rule on the cell's area (the model's springs are too stretchy and noisy
  to snap by themselves). Fine, or try a stiffer, nearly inextensible membrane?
- `oilwater`: the oil's mass is exaggerated (half of water's) so it rises in tens of seconds.

## Fourth round of feedback

### General

1. A "total energy of the entire simulation" graph on every page, and make it very clear which
   ensemble is simulated. Prefer the microcanonical ensemble (energy strictly conserved) and let
   Boltzmann distributions emerge naturally, whenever possible. Recorded in `CLAUDE.md`.
2. Be able to click and move things around, interactively (drag an end of a rubber band, etc.).
   Recorded in `CLAUDE.md`.

### `smell`
1. Diffusion is already very fast, which defeats the point. Make the box much bigger.

### `reversal`
1. A button to nudge just one molecule, perhaps one you select. Nudging a single molecule may
   be enough to see complete mixing after reversing.

### `boltzmann`
1. Default start: "one box very hot, the rest cold".
2. Is "only molecules within a molecule's width of a wall take part" exactly true? Are we doing
   all-pairs LJ with normal cutoffs throughout the box, and the walls are a hard momentum bounce?
3. A total-energy-of-the-whole-simulation graph.

### `osmosis`
1. Another panel under the current U-tube: the real classic U-tube, with walls separating the arms
   all the way and the membrane passable only at the bottom, and many more water molecules (like
   the first panel). Maybe the current one suffers from too few particles.

### `compress`
1. Buttons for a shock: a fast compression and a fast decompression, to see out-of-equilibrium
   effects and perhaps a travelling shockwave.

### `rubber`
1. The "ends held at fraction" slider restarts the simulation; instead, moving it should smoothly
   move the other end and pull the band taut.
2. A couple of times the band broke and the simulation went crazy, beads flying everywhere.
3. "Why stretching warms it": a spring on the left too, to feel how hard the band pulls; and drag
   the other end with the mouse instead of buttons that move it slowly.

### `atmosphere`
1. The slab panel: make it vertical, with the slabs drawn on screen, gravity pointing down, and the
   weight on each slab shown, perhaps colour-coded by the weight of all the slabs above it.
2. Graphs right beside the MD box, perhaps colour-coded, of pressure, density and temperature at
   each height, so they can be read off side by side and the uniform temperature is plain.
3. A "sunshine" button to see ground-heating convection, probably not in MD but in a little
   fluid-dynamics panel. Think about whether to write a GPU fluid-dynamics engine in Rust next to the
   MD engine, and whether any effects want the two merged; write a report to read. For now, only a
   throwaway in JavaScript for this one page's pressure and temperature panel.

### `saline`
1. Keep the current simulation, and add a panel below with much bigger cells and stiffer, nearly
   unstretchable, more fragile springs, so they tear from too much membrane tension, as real cells
   do.

### Fifth round of feedback

1. `osmosis` classic U-tube: with the heat bath off, the temperature should fall: some of the
   thermal kinetic energy pays for the potential energy of the water rising. Plot it, so we can
   see that happen.
2. `osmosis` U-tube: try 10× the molecules and see whether it still simulates smoothly.
3. `atmosphere` fluid panel: something seems wrong; nothing happens, or nothing is rendered.
4. `saline` big cell: smoothly add or remove salt and watch the change spread: a slider, a button,
   or clicking to put salt in, then see the effect.
5. `saline`: wouldn't the red-cell result be the same with no water at all? Do the water and the
   membrane interact in this simulation?
6. `saline`: clicking and dragging to add or remove salt makes it very slow (the frame rate drops).
7. `atmosphere` fluid panel: a strange left-to-right wind blows at the top; it is very chaotic.
   Something seems wrong.
8. `atmosphere`: how does the explanation square with the cold at the top of a mountain or a big
   flat plateau? The sunlit plateau warms the air above it too, so why does height above sea level,
   rather than height above the nearby land, predict temperature?
9. `osmosis` U-tube: if there's no appreciable cooling, where did the energy to lift the liquid
   come from?
10. A hybrid model: water as a continuum, solute as particles that feel drag from the water and
   push momentum back into it. Does it exist? Can osmosis be seen in it? Note it in the fluid report
   for later.
11. `boiling` (pressure cooker): try a hot plate at the bottom instead of the thermostat.
12. `atmosphere` MD: a hot plate at the bottom, off by default, which can be turned on at a chosen
   temperature.
13. A control for the number of particles, on every page really, but at least `atmosphere` and
   `boiling`, to find a count that looks good and still runs fast.
14. `boiling` (pressure cooker): see the hot plate when it's on, and colour molecules by speed or
   energy to track energy flows visually.
15. `boiling`: show both raw per-molecule speed and a smoothed one; make the hot plate the default;
   a vertical temperature indicator next to the box, like the atmosphere page.
16. `boiling`: a new panel under it about latent heat: add heat in increments; it stays liquid,
   then at some point it all goes to vapour.
17. `boiling`: at 24k molecules it all starts red, perhaps from overlapping starting positions
   and an energy blow-up.
18. Besides the 1905 Einstein track, an "evidence for atoms" track (the Brownian motion paper in
   both). Read the 2006 paper and Perrin's *Atoms* in `books/`; write up each 1905 paper (what the
   page and simulation should be) and propose a page for each line of evidence in *Atoms*.
19. `oilwater`: a panel underneath with another simulation, showing emulsification and/or soap.
20. The pressure-cooker page is really a phase-transitions page: make it about phase transitions.
   Besides the boiling simulation, show visually and interactively why different phases win at
   different temperatures: dG/dT = −S at fixed P, so the G of high-entropy phases falls faster;
   the sneeze picture (at low T taking a bit of heat from the surroundings costs them a lot of
   entropy, so order wins; at high T it costs little, so the system takes heat to become a gas);
   both maximise the entropy of system plus surroundings.
21. Is there a faster or better method for showing a liquid–gas transition (continuum, fluid
   dynamics, something else)?
22. Performance: can MD go faster or bigger? How fast are we, in simulated time per wall time?
   Do we gain from the GPU, and below what size is the CPU better? Does the CPU path use SIMD, and
   can Rust SIMD run in the browser? Is the GPU code bound by memory bandwidth, compute, latency or
   kernel launches? Write down for later: careful benchmarks; CPU SIMD in the browser; choosing CPU
   or GPU by machine and box size; a simplified performance model of the GPU simulator that names
   the bottlenecks; whether WebGPU leaves performance on the table (rather than a CUDA version for
   recording explainer videos on the desktop; better if everything runs in the browser).
23. Decouple the simulation from rendering and host–device traffic: the simulation runs
   underneath as fast as the GPU allows, and the page samples the GPU state now and then to
   animate and to read out. The UI offers both: slow motion, or full simulation speed. Then bigger
   systems and many more timesteps, to see real boiling and vapour. Do this first.
24. 3D in the GPU engine, for the demos that need it, with no 2D performance regression.
25. Phase transitions: implement all three, the GPU lattice gas, Gibbs-ensemble Monte Carlo, and
   Shan–Chen lattice Boltzmann, all on the GPU unless something truly doesn't parallelise, so they
   can be compared for explaining and understanding.
26. Higher particle counts on every page, or a control for them: default about 5,000–20,000.
27. Soap: drop soap molecules in by hand.
28. A general rule: avoid a dropdown that resets the simulation to change an amount, quantity or
   position when that could be an interactive act. Always prefer to make the demonstration
   interactive: we are in a lab, and the student should pick something up or use a tool to prod
   it.

### Fourth round: done (to confirm)

- [ ] Every energy plot shows the total energy of the whole simulation (not per molecule), with a
  line naming the ensemble ("Constant energy (the microcanonical ensemble): nothing adds or removes
  energy…", or which heat bath or piston changes it), written by the engine itself.
- [ ] `smell`: a corridor 1600 σ long (38,400 molecules), with the time diffusion alone would take
  to cross it (length²/2D, from the measured D) next to the draught.
- [ ] `reversal`: "One molecule" nudges a single molecule (click one to choose it; it is
  highlighted). Measured: one nudge 25 τ out spoils the return of all 8000.
- [ ] `boltzmann`: starts with one box very hot. Answer to the wall question: yes, the forces are
  the same everywhere (every pair within the repulsion's range, 1.12 σ, pushes regardless of
  boxes), and the walls only bounce molecules' velocities exactly; the text now says exactly
  this. Total-energy plot, flat.
- [ ] `compress`: "Slam in" and "Yank out" (4× the fast speed) with a density and temperature
  profile up to the piston: a slam heats the gas far past the adiabat (kT 1.03 → 8.5, against
  1.41 for a slow push the same distance).
- [ ] `rubber`: the "crazy mode" was an engine bug: bonds longer than 8 σ gave garbage forces
  on the GPU (the CPU twin silently skipped them); fixed on both, with tests at 3–20 σ. The slider
  now moves the posts smoothly without restarting. Part 2 has a spring on the left showing the
  pull, and you drag the right end (stretch 0.996 → 1.20, release → 1.10).
- [ ] `atmosphere`:
  - Density (light and heavy stacked), pressure (the weight of the gas above, against n kT) and
    temperature (blue colder / red hotter than average) beside the column, sharing its height axis.
  - The slabs drawn upright, each coloured by the weight it holds up, with its density beside it
    against the exponential.
  - Part 2, a throwaway JavaScript fluid panel (2D, dry, Boussinesq motion, compressible
    thermodynamics): still air stays still, Sunshine grows a stirred layer cooling at 9.2 K/km
    after 4 h (dry adiabat 9.8), Sunset makes a ground inversion; warm-bubble button, drag to stir.
  - The report: `docs/fluid-engine.md`. In short: yes to a lattice Boltzmann engine in fixed
    point, later; DPD in Rahman first; merge by calibration, side-by-side panels and particles
    carried by flows, not by a concurrent MD/continuum hybrid.
- [ ] Library fix found on the way: rebuilding a simulation (Reset, changing a start menu) while
  a readout was running freed it underneath the readout ("null pointer passed to rust"). Rebuilds
  now wait for the frame in flight and run one at a time.

Still to do (fourth round): `osmosis` classic U-tube panel; `saline` big-cell panel.

### Fifth round: done (to confirm)

- [ ] `osmosis` U-tube temperature: plotted as energy books (lift m g h, heat, interactions, total)
  with a temperature readout and "the cooling the lift alone causes". The prediction is right but
  tiny: lifting 15 σ stores ~8 ε, a drop in kT of ~0.002 (0.3%), below the reading's jiggle and
  below the liquid's own slow drift (a control with no solute cools as much). The total is flat to
  1 ε. The liquid now settles 200 τ as pure water before 30% of the right arm becomes solute (zero
  energy cost). Notes explain where the energy comes from (the heat, driven by the solute's entropy).
- [ ] `osmosis` U-tube 10×: a size menu (33,000 molecules). Runs at ~1,800–3,300 steps/s in Chrome
  (10–16 frames/s at 200 steps/frame); it settles slowly (still warm and compacting after 300 τ)
  and rises slowly. Usable, not yet good.
- [ ] Bugs found on the way: a U-tube wall bead placed on the box's top edge was bounced at
  100 σ/τ (scenario builds now reject particles outside the box); wall beads inside the soft walls'
  range held a constant 64,000 ε; `set_view` takes corners (saline and U-tube views were off).
- [ ] `atmosphere` fluid panel: it started paused (Sunshine alone did nothing); now Sunshine, Warm
  bubble and stirring set it running, an hour a second. The wind across the top was 2D convection
  building a horizontal mean flow (a known 2D artefact); it is now damped, and the plumes rise
  upright (8.9 K/km at 10 h, against 9.8).
- [ ] `atmosphere`: notes on why a sunny plateau is still cold (less greenhouse blanket above,
  air borrowed from the free atmosphere, thin air), with the mass elevation effect.
- [ ] `atmosphere` MD: a hot plate under the floor (off by default) at a chosen kT; a molecule
  count menu (1,500–48,000). With only a plate, the column warms from the bottom and ends at the
  plate's temperature (said in the notes).
- [ ] `boiling`: heat with a bulk bath or a hot plate under the floor (insulating lid); a molecule
  count menu, with the box growing to match; a measured-temperature readout. On the plate, bubbles
  nucleate at the floor.
- [ ] `saline`: the water does interact (it repels the membrane beads) but passes through, so the
  push is the salt's. Measured with the water made invisible (a new `water` 0 option): big-cell
  tension 5.3 instead of 5.9 in pure water, 3.9 instead of 4.2 in saline. Notes added.
- [ ] `saline` brush: strokes are queued and applied in the frame's readout; 60 frames/s while
  dragging. Adding salt picks only water molecules with room for a salt molecule, so it costs no
  energy (public API `set_species(ids, species)`).
- [ ] The hybrid idea (continuum solvent, particle solute with two-way drag) is in
  `docs/fluid-engine.md` §4, item 4.

Still to do: a molecule-count control on the other pages.

### Sixth round: done (to confirm)

- [ ] `boiling`: heat with a hot plate (the floor is a heat bath; the default), a hot layer (a bath
  in the bottom 3 σ, new engine option `with_bath_layer`), or a bath throughout. The plate glows by
  its temperature, under a grey insulating lid. Colour molecules by energy averaged over ~2 τ (the
  default, shows heat moving), by speed right now, or plainly (renderer: a three-stop ramp and a
  per-molecule running average). Density and temperature profiles beside the pot, lined up with
  the box. On the plate, the liquid does not wet the purely repulsive floor and can keep a thin
  vapour gap; the layer heats the liquid directly. Both are offered.
- [ ] The 24k "all red" start was an engine bug: Verlet neighbour lists were sized from the box's
  average density; a pool under a big vapour space is much denser than average, the lists
  overflowed silently and the energy blew up (kT ~3000). Now sized for at least a dense liquid;
  regression test. (It probably also explains the 10× U-tube running warm.)
- [ ] `boiling` part 2, latent heat: a droplet in a sealed flask heated in counted amounts (a heater
  and bursts); temperature against heat added, and where the heat went (speed against pulling
  molecules apart, 56–60% the latter while it evaporates). Evaporation in 2D is slow, so the
  shelf tilts and fast heating overheats the liquid past its critical point; said on the page.
- [ ] `docs/tracks.md`: the 1905 track (dissertation, Brownian motion, light quanta, relativity,
  E = mc²) and an "evidence for atoms" track: a page proposed for each line of evidence in
  Perrin's *Atoms*, with section citations, his table of N, and a suggested order. Read: Perrin's
  *Atoms* (Chapters I–II in full, the opening of every section from §46 on) and Newburgh, Peidle
  and Rueckner (2006) in full.
- [ ] `oilwater` part 2, soap: water, a quarter oil, and soap molecules (a water-loving head bonded
  to two oil-loving tail beads), freshly shaken. Without soap the droplets merge (30 left after
  800 τ, still falling); with plenty of soap they stay small (57) and 88% of the soap sits on a
  boundary, heads in the water and tails in the oil. Soap amount menu, spoon, re-shake, and a
  droplet-count plot that keeps the previous run for comparison. Notes on soap, emulsifiers
  (mustard, egg yolk) and micelles. New scenario `emulsion`.
- [ ] The boiling page is now the phase-transitions page (same URL), with a part 3, "Why the gas
  wins when it's hot": G = H − TS for solid, liquid and gas against temperature (each falling at
  rate S; the lowest wins; drag to set T), a pressure slider that moves the boiling point through
  the gas's entropy (the pressure cooker), and the entropy books of system, surroundings (−ΔH/T,
  the sneeze) and universe when one molecule melts or evaporates. The numbers use the 2D LJ
  energies with a water-like (widened) liquid range, and say so.
- [ ] Faster methods for the liquid–gas transition, and the whole performance question, are written
  up in `docs/performance.md`, for later: measured speeds, a simple model (launch-bound below
  ~20–60k particles, near the bandwidth roofline at 1M), why pages run far slower than the engine
  (one readback per frame), CPU SIMD and threads in the browser, choosing CPU or GPU by machine
  and size, WebGPU against CUDA, a native recorder for videos instead of a CUDA port, and the
  plan. For phase transitions: a GPU lattice gas, Gibbs-ensemble MC and Shan–Chen lattice
  Boltzmann.

### Seventh round: done (to confirm)

- [ ] Full speed (item 23): every simulation now has "Slow motion / Full speed" buttons under its
  controls and a live steps/s and τ/s readout. Slow motion is the old loop (the page's steps per
  frame, then its readout). At full speed the simulation runs underneath: two batches of steps
  always queued on the GPU, each sized to ~10 ms of GPU work, the picture drawn every frame
  straight from GPU memory, and the page's readouts (and the energy plot) sampled five times a
  second, with the steps paused briefly for each so page actions never race them. Measured on the
  pot (6,000 molecules): 3,400 → 8,000 steps/s; at 48,000, 2,000 → 4,000. At 6,000 that is the
  GPU's per-dispatch limit (~10 dispatches a step at ~9 µs each), not the page's any more.
- [ ] 3D (item 24): a 3D engine (`GpuEngine3`, cell list, the same exact integer leapfrog,
  walls, baths, kicks, reversal), with a CPU twin and native tests (kicks agree with the twin
  to f32 precision, reversal restores every bit, energy conserved to 10⁻⁴ per atom, baths reach
  their temperatures). Its own kernels, so 2D is unchanged (A/B benchmark: within 2%). A 3D
  renderer (lit spheres with exact depth, drag to turn, scroll to zoom, cut away the front),
  `SimHost` runs 2D or 3D, and the energy panel's entropy uses the 3D density of states.
- [ ] New page `crystal` (3D): a Lennard-Jones crystal heated or cooled in counted amounts, each
  atom coloured crystal or liquid (Steinhardt q6 bonds), "show only the crystal", temperature
  against heat added (the melting plateau) and crystal fraction against time. Natively: melts in
  ~60 τ at kT 0.9; held at 0.42 the drop stays liquid ~320 τ (supercooled), then freezes. Next
  for it: Verlet lists in 3D (3–4× faster), and dropping in a seed crystal by hand.
- [ ] Phase transitions three ways (item 25), part 1 of 3: a GPU lattice gas (new crate
  `lattice`): Kawasaki Monte Carlo (molecules hop to empty neighbours, number conserved) on
  twelve sublattices of non-interfering pairs, with integer acceptance thresholds computed on the
  CPU, so the GPU and its CPU twin agree bit for bit. Coexisting densities match Onsager's exact
  curve within 2% natively and within 0.008 in Chrome; ~1,000 sweeps/s on 2,040² sites natively,
  ~6,000 sweeps/s on 516² in the browser. New page `coexistence`, part 1: temperature, gravity,
  a hot plate, a dropper and a sponge to add or take molecules by hand, a quench / slab / pool
  start, the measured densities on Onsager's curve, and energy. Lattice Boltzmann and Gibbs
  ensemble next.
- [ ] Part 2 of 3: Shan–Chen lattice Boltzmann on the GPU (`lattice::lbm`): D2Q9, the
  Shan–Chen attraction set from the Carnahan–Starling equation of state at each site's
  temperature (Yuan–Schaefer), exact-difference forcing, gravity, and a temperature field that
  flows with the fluid, diffuses from a hot plate and is taken up where liquid expands (Gong–Cheng),
  so a pool on a hot plate boils: bubbles grow on the plate, rise and burst. Natively the liquid
  density matches the model's coexistence prediction to 0.2%, the vapour to 5–25%. ~17,000
  steps/s on 256² in Chrome. Page `coexistence` part 2: temperature, gravity, hot plate, drops and
  bubbles placed by hand, colour by density or temperature, quench / pool / drop starts.
- [ ] Found by comparing the methods: the molecular-dynamics vapour densities of the 2D LJ
  fluid (the `coexistence` test, and the phase-transitions page's vapour pressure) were about
  ten times too low, because evaporation in 2D is slow and the vapour had not filled up: started
  empty, a slab's vapour was still rising after 1,000 τ; pre-filled at the Gibbs-ensemble value
  (0.024 at kT 0.40), it stayed there. The page's Clausius–Clapeyron slope was still sensible
  (the filling rate scales like the vapour pressure), but its absolute pressures were low. Fixed:
  the pot now starts with its vapour near saturation (a Clausius–Clapeyron fit through the
  Gibbs-ensemble and MD values), and the page says that after a temperature change the vapour
  takes hundreds of τ to catch up. p(0.40) is now 9.3 × 10⁻³ (was 6.8 × 10⁻⁴); noted in research
  §18.8.
- [ ] Part 3 of 3: Gibbs-ensemble Monte Carlo (new crate `mc`), the same 2D LJ as the MD pages,
  on the CPU as WebAssembly: its transfers and volume moves are sequential at the sizes it needs,
  so a GPU would be slower (written down in `mc`'s docs). At kT 0.40 it gives vapour 0.024 and
  liquid 0.747 at equal pressures, matching MD (pre-filled slab: 0.023, 0.742); at 0.44 the
  liquid box fluctuates widely. Bigger boxes are worse (they hold a drop or a bubble). Page
  `coexistence` part 3: the two boxes drawn to scale as they trade area and molecules, density
  traces, and the coexistence curve against MD's; notes comparing the three methods.

### Overnight plan (the user, 2026-10-21, before bed)

"while I sleep continue working on items in the plan/backlog. after phase transitions, go ahead
and implement pages for all the questions on my original list, the 1905 track, the perrin atoms
track, and then the other ones we wrote down in the brainstorm session. then do CPU simd
simulators that can run in the browser, then other performance stuff"

Order: (1) finish the phase-transition items (particle counts on every page, soap dropped in by
hand, interactive changes instead of resets); (2) pages for the remaining questions on the
original list; (3) the 1905 track (docs/tracks.md A1–A5); (4) the evidence-for-atoms track
(B1–B23); (5) the brainstorm ideas; (6) CPU SIMD engines in the browser; (7) the rest of
docs/performance.md.
- [ ] Item 4, more molecules: count selectors on carnot (default 3,200), compress (12,000),
  balloon (4,000) and lipids (1,600 lipids). The balloon's box grows with it: at 4× the density
  in the same box the leak stopped obeying Graham's law (ratio 1.7 against 2.7), because the
  mean free path became shorter than the pores (effusion turned into flow). Pages that are small
  on purpose keep their size: the fluctuation-theorem panel (backward heat flow needs small
  systems), the corner page (2⁻ᴺ), the FEP replicas and the Gibbs boxes.
- [ ] Item 27, soap by hand: the soap panel's amount menu is replaced by a soap dropper. Each click
  switches on 40 soap molecules where it lands (they wait in the water as dormant three-bead
  molecules that behave as water, because bonds cannot be made at run time; species can change).
  New public API `DiscSim.bonds()`.
- [ ] New page `speeds` (original list: Boltzmann vs Maxwell; the average kinetic energy of an
  ideal gas): a 2D and a 3D gas, every molecule's velocity as a dot, a draggable ring whose
  count and density are read out against a same-area patch at the centre, and energy and speed
  histograms against Boltzmann's and Maxwell's curves; heat, cool and kick tools.
- [ ] New page `freezing` (original list: salt and the melting point): a crystal slab under its
  liquid in 3D, held just below its melting point by a gentle bath, with salt poured in by hand
  (atoms too big for the crystal, placed only where they have room). The pure liquid freezes
  (92%); salty, freezing stops part way (51%), because the crystal pushes the salt into the
  remaining liquid until its melting point falls to the set temperature. Notes: entropy of
  mixing, ΔT ≈ x kT²/L, road salt's eutectic, and why boiling shifts less (L_vap ≈ 7 L_fus).
  Bug found on the way: forcing a readout by setting the throttle's last time to 0 fails in a
  page younger than the throttle window; now −∞ (also on the crystal page).
- [ ] New page `enthalpy` (original list: enthalpy vs energy vs heat; why the Gibbs free energy):
  two gases heated alike, one under a locked lid, one under a free piston carrying a weight
  (drag to change it). New scenario `enthalpy` and public API `DiscSim.set_piston_load`.
  Natively c_V = 1.01, c_p = 1.95 (2D ideal: 1, 2). The lid has friction (its bounce is damped
  and the energy returned to the gas as heat), or the undamped bounce stores heat and the numbers
  drift. Notes: Q = ΔU at constant V, ΔH at constant p; −ΔG/T is the total entropy change at
  constant T and p (the sneeze), so G going down is the second law.
- [ ] New page `sneeze` (original list: dQ = T dS and the sneeze; entropy rising with energy): a
  cold and a hot thin gas, click to sneeze an exactly counted burst of heat into a spot, or into
  everyone at once; entropy (N × Boltzmann's H of the energies) against Q/T, and S against E
  (slope 1/T, flattening). The same 300 ε adds 569 k to the cold box and 146 k to the hot one.
- [ ] The triple point (original list): answered on the `crystal` page, whose crystal melts with
  its own vapour around it, so its temperature plateau (kT ≈ 0.62) is the triple point; why only
  one point (two conditions on G, two lines crossing), and why ice in a drink or an iceberg is not
  at it (one atmosphere of air; the vapour's partial pressure is the humidity's).
- [ ] New page `humidity` (original list: why warm air holds more water; saturation): a pool under
  air that never condenses, the vapour starting saturated; temperature slider, a cold glass (a
  strip at the wall that takes molecules' heat), a humidifier (click to turn air into water
  vapour), relative humidity and dew point against the saturation curve (the Gibbs-ensemble fit),
  and fog: cooled, the extra water forms droplets in the air first. New scenario `humid` and
  `scenarios::saturated_vapour_density`.
- [ ] Flat cloud bases (original list): the atmosphere page's convection model gets water ("Make
  it humid"): vapour and cloud carried by the wind, evaporation from moist ground in sunshine,
  condensation with latent heating, virtual-temperature buoyancy, rain-out, and large-scale drying.
  After 3 hours of sun the clouds' bases sit at 1.63 ± 0.25 km against a predicted lifting
  condensation level of 1.41 km (drawn as a dashed line); notes on why they are flat.
- [ ] New page `kidney` (original list: concentrating urine): the countercurrent multiplier as the
  textbook stepwise model (not molecules), drawn as the hairpin and the collecting duct coloured by
  saltiness, with flow; pump strength, ADH, loop length and medullary blood flow as sliders, and
  the pumps' work. Default: urine 1,211 mOsm/L (tip 1,312); no ADH: 100; weaker pumps or a
  shorter loop: less.
- [ ] New page `dissolving` (original list: why salt dissolves; solubility and temperature;
  salt and sugar): a 2D crystal of two kinds of ion (strongly attracted to each other, less to
  water) in a box full of water, held at a temperature; it dissolves more when hotter (entropy
  against a fixed energy cost). Sprinkle salt in by clicking, stir with the spoon. New scenario
  `solubility`. Bug found on the way: the first version moved leftover water onto occupied rows,
  and the start exploded (everything "dissolved" even at ε = 6).
- [ ] Track A2 / B11, new page `brownian` (3D): a grain 5 σ across in a WCA liquid; the microscope
  view of its path at a chosen Δt; ⟨r²⟩ against time; throw it to measure the friction ζ = m / ∫v/v₀ dt;
  Einstein's kT = Dζ against the thermometer (natively 0.0169 × 59 = 1.0 in a 16k box; a smaller box
  gives 0.73, the periodic images' drag on D). New scenario `brownian3`, `BallSim.set_species_velocity`.
- [ ] Engine: neighbour lists now keep each species pair within its own cutoff + skin, not the
  largest cutoff's: the Brownian grain run went from ~1,280 s to 387 s natively, trajectories
  bit-identical; helps every page with big and small particles (heavy, saline, dissolving).
- [ ] Track B10, new page `sedimentation`: Perrin's grains settling under gravity into an
  exponential haze; log-scale height histogram with the fit; kT = mgH against the thermometer; N
  in Perrin's units; stir, gravity, warm and cool. New scenario `perrin`.
- [ ] Track B18, new page `einstein-solid`: two blocks of Einstein oscillators (Monte Carlo of
  quanta, no GPU): heat flows until temperatures match, Boltzmann's distribution emerges, S = k ln Ω
  read out exactly, and the heat capacity measured as you heat one block alone, against Einstein's
  curve (freezing out when cold).
- [ ] Tracks B12–B14, new page `opalescence`: the lattice gas at the critical density and the light
  it would scatter (k⁴ S(k) at "blue", "green" and "red" wavelengths scaled to the lattice, as a
  colour swatch and a brightness against an ideal gas). Very hot: S ≈ 1.2, blue; near T_c: S at
  the red scale 178, white, 55 times brighter. New API `GasSim.sites()`. Notes: why the sky is blue,
  counting molecules from its brightness, critical opalescence.
- [ ] Track B9, new page `viscosity`: a thin and a dense gas (same molecules, densities 0.02 and
  0.08) stirred with a gentle shear wave; its decay gives η = ρ × rate / k²: 0.265 and 0.311 (dilute
  kinetic theory: 0.288), the same within the dense gas's Enskog correction; the velocity profile
  flattening live; mean free path and molecular size from η, Loschmidt's way.
- [ ] Track B20, new page `millikan`: an oil drop between plates (Langevin dynamics in SI units, Brownian
  jiggle included): switch the field to make it fall and rise through the reticle, X-ray it to change its
  charge, take new drops; each fall-and-rise pair gives its radius (Stokes) and charge, into a histogram
  that clusters at whole multiples of e (30 drops in the test: all within 0.25 e).
- [ ] Track B21, new page `counting`: a radioactive sample (independent atoms, Monte Carlo) flashing on a
  scintillation screen; counts per interval against Poisson (variance 2.1 for a mean of 2.2), the decay
  curve, and the half-life from all the flashes (maximum likelihood: 61.7 s for a set 60 s; a fit of
  ln(counts) read 68–70 s, biased by dropping the empty late intervals); sample size, half-life and
  counting interval as sliders.
- [ ] Track A3, new page `light`: the light in a hot box as its modes (Monte Carlo, Metropolis at the walls'
  temperature): in lumps hν it follows Planck's law (within 1.2% near the peak at 3,000 K) and glows orange
  (colour from the CIE curves); made classical, every mode gets kT and the box blazes violet (the
  ultraviolet catastrophe), its energy off the chart. Photoelectric effect: grains of light on sodium
  (W = 2.3 eV) free none at 650 nm however bright, and at 300 nm free electrons of at most hν − W = 1.83 eV
  whatever the brightness (4× brighter: 3.9× the electrons); the wave theory frees them at any colour after
  a delay, faster when brighter. Also: About sections added to counting, einstein-solid and millikan, which
  had none (about.spec only looks at pages that exist when it runs).
- [ ] Track B1, new page `avogadro`: hydrogen and chlorine under the same weighted lid with heat-bath walls,
  as Avogadro's two-atom molecules (left) and Dalton's single atoms (right). Reactions are bonds made and
  broken by the page (new engine API `add_bond`, with `remove_bond` and `set_species`): light (a lamp you hold
  over the gas, or a flash) splits Cl₂; free atoms carry the chain (Cl + H₂ → HCl + H, H + Cl₂ → HCl + Cl)
  and glow; each reaction's heat (scaled down) goes into the atoms, so the lid jumps, then settles as the
  walls take the heat. Test: Avogadro's 1.87 → 1.87 volumes (256 of 300 HCl), Dalton's 2.01 → 1.14 (on its
  way to 1). Engine: `add_bond`; bond kernels kept when a system declares bond types but has no bonds yet.
  Bug found and fixed: the Andersen bath heated a gas under a free piston (it added the piston's jiggle as
  if it were a flow); research.md §18.8.
- [ ] Tracks A4 and A5, new page `relativity`: a relativistic hard-disk gas (c = 1; collisions reverse the
  momenta along the line of centres in the pair's centre-of-momentum frame, conserving energy and momentum
  exactly). Part 1: heat it (buttons, or hold the mouse on it): speeds crowd below c (Maxwell's curve spills
  past it), the energy follows Jüttner within 1% by the thermometer kT = ⟨p·v⟩/2, heat per degree goes from
  1k to 2k. Part 2: push a cold and a hot gas alike (hold the button): the hot one lags; its inertia per
  molecule is (E + pA)/c² (3.09 against 3.07), not E/c², because the gas has no walls; the page explains the
  enthalpy and Laue's walls rather than hiding it.
- [ ] Track B23, new page `perrin`: Perrin's table of N beside the visitor's own values, which the pages now
  compute in laboratory units and keep in this browser (web/src/lib/perrin.ts): sedimentation (RT/mgH),
  brownian (RT/Dζ, new readout), viscosity (Loschmidt, from the molecule's size), millikan (e from the drops'
  common divisor, then F/e; new readout), counting (atoms counted from a weighed sample; new readout), light
  (h from the photoelectrons' slope, k from the spectrum's peak by Wien, N = R/k; new readout). A strip chart
  shows them all against 6.022 × 10²³. Opalescence and the sky do not count yet.
- [ ] Track B8, new page `lines`: Doppler-broadened lines from a gas of hydrogen (m 1) and oxygen (m 16)
  atoms, as a spectroscope looking along x would show them, in lab units (kT 1 = 1,000 K): widths within 1–3%
  of λ√(kT/mc²); a readout weighs the glowing atoms from the width (Buisson and Fabry: hydrogen glows as
  atoms). Drag a box to look at part of the gas, and heat, cool or shove it towards you (a blueshift).
- [ ] Tracks B2 and B19, new page `heatcap`: three constant-energy boxes (atoms, two-atom molecules on
  springs, a 2D LJ crystal) heated by the same counted doses; heat per degree 1.01k, 2.45k, 1.94k against
  equipartition's 1, 2.5, 2; bars show where the energy sits (flying, tumbling, vibrating, stretched); add
  enough heat and the crystal melts. Part 2: real 3D molecules' quantum rotor and oscillator: hydrogen stops
  spinning below ~100 K, vibrations frozen at room temperature, argon's spin never unfreezes (mass in the
  nucleus), with occupation bars of hydrogen's levels. New scenario `heatcap`.
