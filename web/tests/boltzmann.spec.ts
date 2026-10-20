import { expect, test } from "@playwright/test";

test("from equal energies: Boltzmann within a few τ, entropy rises to its ceiling, energy holds", async ({ page }) => {
  await page.goto("pages/boltzmann/index.html");
  await page.evaluate(() => window.boltzmannTest!.ready);
  const r = await page.evaluate(() => window.boltzmannTest!.run(0, 40, 100));
  const last = r.s.length - 1;
  console.log(`boltzmann (equal start): S ${r.s[0].toFixed(2)} → ${r.s[last].toFixed(3)} (max ${r.smax[last].toFixed(3)}); histogram worst ${(100 * r.worst).toFixed(1)}% of peak; total ${r.total[0].toFixed(4)} → ${r.total[last].toFixed(4)}`);
  expect(r.s[0]).toBeLessThan(-1);
  expect(r.s[last]).toBeGreaterThan(r.smax[last] - 0.06);
  // 3072 molecules in 30 bins: a few percent of noise per bin.
  expect(r.worst).toBeLessThan(0.1);
  expect(Math.abs(r.total[last] / r.total[0] - 1)).toBeLessThan(0.005);
});

test("one hot box shares its energy with the others", async ({ page }) => {
  await page.goto("pages/boltzmann/index.html");
  await page.evaluate(() => window.boltzmannTest!.ready);
  const r = await page.evaluate(() => window.boltzmannTest!.run(3, 50, 2000));
  const last = r.s.length - 1;
  console.log(`boltzmann (one hot box): its kT ${r.tbox[0].toFixed(1)} → ${r.tbox[last].toFixed(2)}; S ${r.s[0].toFixed(2)} → ${r.s[last].toFixed(3)}; total ${r.total[0].toFixed(4)} → ${r.total[last].toFixed(4)}`);
  expect(r.tbox[0]).toBeGreaterThan(15);
  expect(r.tbox[last]).toBeLessThan(0.3 * r.tbox[0]);
  expect(r.s[last]).toBeGreaterThan(r.s[0] + 2);
  expect(Math.abs(r.total[last] / r.total[0] - 1)).toBeLessThan(0.01);
});
