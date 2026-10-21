import { expect, test } from "@playwright/test";

// Evaporative cooling: water under warm air ends up colder than the air when the air is dry, and
// at the air's temperature when it is saturated.
test("evaporation cools water below the air, unless the air is saturated", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("pages/sweat/index.html");
  await page.evaluate(() => window.sweatTest!.ready);
  const dry = await page.evaluate(() => window.sweatTest!.run(0, 120_000));
  const wet = await page.evaluate(() => window.sweatTest!.run(1, 120_000));
  console.log(`sweat: dry air: water ${dry.water.toFixed(3)}, air ${dry.air.toFixed(3)}; saturated: water ${wet.water.toFixed(3)}, air ${wet.air.toFixed(3)}`);
  expect(wet.water - dry.water).toBeGreaterThan(0.03);
  expect(dry.water).toBeLessThan(0.42);
});
