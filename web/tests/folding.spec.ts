import { expect, test } from "@playwright/test";

// A lattice protein folds when cool, melts when hot, swells in acid or alkali, and salt brings
// it back.
test("a lattice protein folds, melts, and unfolds in acid until salt screens it", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/folding/index.html");
  const run = (kT: number, pH: number, salt: number) => page.evaluate(([a, b, c]) => window.foldingTest!.run(a, b, c, 20000), [kT, pH, salt]);
  const cool = await run(0.3, 7, 0.2);
  const hot = await run(0.7, 7, 0.2);
  const acid = await run(0.3, 1, 0.2);
  const salty = await run(0.3, 1, 1);
  console.log(`folding: cool ${cool.contacts.toFixed(1)} contacts (Rg ${cool.rg.toFixed(2)}), hot ${hot.contacts.toFixed(1)} (${hot.rg.toFixed(2)}), acid ${acid.contacts.toFixed(1)} (${acid.rg.toFixed(2)}), acid and salt ${salty.contacts.toFixed(1)} (${salty.rg.toFixed(2)})`);
  expect(cool.contacts).toBeGreaterThan(hot.contacts + 2);
  expect(acid.rg).toBeGreaterThan(cool.rg + 0.3);
  expect(salty.rg).toBeLessThan(acid.rg - 0.15);
});
