import { expect, test } from "@playwright/test";

// A relativistic gas: heated, its molecules stay below c while its energy follows Jüttner's
// formula; pushed, a hot gas's inertia is its energy over c².
test("a hot relativistic gas: speeds below c, energy by Jüttner, inertia E/c²", async ({ page }) => {
  await page.goto("pages/relativity/index.html");
  for (const doses of [0, 4, 16]) {
    const r = await page.evaluate((d) => window.relativityTest!.heat(d, 4000), doses);
    console.log(`relativity: ${doses} doses: kT ${r.kT.toFixed(3)}, E ${r.e.toFixed(3)} (Jüttner ${r.juttner.toFixed(3)}), speeds mean ${r.vMean.toFixed(3)}, max ${r.vMax.toFixed(4)}, drift ${r.drift.toExponential(1)}`);
    expect(Math.abs(r.e - r.juttner)).toBeLessThan(0.03 * r.e);
    expect(r.vMax).toBeLessThan(1);
    expect(Math.abs(r.drift)).toBeLessThan(1e-9);
  }
  // The molecules' mean flow jiggles: average the inertia over several starts.
  const p = await page.evaluate(() => {
    const k = 8;
    const sum = { cold: { m: 0, e: 0, h: 0 }, hot: { m: 0, e: 0, h: 0 } };
    for (let s = 0; s < k; s++) {
      const r = window.relativityTest!.push(120, s, 0.0015);
      for (const g of ["cold", "hot"] as const) for (const f of ["m", "e", "h"] as const) sum[g][f] += r[g][f] / k;
    }
    return sum;
  });
  console.log(`relativity: inertia per molecule: cold ${p.cold.m.toFixed(3)} (E ${p.cold.e.toFixed(3)}, E + pA ${p.cold.h.toFixed(3)}), hot ${p.hot.m.toFixed(3)} (E ${p.hot.e.toFixed(3)}, E + pA ${p.hot.h.toFixed(3)})`);
  // A gas on its own (no walls) has the inertia of its enthalpy.
  expect(Math.abs(p.cold.m / p.cold.h - 1)).toBeLessThan(0.05);
  expect(Math.abs(p.hot.m / p.hot.h - 1)).toBeLessThan(0.05);
  expect(p.hot.m / p.cold.m).toBeGreaterThan(2);
});
