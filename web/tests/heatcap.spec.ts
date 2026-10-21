import { expect, test } from "@playwright/test";

// Equipartition in 2D: heat per degree per molecule 1k for atoms, 2.5k for two-atom molecules on
// springs, 2k for a crystal; quantum levels freeze hydrogen's spin below ~100 K.
test("classical heat capacities by equipartition; quanta freeze spin and vibration", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/heatcap/index.html");
  await page.evaluate(() => window.heatcapTest!.ready);
  const c = await page.evaluate(() => window.heatcapTest!.measure(3, 20000));
  console.log(`heatcap: atoms ${c[0].toFixed(3)}, molecules ${c[1].toFixed(3)}, crystal ${c[2].toFixed(3)}`);
  [1, 2.5, 2].forEach((want, k) => expect(Math.abs(c[k] / want - 1)).toBeLessThan(0.12));
  const q = await page.evaluate(() => [20, 300, 10000].map((t) => window.heatcapTest!.quantum(t)));
  console.log(`heatcap: H₂, N₂, argon at 20 K ${q[0].map((x) => x.toFixed(2))}, 300 K ${q[1].map((x) => x.toFixed(2))}, 10,000 K ${q[2].map((x) => x.toFixed(2))}`);
  expect(q[0][0]).toBeLessThan(1.55); // H₂ not spinning at 20 K
  expect(Math.abs(q[1][0] - 2.5)).toBeLessThan(0.05); // spinning, not vibrating, at 300 K
  expect(Math.abs(q[1][1] - 2.5)).toBeLessThan(0.05);
  expect(q[2][2]).toBe(1.5); // argon never spins
});
