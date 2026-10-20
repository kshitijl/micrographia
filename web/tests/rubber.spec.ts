import { expect, test } from "@playwright/test";

test("the chains' pull is proportional to temperature, and fast stretching warms them", async ({ page }) => {
  await page.goto("pages/rubber/index.html");
  await page.evaluate(() => window.rubberTest!.ready);
  const f1 = await page.evaluate(() => window.rubberTest!.pull(1.0, 250));
  const f2 = await page.evaluate(() => window.rubberTest!.pull(2.0, 250));
  console.log(`rubber: pull at T=1 ${f1.toFixed(3)}, at T=2 ${f2.toFixed(3)} (ratio ${(f2 / f1).toFixed(2)})`);
  expect(f1).toBeGreaterThan(0);
  // Measured with the spring gauge: the chains shorten as they pull harder (the handles come in),
  // so the ratio is below the pinned-end value of ~2 (natively ~1.55: crates/md/tests/rubber.rs).
  expect(f2 / f1).toBeGreaterThan(1.3);
  expect(f2 / f1).toBeLessThan(2.8);
  await page.reload();
  await page.evaluate(() => window.rubberTest!.ready);
  // Stretch from 50% to ~80% of the contour length at 0.15 σ/τ (insulated), then release.
  const [t0, t1, t2] = await page.evaluate(() => window.rubberTest!.stretchAndRelease(25000));
  console.log(`rubber: insulated stretch T ${t0.toFixed(3)} → ${t1.toFixed(3)}, release → ${t2.toFixed(3)}`);
  expect(t1).toBeGreaterThan(t0 * 1.03);
  expect(t2).toBeLessThan(t1 * 0.98);
});
