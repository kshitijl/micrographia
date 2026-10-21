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
