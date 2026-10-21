# Where Micrographia stands, and where it should go

Written 2026-10-21, at the end of a long session that ran overnight: three new tracks of pages, a
CPU SIMD engine and a video recorder. Section 9 records the user's replies and decisions, and
overrides sections 1–8 where they disagree; section 10 is the resulting backlog. The user's
replies are quoted verbatim in docs/decisions.md. It records what was done, what we learnt, and what should
change. It is opinionated on purpose: the user asked what is stupid, what is pointless, and what
we are not noticing.

## 1. Where things stand

- **50 pages**, 56 browser test files, about 18,000 lines of Rust and 15,000 of TypeScript, and
  about 300 KB of Markdown (research.md alone is 106 KB).
- **This session:** 16 new pages, plus fixes and N readouts on earlier ones:
  - the 1905 track: `light`, `relativity`;
  - the Perrin track: `avogadro`, `lines`, `heatcap`, `cloud`, `perrin`;
  - brainstorm questions: `motor`, `depletion`, `sweat`, `soda`, `folding`, `blood`.
- **Engine work this session:**
  - `add_bond` (reactions);
  - a bug fixed: the bath heated gas under a free piston;
  - a fast CPU pair path (5.5×, vectorised);
  - `CpuDiscSim`: WebAssembly SIMD, a 2D canvas, the fallback without WebGPU;
  - `selftest/engines.html`;
  - the renderer split into its own crate;
  - `cargo xtask record` for video.
- **Deferred, with reasons** (initial-review.md, research.md §18.8):
  - A1: Einstein's suspension viscosity;
  - cavitation;
  - Maxwell's trapdoor.
- **`initial-review.md` has 93 unchecked "to confirm" items.** Almost everything built since the
  first review is unreviewed by the one person who decides whether it is right.

## 2. Learnings (technical)

- **2D Lennard-Jones is a poor material for real-world questions.** Its liquid exists only between
  kT ≈ 0.41 and 0.46. This made several pages hard or impossible:
  - cavitation (no tensile strength left);
  - soda (the pool evaporates at 0.46);
  - humidity and sweat (squeezed into a narrow window);
  - and, before this session, boiling and freezing (which moved to 3D).

  Half the tuning in this session was fighting that window, not explaining physics.
- **The physical timescale, not the GPU, is the bottleneck.**
  - Evaporation, gas exchange, diffusion through a pool and CO loading are slow processes.
  - The engine already runs near its memory-bandwidth limit at large N, and at about 22k steps/s
    at N = 500 in Chrome.
  - What makes a page slow is waiting for rare or slow events.
  - More FLOPs will not fix that. Smarter methods will: Monte Carlo moves, lattice models,
    kinetic models, coarse-graining, or honest time-lapse.
- **The CPU SIMD engine does not beat the GPU in the browser** on this Mac (2–14× slower at every
  size), and Safari 26 now ships WebGPU.
  - Its real value is small: a fallback, and a reference.
  - More CPU speed work (Verlet lists, cluster tiles, threads with COOP/COEP) would be pushing
    against a limit that bears no fruit for pages. See §4.
- **The f32 path is not bit-exact between CPU and GPU.**
  - The GPU can round a division differently.
  - Earlier comments and commit messages said "bit-exact with the GPU", which was wrong; corrected.
  - The integer path is the cross-device one.
  - The reversal page is fine, because it reverses on the same device.
- **Bulk heat baths keep biting.** The Andersen bath:
  - killed flow (earlier);
  - heated a gas under a free piston (this session);
  - slows diffusion (the soda page).

  Friction on a lid without matching noise made it a cold wall. **Rule: physical baths (walls),
  or an Ornstein–Uhlenbeck pair (friction plus its noise), never friction alone.**
- **Molecular-size "colloids" are not colloids.**
  - 3–6 σ grains lowered a WCA liquid's viscosity rather than raising it by 2.5φ.
  - Removing molecules to make room for them diluted the solvent.
  - Continuum results need a scale separation the browser cannot afford in 3D.
