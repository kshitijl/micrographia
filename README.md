# Micrographia

Browser pages that answer questions about heat, entropy and everyday physics with live
molecular simulations on the GPU (WebGPU), so anyone with a phone or a laptop can see how the
nano world works. Named after Robert Hooke's *Micrographia* (1665), the first popular book of
what the microscope showed. The molecular dynamics engine is **Rahman**, after Aneesur Rahman, who ran
the first realistic molecular dynamics simulation (liquid argon, 1964).

To look at the pages: `cargo xtask dev`, then open http://localhost:5173 in a browser with
WebGPU (Chrome, Edge, or Safari 26+).

## Layout

```
crates/gpu-prims   GPU primitives (scan, sort, reduce, 64-bit atomics, Threefry) + self-checks
crates/md-core     units, fixed-point state, initial conditions, pair potentials
crates/md-ref      CPU engines (integer positions with f32/f64 forces, with a fast SIMD pair path;
                   float MD); in the browser as CpuDiscSim, the fallback without WebGPU
crates/md          GPU engine (integer positions, f32 forces) and GPU instruments
crates/scenarios   the systems behind the pages (pure data, runnable on any engine)
crates/render      the 2D renderer: discs from the GPU buffers, to a canvas or an offscreen texture
crates/wasm        browser API (wasm-bindgen): simulations for pages, 3D renderer, self-tests
crates/record      headless video recorder (cargo xtask record)
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
| `bench` | native benchmarks (`--cpu` for the CPU engine too) |
| `record` | record a scenario as a video, headless, at any size (`record boiling --size 1920x1080 --frames 600`); needs ffmpeg |
| `all` | `test` + `test-browser` |

Requirements: Rust with the `wasm32-unknown-unknown` target, `wasm-bindgen-cli` matching the
`wasm-bindgen` version in `Cargo.lock` (`cargo install wasm-bindgen-cli --version 0.2.129`),
Node, and Google Chrome for the browser tests.
