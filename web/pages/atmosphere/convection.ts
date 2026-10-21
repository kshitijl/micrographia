// A throwaway fluid-dynamics panel for the atmosphere page: a 2D slice of air 20 km wide and
// 10 km tall, heated from the ground by the sun. Not molecules: a continuum model, written in
// plain JavaScript on the CPU (see docs/fluid-engine.md for what a proper GPU one would be).
//
// The model (dry, 2D, Boussinesq for the motion, compressible for the thermodynamics):
//  - vorticity ω and stream function ψ (u = ∂ψ/∂z, w = −∂ψ/∂x, ∇²ψ = −ω), periodic in x,
//    free-slip ground and lid;
//  - potential temperature θ, the temperature a parcel would have if brought down to the
//    ground's pressure without exchanging heat. Moving air carries θ unchanged;
//  - buoyancy: Dω/Dt = (g/θ̄) ∂θ/∂x + ν∇²ω;
//  - pressure from hydrostatic balance with the average temperature at each height, and the
//    temperature itself from θ: T = θ (p/p₀)^(R/cp).
//  - optionally ("Make it humid"), water: vapour q and cloud water c (kg per kg of air), carried
//    by the wind. Moist ground in sunshine evaporates into the lowest air; where air is
//    supersaturated the excess condenses into cloud, releasing its latent heat into θ; cloud in
//    unsaturated air evaporates again, taking the heat back; buoyancy uses the virtual
//    potential temperature θ(1 + 0.61q − c) (vapour is lighter than air, cloud water heavier).
//    Cloud water beyond 1 g/kg rains out; the vapour relaxes towards its starting profile on the
//    radiation's time scale (large-scale drying by sinking air, which a 2D slice cannot hold).
// Advection is semi-Lagrangian; the Poisson equation is solved exactly (FFT across, tridiagonal
// up).
import { $, cssVar } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { side, warmth } from "../../src/lib/profiles";

const NX = 128;
const NZ = 64; // intervals; rows j = 0..NZ
const ROWS = NZ + 1;
const LX = 20_000; // m
const LZ = 10_000; // m
const D = LZ / NZ; // grid spacing, m
const G = 9.81;
const R = 287;
const CP = 1004;
const KAPPA = R / CP;
const P0 = 1013.25; // hPa at the ground
const T0 = 288; // K, the starting (isothermal) temperature
const NU = 20; // m²/s: eddy viscosity and diffusivity
const DT = 15; // s
const SUN = 2000; // W/m², several times a real sunny afternoon's, so the hours go faster
const NIGHT = -100; // W/m²: the ground radiating to a clear night sky
const RHO0 = 1.2; // kg/m³, near the ground
const RADIATE = 12 * 3600; // s: the air radiates towards the starting temperature on this time scale
const MEAN_WIND = 600; // s: the horizontal mean wind is damped on this time scale (see step)
const DRAG = 3 * 3600; // s: a slow drag on the wind (2D flows otherwise pile up into giant eddies)

const at = (i: number, j: number) => j * NX + ((i + NX) % NX);

let omega = new Float64Array(NX * ROWS);
let psi = new Float64Array(NX * ROWS);
let theta = new Float64Array(NX * ROWS);
const u = new Float64Array(NX * ROWS);
const w = new Float64Array(NX * ROWS);
let thetaStart = new Float64Array(ROWS); // the isothermal start, for comparison
let pStart = new Float64Array(ROWS);
let seconds = 0;
/** Still: no heat through the ground (the start). Day: sunshine warms it. Night: it cools. */
let sky: "still" | "day" | "night" = "still";
let running = false;
let tracers: Float64Array = new Float64Array(0); // x, z pairs
// Water: vapour and cloud (kg/kg), and whether it is on.
let q = new Float64Array(NX * ROWS);
let cloud = new Float64Array(NX * ROWS);
/** The vapour's starting profile (per row), which the air relaxes back towards. */
let qStart = new Float64Array(ROWS);
let humid = false;
const LV = 2.5e6; // J/kg, latent heat of vaporisation
const RV = 461.5; // J/(kg K), water vapour
/** The moist ground's relative humidity for the air touching it. */
const GROUND_RH = 0.8;
/** Saturation vapour pressure (hPa) at T (K), Bolton's formula. */
const esat = (t: number) => 6.112 * Math.exp((17.67 * (t - 273.15)) / (t - 29.65));
/** Saturation mixing ratio (kg/kg) at T (K) and p (hPa). */
const qsat = (t: number, p: number) => (0.622 * esat(t)) / (p - esat(t));

