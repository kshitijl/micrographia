import { expect, test } from "@playwright/test";

test("vapour pressure rises steeply with temperature (Clausius–Clapeyron)", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/boiling/index.html");
  await page.evaluate(() => window.boilingTest!.ready);
  const low = await page.evaluate(() => window.boilingTest!.measure(0.4, 20000, 100));
  const high = await page.evaluate(() => window.boilingTest!.measure(0.45, 20000, 100));
  const latent = await page.evaluate(() => window.boilingTest!.record());
  console.log(`boiling: p(0.40) ${low.p.toExponential(2)}, p(0.45) ${high.p.toExponential(2)}, liquid ${low.liquid.toFixed(3)} → ${high.liquid.toFixed(3)}, latent heat ${latent.toFixed(2)} ε`);
  expect(high.p / low.p).toBeGreaterThan(1.8);
  expect(latent).toBeGreaterThan(1.5);
  expect(latent).toBeLessThan(6);
  expect(low.liquid).toBeGreaterThan(high.liquid);
});
