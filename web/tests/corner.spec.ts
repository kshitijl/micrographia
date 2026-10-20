import { expect, test } from "@playwright/test";

test("all-in-one-half events occur at about 2^-N for tiny molecules", async ({ page }) => {
  await page.goto("pages/corner/index.html");
  await page.evaluate(() => window.cornerTest!.ready);
  const r = await page.evaluate(() => window.cornerTest!.run(200));
  const ratio = r.events / r.samples / r.predicted;
  console.log(`corner: ${r.events} events in ${r.samples} box-snapshots; measured/2^-N = ${ratio.toFixed(2)}, measured/corrected = ${(r.events / r.samples / r.corrected).toFixed(2)}`);
  // Native runs over four seeds: 0.92–1.01 of 2^-N (mean 0.95), against 0.955 predicted with
  // the molecules' size (crates/md/tests/corner.rs). This seed gives ~0.9.
  expect(ratio).toBeGreaterThan(0.85);
  expect(ratio).toBeLessThan(1.1);
  expect(r.events / r.samples / r.corrected).toBeGreaterThan(0.85);
  await page.screenshot({ path: "test-results/corner.png", fullPage: false });
});

test("crowded molecules gather less often, as the size correction predicts", async ({ page }) => {
  await page.goto("pages/corner/index.html");
  await page.evaluate(() => window.cornerTest!.ready);
  const r = await page.evaluate(() => window.cornerTest!.run(60, 1, 6));
  const ideal = r.events / r.samples / r.predicted;
  const corrected = r.events / r.samples / r.corrected;
  console.log(`corner (1 σ, 6 per box): ${r.events} events; measured/2^-N = ${ideal.toFixed(2)}, measured/corrected = ${corrected.toFixed(2)}`);
  expect(ideal).toBeLessThan(0.75);
  expect(corrected).toBeGreaterThan(0.7);
  expect(corrected).toBeLessThan(1.3);
});