// Row averages, refreshed after each frame.
const thetaBar = new Float64Array(ROWS);
const pBar = new Float64Array(ROWS);
const tBar = new Float64Array(ROWS);
const rhoBar = new Float64Array(ROWS);

let seed = 1;
const rand = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);

function reset() {
  omega = new Float64Array(NX * ROWS);
  psi = new Float64Array(NX * ROWS);
  theta = new Float64Array(NX * ROWS);
  thetaStart = new Float64Array(ROWS);
  pStart = new Float64Array(ROWS);
  seed = 1;
  for (let j = 0; j < ROWS; j++) {
    const z = j * D;
    pStart[j] = P0 * Math.exp((-G * z) / (R * T0));
    thetaStart[j] = T0 * (P0 / pStart[j]) ** KAPPA;
    for (let i = 0; i < NX; i++) theta[at(i, j)] = thetaStart[j] + 1e-3 * (rand() - 0.5);
  }
  tracers = new Float64Array(2 * 900);
  for (let k = 0; k < 900; k++) {
    tracers[2 * k] = rand() * LX;
    tracers[2 * k + 1] = rand() * LZ;
  }
  seconds = 0;
  u.fill(0);
  w.fill(0);
  q = new Float64Array(NX * ROWS);
  cloud = new Float64Array(NX * ROWS);
  if (humid) moisten();
  averages();
}

/** Humid air: vapour at 70% of saturation near the ground, falling off with height (a scale
 * height of 2.5 km), nowhere saturated at the start. */
function moisten() {
  for (let j = 0; j < ROWS; j++) {
    const t = thetaStart[j] * (pStart[j] / P0) ** KAPPA;
    const qs = qsat(t, pStart[j]);
    qStart[j] = Math.min(0.7 * qsat(T0, P0) * Math.exp((-j * D) / 2500), 0.9 * qs);
    for (let i = 0; i < NX; i++) q[at(i, j)] = qStart[j];
  }
}

// ---- The Poisson solve: FFT along x, then a tridiagonal solve up each column of modes. ----
const re = new Float64Array(NX * ROWS);
const im = new Float64Array(NX * ROWS);
const cosT = Float64Array.from({ length: NX / 2 }, (_, k) => Math.cos((2 * Math.PI * k) / NX));
const sinT = Float64Array.from({ length: NX / 2 }, (_, k) => Math.sin((2 * Math.PI * k) / NX));
const rev = Uint32Array.from({ length: NX }, (_, i) => {
  let r = 0;
  for (let b = 1, x = i; b < NX; b <<= 1, x >>= 1) r = (r << 1) | (x & 1);
  return r;
});

/** In-place radix-2 FFT of row `j` of (re, im); `sign` −1 forward, +1 inverse (unscaled). */
function fft(j: number, sign: number) {
  const o = j * NX;
  for (let i = 0; i < NX; i++) {
    const r = rev[i];
    if (r > i) {
      [re[o + i], re[o + r]] = [re[o + r], re[o + i]];
      [im[o + i], im[o + r]] = [im[o + r], im[o + i]];
    }
  }
  for (let size = 2; size <= NX; size <<= 1) {
    const half = size >> 1;
    const step = NX / size;
    for (let s = 0; s < NX; s += size) {
      for (let k = 0; k < half; k++) {
        const [c, sn] = [cosT[k * step], sign * sinT[k * step]];
        const [a, b] = [o + s + k, o + s + k + half];
        const tr = re[b] * c - im[b] * sn;
        const ti = re[b] * sn + im[b] * c;
        re[b] = re[a] - tr;
        im[b] = im[a] - ti;
        re[a] += tr;
        im[a] += ti;
      }
    }
  }
}

