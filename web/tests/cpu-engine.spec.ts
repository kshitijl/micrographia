import { expect, test } from "@playwright/test";

// Pages that allow it run on the CPU engine (WebAssembly SIMD, a 2D canvas) where WebGPU is
// missing; `?engine=cpu` asks for it. The physics comes out the same.
test("the heat-capacity page on the CPU engine", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/heatcap/index.html?engine=cpu");
  await page.evaluate(() => window.heatcapTest!.ready);
  const about = await page.locator("section.about-sim").first().innerText();
  expect(about).toMatch(/processor/);
  const c = await page.evaluate(() => window.heatcapTest!.measure(3, 20000));
  console.log(`cpu engine: heat capacities ${c.map((x) => x.toFixed(3)).join(", ")}`);
  [1, 2.5, 2].forEach((want, k) => expect(Math.abs(c[k] / want - 1)).toBeLessThan(0.12));
});

test("the spectral-lines page on the CPU engine", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/lines/index.html?engine=cpu");
  await page.evaluate(() => window.linesTest!.ready);
  const r = await page.evaluate(() => window.linesTest!.measure(1, 20000));
  console.log(`cpu engine: line widths ${r.widths.map((w) => w.toFixed(2)).join(", ")} pm (Doppler ${r.doppler.map((w) => w.toFixed(2)).join(", ")})`);
  for (let k = 0; k < 2; k++) expect(Math.abs(r.widths[k] / r.doppler[k] - 1)).toBeLessThan(0.08);
});
