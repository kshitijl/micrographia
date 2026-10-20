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

test("the fluid panel: still air stays still; sunshine stirs a layer that cools with height at about the dry adiabat; night makes an inversion", async ({ page }) => {
  await page.goto("pages/atmosphere/index.html");
  const still = await page.evaluate(() => window.convectionTest!.run(2, "still"));
  const day = await page.evaluate(() => window.convectionTest!.run(4, "day"));
  const night = await page.evaluate(() => window.convectionTest!.run(2, "night"));
  console.log(`convection: still ${JSON.stringify(still)}; day ${JSON.stringify(day)}; night ${JSON.stringify(night)}`);
  expect(still.maxSpeed).toBeLessThan(0.01);
  expect(still.depth).toBe(0);
  // Measured: 2.3 km deep, 9.2 K/km after 4 h (the dry adiabat is g/cp = 9.8 K/km).
  expect(day.depth).toBeGreaterThan(1500);
  expect(day.depth).toBeLessThan(3500);
  expect(day.lapse).toBeGreaterThan(8);
  expect(day.lapse).toBeLessThan(11);
  expect(night.ground).toBeLessThan(287);
  expect(night.maxSpeed).toBeLessThan(0.01);
});