const cp = new Float64Array(ROWS);
const dr = new Float64Array(ROWS);
const di = new Float64Array(ROWS);
function solvePsi() {
  for (let j = 0; j < ROWS; j++) {
    for (let i = 0; i < NX; i++) {
      re[j * NX + i] = j === 0 || j === NZ ? 0 : -D * D * omega[j * NX + i];
      im[j * NX + i] = 0;
    }
    fft(j, -1);
  }
  // For each mode k: ψ[j−1] − (2 + μ) ψ[j] + ψ[j+1] = rhs[j], ψ = 0 at the ground and the lid.
  for (let k = 0; k < NX; k++) {
    const b = -(2 + 2 - 2 * Math.cos((2 * Math.PI * k) / NX));
    cp[1] = 1 / b;
    dr[1] = re[NX + k] / b;
    di[1] = im[NX + k] / b;
    for (let j = 2; j < NZ; j++) {
      const m = b - cp[j - 1];
      cp[j] = 1 / m;
      dr[j] = (re[j * NX + k] - dr[j - 1]) / m;
      di[j] = (im[j * NX + k] - di[j - 1]) / m;
    }
    re[(NZ - 1) * NX + k] = dr[NZ - 1];
    im[(NZ - 1) * NX + k] = di[NZ - 1];
    for (let j = NZ - 2; j >= 1; j--) {
      re[j * NX + k] = dr[j] - cp[j] * re[(j + 1) * NX + k];
      im[j * NX + k] = di[j] - cp[j] * im[(j + 1) * NX + k];
    }
    re[k] = im[k] = re[NZ * NX + k] = im[NZ * NX + k] = 0;
  }
  for (let j = 0; j < ROWS; j++) {
    fft(j, 1);
    for (let i = 0; i < NX; i++) psi[j * NX + i] = re[j * NX + i] / NX;
  }
}

function velocities() {
  for (let j = 0; j < ROWS; j++) {
    for (let i = 0; i < NX; i++) {
      const k = at(i, j);
      u[k] =
        j === 0 ? (psi[at(i, 1)] - psi[k]) / D : j === NZ ? (psi[k] - psi[at(i, NZ - 1)]) / D : (psi[at(i, j + 1)] - psi[at(i, j - 1)]) / (2 * D);
      w[k] = j === 0 || j === NZ ? 0 : -(psi[at(i + 1, j)] - psi[at(i - 1, j)]) / (2 * D);
    }
  }
}

/** Bilinear sample of `f` at (x, z) in metres, periodic in x, clamped in z. */
function sample(f: Float64Array, x: number, z: number) {
  const gx = x / D;
  const gz = Math.max(0, Math.min(NZ - 1e-9, z / D));
  const [i0, j0] = [Math.floor(gx), Math.floor(gz)];
  const [fx, fz] = [gx - i0, gz - j0];
  const a = f[at(i0, j0)] * (1 - fx) + f[at(i0 + 1, j0)] * fx;
  const b = f[at(i0, j0 + 1)] * (1 - fx) + f[at(i0 + 1, j0 + 1)] * fx;
  return a * (1 - fz) + b * fz;
}

/** Semi-Lagrangian advection: each grid point takes the value from where its air came from. */
function advect(f: Float64Array) {
  const out = new Float64Array(f.length);
  for (let j = 0; j < ROWS; j++) {
    for (let i = 0; i < NX; i++) {
      const k = at(i, j);
      // Midpoint back-trace.
      const [xm, zm] = [i * D - 0.5 * DT * u[k], j * D - 0.5 * DT * w[k]];
      out[k] = sample(f, i * D - DT * sample(u, xm, zm), j * D - DT * sample(w, xm, zm));
    }
  }
  return out;
}

function diffuse(f: Float64Array) {
  const out = f.slice();
  const c = (NU * DT) / (D * D);
  for (let j = 1; j < NZ; j++) {
    for (let i = 0; i < NX; i++) {
      const k = at(i, j);
      out[k] += c * (f[at(i + 1, j)] + f[at(i - 1, j)] + f[at(i, j + 1)] + f[at(i, j - 1)] - 4 * f[k]);
    }
  }
  return out;
}

