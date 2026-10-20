import { expect, test } from "@playwright/test";

test("from an even start, an isolated column settles into each species' exponential, at one temperature", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/atmosphere/index.html");
  await page.evaluate(() => window.atmosphereTest!.ready);
  // Settle for 700 τ (native runs: ~500–800 τ from an even start), then average 150 snapshots.
  const r = await page.evaluate(() => window.atmosphereTest!.run(150, 280000));
  console.log(`atmosphere: slopes ${r.slopes.map((s) => s.toFixed(4))} vs ${r.predicted.map((s) => s.toFixed(4))}; T bottom ${r.tBottom.toFixed(3)}, top ${r.tTop.toFixed(3)}`);
  for (let s = 0; s < 2; s++) expect(Math.abs(r.slopes[s] / r.predicted[s] - 1)).toBeLessThan(0.25);
  expect(Math.abs(r.tTop / r.tBottom - 1)).toBeLessThan(0.1);
});
