import { expect, test } from "@playwright/test";

test("the chains' pull is proportional to temperature, and fast stretching warms them", async ({ page }) => {
  await page.goto("pages/rubber/index.html");
  await page.evaluate(() => window.rubberTest!.ready);
  const f1 = await page.evaluate(() => window.rubberTest!.pull(1.0, 250));
  const f2 = await page.evaluate(() => window.rubberTest!.pull(2.0, 250));
  console.log(`rubber: pull at T=1 ${f1.toFixed(3)}, at T=2 ${f2.toFixed(3)} (ratio ${(f2 / f1).toFixed(2)})`);
  expect(f1).toBeGreaterThan(0);
  // Tension averages converge slowly (natively, with 100 τ windows, the ratio is 2.0–2.1).
  expect(f2 / f1).toBeGreaterThan(1.5);
  expect(f2 / f1).toBeLessThan(2.8);
  await page.reload();
  await page.evaluate(() => window.rubberTest!.ready);
  const [t0, t1] = await page.evaluate(() => window.rubberTest!.stretch(3000));
  console.log(`rubber: fast stretch T ${t0.toFixed(3)} → ${t1.toFixed(3)}`);
  expect(t1).toBeGreaterThan(t0 * 1.03);
});
