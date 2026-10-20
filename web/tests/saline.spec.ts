import { expect, test } from "@playwright/test";

test("cells burst in pure water, keep their size in saline and shrink in brine", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("pages/saline/index.html");
  await page.evaluate(() => window.salineTest!.ready);
  const water = await page.evaluate(() => window.salineTest!.run(0, 100000));
  const saline = await page.evaluate(() => window.salineTest!.run(0.35, 100000));
  const brine = await page.evaluate(() => window.salineTest!.run(0.9, 100000));
  console.log(`saline: water ${water.burst} burst; saline ${saline.burst} burst, area ${saline.area.toFixed(0)}; brine ${brine.burst} burst, area ${brine.area.toFixed(0)}`);
  expect(water.burst).toBeGreaterThanOrEqual(2);
  expect(saline.burst).toBe(0);
  expect(brine.burst).toBe(0);
  expect(saline.area).toBeGreaterThan(brine.area * 1.15);
});
