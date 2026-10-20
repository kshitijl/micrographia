# Initial review of the pages (October 2026)

A working file, not permanent: Kshitij's feedback on each page as given, what was done about
it, and decisions still being discussed. Check each item off once it is done and confirmed.
Delete this file once everything here has been settled.

## Decisions

- **Names.** The MD engine is **Brownie** (Robert Brown; the Kodak Brownie camera that put
  photography in everyone's hands). The project is **Micrographia** (Hooke's 1665 book showing
  the public the microscopic world). Done so far: README, site title, every page's top bar.
  Open: rename the `md` crate to `brownie` (and `md-core`/`md-ref` to match)? Not done yet.
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
