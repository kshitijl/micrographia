# Working on Micrographia

Micrographia is browser pages that answer physics questions with live simulations; Rahman (after
Aneesur Rahman, who ran the first realistic MD simulation in 1964) is its GPU molecular dynamics
engine. See `README.md`, `docs/status.md` and `docs/architecture.md`.

## Process

- Commit often: after each change that works (a page fixed, a feature added, a test passing), not
  in big batches at the end. Commit locally; do not push unless asked.
- Page review feedback goes into `initial-review.md` as it is given, with what was done about it.

## Code

- Before changing integrators, thermostats, walls, scenario set-up or measurements, read
  `docs/research.md` §18 (lessons from the textbooks and from the bugs found so far, with
  chapter and page citations).

- Rahman will be published for others to use, so its API matters. Build the pages on the public
  API (the scenario builders, the engine's methods, the wasm `DiscSim` and `web/src/lib`), not on
  internals; when a page needs something new, add it to the public API properly.

- Grug brain: dumb, simple code when possible. No cleverness or abstraction that isn't paying for
  itself.
- Make illegal states unrepresentable (a generalisation of "parse, don't validate"): use sum
  types (enums with data) rather than flags plus fields that only sometimes mean something, parse
  inputs once into types that can only hold valid values, and so on. Not everything has to live in
  the type system: stop where it would complicate things.
- Performance comes first in the engines. Hot loops, core data structures and data layouts
  (GPU buffers, shaders, the CPU twins' inner loops) are designed for speed and correctness;
  type-level niceties there must not cost performance. The type discipline above applies mainly
  to glue code, configuration, scenarios and page logic.

## Pages

- Interactivity: users should be able to reach in and mess with the simulation. Click around,
  select molecules (drag a box), give them kicks or nudges, freeze them, shake things up, change
  parameters while running. Prefer that over passive watching.
- Show a total-energy graph (and, where it means something, entropy) on simulation pages, so
  conservation is visible and so is what a kick or a freeze does to it.
- Each page says at the bottom what kind of simulation it is: method (molecular dynamics, ...),
  potentials, ensemble (constant energy, thermostat, heat-bath walls), size, time step.
- Make it visceral: the viewer should feel the forces and effects, not read them off a number.
  Things should visibly bend, stretch, burst, rise, crumple; springs coloured by their tension,
  walls that bow, membranes that bulge.
- A theme throughout: entropy matters more as temperature rises. An effect that gets stronger
  when hotter (rubber's pull, osmotic pressure, depletion forces) is evidence that it is driven by
  entropy; pages should show and say so.
- Prefer the microcanonical ensemble: energy strictly conserved (no thermostat), with Boltzmann
  distributions emerging by themselves. Use a heat bath only when the physics needs one, and say
  so. Every page shows the total energy of the whole simulation and states its ensemble plainly.
- Never make a dropdown (or any control) that resets the simulation to change an amount, a
  quantity or a position when the change could be an act inside the running simulation: pour it
  in, drop it, pull it out, drag it. We are in a lab; the student picks something up or uses a
  tool to prod the system. Resetting is for starting over, not for changing things.
- Let users grab things: click and drag objects (an end of a rubber band, a piston, a membrane)
  rather than only pressing buttons that move them for you.
