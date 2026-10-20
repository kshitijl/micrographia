import { expect, test } from "@playwright/test";

test("light gas leaks faster, by about the square root of the mass ratio", async ({ page }) => {
  await page.goto("pages/balloon/index.html");
  await page.evaluate(() => window.balloonTest!.ready);
  const r = await page.evaluate(() => window.balloonTest!.run(100000));
  console.log(`balloon: leak-rate ratio ${r.ratio.toFixed(2)} vs Graham ${r.graham.toFixed(2)}`);
  expect(r.ratio).toBeGreaterThan(0.7 * r.graham);
  expect(r.ratio).toBeLessThan(1.15 * r.graham);
});