function step() {
  velocities();
  omega = diffuse(advect(omega));
  // Diffuse only θ's departure from the still, isothermal start, so that still air stays still
  // (mixing θ itself would stir it towards the adiabat with no wind at all).
  theta = advect(theta);
  for (let k = 0; k < theta.length; k++) theta[k] -= thetaStart[Math.floor(k / NX)];
  theta = diffuse(theta);
  // Radiation: the air relaxes towards the starting temperature (no effect on the still start).
  for (let k = 0; k < theta.length; k++) theta[k] -= (DT / RADIATE) * theta[k];
  for (let k = 0; k < omega.length; k++) omega[k] -= (DT / DRAG) * omega[k];
  // The ground: sunshine warms the lowest air (a little unevenly, which seeds the plumes); at
  // night it cools instead. The ground and the lid otherwise pass no heat.
  const flux = sky === "day" ? SUN : sky === "night" && thetaBar[1] > 250 ? NIGHT : 0;
  for (let i = 0; i < NX; i++) {
    const uneven = sky === "day" ? 1 + 0.6 * (rand() - 0.5) : 1;
    theta[at(i, 1)] += (flux * uneven * DT) / (RHO0 * CP * D);
  }
  for (let i = 0; i < NX; i++) {
    theta[at(i, 0)] = theta[at(i, 1)];
    theta[at(i, NZ)] = theta[at(i, NZ - 1)];
  }
  for (let k = 0; k < theta.length; k++) theta[k] += thetaStart[Math.floor(k / NX)];
  if (humid) water(flux > 0);
  // Buoyancy turns sideways differences in θ (θ_v, with water) into spin.
  const tv = humid ? theta.map((th, k) => th * (1 + 0.61 * q[k] - cloud[k])) : theta;
  for (let j = 1; j < NZ; j++) {
    let mean = 0;
    for (let i = 0; i < NX; i++) mean += tv[at(i, j)];
    mean /= NX;
    for (let i = 0; i < NX; i++) omega[at(i, j)] += ((DT * G) / mean) * ((tv[at(i + 1, j)] - tv[at(i - 1, j)]) / (2 * D));
  }
  for (let i = 0; i < NX; i++) omega[at(i, 0)] = omega[at(i, NZ)] = 0;
  // No mean wind: 2D convection between slippery plates feeds energy into a horizontal mean wind
  // (a sheared "zonal" flow, a known 2D artefact that tilts the plumes and blows across the top);
  // 3D turbulence does not build one. Damp the average spin at each height over ~10 minutes.
  for (let j = 1; j < NZ; j++) {
    let mean = 0;
    for (let i = 0; i < NX; i++) mean += omega[at(i, j)];
    mean /= NX;
    for (let i = 0; i < NX; i++) omega[at(i, j)] -= (DT / MEAN_WIND) * mean;
  }
  // A sponge under the lid: the real atmosphere has no lid, and waves set off by the plumes
  // travel up and away; here they would bounce back and fill the sky. Damp the wind and the
  // sideways differences in θ over the top 4 km (the average at each height is untouched).
  for (let j = Math.ceil(0.6 * NZ); j < NZ; j++) {
    const keep = 1 - (DT / 300) * ((j / NZ - 0.6) / 0.4) ** 2;
    let mean = 0;
    for (let i = 0; i < NX; i++) mean += theta[at(i, j)];
    mean /= NX;
    for (let i = 0; i < NX; i++) {
      omega[at(i, j)] *= keep;
      theta[at(i, j)] = mean + (theta[at(i, j)] - mean) * keep;
    }
  }
  solvePsi();
  for (let k = 0; k < tracers.length; k += 2) {
    const [x, z] = [tracers[k], tracers[k + 1]];
    tracers[k] = (((x + DT * sample(u, x, z)) % LX) + LX) % LX;
    tracers[k + 1] = Math.max(1, Math.min(LZ - 1, z + DT * sample(w, x, z)));
  }
  seconds += DT;
}

/** Water: carry vapour and cloud with the wind, evaporate from moist ground in sunshine, and
 * condense or evaporate cloud so no air is supersaturated, moving the latent heat in or out of θ. */
