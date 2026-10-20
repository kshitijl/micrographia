# Working on Micrographia

Micrographia is browser pages that answer physics questions with live simulations; Brownie is
its GPU molecular dynamics engine. See `README.md`, `docs/status.md` and `docs/architecture.md`.

## Process

- Commit often: after each change that works (a page fixed, a feature added, a test passing), not
  in big batches at the end. Commit locally; do not push unless asked.
- Page review feedback goes into `initial-review.md` as it is given, with what was done about it.

## Code

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
