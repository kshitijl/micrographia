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
