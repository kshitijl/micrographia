import { expect, test } from "@playwright/test";

test("weak cross-attraction separates at low temperature; equal attraction stays mixed", async ({ page }) => {
  await page.goto("pages/oilwater/index.html");
  await page.evaluate(() => window.oilTest!.ready);
  const sep = await page.evaluate(() => window.oilTest!.run(40000, 0.2, 0.7));
  await page.locator("#stage").screenshot({ path: "test-results/oilwater-separated.png" });
  await page.reload();
  await page.evaluate(() => window.oilTest!.ready);
  const mixed = await page.evaluate(() => window.oilTest!.run(20000, 1.0, 0.7));
  console.log(`oil/water mixing after 200 τ: cross 0.2 → ${sep.toFixed(2)}, cross 1.0 → ${mixed.toFixed(2)}`);
  expect(sep).toBeLessThan(0.7);
  expect(mixed).toBeGreaterThan(0.85);
});
