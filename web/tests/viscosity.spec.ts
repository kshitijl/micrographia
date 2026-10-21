import { expect, test } from "@playwright/test";

// Maxwell: a gas's viscosity does not depend on its density.
test("a thin and a dense gas have the same viscosity", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/viscosity/index.html");
  await page.evaluate(() => window.viscosityTest!.ready);
  const r = await page.evaluate(() => window.viscosityTest!.decay(60000));
  console.log(`viscosity: thin η ${r.etaA.toFixed(3)} (rate ${r.rateA.toFixed(4)}), dense η ${r.etaB.toFixed(3)} (rate ${r.rateB.toFixed(4)})`);
  expect(r.etaA).toBeGreaterThan(0.1);
  expect(Math.abs(r.etaA / r.etaB - 1)).toBeLessThan(0.3);
});
