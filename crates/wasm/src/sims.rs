//! Simulations exposed to pages: one GPU engine running a named scenario, with a renderer, an
//! observer and the saved initial state.

use crate::render::{Renderer, NO_HIGHLIGHT, RECT_FLOATS};
use crate::{browser_gpu, err, obj};
use gpu_prims::Prims;
use js_sys::{Float32Array, Float64Array, Object, Uint32Array};
use md::observe::{mixing, segregation, Observer};
use md::GpuEngine;
use md_core::{fixed, Pair, State, System, Wall};
use scenarios::{Finish, Options};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

pub(crate) fn options(o: &JsValue) -> Options {
    let mut out = Options::default();
    if let Some(obj) = o.dyn_ref::<Object>() {
        for entry in Object::entries(obj).iter() {
            let pair = js_sys::Array::from(&entry);
            if let (Some(k), Some(v)) = (pair.get(0).as_string(), pair.get(1).as_f64()) {
                out = out.with(&k, v);
            }
        }
    }
    out
}

/// A 2D particle simulation on the GPU, drawn into a canvas.
#[wasm_bindgen]
pub struct DiscSim {
    engine: GpuEngine,
    observer: Observer,
    renderer: Renderer,
    initial: State,
    /// Averaging time (τ) for `smooth_speed_colors`.
    smooth_window: f64,
}

#[wasm_bindgen]
impl DiscSim {
    /// Build scenario `name` (see the `scenarios` crate) with numeric `options`, run its
    /// warm-up on the GPU, and attach a renderer to `canvas`.
    pub async fn create(canvas: web_sys::HtmlCanvasElement, name: String, opts: JsValue) -> Result<DiscSim, JsError> {
        let (w, h) = (canvas.width(), canvas.height());
        let (gpu, surface) = browser_gpu(Some(canvas)).await?;
        let prims = Prims::new(&gpu);
        let o = options(&opts);
        let sc = scenarios::build(&name, &o).map_err(err)?;
        let mut engine = GpuEngine::new(&prims, sc.sys.clone(), &sc.state);
        let mut state = sc.state.clone();
        if sc.warmup > 0 || sc.finish != Finish::None {
            engine.step(sc.warmup);
            state = engine.read_state().await;
            sc.finish.apply(&mut state, &sc.sys);
            engine.upload(&state);
        }
        let observer = Observer::new(&engine, sc.grid, sc.marked);
        let renderer = Renderer::new(&gpu, surface.expect("surface"), &engine, &sc.radii, w, h);
        Ok(DiscSim { engine, observer, renderer, initial: state, smooth_window: 0.0 })
    }

    /// Spatial dimensions (2; `BallSim` has 3).
    pub fn dims(&self) -> u32 {
        2
    }

    pub fn step(&mut self, k: u32) {
        self.engine.step(k as usize);
        if self.renderer.params.smoothed > 0.0 {
            self.renderer.smooth_heat(k as f64 * self.engine.sys.dt, self.smooth_window);
        }
    }

    /// Resolves when the GPU has finished all work submitted so far.
    pub fn sync(&self) -> js_sys::Promise {
        let (tx, rx) = futures_channel::oneshot::channel::<()>();
        self.engine.gpu().queue.on_submitted_work_done(move || {
            let _ = tx.send(());
        });
        future_to_promise(async move {
            let _ = rx.await;
            Ok(JsValue::UNDEFINED)
        })
    }

    pub fn reverse(&mut self) {
        self.engine.reverse();
    }

    /// Flip the least significant bit (2⁻³² σ) of the x coordinate of the particle with id `i`.
    pub fn flip_bit(&mut self, i: u32) {
        self.engine.flip_bit(i, 0, 0);
    }

    /// Flip the least significant bit (2⁻³² σ) of the x coordinate of every particle whose
    /// species is in the bitmask `species`.
    pub fn flip_species_bit(&mut self, species: u32) {
        self.engine.flip_species_bit(species, 0, 0);
    }

    /// Resolves to the positions (σ, x and y interleaved, as f64, which holds every fixed-point
    /// position exactly) of the particles with the given ids.
    pub fn positions_of(&self, ids: Vec<u32>) -> js_sys::Promise {
        let fut = self.engine.read_positions();
        future_to_promise(async move {
            let p = fut.await;
            let out: Vec<f64> = ids.iter().flat_map(|&i| {
                let q = p[i as usize];
                [fixed::to_f64(q[0]), fixed::to_f64(q[1])]
            }).collect();
            Ok(js_sys::Float64Array::from(&out[..]).into())
        })
    }

