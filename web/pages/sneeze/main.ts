import { DiscSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { entropy } from "../../src/lib/interact";
import { LinesPlot } from "../../src/lib/plot";

// Two gases, cold and hot, each conserving its energy; a click adds an exactly counted burst of
// heat Q to the molecules in a small square, and the page measures the entropy it adds (the part
// from the molecules' energies, N × Boltzmann's H per molecule) against Q/T.

const N = 6000;
/** Half the side of the sneeze's square (σ). */
const SPOT = 5;
const qIn = $<HTMLInputElement>("q");
const se = new LinesPlot($<HTMLCanvasElement>("se"), { xLabel: "energy added since the start (ε)", yLabel: "entropy gained (k)", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const st = new LinesPlot($<HTMLCanvasElement>("st"), { xLabel: "time (τ)", yLabel: "entropy gained (k)", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

interface Box {
  t0: number;
  /** Entropy (smoothed over recent readouts) and energy now; their values at the start. */
  s: number;
  e: number;
  start: { s: number; e: number } | null;
  /** The last sneeze: the entropy just before it, Q and T then. */
  sneeze: { s: number; q: number; t: number } | null;
  t: number;
  seLine: { x: number; y: number }[];
  stLine: { x: number; y: number }[];
}
const boxes: Record<"C" | "H", Box> = {
  C: { t0: 0.5, s: NaN, e: NaN, start: null, sneeze: null, t: NaN, seLine: [], stLine: [] },
  H: { t0: 2.0, s: NaN, e: NaN, start: null, sneeze: null, t: NaN, seLine: [], stLine: [] },
};

async function readout(key: "C" | "H", sim: DiscSim) {
  const b = boxes[key];
  const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
  const pe = (await sim.potential_energy()) as number;
  const k = ke.reduce((a, x) => a + x, 0);
  // Entropy of the energies, per molecule, scaled to the box's temperature for the histogram
  // bins (a fixed relative resolution), times N.
  const s = N * entropy(ke, 0.05 * b.t0);
  b.s = Number.isFinite(b.s) ? 0.8 * b.s + 0.2 * s : s;
  b.e = k + pe;
  b.t = k / N;
  b.start ??= { s: b.s, e: b.e };
  const time = sim.steps() * sim.dt();
  b.seLine.push({ x: b.e - b.start.e, y: b.s - b.start.s });
  b.stLine.push({ x: time, y: b.s - b.start.s });
  for (const l of [b.seLine, b.stLine]) if (l.length > 3000) l.splice(0, 1);
  show();
}

function show() {
  const [c, h] = [boxes.C, boxes.H];
  se.set([
    { points: c.seLine, color: cssVar("--plot-back") },
    { points: h.seLine, color: cssVar("--highlight") },
  ]);
  st.set([
    { points: c.stLine, color: cssVar("--plot-back") },
    { points: h.stLine, color: cssVar("--highlight") },
  ]);
  $("t").textContent = `${c.t.toFixed(3)}, ${h.t.toFixed(3)}`;
  if (c.start && h.start) $("s").textContent = `+${(c.s - c.start.s).toFixed(0)}, +${(h.s - h.start.s).toFixed(0)}`;
  for (const [key, b] of [["C", c], ["H", h]] as const) {
    if (b.sneeze) $(`ds${key}`).textContent = `${(b.s - b.sneeze.s).toFixed(0)} k; Q/T = ${(b.sneeze.q / b.sneeze.t).toFixed(0)} k`;
  }
}

function makeHost(key: "C" | "H") {
  const b = boxes[key];
  const host = new SimHost<DiscSim>({
    canvas: $<HTMLCanvasElement>(`canvas${key}`),
    overlay: $(`overlay${key}`),
    scenario: "boltzmann",
    options: () => ({ n: N, density: 0.05, aspect: 1, temperature: b.t0, seed: key === "C" ? 3 : 4 }),
    names: ["molecules", "one marked molecule"],
    label: key === "C" ? "the library" : "the rock concert",
    colors: ["--species-2", "--species-2"],
    stepsPerFrame: () => 20,
    afterSteps: (sim) => readout(key, sim),
    onCreate: async (sim) => {
      Object.assign(b, { s: NaN, e: NaN, start: null, sneeze: null, seLine: [], stLine: [] });
      sim.set_speed_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--highlight"), ...cssColor("--cream")]));
      sim.color_by_speed(2.5 * Math.sqrt(b.t0));
      sim.smooth_speed_colors(1);
      await readout(key, sim);
    },
  });
  // Click to sneeze: add Q, exactly, to the molecules in a square around the click.
  host.canvas.addEventListener("pointerdown", (e) => {
    const sim = host.sim;
    if (!sim) return;
    const r = host.canvas.getBoundingClientRect();
    const [x0, y0, x1, y1] = Array.from(sim.view());
    const x = x0 + ((e.clientX - r.left) / r.width) * (x1 - x0);
    const y = y1 - ((e.clientY - r.top) / r.height) * (y1 - y0);
    void host.exclusive((s) => sneeze(key, s, x, y, Number(qIn.value)));
  });
  return host;
}

/** Add `q` to the molecules within `half` σ of (x, y) (or to all of them, if `half` is
 * Infinity), exactly, by scaling their velocities. */
async function sneeze(key: "C" | "H", sim: DiscSim, x: number, y: number, q: number, half = SPOT) {
  const b = boxes[key];
  const { pos } = (await sim.all_positions()) as { pos: Float32Array };
  const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
  let k = 0;
  for (let i = 0; i < ke.length; i++) if (Math.abs(pos[2 * i] - x) < half && Math.abs(pos[2 * i + 1] - y) < half) k += ke[i];
  if (k <= 0) return;
  const s = Math.sqrt(1 + q / k);
  b.sneeze = { s: b.s, q: k * (s * s - 1), t: b.t };
  const h = Math.min(half, 1e6);
  sim.kick_region(x - h, y - h, x + h, y + h, s, 0, 0);
  sim.render();
}

const hostC = makeHost("C");
const hostH = makeHost("H");
const both = [hostC, hostH];

$("run").onclick = () => {
  const run = !hostC.running;
  both.forEach((h) => (h.running = run));
  $("run").textContent = run ? "Pause both" : "Run both";
};
$("reset").onclick = () => {
  for (const h of both) {
    const run = h.running;
    void h.create().then(() => (h.running = run));
  }
};
for (const [key, host] of [["C", hostC], ["H", hostH]] as const) {
  $(`all${key}`).onclick = () => {
    if (!host.sim) return;
    void host.exclusive((s) => sneeze(key, s, 0, 0, Number(qIn.value), Infinity));
  };
}
const showQ = () => ($("qv").textContent = `Q = ${qIn.value} ε`);
qIn.oninput = showQ;
showQ();

declare global {
  interface Window {
    sneezeTest?: { ready: Promise<void>; sneeze(key: "C" | "H", q: number, steps: number): Promise<{ ds: number; qt: number }> };
  }
}

window.sneezeTest = {
  ready: Promise.all(both.map((h) => h.ready)).then(() => {}),
  /** Settle, sneeze `q` into the whole of one box at once, run `steps`, and compare the entropy
   * gained (averaged over the end) with Q/T. */
  async sneeze(key: "C" | "H", q: number, steps: number) {
    const host = key === "C" ? hostC : hostH;
    const b = boxes[key];
    return host.exclusive(async (sim) => {
      const avg = async (k: number) => {
        // Start the display's running average afresh, so it does not lag into the measurement.
        b.s = NaN;
        let s = 0;
        for (let j = 0; j < k; j++) {
          sim.step(200);
          await readout(key, sim);
          s += b.s / k;
        }
        return s;
      };
      const before = await avg(30);
      const [w, h] = Array.from(sim.box_size());
      await sneeze(key, sim, w / 2, h / 2, q, Infinity);
      const t = b.sneeze!.t;
      const qq = b.sneeze!.q;
      for (let j = 0; j < steps / 1000; j++) sim.step(1000);
      const after = await avg(30);
      return { ds: after - before, qt: qq / t };
    });
  },
};
