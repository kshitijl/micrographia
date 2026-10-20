import { expect, test } from "@playwright/test";

test("each species falls off with its own scale height, at one temperature", async ({ page }) => {
  await page.goto("pages/atmosphere/index.html");
  await page.evaluate(() => window.atmosphereTest!.ready);
  const r = await page.evaluate(() => window.atmosphereTest!.run(150));
  console.log(`atmosphere: slopes ${r.slopes.map((s) => s.toFixed(4))} vs ${r.predicted.map((s) => s.toFixed(4))}; T bottom ${r.tBottom.toFixed(3)}, top ${r.tTop.toFixed(3)}`);
  for (let s = 0; s < 2; s++) expect(Math.abs(r.slopes[s] / r.predicted[s] - 1)).toBeLessThan(0.15);
  expect(Math.abs(r.tTop / r.tBottom - 1)).toBeLessThan(0.08);
});
