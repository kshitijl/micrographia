import { loadWasm, selftest, webgpuProblem } from "../src/lib/wasm";

interface Result {
  name: string;
  ok: boolean;
  message: string;
  ms: number;
}

declare global {
  interface Window {
    selftestResults?: Promise<{ adapter: string; results: Result[] }>;
  }
}

async function run() {
  const problem = await webgpuProblem();
  if (problem) throw new Error(problem);
  await loadWasm();
  const out = (await selftest()) as { adapter: string; results: Result[] };
  document.getElementById("adapter")!.textContent = `Adapter: ${out.adapter}`;
  const rows = document.getElementById("rows")!;
  rows.replaceChildren(
    ...out.results.map((r) => {
      const tr = document.createElement("tr");
      for (const [text, color] of [
        [r.name, ""],
        [r.ok ? "pass" : "FAIL", r.ok ? "var(--good)" : "var(--bad)"],
        [r.message, ""],
        [r.ms.toFixed(0), ""],
      ]) {
        const td = document.createElement("td");
        td.textContent = text;
        if (color) td.style.color = color;
        tr.append(td);
      }
      return tr;
    }),
  );
  return out;
}

window.selftestResults = run();
window.selftestResults.catch((e) => {
  document.getElementById("rows")!.innerHTML = "";
  document.getElementById("adapter")!.textContent = String(e);
});