- **Soft potentials blur "entropic" textbook laws.**
  - The depletion force grows less than ∝ T, because hot molecules squash closer.
  - Pages that state laws (∝ T, exactly 2.5φ, Haldane's ratio) should test against the law with the
    model's actual geometry, as `depletion` does now, not the hard-sphere ideal.
- **"Instant equilibrium" models can mislead.**
  - The first `blood` model let CO jump onto the sites oxygen vacated in the tissues. That gave the
    opposite of the real effect (no loss of delivery).
  - The physics was in the kinetics: CO exchanges only in the lungs, and slowly.
  - Before claiming a mechanism, ask which processes are fast and which are slow.
- **Profiling notes:**
  - `sample` only works on runs longer than a few seconds.
  - Vectorisation in Rust was blocked first by bounds checks, then by mixing i64 and f32 lanes in
    one loop.
  - Splitting loops and slicing to exact lengths fixed it.
  - The remaining cost was per-row overhead, not arithmetic.
- **Test infrastructure is straining.**
  - The full suite takes 40+ minutes and contends for the GPU.
  - It twice hung at exit after all its tests had passed.
  - Several thresholds were tuned on single runs.
  - It needs tiers (§5).

## 3. What we are not noticing

1. **Nobody has used these pages.**
   - Not one student, child or friend has tried them; the only reader is the author, at the end of
     a test run.
   - There is no deployment and no URL to share.
   - Every page's design ("drag a box, then press Heat (1.5× faster)") is unvalidated.
   - We do not know which pages people find magical and which they bounce off.
   - This is the biggest blind spot. It costs an afternoon to fix.
2. **We are in a page-factory rut.**
   - The loop is: pick a question, write a scenario, tune until a native experiment matches theory
     within 10–30%, wrap it in the same template, write a test, commit.
   - The output is consistent and defensible, and increasingly samey: discs on a dark stage, a
     plot, six readouts and three paragraphs.
   - The 51st page adds less than polishing the best 10 would.
   - Quantity has outrun review (93 open items) and outrun maintenance (a 40-minute suite).
3. **The pages read more than they show.**
   - Most have more words than motion: readout panels of four to six numbers, long notes, caveats.
   - CLAUDE.md says "make it visceral"; the honest score is about half.
   - Lids bounce and tracks draw, but most effects are told by a number turning from 0.75 to 0.52.
   - The single most convincing moments are visual and live:
     - the reversal (un-stirring);
     - the CO cutting oxygen delivery while you watch the colours;
     - the lid jumping in the hydrogen–chlorine explosion;
     - the speeds piling up below c.
4. **Phones.**
   - "Anyone with a phone" is in the README's first paragraph, and nothing has been measured on a
     phone.
   - Default sizes (5,000–20,000 particles) were chosen on an M1 Pro.
   - Layouts are designed at 1300 px wide.
   - Touch drag-to-select collides with scrolling.
5. **MD is the hammer, and not every question is a nail.**
   - Ten of the 50 pages are not MD at all: kidney, light, relativity, motor, folding, blood,
     counting, Millikan, the Einstein solid, and parts of others. Several are among the clearest.
   - Several MD pages are really page-level rules running on top of an MD gas: the reactions on
     `avogadro`, the ionisation on `cloud`, the wind on `sweat`. That is fine, but they should say so
     at the top, not in a caveat.
   - The site's identity could be "the right model for each question, with the molecular one when
     it is the point", not "everything is GPU MD".
6. **The engine's API is being worked around rather than extended.**
   - Reactions, ionisation, renewal winds, lids with OU noise and depletion force sums all live in
     page JavaScript, reading every position back each frame.
   - If Rahman is to be published (CLAUDE.md), the missing piece is clear: hooks.
     - Per-step rules on the GPU: region kicks, species-switch rules on contact, simple bond
       formation.
     - Reductions for force sums.

     Otherwise every outside user rewrites what the pages did.
7. **The Perrin table is the best idea on the site, and it is buried.**
   - It turns 50 separate pages into one quest: you measure Avogadro's number six different ways
     and watch your values agree.
   - That is a narrative, a course, and a reason to come back. Right now it is one link among 50.

## 4. What to stop

- **Stop adding pages for a while.** Review, curate and polish first. A page that is wrong in a way
  only the user would notice costs more later than it saved now.
- **Stop CPU speed work.**
  - Keep `CpuDiscSim` as a fallback.
  - Drop Verlet lists, cluster tiles and wasm threads for the CPU.
  - Drop the planned engine auto-selection: on desktops the GPU wins.
- **Stop chasing GPU throughput for its own sake.**
  - Big N is near the bandwidth limit.
  - Small N is fast enough for slow-motion pages.
  - The only pages that need more speed need different physics (§2), not faster MD.
- **Stop forcing 2D LJ into liquid-water questions** (cavitation, ice, soda, sweat): use 3D, a
  water-like model, or say plainly that the liquid is a toy with a window of 0.05 kT.
- **Stop single-run thresholds** in tests: seeds and block errors (research.md §18.3 already says so).
- **Stop deferring hard cases silently.** A1, the demon and cavitation are recorded; they should
  stay deferred unless a new method makes them easy (§6), not be retried by tuning.

## 5. Low-hanging fruit

1. **Deploy.**
   - `vite build` is static.
   - GitHub Pages or Cloudflare Pages takes ten minutes.
   - Add COOP/COEP headers only if threads are ever needed.
   - Share one page with three people and watch them use it.
2. **Phone sensors.** Tilt the phone and gravity turns (`DeviceOrientation`); shake it and the gas
   heats (`DeviceMotion`). The atmosphere, Perrin's grains, the boiling pot and the sweat film all
   have gravity already. This is the most visceral interaction available, and a few dozen lines.
3. **Sound.**
   - Geiger clicks on `counting` and `cloud`.
   - Pressure as pitch on the piston pages.
   - Collisions with walls as a crackle.
   - Web Audio, a few lines each.

   Hearing a gas get hotter is visceral in a way a number is not.
4. **A real unit system, site-wide.**
   - Map ε, σ and τ to argon (σ = 3.4 Å, ε/k = 120 K, τ ≈ 2 ps) once, and show kelvin, nanometres
     and picoseconds everywhere, with σ units on a toggle.
   - Several pages (lines, sedimentation, Millikan, perrin) already invented their own lab units
     ad hoc.
5. **Adaptive size.** Pick N from a 200 ms calibration run on page load (the engines self-test
   already measures it), so phones get 2,000 particles and desktops 20,000.
6. **Test tiers.**
   - A fast smoke tier (every page loads, runs 100 steps, has its About section): under 2 minutes,
     run on every commit.
   - The physics tier (the current specs) nightly or before a push.
   - Fix the hang at exit (probably the reused vite preview server).
7. **A "model or molecules" badge** at the top of each page: *molecular dynamics*, *Monte Carlo of
   a model*, or *a rule on top of molecules*, so honesty is up front instead of in the caveats.
8. **Make the Perrin quest the front door.** Ask "How do we know atoms exist?", and lead through
   six measurements to your table, with the 50 other questions as side trips.

## 6. Potentially really cool (think big)

- **One lab, not 50 pages.**
  - A shared workbench with tools that persist across experiments: a burner, a piston, a salt
    shaker, a membrane, a lamp, a pipette, a thermometer and a pressure gauge.
  - Each question becomes a preset arrangement. The student can keep going ("what if I add salt
    here too?"), because the tools work anywhere.
  - The engine already supports most of the tools; the pages hard-wire them.
- **Ask the simulation.**
  - A small "why did that happen?" box on each page, answered by an LLM given the scenario, the
    current readouts and the page's notes.
  - Further out, the user types a new question ("what if the molecules were twice as heavy?") and
    the scenario builders, which are already a clean API, compose a new experiment.
  - The scenario crate is close to being a language for this.
- **Class mode.**
  - Thirty students each measure a few Millikan drops or a few Perrin heights on their phones.
  - The class histogram fills in live on the teacher's screen.
  - Measurements of N from the whole class form the Perrin table.
  - This needs shared state, which is a small server or a hosted page with shared storage.
- **Video series.**
  - The recorder now renders any 2D scenario at 4K, independent of real time.
  - A one-minute explainer per question (recorded run, voice-over, the page linked) would reach
    far more people than the pages alone.
  - The pages then become the "try it yourself" follow-up.
- **Water.**
  - A 2D water model with hydrogen bonds: the Mercedes–Benz model, three arms per molecule, cheap
    on the GPU.
  - It has a density maximum, ice that floats, hydrophobic effects and a real liquid range.
  - It would fix half of §2's complaints, and "why does ice float?" (on the brainstorm list) is one
    of the most-asked physics questions there is.
  - This is the one engine feature worth a big push.
- **Enhanced sampling as a visible tool.**
  - Umbrella sampling, metadynamics and replica exchange shown as part of the page, not hidden.
  - For example, a "speed up rare events" button that visibly adds a bias and then shows how
    the true rate is recovered.
  - The FEP page already does a version of this, and it is the user's own field (DESRES).
  - Nobody explains free-energy methods interactively; that could be a distinctive niche.
- **Hardware in the loop.** A $10 temperature sensor or the phone's barometer feeding a real
  measurement into a page (the pressure at your altitude, for the atmosphere page).

## 7. Recommended next steps, in order

1. Review the backlog with the user: the 93 items in initial-review.md, triaged into
   confirmed, fix and drop. Delete or merge the weakest pages.
2. Deploy, and watch three people use five pages. Write down what they do.
3. Phones: measure, adapt N, fix touch, add tilt-for-gravity.
4. Test tiers; fix the hang at exit.
5. Pick 10 flagship pages and polish them: fewer numbers, more motion, sound, one sentence of
   text visible before the fold.
6. The Perrin quest as the front door; a site-wide unit system.
7. Then, and only then, new physics. First the water model; then the deferred items (A1 with big
   grains in 3D at equal pressure; the demon with free areas measured); then the rest of the
   brainstorm list, choosing the method per question.
8. Engine hooks for page rules on the GPU (contact reactions, region rules, force sums) before
   publishing Rahman, with one external-style example built only on the public API.

## 8. A note on what went well

- **The working pattern held up and caught real bugs.** Native experiment first, then the page,
  then a test against theory with the model's real geometry, then honest caveats. It found:
  - the Andersen piston bug;
  - the CO kinetics mistake;
  - the dilution in the suspension;
  - the false bit-exactness claim.
- **Several new pages are genuinely good** and would be worth polishing first:
  - `relativity`: the lag of a hot gas, explained through the enthalpy and Laue rather than hidden;
  - `blood`: the left shift you can watch;
  - `avogadro`: the explosion that settles to two volumes;
  - `cloud`: the Bragg peak drawn as droplets.

The problem is not quality. It is that nobody but the author has seen any of it.

## 9. The user's responses and decisions (2026-10-21)

The user read sections 1–8 and replied. Where this section disagrees with them, this section
wins. Nothing here is private: it is all going on GitHub, with the reasoning and the ways of
working, because this is textbook physics and simulation methods.

### How to see the build so far

- **The 50 pages are idea generation and prototyping.** The page-factory rut is fine for two days
  of building. The point was to have many candidates to choose from.
- **The plan:**
  - review what has been built (the 93 open items in initial-review.md);
  - pick 2–5 pages to polish and make great;
  - deploy them one by one, the best first;
  - record a video explainer for each, and publish it alongside.

### Guiding questions

- **"How do we know atoms exist?" is a guiding question for Micrographia,** with the Perrin
  table at its heart. It is probably not the only one, but it is a great one.
- **It connects to osmosis through Einstein's 1905 papers.** Osmosis was the user's first
  interest in this material: van 't Hoff's osmotic pressure is the ideal-gas law for solutes, and
  Einstein's Brownian-motion paper rests on exactly that analogy.
- **Free energy as a theme throughout.**
  - Free energy is mysterious to many people.
  - Micrographia should show that it is really the entropy of the whole universe: energy released
    in one small place creates more microstates for everything else.
  - It should also show, repeatedly, the temperature dependence of entropic effects and forces.
  - Free-energy methods belong to this theme too.
  - The user worked in FEP and will make an explainer video on alchemical FEP. The current `fep`
    page is not good and needs a lot of work.

### Molecular dynamics, and other methods

- **Don't force MD onto every question,** but MD stays the heart of the site. Atoms jiggling and
  giving rise to emergent behaviour is the most intuitive picture and the easiest sell: everyone
  has heard about atoms, and here they are.
- **"How this simulation works" needs a rethink.**
  - The paragraph at the bottom of each page is hard to read; at the least it should be a table.
  - We will probably need an explainer page for each method (molecular dynamics, Monte Carlo,
    lattice Boltzmann, lattice models, Langevin dynamics…), linked from each scenario page.
- **Units: keep theorist units** ("k = G = e = 2π = 1"), not argon's SI units. Section 5's proposed
  unit system is dropped.

### Rahman as a published simulator

The goal is a modern, fresh MD simulator, within reach at least for education and exploration,
if not for production MD. It should be:
- written in a modern systems language (Rust), with lots of tests;
- able to run on a Mac or any other GPU, natively and in the browser;
- under a very permissive licence (MIT or similar);
- rich in features, among them FEP, reversibility, and exact reproducibility.

University labs and PhD students could build on it. Others could fork it to make explainers like
these.

- **Performance matters for that,** so section 4's "stop GPU work" is premature.
  - **Benchmark against other MD packages first,** on the GPU and on the CPU: build them on the
    same machines and run the same systems.
  - Candidates: LAMMPS, HOOMD-blue, OpenMM, GROMACS; jax-md and similar for the browser and
    Python side.
  - Then decide where Rahman is competitive and where it is worth pushing.
- **A playground**, like Shadertoy:
  - a page where people write a few lines of JavaScript against a small, friendly API (make a
    box, add particles, set potentials, run, kick, plot), then simulate it and mess with it;
  - JavaScript, because it is native to the browser.

  This is also a good forcing function for the public API (section 3, item 6).

### CPU or GPU

- The question: does the CPU beat the GPU at small sizes? The measured answer:
  - **Natively** (M1 Pro), at N = 1,000, they tie: the CPU's fast path runs 10.0k steps/s on one
    core (LJ), the GPU 9.7k.
  - **In Chrome**, the GPU wins at every size measured: 22.2k against 11.3k at N = 500, 2.1k
    against 0.18k at 32,000.
  - The earlier estimate that the CPU would win by ten times at small N was not borne out.
- What could still change it:
  - CPU threads;
  - a better CPU kernel (Verlet lists, cluster tiles);
  - devices with weak GPUs or costly dispatches (phones, unmeasured);
  - and tiny systems (a few hundred particles), not yet measured.
- **Choose the backend by device, not by a calibration run,** if devices behave alike: every
  iPhone 15 performs about the same, as does every M2 MacBook Pro. A table of measured devices,
  with a short calibration run only for unknown ones.
- The same table sets each simulation's size per device (section 5, item 5).

### Tests

- A fast smoke tier is a good idea, but "nightly" needs thought: development happens on a laptop,
  and there is no server.
  - Options: the user's desktop as a runner (it has a GPU), or a scheduled job when the laptop is
    idle.
  - GitHub's hosted runners have no GPU, so the physics tier cannot run there.

### Phones

- Measure on real phones over the local network: serve from the laptop, and load it on the phone
  through **Tailscale**.

### Accepted, not now

- "Ask the simulation" (an LLM explaining the page): not for now.
- Class mode (a class's measurements gathered live): a good idea, not for now.

## 10. Backlog (from section 9)

1. **Review** the 93 open items in initial-review.md with the user, and pick 2–5 pages to polish.
2. **Phones:** serve over Tailscale and measure every candidate page on a phone (speed, layout,
   touch).
3. **Tilt the phone to turn gravity** (`DeviceOrientation`), and shake it to heat (`DeviceMotion`),
   on the pages that have gravity (atmosphere, sedimentation, boiling, sweat, humidity, Brownian).
4. **Sound.**
   - Decide which pages benefit: Geiger clicks for `counting` and `cloud`; pressure as pitch on
     piston pages; wall hits as crackle; bubbles and boiling; a membrane's knocks for osmosis.
   - Make the recorder able to write the simulation's sound into the video too, for the explainer
     videos.
5. **"How this simulation works":** a table instead of a paragraph, and an explainer page for each
   method, linked from the scenario pages.
6. **The Perrin quest**, "How do we know atoms exist?", as a guiding thread, connected through
   Einstein 1905 to osmosis.
7. **Free energy as a theme:**
   - free energy as the entropy of the universe, energy released becoming microstates elsewhere;
   - entropic effects growing with temperature;
   - rebuild the `fep` page, alongside the user's video on alchemical FEP.
8. **Benchmarks against other MD packages** (LAMMPS, HOOMD-blue, OpenMM, GROMACS; GPU and CPU), on
   the same systems and machines. Then decide on further GPU and CPU performance work.
9. **Backend and size by device:** a table of measured devices (phones, Macs), with calibration
   only for unknown ones.
10. **A JavaScript playground** for Rahman (Shadertoy-like): a small friendly API, a few lines to set
    up a box and play with it. This also drives the public API (with GPU hooks for page rules).
11. **Publishing Rahman:** permissive licence, documentation, examples built only on the public
    API.
12. **2D water** (the Mercedes–Benz model, hydrogen-bond network). First find its killer demo:
    - something fast enough to happen in a browser;
    - that needs the hydrogen-bond network;
    - candidates: ice freezing and floating; oil and water (the hydrophobic effect as a bond
      network forced to rearrange); the density maximum.
13. **One lab:** tools that persist across experiments (burner, piston, salt shaker, membrane,
    lamp, gauges).
14. **Test tiers:** a smoke tier on every commit; decide where the physics tier runs (the desktop
    as a GPU runner?); fix the suite's hang at exit.
15. **Later, not now:** "ask the simulation"; class mode.
