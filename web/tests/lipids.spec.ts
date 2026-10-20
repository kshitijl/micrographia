import { expect, test } from "@playwright/test";

test("lipids aggregate from a random start", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/lipids/index.html");
  await page.evaluate(() => window.lipidsTest!.ready);
  const [agg, big] = await page.evaluate(() => window.lipidsTest!.run(100000));
  console.log(`lipids: ${(100 * agg).toFixed(0)}% in aggregates, largest ${big}`);
  await page.locator("#stage").screenshot({ path: "test-results/lipids.png" });
  expect(agg).toBeGreaterThan(0.85);
  expect(big).toBeGreaterThan(20);
});