function water(sunny: boolean) {
  q = diffuse(advect(q));
  cloud = diffuse(advect(cloud));
  for (let j = 0; j < ROWS; j++) {
    const p = pBar[j];
    for (let i = 0; i < NX; i++) {
      const k = at(i, j);
      if (j <= 1 && sunny) {
        // The ground evaporates towards GROUND_RH of saturation at the air's temperature.
        const t = theta[k] * (p / P0) ** KAPPA;
        const target = GROUND_RH * qsat(t, p);
        if (q[k] < target) q[k] += 0.01 * (target - q[k]);
      }
      // Large-scale drying (subsiding air between the plumes, which this 2D slice cannot hold),
      // on the same time scale as the radiation; and rain: cloud water beyond 1 g/kg falls out
      // within ~20 minutes.
      q[k] -= (DT / RADIATE) * (q[k] - qStart[j]);
      if (cloud[k] > 1e-3) cloud[k] -= (DT / 1200) * (cloud[k] - 1e-3);
      // Saturation adjustment, linearised: condensing dq warms the air, which raises qsat.
      const exner = (p / P0) ** KAPPA;
      const t = theta[k] * exner;
      const qs = qsat(t, p);
      let dq = (q[k] - qs) / (1 + (LV * LV * qs) / (CP * RV * t * t));
      if (dq < 0) dq = Math.max(dq, -cloud[k]); // evaporate only cloud that is there
      if (dq !== 0) {
        q[k] -= dq;
        cloud[k] += dq;
        theta[k] += (LV / CP) * (dq / exner);
      }
    }
  }
  for (let i = 0; i < NX; i++) {
    for (const f of [q, cloud]) {
      f[at(i, NZ)] = f[at(i, NZ - 1)];
    }
    cloud[at(i, 0)] = 0;
  }
}

/** Cloud bases: in each column with cloud, the height of its lowest cloudy cell (cloud water
 * above 0.05 g/kg); and the lifting condensation level predicted for the air at the ground:
 * about 125 m per kelvin of difference between its temperature and its dew point. */
function cloudBases() {
  const bases: number[] = [];
  for (let i = 0; i < NX; i++)
    for (let j = 1; j < NZ; j++)
      if (cloud[at(i, j)] > 5e-5) {
        bases.push(j * D);
        break;
      }
  let qg = 0;
  for (let i = 0; i < NX; i++) qg += q[at(i, 1)] / NX;
  const e = (qg * pBar[1]) / (0.622 + qg);
  const lnr = Math.log(e / 6.112);
  const dew = 273.15 + (243.5 * lnr) / (17.67 - lnr);
  return { bases, lcl: 125 * (tBar[1] - dew) };
}

/** Row averages of θ, and from them the hydrostatic pressure, the temperature and the density. */
function averages() {
  for (let j = 0; j < ROWS; j++) {
    let s = 0;
    for (let i = 0; i < NX; i++) s += theta[at(i, j)];
    thetaBar[j] = s / NX;
  }
  pBar[0] = P0;
  for (let j = 0; j < ROWS; j++) {
    if (j > 0) pBar[j] = pBar[j - 1] * Math.exp((-G * D) / (R * tBar[j - 1]));
    tBar[j] = thetaBar[j] * (pBar[j] / P0) ** KAPPA;
    rhoBar[j] = (pBar[j] * 100) / (R * tBar[j]);
  }
}

/** The depth of the stirred layer: the highest level whose average θ has moved from the start. */
function stirredDepth() {
  for (let j = NZ - 1; j > 0; j--) if (Math.abs(thetaBar[j] - thetaStart[j]) > 0.3) return j * D;
  return 0;
}

/** Temperature lapse rate (K/km, positive when colder upward) over the middle of the stirred layer. */
function lapse(depth: number) {
  if (depth < 800) return NaN;
  const [j0, j1] = [Math.round((0.2 * depth) / D), Math.round((0.7 * depth) / D)];
  return ((tBar[j0] - tBar[j1]) / ((j1 - j0) * D)) * 1000;
}

// ---- Drawing. ----
const canvas = $<HTMLCanvasElement>("fluid");
const field = document.createElement("canvas");
field.width = NX;
field.height = ROWS;
const fctx = field.getContext("2d")!;
const img = fctx.createImageData(NX, ROWS);

