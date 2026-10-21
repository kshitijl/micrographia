//! 3D simulations for pages: one `GpuEngine3` running a named 3D scenario, drawn as spheres.
//! The methods a page's host uses (`step`, `sync`, `render`, `kinetic_energies`, ...) match
//! `DiscSim`'s, so the same `SimHost` runs either.

use crate::render3::{Renderer3, NO_HIGHLIGHT};
use crate::sims::options;
use crate::{browser_gpu, err, obj};
use gpu_prims::Prims;
use js_sys::{Float32Array, Float64Array, Uint32Array, Uint8Array};
use md::GpuEngine3;
use md_core::{fixed, Pair, Wall};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

/// A 3D particle simulation on the GPU, drawn into a canvas.
#[wasm_bindgen]
pub struct BallSim {
    engine: GpuEngine3,
    renderer: Renderer3,
}

#[wasm_bindgen]
impl BallSim {
    /// Build 3D scenario `name` (see `scenarios::three`) with numeric `options`, and attach a
    /// renderer to `canvas`.
    pub async fn create(canvas: web_sys::HtmlCanvasElement, name: String, opts: JsValue) -> Result<BallSim, JsError> {
        let (w, h) = (canvas.width(), canvas.height());
        let (gpu, surface) = browser_gpu(Some(canvas)).await?;
        let prims = Prims::new(&gpu);
        let sc = scenarios::three::build3(&name, &options(&opts)).map_err(err)?;
        let engine = GpuEngine3::new(&prims, sc.sys.clone(), &sc.state);
        let renderer = Renderer3::new(&gpu, surface.expect("surface"), &engine, &sc.radii, w, h);
        Ok(BallSim { engine, renderer })
    }

    /// Spatial dimensions (3; `DiscSim` has 2).
    pub fn dims(&self) -> u32 {
        3
    }

