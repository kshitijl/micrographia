import { expect, test } from "@playwright/test";

// Hot, the lattice gas is about as uneven as an ideal gas at every scale (S ≈ 1: Rayleigh's blue);
// near the critical point the long-wavelength unevenness grows many times (opalescence).
test("fluctuations grow near the critical point and the scattered light whitens", async ({ page }) => {
  test.setTimeout(200_000);
  await page.goto("pages/opalescence/index.html");
  await page.evaluate(() => window.opalescenceTest!.ready);
  const hot = await page.evaluate(() => window.opalescenceTest!.measure(5, 4000));
  const near = await page.evaluate(() => window.opalescenceTest!.measure(0.6, 40000));
  console.log(`opalescence: kT 5: S blue ${hot.blue.toFixed(2)}, red ${hot.red.toFixed(2)}, brightness ×${hot.bright.toFixed(1)}; kT 0.6: S blue ${near.blue.toFixed(1)}, red ${near.red.toFixed(1)}, brightness ×${near.bright.toFixed(0)}`);
  expect(hot.blue).toBeLessThan(1.5);
  expect(near.red).toBeGreaterThan(5 * hot.red);
  expect(near.red / near.blue).toBeGreaterThan(1.2);
});
