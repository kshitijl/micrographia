import { expect, test } from "@playwright/test";

// Independent decays: counts per interval with variance ≈ mean (Poisson), and the half-life
// recovered from the falling rate.
test("radioactive counts are Poisson and the rate halves every half-life", async ({ page }) => {
  await page.goto("pages/counting/index.html");
  const steady = await page.evaluate(() => window.countingTest!.run(200, 600));
  const r = await page.evaluate(() => window.countingTest!.run(300, 60));
  console.log(`counting: mean ${steady.mean.toFixed(1)}, variance ${steady.variance.toFixed(1)} (Poisson: equal); half-life ${r.half.toFixed(1)} s (set 60)`);
  expect(steady.variance / steady.mean).toBeGreaterThan(0.75);
  expect(steady.variance / steady.mean).toBeLessThan(1.35);
  expect(Math.abs(r.half - 60)).toBeLessThan(8);
});
