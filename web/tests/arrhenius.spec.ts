import { expect, test } from "@playwright/test";

test("escape rates follow Arrhenius's law", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("pages/arrhenius/index.html");
  await page.evaluate(() => window.arrheniusTest!.ready);
  const r = await page.evaluate(() => window.arrheniusTest!.sweep(200));
  console.log(`arrhenius: Ea ${r.ea.toFixed(2)} ε, rates ${r.rates.map((k) => k.toFixed(4)).join(" ")}`);
  // Rates are listed hot to cold.
  for (let k = 1; k < r.rates.length; k++) expect(r.rates[k]).toBeLessThan(r.rates[k - 1]);
  // Native runs of the same sweep give 5.4 ε from the end points (crates/md/tests/binding.rs).
  expect(r.ea).toBeGreaterThan(4.3);
  expect(r.ea).toBeLessThan(6.5);
});
