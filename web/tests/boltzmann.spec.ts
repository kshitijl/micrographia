import { expect, test } from "@playwright/test";

for (const [k, view] of [
  [1, "energy"],
  [100, "energy"],
  [1, "speed"],
] as const) {
  test(`energy histogram matches theory (group ${k}, ${view})`, async ({ page }) => {
    await page.goto("pages/boltzmann/index.html");
    await page.evaluate(() => window.boltzmannTest!.ready);
    const r = await page.evaluate(([k, v]) => window.boltzmannTest!.collect(30, k as number, v as string), [k, view]);
    // Compare where the density is appreciable; allow for discrete bins and finite sampling.
    const peak = Math.max(...r.theory);
    let worst = 0;
    r.theory.forEach((t, i) => {
      if (t > 0.1 * peak) worst = Math.max(worst, Math.abs(r.measured[i] - t) / peak);
    });
    console.log(`group ${k} ${view}: worst deviation ${(100 * worst).toFixed(1)}% of the peak`);
    expect(worst).toBeLessThan(0.08);
  });
}
