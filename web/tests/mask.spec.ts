import { expect, test } from "@playwright/test";

test("fibres catch particles much smaller than their gaps", async ({ page }) => {
  test.setTimeout(900_000);
  await page.goto("pages/mask/index.html");
  await page.evaluate(() => window.maskTest!.ready);
  const eff = await page.evaluate(() => window.maskTest!.sweep(150));
  console.log(`mask: caught per pass for sizes 0.3, 1.2, 2.4, 4.8 σ: ${eff.map((e) => e.toFixed(2)).join(" ")}`);
  // Native runs: 0.41, 0.36, 0.40, 0.76 (crates/md/tests/mask.rs). A sieve would catch none.
  for (const e of eff.slice(0, 3)) expect(e).toBeGreaterThan(0.2);
  expect(eff[3]).toBeGreaterThan(eff[1] + 0.2);
});
