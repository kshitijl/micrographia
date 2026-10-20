# Statistical mechanics, simulated

Browser pages that answer questions about heat, entropy and everyday physics with live
molecular simulations on the GPU (WebGPU). See `questions.md` for the questions,
`docs/architecture.md` for the design and `docs/research.md` for background.

## Layout

```
crates/gpu-prims   GPU primitives (scan, sort, reduce, 64-bit atomics, Threefry) + self-checks
crates/md-core     units, fixed-point state, initial conditions, pair potentials
crates/md-ref      CPU reference engines (integer positions with f32/f64 forces; float MD)
crates/md          GPU engine (integer positions, f32 forces) and GPU instruments
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
