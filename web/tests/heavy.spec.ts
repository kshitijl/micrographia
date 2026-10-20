import { expect, test } from "@playwright/test";

test("light, heavy and ball species share kinetic energy equally", async ({ page }) => {
  await page.goto("pages/heavy/index.html");
  await page.evaluate(() => window.heavyTest!.ready);
  const ke = await page.evaluate(() => window.heavyTest!.run(120));
  console.log(`mean kinetic energy: light ${ke[0].toFixed(3)}, heavy ${ke[1].toFixed(3)}, ball ${ke[2].toFixed(3)}`);
  expect(Math.abs(ke[1] / ke[0] - 1)).toBeLessThan(0.05);
  // Three balls over a short run: a loose check.
  expect(Math.abs(ke[2] / ke[0] - 1)).toBeLessThan(0.5);
});
