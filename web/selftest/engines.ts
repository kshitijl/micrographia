import { CpuDiscSim, DiscSim, loadWasm } from "../src/lib/wasm";

// The CPU engine against the GPU engine, in this browser: the same positions after the same
// steps, to the last bits where the GPU's float division rounds as the CPU's does (most pairs),
// tracking closely where it does not; and the speed of each by size.

interface Row { name: string; ok: boolean; message: string }
declare global {
  interface Window {
    engineResults?: Promise<{ rows: Row[]; speeds: { scenario: string; n: number; cpu: number; gpu: number }[] }>;
  }
}

const canvas = (id: string) => {
  // A fresh canvas each time: a canvas keeps whichever context it first gave out.
  const c = document.createElement("canvas");
  c.width = c.height = 64;
  c.id = id;
  document.getElementById(id)?.replaceWith(c);
  return c;
};

async function compare(name: string, opts: Record<string, number>, steps: number, exact: boolean): Promise<Row> {
  const gpu = await DiscSim.create(canvas("a"), name, opts);
  const cpu = CpuDiscSim.create(canvas("b"), name, opts);
  const n = gpu.n();
  const ids = Uint32Array.from({ length: n }, (_, i) => i);
  for (const s of [gpu, cpu]) s.step(steps);
  const [a, b] = [(await gpu.positions_of(ids)) as Float64Array, (await cpu.positions_of(ids)) as Float64Array];
  const box = Array.from(gpu.box_size());
  let [same, worst] = [0, 0];
  for (let i = 0; i < a.length; i++) {
    if (a[i] === b[i]) same++;
    let d = Math.abs(a[i] - b[i]);
    d = Math.min(d, box[i % 2] - d);
    worst = Math.max(worst, d);
  }
  gpu.free();
  cpu.free();
  // Tracking closely (exactly, where every rounding agreed).
  const ok = exact ? same === a.length : worst < 1e-3;
  return { name: `${name} (${n} particles, ${steps} steps)`, ok, message: `${same} of ${a.length} coordinates identical; worst difference ${worst.toExponential(1)} σ` };
}

async function speed(name: string, opts: Record<string, number>) {
  const gpu = await DiscSim.create(canvas("a"), name, opts);
  const cpu = CpuDiscSim.create(canvas("b"), name, opts);
  const n = gpu.n();
  const time = async (f: () => Promise<unknown>) => {
    await f();
    const t = performance.now();
    await f();
    return performance.now() - t;
  };
  const steps = Math.max(50, Math.round(2e6 / n));
  const g = await time(async () => {
    gpu.step(steps);
    await gpu.sync();
  });
  const c = await time(async () => cpu.step(Math.max(10, Math.round(steps / 4))));
  gpu.free();
  cpu.free();
  return { scenario: name, n, gpu: (1000 * steps) / g, cpu: (1000 * Math.max(10, Math.round(steps / 4))) / c };
}

window.engineResults = (async () => {
  await loadWasm();
  const rows: Row[] = [];
  const show = () => {
    document.getElementById("rows")!.innerHTML = rows.map((r) => `<tr><td>${r.name}</td><td>${r.ok ? "pass" : "FAIL"}</td><td>${r.message}</td></tr>`).join("");
  };
  for (const [name, opts, steps, exact] of [
    ["heatcap", { kind: 0, n: 400, seed: 3 }, 200, false],
    ["heatcap", { kind: 0, n: 3000, density: 0.4, seed: 5 }, 200, false],
    ["heatcap", { kind: 2, n: 400, seed: 3 }, 50, false],
    ["heatcap", { kind: 1, n: 400, seed: 3 }, 50, false],
  ] as const) {
    rows.push(await compare(name, opts, steps, exact));
    show();
  }
  const speeds = [];
  for (const n of [500, 2000, 8000, 32000]) {
    const s = await speed("heatcap", { kind: 0, n, density: 0.4, seed: 1 });
    speeds.push(s);
    rows.push({ name: `speed, ${n} WCA molecules`, ok: true, message: `CPU ${s.cpu.toFixed(0)} steps/s, GPU ${s.gpu.toFixed(0)} steps/s` });
    show();
  }
  return { rows, speeds };
})();
