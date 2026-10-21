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
