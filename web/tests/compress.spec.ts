import { expect, test } from "@playwright/test";

test("compression heats the gas along the adiabat", async ({ page }) => {
  await page.goto("pages/compress/index.html");
  await page.evaluate(() => window.compressTest!.ready);
  const r = await page.evaluate(() => window.compressTest!.run(-0.2, 40000));
  const ratio = (r.t1 * r.a1) / (r.t0 * r.a0);
  console.log(`compress: T ${r.t0.toFixed(3)} → ${r.t1.toFixed(3)}, A ${r.a0.toFixed(0)} → ${r.a1.toFixed(0)}, T·A ratio ${ratio.toFixed(3)}`);
  expect(r.t1).toBeGreaterThan(1.3 * r.t0);
  expect(ratio).toBeGreaterThan(0.9);
  expect(ratio).toBeLessThan(1.25);
});

test("a supersonic slam heats the gas beyond the adiabat (shocks make entropy)", async ({ page }) => {
  await page.goto("pages/compress/index.html");
  await page.evaluate(() => window.compressTest!.ready);
  // 4 σ/τ for 2000 steps: 40 σ down, about a quarter of the way.
  const r = await page.evaluate(() => window.compressTest!.run(-4, 2000));
  const adiabat = (r.t0 * r.a0) / r.a1;
  console.log(`compress (slam): T ${r.t0.toFixed(3)} → ${r.t1.toFixed(3)}, adiabat ${adiabat.toFixed(3)}, area ${r.a0.toFixed(0)} → ${r.a1.toFixed(0)}`);
  expect(r.t1).toBeGreaterThan(1.15 * adiabat);
});
