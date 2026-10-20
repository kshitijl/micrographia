import { expect, test } from "@playwright/test";

test("still air spreads the puff as √t; a draught carries it as t", async ({ page }) => {
  await page.goto("pages/smell/index.html");
  await page.evaluate(() => window.smellTest!.ready);
  const still = await page.evaluate(() => window.smellTest!.run(400, 0));
  const windy = await page.evaluate(() => window.smellTest!.run(400, 0.003));
  console.log(`smell: still air D ${still.d.toFixed(2)}, drift ${still.drift.toFixed(1)}, spread ${still.sd.toFixed(1)}; draught drift ${windy.drift.toFixed(1)} over ${windy.t.toFixed(0)} τ`);
  expect(still.d).toBeGreaterThan(0.3);
  // The puff's centre wanders by about spread/√(number of molecules) ≈ 2 σ: allow 4× that.
  expect(Math.abs(still.drift)).toBeLessThan((4 * still.sd) / Math.sqrt(90));
  // The draught starts from rest and spins up (viscous momentum diffusion across the corridor);
  // by 200 τ it has carried the puff well beyond its diffusive spread, and the gap keeps growing.
  expect(windy.drift).toBeGreaterThan(1.5 * Math.sqrt(2 * still.d * windy.t));
});
