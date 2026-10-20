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