    /// Ids of `count` particles of species `species`, spread over where that species started:
    /// the one nearest its centre and the ones nearest points at `spread` of its extent around it.
    pub fn representatives(&self, species: u32, count: u32, spread: f64) -> Vec<u32> {
        let s = &self.initial;
        let pts: Vec<(u32, [f64; 2])> = (0..s.n()).filter(|&i| s.species[i] == species).map(|i| (i as u32, [fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1])])).collect();
        if pts.is_empty() {
            return vec![];
        }
        let c = pts.iter().fold([0.0, 0.0], |a, (_, p)| [a[0] + p[0], a[1] + p[1]]).map(|x| x / pts.len() as f64);
        let r = pts.iter().map(|(_, p)| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt()).fold(0.0, f64::max);
        let mut targets = vec![c];
        for k in 1..count {
            let a = std::f64::consts::TAU * (k - 1) as f64 / (count - 1).max(1) as f64 + 0.4;
            targets.push([c[0] + spread * r * a.cos(), c[1] + spread * r * a.sin()]);
        }
        targets
            .iter()
            .map(|t| pts.iter().min_by(|a, b| {
                let d = |p: &[f64; 2]| (p[0] - t[0]).powi(2) + (p[1] - t[1]).powi(2);
                d(&a.1).total_cmp(&d(&b.1))
            }).unwrap().0)
            .collect()
    }

    /// Id of the particle of species `species` nearest the box centre in the initial state.
    pub fn central(&self, species: u32) -> u32 {
        let b = self.engine.sys.box_len;
        let c = [b[0] / 2, b[1] / 2];
        (0..self.initial.n())
            .filter(|&i| self.initial.species[i] == species)
            .min_by_key(|&i| {
                let d = [self.initial.pos[i][0] - c[0], self.initial.pos[i][1] - c[1]];
                (d[0] as i128).pow(2) + (d[1] as i128).pow(2)
            })
            .unwrap_or(0) as u32
    }

    /// Kept for the reversal page.
    pub fn central_marked(&self) -> u32 {
        self.central(1)
    }

    /// Signed step count: positive while running forward, negative while running back.
    pub fn steps(&self) -> f64 {
        self.engine.steps as f64
    }

    pub fn n(&self) -> u32 {
        self.engine.n()
    }

    pub fn dt(&self) -> f64 {
        self.engine.sys.dt
    }

    pub fn box_size(&self) -> Vec<f64> {
        self.engine.sys.box_f64().to_vec()
    }

    /// Compartments per axis (1 if none).
    pub fn compartments(&self) -> Vec<u32> {
        self.engine.sys.compartment_counts().iter().map(|&c| c as u32).collect()
    }

    pub fn grid(&self) -> Vec<u32> {
        self.observer.grid.to_vec()
    }

    /// Mass of each species.
    pub fn masses(&self) -> Vec<f64> {
        self.engine.sys.masses.clone()
    }

    /// Resolves to `{mixing, segregation, temperature}` (temperature from the half-step kinetic
    /// energy).
    pub fn observe(&self) -> js_sys::Promise {
        let fut = self.observer.read();
        let dof = (self.engine.n() as f64 - 1.0).max(1.0);
        future_to_promise(async move {
            let r = fut.await;
            Ok(obj(&[("mixing", mixing(&r).into()), ("segregation", segregation(&r).into()), ("temperature", (r.kinetic / dof).into())]))
        })
    }

    /// Resolves to `{all, marked}`: particle counts per histogram bin (row-major, `grid()`
    /// wide), for all particles and for the scenario's marked species.
    pub fn histogram(&self) -> js_sys::Promise {
        let fut = self.observer.read();
        future_to_promise(async move {
            let r = fut.await;
            Ok(obj(&[("all", Uint32Array::from(&r.all[..]).into()), ("marked", Uint32Array::from(&r.marked[..]).into())]))
        })
    }

    /// Resolves to `{ke, species}`: each particle's kinetic energy (ε, half-step velocity) and
    /// species, in id order.
    pub fn kinetic_energies(&self) -> js_sys::Promise {
        let fut = self.observer.read_kinetic_energies(&self.engine);
        future_to_promise(async move {
            let (ke, sp) = fut.await;
            Ok(obj(&[("ke", Float32Array::from(&ke[..]).into()), ("species", Uint32Array::from(&sp[..]).into())]))
        })
    }

    /// Resolves to the total potential energy (ε) at the last force evaluation, including
    /// m g h under gravity.
    pub fn potential_energy(&self) -> js_sys::Promise {
        let kicks = self.engine.buf.read_kicks();
        // With gravity the potential energy also includes m g h (relative to the origin).
        let g = self.engine.sys.gravity;
        let gravity = (g != [0.0, 0.0]).then(|| (self.engine.read_state(), self.engine.sys.masses.clone()));
        future_to_promise(async move {
            let (_, pe) = kicks.await;
            let mut u: f64 = pe.iter().map(|&x| x as f64).sum();
            if let Some((state, masses)) = gravity {
                let s = state.await;
                for i in 0..s.n() {
                    let m = masses[s.species[i] as usize];
                    u -= m * (g[0] * fixed::to_f64(s.pos[i][0]) + g[1] * fixed::to_f64(s.pos[i][1]));
                }
            }
            Ok(u.into())
        })
    }

    /// Resolves to `{energy, impulse, hits}`, each `7 × per_wall` long (wall-major; walls are
    /// x-low, x-high, y-low, y-high, then the membrane hit from its low and high sides, then the
    /// bulk heat bath in slot 0 of the last), split by compartment along each wall.
    pub fn ledger(&self, per_wall: u32) -> js_sys::Promise {
        let fut = self.engine.buf.read_compartment_ledger(per_wall as usize);
        future_to_promise(async move {
            let l = fut.await;
            let flat = |v: &Vec<Vec<f64>>| Float64Array::from(&v.iter().flatten().copied().collect::<Vec<f64>>()[..]);
            let hits: Vec<f64> = l.hits.iter().flatten().map(|&h| h as f64).collect();
            Ok(obj(&[("energy", flat(&l.energy).into()), ("impulse", flat(&l.impulse).into()), ("hits", Float64Array::from(&hits[..]).into())]))
        })
    }

    /// Resolves to a Float32Array of every particle's (vx, vy) in σ/τ (the half-step velocity),
    /// in id order.
    pub fn velocities(&self) -> js_sys::Promise {
        let fut = self.engine.read_state();
        let dt = self.engine.sys.dt;
        future_to_promise(async move {
            let s = fut.await;
            let v: Vec<f32> = s.vel.iter().flat_map(|q| q.map(|x| (x as f64 * fixed::UNIT / dt) as f32)).collect();
            Ok(Float32Array::from(&v[..]).into())
        })
    }

    /// Resolves to `{pos, species}`: (x, y) in σ of every particle, and species, in id order.
    pub fn all_positions(&self) -> js_sys::Promise {
        let fut = self.engine.read_state();
        future_to_promise(async move {
            let s = fut.await;
            let pos: Vec<f32> = s.pos.iter().flat_map(|p| [fixed::to_f64(p[0]) as f32, fixed::to_f64(p[1]) as f32]).collect();
            Ok(obj(&[("pos", Float32Array::from(&pos[..]).into()), ("species", Uint32Array::from(&s.species[..]).into())]))
        })
    }

    /// Resolves to a Float32Array of (x, y) in σ for every particle of `species`, in id order.
    pub fn positions(&self, species: u32) -> js_sys::Promise {
        let fut = self.engine.read_state();
        future_to_promise(async move {
            let s = fut.await;
            let mut out = Vec::new();
            for i in 0..s.n() {
                if s.species[i] == species {
                    out.push(fixed::to_f64(s.pos[i][0]) as f32);
                    out.push(fixed::to_f64(s.pos[i][1]) as f32);
                }
            }
            Ok(Float32Array::from(&out[..]).into())
        })
    }

    /// Resolves to `{identical, differ, maxDeviation, hash, initialHash}`.
    pub fn compare_to_initial(&self) -> js_sys::Promise {
        let fut = self.engine.read_state();
        let initial = self.initial.clone();
        let sys: System = self.engine.sys.clone();
        future_to_promise(async move {
            let s = fut.await;
            let (differ, max) = s.position_difference(&initial, &sys);
            Ok(obj(&[
                ("identical", (s == initial).into()),
                ("differ", (differ as u32).into()),
                ("maxDeviation", max.into()),
                ("hash", format!("{:016x}", s.hash()).into()),
                ("initialHash", format!("{:016x}", initial.hash()).into()),
            ]))
        })
    }

    /// Change the LJ well depth between species `a` and `b` (cutoff and size unchanged).
    pub fn set_attraction(&mut self, a: u32, b: u32, eps: f64) {
        let p = *self.engine.sys.pair(a, b);
        let sys = self.engine.sys.clone().with_pair(a, b, Pair { eps, ..p });
        self.engine.update_system(sys);
    }

    /// Set the temperature of every thermal wall (`which`: -1 for all, else wall 0..3).
    pub fn set_wall_temperature(&mut self, which: i32, t: f64) {
        let mut sys = self.engine.sys.clone();
        for axis in 0..2 {
            if let Some(ws) = sys.walls[axis].as_mut() {
                for side in 0..2 {
                    if (which < 0 || which as usize == 2 * axis + side) && matches!(ws[side], Wall::Thermal(_)) {
                        ws[side] = Wall::Thermal(t);
                    }
                }
            }
        }
        self.engine.update_system(sys);
    }

    /// Retune the bulk heat bath (the scenario must have one).
    pub fn set_thermostat(&mut self, t: f64, rate: f64) {
        let sys = self.engine.sys.clone().with_thermostat(t, rate);
        self.engine.update_system(sys);
    }

    /// Set the fan's speed (the thermostat's mean x velocity, σ/τ), keeping its region.
    pub fn set_fan_speed(&mut self, u: f64) {
        let mut sys = self.engine.sys.clone();
        if let Some(th) = sys.thermostat.as_mut() {
            th.drift = u;
        }
        self.engine.update_system(sys);
    }

    /// Give every particle of `species` velocity (vx, vy) σ/τ (e.g. pull a pinned anchor).
    pub fn set_species_velocity(&mut self, species: u32, vx: f64, vy: f64) {
        self.engine.set_species_velocity(species, [vx, vy]);
    }

    /// Set a fixed-speed piston's speed (σ/τ; negative compresses).
    pub fn set_piston_speed(&mut self, u: f64) {
        self.engine.set_piston_speed(u);
    }

    /// A free piston's mass and the load pushing it down (ε/σ; the pressure times the width).
    /// A huge mass (1e12) locks it in place; its velocity is set to zero either way.
    pub fn set_piston_load(&mut self, mass: f64, force: f64) {
        let mut sys = self.engine.sys.clone();
        if let Some(p) = sys.piston.as_mut() {
            if let md_core::PistonDrive::Free { .. } = p.drive {
                p.drive = md_core::PistonDrive::Free { mass, force };
                self.engine.update_system(sys);
                self.engine.set_piston_speed(0.0);
            }
        }
    }

    /// Resolves to `[height σ, velocity σ/τ, gas force on it ε/σ, work done on it by the gas ε]`.
    pub fn piston(&self) -> js_sys::Promise {
        let fut = self.engine.read_piston();
        future_to_promise(async move { Ok(Float64Array::from(&fut.await[..]).into()) })
    }

    /// Make one wall (`which`: 0 x-low, 1 x-high, 2 y-low, i.e. the floor, 3 y-high) a heat bath
    /// at `t` (`t` > 0), a hot or cold plate, or an insulating hard wall (`t` = 0). Thermal and hard
    /// walls are planes, so switching between them (or from a soft wall to either) never puts a
    /// molecule inside a wall's repulsive layer. The axis must have walls.
    pub fn set_wall(&mut self, which: u32, t: f64) {
        let mut sys = self.engine.sys.clone();
        let (axis, side) = ((which / 2) as usize, (which % 2) as usize);
        if let Some(ws) = sys.walls.get_mut(axis).and_then(|w| w.as_mut()) {
            ws[side] = if t > 0.0 { Wall::Thermal(t) } else { Wall::Hard };
            self.engine.update_system(sys);
        }
    }

    /// Make every wall a heat bath at `t` (`t` > 0) or insulating (`t` = 0). Insulating walls are
    /// hard (specular) planes like the thermal ones, so switching never puts a molecule inside a
    /// soft wall's repulsive layer. Pistons and membranes are unaffected.
    pub fn set_walls_thermal(&mut self, t: f64) {
        let mut sys = self.engine.sys.clone();
        let piston = sys.piston.is_some();
        for axis in 0..2 {
            if let Some(ws) = sys.walls[axis].as_mut() {
                for side in 0..2 {
                    if axis == 1 && side == 1 && piston {
                        continue;
                    }
                    ws[side] = if t > 0.0 { Wall::Thermal(t) } else { Wall::Hard };
                }
            }
        }
        self.engine.update_system(sys);
    }

    pub fn set_gravity(&mut self, gx: f64, gy: f64) {
        let sys = self.engine.sys.clone().with_gravity([gx, gy]);
        self.engine.update_system(sys);
    }

    pub fn render(&mut self) {
        self.renderer.render();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    /// 24 floats: four species colours, the highlight colour, the background (RGBA, sRGB).
    pub fn set_colors(&mut self, c: Vec<f32>) {
        let p = &mut self.renderer.params;
        for s in 0..4 {
            p.colors[s].copy_from_slice(&c[4 * s..4 * s + 4]);
        }
        p.highlight_color.copy_from_slice(&c[16..20]);
        self.renderer.background = wgpu::Color { r: c[20] as f64, g: c[21] as f64, b: c[22] as f64, a: c[23] as f64 };
    }

    /// Draw the particle with id `i` enlarged in the highlight colour (negative for none).
    pub fn set_highlight(&mut self, i: i32) {
        self.renderer.params.highlight = if i < 0 { NO_HIGHLIGHT } else { i as u32 };
    }

    /// Kick, freeze or shove the unpinned particles in the rectangle (x0, y0)–(x1, y1) (σ):
    /// velocities are multiplied by `scale`, then (vx, vy) σ/τ is added.
    #[allow(clippy::too_many_arguments)]
    pub fn kick_region(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, scale: f64, vx: f64, vy: f64) {
        self.engine.kick_region([x0.min(x1), y0.min(y1)], [x0.max(x1), y0.max(y1)], scale, [vx, vy]);
    }

    /// Set the species of the molecules with these ids (as `all_positions` orders them): add salt
    /// to water, or take it out. Choose molecules with room for their new size.
    pub fn set_species(&mut self, ids: Vec<u32>, species: u32) {
        self.engine.set_species(&ids, species);
    }

    /// "About this simulation": plain-language bullet points (method, forces, boundaries,
    /// ensemble). `names` are the page's names for the species.
    pub fn describe(&self, names: Vec<String>) -> Vec<String> {
        self.engine.sys.describe(self.engine.n() as usize, &names)
    }

    /// The bonds, three numbers each: the two particle ids and the bond type.
    pub fn bonds(&self) -> Vec<u32> {
        self.engine.sys.bonds.iter().flatten().copied().collect()
    }

    /// Break the bond between particles `a` and `b` (ids) for good. Returns whether it existed.
    pub fn remove_bond(&mut self, a: u32, b: u32) -> bool {
        self.engine.remove_bond(a, b)
    }

    /// Bond particles `a` and `b` (ids) with bond type `ty`, as in a reaction (the scenario must
    /// declare that bond type). Returns false if they are already bonded or either is full.
    pub fn add_bond(&mut self, a: u32, b: u32, ty: u32) -> bool {
        self.engine.add_bond(a, b, ty)
    }

    /// How energy can enter or leave the system ("" if it is conserved).
    pub fn energy_note(&self) -> String {
        self.engine.sys.energy_note()
    }

    /// The region of the box drawn on the canvas: [x0, y0, x1, y1] in σ, y upwards.
    pub fn view(&self) -> Vec<f32> {
        let p = &self.renderer.params;
        vec![p.view_lo[0], p.view_lo[1], p.view_hi[0], p.view_hi[1]]
    }

    /// Show only the region (x0, y0)–(x1, y1) of the box, in σ.
    pub fn set_view(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.renderer.set_view([x0, y0], [x1, y1]);
    }

    /// Colour molecules by speed, cool to hot, saturating at `v_max` σ/τ (0 turns it off).
    pub fn color_by_speed(&mut self, v_max: f64) {
        self.renderer.params.speed_scale = if v_max > 0.0 { (1.0 / (v_max * self.engine.sys.dt * md_core::ONE as f64)) as f32 } else { 0.0 };
    }

    /// Colour by each molecule's speed averaged over the last ~`window` τ (0: the instantaneous
    /// speed). The average is of the squared speed, so the colour follows recent kinetic energy:
    /// heat spreading through a liquid shows as a moving front rather than speckle.
    pub fn smooth_speed_colors(&mut self, window: f64) {
        self.smooth_window = window;
        self.renderer.params.smoothed = if window > 0.0 { 1.0 } else { 0.0 };
        self.renderer.reset_heat();
        if window > 0.0 {
            self.renderer.smooth_heat(0.0, window);
        }
    }

    /// Colours for `color_by_speed`, RGBA each: cool and hot (8 floats), or cool, hot and a middle
    /// colour for a three-stop ramp (12 floats).
    pub fn set_speed_colors(&mut self, c: Vec<f32>) {
        self.renderer.params.cool_color.copy_from_slice(&c[0..4]);
        self.renderer.params.hot_color.copy_from_slice(&c[4..8]);
        self.renderer.params.mid_color = if c.len() >= 12 { [c[8], c[9], c[10], c[11]] } else { [0.0; 4] };
    }

    /// Overlay rectangles, 8 floats each: x0, y0, x1, y1 (σ), r, g, b, a.
    pub fn set_overlay(&mut self, rects: Vec<f32>) {
        assert_eq!(rects.len() % RECT_FLOATS, 0);
        self.renderer.set_rects(&rects);
    }
}
