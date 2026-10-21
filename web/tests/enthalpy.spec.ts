import { expect, test } from "@playwright/test";

// The same heat warms a gas twice as much under a locked lid as under a free, weighted one
// (2D ideal gas: c_V = 1, c_p = 2 per molecule).
test("constant pressure takes twice the heat per degree of constant volume", async ({ page }) => {
  test.setTimeout(300_000);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("pages/enthalpy/index.html");
  await page.evaluate(() => window.enthalpyTest!.ready);
  const r = await page.evaluate(() => window.enthalpyTest!.heat(0.3, 200));
  console.log(`enthalpy: ΔT per unit heat per molecule: locked ${r.v.toFixed(3)}, free ${r.p.toFixed(3)}; the free lid rose ${r.dh.toFixed(1)} σ`);
  expect(Math.abs(r.v - 1)).toBeLessThan(0.1);
  expect(Math.abs(r.p - 0.5)).toBeLessThan(0.08);
  expect(r.dh).toBeGreaterThan(10);
  expect(errors).toEqual([]);
});
