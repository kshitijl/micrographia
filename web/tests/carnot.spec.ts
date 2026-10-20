import { expect, test } from "@playwright/test";

test("the engine stays below the Carnot limit, and gets closer when slower", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/carnot/index.html");
  await page.evaluate(() => window.carnotTest!.ready);
  const slow = await page.evaluate(() => window.carnotTest!.cycles(4, 0.1));
  const fast = await page.evaluate(() => window.carnotTest!.cycles(4, 0.5));
  console.log(`carnot: limit ${slow.limit.toFixed(3)}; slow: total ${slow.total.toFixed(3)} (cycles ${slow.eff.map((e) => e.toFixed(2))}); fast: total ${fast.total.toFixed(3)} (cycles ${fast.eff.map((e) => e.toFixed(2))})`);
  expect(slow.total).toBeLessThan(slow.limit * 1.05);
  expect(slow.total).toBeGreaterThan(slow.limit * 0.7);
  expect(fast.total).toBeLessThan(slow.total);
});
