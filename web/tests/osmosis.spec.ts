import { expect, test } from "@playwright/test";

test("the push on the membrane is close to van 't Hoff's c kT", async ({ page }) => {
  await page.goto("pages/osmosis/index.html");
  await page.evaluate(() => window.osmosisTest!.ready);
  const r = await page.evaluate(() => window.osmosisTest!.measure(60000));
  console.log(`osmosis: Π = ${r.pi.toFixed(4)}, c kT = ${(r.c * r.t).toFixed(4)}`);
  expect(r.pi / (r.c * r.t)).toBeGreaterThan(0.6);
  expect(r.pi / (r.c * r.t)).toBeLessThan(1.25);
});
