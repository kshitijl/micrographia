import { expect, test } from "@playwright/test";

// More salt dissolves in hot water than in cold, in the same time (natively, after 400 τ: 17 of
// 256 ions at kT 0.7, 70 at 1.1).
test("salt dissolves more in hot water", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/dissolving/index.html");
  await page.evaluate(() => window.dissolvingTest!.ready);
  const cold = await page.evaluate(() => window.dissolvingTest!.run(0.7, 80000));
  const hot = await page.evaluate(() => window.dissolvingTest!.run(1.1, 80000));
  console.log(`dissolving after 400 τ: ${cold} ions at kT 0.7, ${hot} at 1.1`);
  expect(hot).toBeGreaterThan(1.8 * cold);
  expect(hot).toBeGreaterThan(30);
});
