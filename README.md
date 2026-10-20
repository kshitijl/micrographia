# Micrographia

Browser pages that answer questions about heat, entropy and everyday physics with live
molecular simulations on the GPU (WebGPU), so anyone with a phone or a laptop can see how the
nano world works. Named after Robert Hooke's *Micrographia* (1665), the first popular book of
what the microscope showed. The molecular dynamics engine is **Brownie**, after Robert Brown,
who first saw molecules' jiggling (in pollen grains), and the Kodak Brownie, the cheap camera
that put photography in everyone's hands. See `questions.md` for the questions,
`docs/status.md` for what exists and how it was checked, `docs/architecture.md` for the design
and `docs/research.md` for background.

To look at the pages: `cargo xtask dev`, then open http://localhost:5173 in a browser with
WebGPU (Chrome, Edge, or Safari 26+).

## Layout

```
crates/gpu-prims   GPU primitives (scan, sort, reduce, 64-bit atomics, Threefry) + self-checks
crates/md-core     units, fixed-point state, initial conditions, pair potentials
crates/md-ref      CPU reference engines (integer positions with f32/f64 forces; float MD)
crates/md          GPU engine (integer positions, f32 forces) and GPU instruments
crates/scenarios   the systems behind the pages (pure data, runnable on any engine)
crates/wasm        browser API (wasm-bindgen): simulations for pages, renderer, self-tests
crates/bench       native benchmarks
xtask              project commands
web/               Vite + TypeScript pages and Playwright tests
```

## Commands

`cargo xtask <command>` (a small Rust program in `xtask/`, so nothing else to install):

| command | does |
|---|---|
| `test` | native tests: CPU engines, GPU primitives and GPU engine (needs a GPU) |
| `wasm` | build the browser module into `web/src/wasm` |
| `web` | wasm + production build of the pages into `web/dist` |
| `dev` | wasm + Vite dev server at http://localhost:5173 |
| `test-browser` | web + Playwright tests in headless Chrome |
| `bench` | native benchmarks |
| `all` | `test` + `test-browser` |

Requirements: Rust with the `wasm32-unknown-unknown` target, `wasm-bindgen-cli` matching the
`wasm-bindgen` version in `Cargo.lock` (`cargo install wasm-bindgen-cli --version 0.2.129`),
Node, and Google Chrome for the browser tests.
