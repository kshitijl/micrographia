import { expect, test } from "@playwright/test";

test("heat sometimes flows backwards, at the rate the fluctuation theorem predicts", async ({ page }) => {
  await page.goto("pages/heatflow/index.html");
  await page.evaluate(() => window.heatflowTest!.ready);
  const r = await page.evaluate(() => window.heatflowTest!.run(150));
  console.log(`heatflow: ${r.n} windows, ${(100 * r.negative).toFixed(1)}% backwards, slope ${r.slope} vs predicted ${r.predicted.toFixed(3)}`);
  expect(r.negative).toBeGreaterThan(0.05);
  expect(r.negative).toBeLessThan(0.5);
  expect(Math.abs(r.slope / r.predicted - 1)).toBeLessThan(0.35);
});

test("molecules leave each wall with speeds drawn at its temperature", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/heatflow/index.html");
  await page.evaluate(() => window.wallsTest!.ready);
  const d = await page.evaluate(() => window.wallsTest!.run(8000));
  const meanSq = (vs: number[]) => vs.reduce((a, v) => a + v * v, 0) / vs.length;
  // For the flux-weighted 2D Maxwell distribution ⟨v²⟩ = 3 kT (m = 1).
  console.log(`walls: ${d.hot.length} hot departures, ⟨v²⟩/3 = ${(meanSq(d.hot) / 3).toFixed(2)} (2.5); ${d.cold.length} cold, ${(meanSq(d.cold) / 3).toFixed(2)} (0.4)`);
  expect(d.hot.length).toBeGreaterThan(100);
  expect(d.cold.length).toBeGreaterThan(100);
  expect(Math.abs(meanSq(d.hot) / 3 / 2.5 - 1)).toBeLessThan(0.25);
  expect(Math.abs(meanSq(d.cold) / 3 / 0.4 - 1)).toBeLessThan(0.25);
});
