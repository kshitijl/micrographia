import { expect, test } from "@playwright/test";

// Quanta hopping at random: heat flows until the two blocks share a temperature, and the atoms'
// quanta settle into Boltzmann's distribution (slope of ln count against n = −ε/kT).
test("random hops of quanta give equal temperatures and Boltzmann's distribution", async ({ page }) => {
  await page.goto("pages/einstein-solid/index.html");
  const r = await page.evaluate(() => window.einsteinTest!.relax(4_000_000));
  console.log(`einstein solid: kT/ε ${r.t.map((x) => x.toFixed(3)).join(", ")}; slope ${r.slope.toFixed(3)} (−ε/kT = ${(-1 / r.t[0]).toFixed(3)})`);
  expect(Math.abs(r.t[0] - r.t[1])).toBeLessThan(0.1 * r.t[0]);
  expect(Math.abs(r.slope + 1 / r.t[0])).toBeLessThan(0.15 / r.t[0]);
});
