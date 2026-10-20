import { expect, test } from "@playwright/test";

test("all-in-one-half events occur at about 2^-N", async ({ page }) => {
  await page.goto("pages/corner/index.html");
  await page.evaluate(() => window.cornerTest!.ready);
  const r = await page.evaluate(() => window.cornerTest!.run(40));
  const ratio = r.events / r.samples / r.predicted;
  console.log(`corner: ${r.events} events in ${r.samples} box-snapshots; measured/predicted = ${ratio.toFixed(2)}`);
  expect(ratio).toBeGreaterThan(0.7);
  expect(ratio).toBeLessThan(1.3);
  await page.screenshot({ path: "test-results/corner.png", fullPage: false });
});
