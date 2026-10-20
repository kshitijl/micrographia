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

test("an exact return erases the traced paths; a nudged one leaves them", async ({ page }) => {
  await page.goto("pages/reversal/index.html");
  await page.evaluate(() => window.reversalTest!.ready);
  await page.locator("#size").selectOption("2000");
  await expect(page.locator("#overlay")).toBeHidden();
  await page.locator("#speed").fill("64");
  // Exact: there and back, and every recorded point is erased.
  await page.locator("#run").click();
  await expect.poll(async () => Number(await page.locator("#time").textContent()), { timeout: 30_000 }).toBeGreaterThan(4);
  const out = await page.evaluate(() => window.reversalTest!.trails());
  await page.locator("#reverse").click();
  await expect(page.locator("#status")).toContainText("matches the start", { timeout: 60_000 });
  const exact = await page.evaluate(() => window.reversalTest!.trails());
  console.log(`reversal: ${out.going} samples out; after an exact return ${exact.going} left, ${exact.kept} kept`);
  expect(out.traced).toBe(4);
  expect(out.going).toBeGreaterThan(20);
  expect(exact.kept).toBe(0);
  expect(exact.going).toBeLessThanOrEqual(1);
  // Nudged: run again, nudge all the cream, return: the paths stay.
  await page.locator("#reset").click();
  await expect(page.locator("#overlay")).toBeHidden();
  await page.locator("#run").click();
  await expect.poll(async () => Number(await page.locator("#time").textContent()), { timeout: 30_000 }).toBeGreaterThan(8);
  await page.locator("#nudge-cream").click();
  await expect(page.locator("#status")).toContainText("Nudged all the cream");
  await page.locator("#reverse").click();
  await expect(page.locator("#status")).toContainText("somewhere else", { timeout: 60_000 });
  await expect(page.locator("#status")).toContainText("all the cream");
  const nudged = await page.evaluate(() => window.reversalTest!.trails());
  console.log(`reversal: after a nudged return ${nudged.kept} points kept`);
  expect(nudged.kept).toBeGreaterThan(20);
  await page.screenshot({ path: "test-results/reversal-nudged.png" });
});
