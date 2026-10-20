import { expect, test } from "@playwright/test";

test("exact reversal restores the initial state; a one-bit nudge does not", async ({ page }) => {
  page.on("console", (m) => console.log(`[page] ${m.text()}`));
  await page.goto("pages/reversal/index.html");
  await page.evaluate(() => window.reversalTest!.ready);
  const exact = await page.evaluate(() => window.reversalTest!.roundTrip(2000, false));
  expect(exact.identical).toBe(true);
  const nudged = await page.evaluate(() => window.reversalTest!.roundTrip(6000, true));
  console.log(`nudged: ${nudged.differ} particles differ, max ${nudged.maxDeviation.toFixed(2)} σ`);
  expect(nudged.identical).toBe(false);
  expect(nudged.differ).toBeGreaterThan(4000);
});

test("the page's own buttons run forward, reverse, and report an exact return", async ({ page }) => {
  await page.goto("pages/reversal/index.html");
  await page.evaluate(() => window.reversalTest!.ready);
  await page.locator("#size").selectOption("2000");
  await expect(page.locator("#overlay")).toBeHidden();
  await page.locator("#speed").fill("64");
  await page.locator("#run").click();
  await expect.poll(async () => Number(await page.locator("#time").textContent()), { timeout: 30_000 }).toBeGreaterThan(3);
  await page.locator("#reverse").click();
  await expect(page.locator("#status")).toContainText("matches the start", { timeout: 60_000 });
  await page.screenshot({ path: "test-results/reversal.png" });
});