function draw() {
  averages();
  const dpr = window.devicePixelRatio || 1;
  const [W, H] = [canvas.clientWidth, canvas.clientHeight];
  if (canvas.width !== Math.round(W * dpr)) {
    canvas.width = Math.round(W * dpr);
    canvas.height = Math.round(H * dpr);
  }
  // Colour: warmer (red) or cooler (blue) than the average at that height, on the stage colour.
  const bg = cssColor("--stage-bg").map((c) => c * 255);
  const [hot, cold] = [[215, 65, 45], [60, 120, 210]];
  for (let j = 0; j < ROWS; j++) {
    for (let i = 0; i < NX; i++) {
      const a = Math.max(-1, Math.min(1, (theta[at(i, j)] - thetaBar[j]) / 2));
      const c = a > 0 ? hot : cold;
      const o = 4 * ((NZ - j) * NX + i);
      for (let q = 0; q < 3; q++) img.data[o + q] = bg[q] + (c[q] - bg[q]) * Math.abs(a);
      img.data[o + 3] = 255;
    }
  }
  fctx.putImageData(img, 0, 0);
  const ctx = canvas.getContext("2d")!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.imageSmoothingEnabled = true;
  ctx.drawImage(field, 0, 0, W, H);
  ctx.fillStyle = cssVar("--cream");
  ctx.globalAlpha = 0.75;
  for (let k = 0; k < tracers.length; k += 2) ctx.fillRect((tracers[k] / LX) * W - 1, H - (tracers[k + 1] / LZ) * H - 1, 2, 2);
  ctx.globalAlpha = 1;
  if (humid) {
    // Clouds: white, thicker where there is more cloud water.
    const cimg = fctx.createImageData(NX, ROWS);
    for (let j = 0; j < ROWS; j++)
      for (let i = 0; i < NX; i++) {
        const o = 4 * ((NZ - j) * NX + i);
        cimg.data[o] = cimg.data[o + 1] = cimg.data[o + 2] = 245;
        cimg.data[o + 3] = Math.min(255, (cloud[at(i, j)] / 4e-4) * 255);
      }
    fctx.putImageData(cimg, 0, 0);
    ctx.drawImage(field, 0, 0, W, H);
    const { bases, lcl } = cloudBases();
    if (bases.length) {
      const mean = bases.reduce((a, b) => a + b, 0) / bases.length;
      $("fbase").textContent = `${(mean / 1000).toFixed(2)} km (predicted ${(lcl / 1000).toFixed(2)} km)`;
    } else $("fbase").textContent = `none yet (predicted ${(lcl / 1000).toFixed(2)} km)`;
    ctx.strokeStyle = "rgba(255,255,255,0.6)";
    ctx.setLineDash([4, 4]);
    ctx.beginPath();
    ctx.moveTo(0, H - (lcl / LZ) * H);
    ctx.lineTo(W, H - (lcl / LZ) * H);
    ctx.stroke();
    ctx.setLineDash([]);
  }
  if (sky === "day") {
    ctx.fillStyle = "rgba(255, 210, 90, 0.9)";
    ctx.fillRect(0, H - 3, W, 3);
  }

  // Side profiles, two rows per bar.
  const bins = NZ / 2;
  const bin = (f: Float64Array) => Array.from({ length: bins }, (_, b) => (f[2 * b] + f[2 * b + 1] + f[2 * b + 2]) / 3);
  const tStart = Array.from({ length: bins }, () => T0);
  const pS = bin(pStart);
  const rhoS = pS.map((p) => (p * 100) / (R * T0));
  const t = bin(tBar);
  side("fluidRho", { values: [bin(rhoBar)], colors: [cssVar("--cream")], dashed: rhoS, hi: 1.4, label: (v) => v.toFixed(1) });
  side("fluidP", { values: [bin(pBar)], colors: [cssVar("--plot-back")], dashed: pS, hi: 1100, label: (v) => v.toFixed(0) });
  const lo = Math.floor(Math.min(...t, T0) / 10) * 10 - 10;
  const hi = Math.ceil(Math.max(...t, T0) / 10) * 10 + 5;
  // The dry adiabat from the air at the ground: the temperature air from the ground would have
  // if lifted without exchanging heat, cooling as it expands into lower pressure.
  const adiabat = bin(pBar).map((p) => tBar[0] * (p / P0) ** KAPPA);
  side("fluidT", { values: [t], colors: [(b) => warmth(1 + (t[b] - T0) / 10, 1)], dashed: tStart, guide: sky === "still" ? undefined : adiabat, lo, hi, label: (v) => `${(v - 273).toFixed(0)}°` });

  const depth = stirredDepth();
  const l = lapse(depth);
  $("fclock").textContent = `${(seconds / 3600).toFixed(1)} h`;
  $("fground").textContent = `${(tBar[0] - 273.15).toFixed(1)} °C`;
  $("fdepth").textContent = depth > 0 ? `${(depth / 1000).toFixed(1)} km` : "–";
  $("flapse").textContent = Number.isFinite(l) ? `${l.toFixed(1)} K/km` : "–";
}

