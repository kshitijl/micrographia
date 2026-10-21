import { expect, test } from "@playwright/test";

// Gas solubility: the drink holds relatively less CO₂ when warmer; opened, it loses it.
test("warm water holds less gas; an open bottle loses it", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("pages/soda/index.html");
  await page.evaluate(() => window.sodaTest!.ready);
  const cold = await page.evaluate(() => window.sodaTest!.settle(0.4, 300_000));
  const warm = await page.evaluate(() => window.sodaTest!.settle(0.45, 300_000));
  const vent = await page.evaluate(() => window.sodaTest!.vent(200_000));
  console.log(`soda: crowding ratio cold ${cold.ratio.toFixed(2)} (dissolved ${cold.dissolved.toFixed(0)}), warm ${warm.ratio.toFixed(2)} (${warm.dissolved.toFixed(0)}); opened: ${vent.before} → ${vent.after}`);
  expect(cold.ratio).toBeGreaterThan(1.4 * warm.ratio);
  expect(vent.after).toBeLessThan(0.7 * vent.before);
});
