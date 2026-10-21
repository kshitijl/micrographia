import { expect, test } from "@playwright/test";

// 3D: a Lennard-Jones crystal stays crystalline when cold, and the heater melts it, with the
// temperature stalling while it does (measured: 96% crystal at kT 0.33; after 1.6 ε per atom,
// 35% crystal at kT 0.62).
test("a 3D crystal stays solid when cold and melts when heated", async ({ page }) => {
  test.setTimeout(240_000);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("pages/crystal/index.html");
  await page.evaluate(() => window.crystalTest!.ready);
  const cold = await page.evaluate(() => window.crystalTest!.run(2000, 0));
  const hot = await page.evaluate(() => window.crystalTest!.run(16000, 20));
  console.log(`crystal: cold kT ${cold.t.toFixed(3)}, ${Math.round(100 * cold.crystal)}% crystal; heated by ${hot.heat.toFixed(2)} ε per atom: kT ${hot.t.toFixed(3)}, ${Math.round(100 * hot.crystal)}% crystal`);
  expect(cold.crystal).toBeGreaterThan(0.85);
  expect(hot.heat).toBeGreaterThan(1.5);
  // Melting under way...
  expect(hot.crystal).toBeLessThan(0.6);
  // ...and the temperature stalled: without melting, 1.6 ε per atom would have warmed the crystal
  // (3k per atom, Dulong and Petit) by ~0.53, to ~0.86; the latent heat holds it near 0.6.
  expect(hot.t).toBeLessThan(0.7);
  expect(errors).toEqual([]);
});
