import { expect, test } from "@playwright/test";

// The lattice gas (Kawasaki Monte Carlo on the GPU) against Onsager's exact coexistence curve,
// in the browser's shader compiler.
test("the lattice gas's liquid and vapour densities match Onsager's", async ({ page }) => {
  test.setTimeout(120_000);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("pages/coexistence/index.html");
  await page.evaluate(() => window.coexistenceTest!.ready);
  for (const t of [0.45, 0.5]) {
    const m = await page.evaluate((t) => window.coexistenceTest!.measure(t, 8000, 258), t);
    console.log(`lattice gas, kT ${t}: liquid ${m.liquid.toFixed(4)} (exact ${m.exact![1].toFixed(4)}), vapour ${m.vapour.toFixed(4)} (exact ${m.exact![0].toFixed(4)})`);
    expect(Math.abs(m.liquid - m.exact![1])).toBeLessThan(0.008);
    expect(Math.abs(m.vapour - m.exact![0])).toBeLessThan(0.008);
  }
  expect(errors).toEqual([]);
});

// Lattice Boltzmann: a pool on a hot plate boils. Seen in Chrome: a vapour bubble grows on the
// plate, rises and bursts at the surface, then the next.
test("a lattice Boltzmann pool on a hot plate boils", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto("pages/coexistence/index.html");
  await page.evaluate(() => window.coexistenceTest!.ready);
  await page.evaluate(() => window.coexistenceTest!.boil(1.1, 0.00002, 4000));
  const c = await page.evaluate(() => window.coexistenceTest!.fluidColumn([1, 5, 50, 150]));
  console.log(`boiling: T/T_c and density up the middle: ${c.t.map((t, k) => `${t.toFixed(2)}/${c.rho[k].toFixed(3)}`).join("  ")}`);
  // The plate has heated the bottom past the critical temperature, and vapour has formed there,
  // under the pool (whose liquid is ~0.3).
  expect(c.t[0]).toBeGreaterThan(1.0);
  expect(c.rho[1]).toBeLessThan(0.1);
  expect(c.rho[2]).toBeGreaterThan(0.25);
});