    pub fn step(&mut self, k: u32) {
        self.engine.step(k as usize);
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

    pub fn masses(&self) -> Vec<f64> {
        self.engine.sys.masses.clone()
    }

    /// Resolves to `{ke, species}`: each particle's kinetic energy (ε, half-step velocity) and
    /// species, in id order.
    pub fn kinetic_energies(&self) -> js_sys::Promise {
        let fut = self.engine.read_kinetic();
        future_to_promise(async move {
            let (ke, sp) = fut.await;
            Ok(obj(&[("ke", Float32Array::from(&ke[..]).into()), ("species", Uint32Array::from(&sp[..]).into())]))
        })
    }

    /// Resolves to the total potential energy (ε) at the last force evaluation, including m g h
    /// under gravity.
    pub fn potential_energy(&self) -> js_sys::Promise {
        let pe = self.engine.read_potential();
        let g = self.engine.sys.gravity;
        let gravity = (g != [0.0; 3]).then(|| (self.engine.read_state(), self.engine.sys.masses.clone()));
        future_to_promise(async move {
            let mut u = pe.await;
            if let Some((state, masses)) = gravity {
                let s = state.await;
                for i in 0..s.n() {
                    let m = masses[s.species[i] as usize];
                    u -= m * (0..3).map(|a| g[a] * fixed::to_f64(s.pos[i][a])).sum::<f64>();
                }
            }
            Ok(u.into())
        })
    }

    /// Resolves to `{pos, species}`: (x, y, z) in σ of every particle, and species, in id order.
    pub fn all_positions(&self) -> js_sys::Promise {
        let fut = self.engine.read_state();
        future_to_promise(async move {
            let s = fut.await;
            let pos: Vec<f32> = s.pos.iter().flat_map(|p| p.map(|x| fixed::to_f64(x) as f32)).collect();
            Ok(obj(&[("pos", Float32Array::from(&pos[..]).into()), ("species", Uint32Array::from(&s.species[..]).into())]))
        })
    }

    /// Resolves to a Float32Array of every particle's (vx, vy, vz) in σ/τ (the half-step
    /// velocity), in id order.
    pub fn velocities(&self) -> js_sys::Promise {
        let fut = self.engine.read_state();
        let dt = self.engine.sys.dt;
        future_to_promise(async move {
            let s = fut.await;
            let v: Vec<f32> = s.vel.iter().flat_map(|q| q.map(|x| (x as f64 * fixed::UNIT / dt) as f32)).collect();
            Ok(Float32Array::from(&v[..]).into())
        })
    }

    /// Resolves to the positions (σ, x y z interleaved, f64) of the particles with these ids.
    pub fn positions_of(&self, ids: Vec<u32>) -> js_sys::Promise {
        let fut = self.engine.read_state();
        future_to_promise(async move {
            let s = fut.await;
            let out: Vec<f64> = ids.iter().flat_map(|&i| s.pos[i as usize].map(fixed::to_f64)).collect();
            Ok(Float64Array::from(&out[..]).into())
        })
    }

    /// Resolves to a Uint8Array, by id: how many of each molecule's neighbours (within `r_cut` σ)
    /// are arranged around it as around itself (crystal bonds, from Steinhardt's q6; see
    /// `md_core::order`). 7 or more: the molecule is part of a crystal.
    pub fn crystal_bonds(&self, r_cut: f64) -> js_sys::Promise {
        let fut = self.engine.read_state();
        let (box_len, periodic) = (self.engine.sys.box_len, [0, 1, 2].map(|a| self.engine.sys.periodic(a)));
        future_to_promise(async move {
            let s = fut.await;
            let b = md_core::order::solid_bonds(&s.pos, box_len, periodic, r_cut);
            Ok(Uint8Array::from(&b[..]).into())
        })
    }

    /// Resolves to `{energy, impulse, hits}` (7 each: walls x-low, x-high, y-low, y-high, z-low,
    /// z-high, then the bulk heat bath).
    pub fn ledger(&self) -> js_sys::Promise {
        let fut = self.engine.read_ledger();
        future_to_promise(async move {
            let l = fut.await;
            let hits: Vec<f64> = l.hits.iter().map(|&h| h as f64).collect();
            Ok(obj(&[("energy", Float64Array::from(&l.energy[..]).into()), ("impulse", Float64Array::from(&l.impulse[..]).into()), ("hits", Float64Array::from(&hits[..]).into())]))
        })
    }

    /// Kick, freeze or shove the unpinned particles in the box (x0, y0, z0)–(x1, y1, z1) (σ):
    /// velocities are multiplied by `scale`, then (vx, vy, vz) σ/τ is added.
    #[allow(clippy::too_many_arguments)]
    pub fn kick_region(&mut self, x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64, scale: f64, vx: f64, vy: f64, vz: f64) {
        self.engine.kick_region([x0.min(x1), y0.min(y1), z0.min(z1)], [x0.max(x1), y0.max(y1), z0.max(z1)], scale, [vx, vy, vz]);
    }

    /// Multiply every velocity by `scale`: heat (> 1) or cool (< 1) the whole system, adding
    /// exactly K (scale² − 1).
    pub fn scale_velocities(&mut self, scale: f64) {
        let b = self.engine.sys.box_f64();
        self.engine.kick_region([-1.0; 3], [b[0] + 1.0, b[1] + 1.0, b[2] + 1.0], scale, [0.0; 3]);
    }

    /// Give every particle of `species` the velocity (vx, vy, vz) σ/τ.
    pub fn set_species_velocity(&mut self, species: u32, vx: f64, vy: f64, vz: f64) {
        self.engine.set_species_velocity(species, [vx, vy, vz]);
    }

    pub fn set_species(&mut self, ids: Vec<u32>, species: u32) {
        self.engine.set_species(&ids, species);
    }

    /// Change the LJ well depth between species `a` and `b` (cutoff and size unchanged).
    pub fn set_attraction(&mut self, a: u32, b: u32, eps: f64) {
        let p = *self.engine.sys.pair(a, b);
        let sys = self.engine.sys.clone().with_pair(a, b, Pair { eps, ..p });
        self.engine.update_system(sys);
    }

    /// Retune the bulk heat bath (the scenario must have one).
    pub fn set_thermostat(&mut self, t: f64, rate: f64) {
        let sys = self.engine.sys.clone().with_thermostat(t, rate);
        self.engine.update_system(sys);
    }

    /// Make one wall (`which` = 2·axis + side) a heat bath at `t` (> 0) or a hard wall (0).
    pub fn set_wall(&mut self, which: u32, t: f64) {
        let mut sys = self.engine.sys.clone();
        let (axis, side) = ((which / 2) as usize, (which % 2) as usize);
        if let Some(ws) = sys.walls.get_mut(axis).and_then(|w| w.as_mut()) {
            ws[side] = if t > 0.0 { Wall::Thermal(t) } else { Wall::Hard };
            self.engine.update_system(sys);
        }
    }

    pub fn set_gravity(&mut self, gx: f64, gy: f64, gz: f64) {
        let sys = self.engine.sys.clone().with_gravity([gx, gy, gz]);
        self.engine.update_system(sys);
    }

    pub fn describe(&self, names: Vec<String>) -> Vec<String> {
        self.engine.sys.describe(self.engine.n() as usize, &names)
    }

    pub fn energy_note(&self) -> String {
        self.engine.sys.energy_note()
    }

    pub fn render(&mut self) {
        self.renderer.render();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    /// 24 floats: four species colours, the highlight colour, the background (RGBA, sRGB). The
    /// box's edges are drawn in a faint mix of the highlight and background.
    pub fn set_colors(&mut self, c: Vec<f32>) {
        let p = &mut self.renderer.params;
        for s in 0..4 {
            p.colors[s].copy_from_slice(&c[4 * s..4 * s + 4]);
        }
        p.highlight_color.copy_from_slice(&c[16..20]);
        p.background.copy_from_slice(&c[20..24]);
        p.edge_color = [0.55 * c[20] + 0.45, 0.55 * c[21] + 0.45, 0.55 * c[22] + 0.45, 1.0];
    }

    /// Draw the particle with id `i` enlarged in the highlight colour (negative for none).
    pub fn set_highlight(&mut self, i: i32) {
        self.renderer.params.highlight = if i < 0 { NO_HIGHLIGHT } else { i as u32 };
    }

    /// Where the camera looks from: `yaw` around the vertical (radians), `pitch` above the
    /// horizontal, and `distance` in box diagonals (about 1.3 shows the whole box).
    pub fn set_camera(&mut self, yaw: f32, pitch: f32, distance: f32) {
        self.renderer.camera = crate::render3::Camera { yaw, pitch: pitch.clamp(-1.5, 1.5), distance: distance.max(0.6) };
    }

    /// `[yaw, pitch, distance]`.
    pub fn camera(&self) -> Vec<f32> {
        let c = self.renderer.camera;
        vec![c.yaw, c.pitch, c.distance]
    }

    /// Cut away the front `fraction` of the box (as the camera sees it), to look inside.
    pub fn set_cut(&mut self, fraction: f32) {
        self.renderer.cut = fraction.clamp(0.0, 1.0);
    }

    /// Colour by species (the default).
    pub fn color_by_species(&mut self) {
        self.renderer.params.color_mode = 0;
    }

    /// Colour by speed, cool to hot, saturating at `v_max` σ/τ.
    pub fn color_by_speed(&mut self, v_max: f64) {
        self.renderer.params.color_mode = 1;
        self.renderer.params.speed_scale = (1.0 / (v_max * self.engine.sys.dt * md_core::ONE as f64)) as f32;
    }

    /// Colour by values the page computes, one per particle by id, on the cool–hot ramp from 0
    /// to 1; a negative value hides the particle.
    pub fn color_by_values(&mut self, values: Vec<f32>) {
        assert_eq!(values.len(), self.engine.n() as usize);
        self.renderer.params.color_mode = 2;
        self.renderer.gpu_write_values(&values);
    }

    /// Colours for the speed or value ramp, RGBA each: cool and hot (8 floats), or cool, hot and
    /// a middle colour (12 floats).
    pub fn set_ramp_colors(&mut self, c: Vec<f32>) {
        let p = &mut self.renderer.params;
        p.cool_color.copy_from_slice(&c[0..4]);
        p.hot_color.copy_from_slice(&c[4..8]);
        p.mid_color = if c.len() >= 12 { [c[8], c[9], c[10], c[11]] } else { [0.0; 4] };
    }
}
