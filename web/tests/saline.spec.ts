import { expect, test } from "@playwright/test";

test("the cell swells in pure water and shrinks in brine, relative to saline", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/saline/index.html");
  await page.evaluate(() => window.salineTest!.ready);
  const water = await page.evaluate(() => window.salineTest!.area(0, 60000));
  const saline = await page.evaluate(() => window.salineTest!.area(0.35, 60000));
  const brine = await page.evaluate(() => window.salineTest!.area(0.9, 60000));
  console.log(`saline: cell area in water ${water.toFixed(0)}, saline ${saline.toFixed(0)}, brine ${brine.toFixed(0)}`);
  expect(water).toBeGreaterThan(saline);
  expect(saline).toBeGreaterThan(brine * 1.1);
});
