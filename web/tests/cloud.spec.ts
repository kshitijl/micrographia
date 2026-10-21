import { expect, test } from "@playwright/test";

// A cloud chamber: an α particle's track is about as long as Bethe's slowing allows (v⁴/4C), with
// most of its ions near the end (the Bragg peak).
test("an α particle leaves a straight track, thickest at its end", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/cloud/index.html");
  await page.evaluate(() => window.cloudTest!.ready);
  const r = await page.evaluate(() => window.cloudTest!.shoot(10, 60000));
  console.log(`cloud: track ${r.length.toFixed(0)} σ, ${r.made} ions (first third ${r.firstThird}, last third ${r.lastThird}), ${r.kinks} sharp turns`);
  expect(r.length).toBeGreaterThan(80);
  expect(r.length).toBeLessThan(250);
  expect(r.lastThird).toBeGreaterThan(1.5 * r.firstThird);
  expect(r.made).toBeGreaterThan(150);
});