function frame() {
  if (running) {
    for (let k = 0; k < 4; k++) step(); // an hour of air every second or so
    draw();
  }
  requestAnimationFrame(frame);
}

// ---- Controls: sunshine, a warm bubble, and stirring by dragging. ----
const sunBtn = $("sun");
function setSky(s: typeof sky) {
  sky = s;
  sunBtn.textContent = s === "day" ? "Sunset" : "Sunshine";
}
function setRunning(on: boolean) {
  running = on;
  $("frun").textContent = on ? "Pause" : "Run";
}
// Sunshine, a warm bubble or a stir set the air going if it was paused.
sunBtn.onclick = () => {
  setSky(sky === "day" ? "night" : "day");
  setRunning(true);
};
$("frun").onclick = () => setRunning(!running);
$("freset").onclick = () => {
  reset();
  setSky("still");
  setRunning(false);
  draw();
};
$("humid").onclick = () => {
  humid = !humid;
  $("humid").textContent = humid ? "Make it dry" : "Make it humid";
  if (humid) moisten();
  else {
    q.fill(0);
    cloud.fill(0);
    $("fbase").textContent = "–";
  }
  draw();
};
$("bubble").onclick = () => {
  // A blob of air 3 K warmer than its surroundings, near the ground in the middle.
  for (let j = 1; j < NZ; j++)
    for (let i = 0; i < NX; i++) {
      const r2 = ((i - NX / 2) ** 2 + (j - 6) ** 2) / 16;
      theta[at(i, j)] += 3 * Math.exp(-r2);
    }
  setRunning(true);
  draw();
};
let last: { x: number; z: number } | null = null;
canvas.addEventListener("pointerdown", (e) => {
  canvas.setPointerCapture(e.pointerId);
  last = toGrid(e);
  setRunning(true);
});
canvas.addEventListener("pointerup", () => (last = null));
canvas.addEventListener("pointermove", (e) => {
  if (!last) return;
  const now = toGrid(e);
  // Push the air along the drag: the curl of a blob of velocity (dx, dz) adds spin around it.
  const [vx, vz] = [((now.x - last.x) * D) / 60, ((now.z - last.z) * D) / 60];
  for (let j = 1; j < NZ; j++)
    for (let i = 0; i < NX; i++) {
      let dx = i - now.x;
      dx -= NX * Math.round(dx / NX);
      const dz = j - now.z;
      const s2 = 9;
      const g = Math.exp(-(dx * dx + dz * dz) / s2);
      // ω = ∂w/∂x − ∂u/∂z for (u, w) = (vx, vz) g.
      omega[at(i, j)] += (vz * (-2 * dx * g) / s2 - vx * (-2 * dz * g) / s2) / D;
    }
  last = now;
  solvePsi();
});
function toGrid(e: PointerEvent) {
  const r = canvas.getBoundingClientRect();
  return { x: ((e.clientX - r.left) / r.width) * NX, z: (1 - (e.clientY - r.top) / r.height) * NZ };
}

reset();
requestAnimationFrame(() => draw());
requestAnimationFrame(frame);

declare global {
  interface Window {
    convectionTest?: {
      run(hours: number, s: "still" | "day" | "night"): { depth: number; lapse: number; ground: number; maxSpeed: number };
      /** Humid air in sunshine for `hours`: the cloud bases (each column's lowest cloud, m) and
       * the predicted lifting condensation level. */
      clouds(hours: number): { bases: number[]; lcl: number };
    };
  }
}

window.convectionTest = {
  run(hours: number, s: "still" | "day" | "night") {
    setRunning(false);
    reset();
    setSky(s);
    while (seconds < hours * 3600) step();
    draw();
    const depth = stirredDepth();
    let maxSpeed = 0;
    for (let k = 0; k < u.length; k++) maxSpeed = Math.max(maxSpeed, Math.hypot(u[k], w[k]));
    return { depth, lapse: lapse(depth), ground: tBar[0], maxSpeed };
  },
  clouds(hours: number) {
    setRunning(false);
    humid = true;
    $("humid").textContent = "Make it dry";
    reset();
    setSky("day");
    while (seconds < hours * 3600) step();
    draw();
    return cloudBases();
  },
};
