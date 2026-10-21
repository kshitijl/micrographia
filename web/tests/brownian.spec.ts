import { expect, test } from "@playwright/test";

// Einstein's relation for a Brownian grain in 3D: its spreading D and the friction ζ measured by
// throwing it give D ζ = kT (here 1).
test("a Brownian grain's diffusion and friction give kT (Einstein's relation)", async ({ page }) => {
  test.setTimeout(600_000);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("pages/brownian/index.html");
  await page.evaluate(() => window.brownianTest!.ready);
  const r = await page.evaluate(() => window.brownianTest!.measure(60000, 12));
  console.log(`brownian: D ${r.d.toFixed(4)}, ζ ${r.zeta.toFixed(0)}, Dζ = ${r.kt.toFixed(2)} (kT = 1)`);
  // Measured 0.77 in Chrome (D 0.0155, ζ 50): the box's periodic images slow the grain's spreading.
  expect(Math.abs(r.kt - 1)).toBeLessThan(0.4);
  expect(errors).toEqual([]);
});
