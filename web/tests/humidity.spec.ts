import { expect, test } from "@playwright/test";

// Saturated air over water at kT 0.44, cooled to 0.40: the vapour condenses down to the
// saturated density there (0.023 per σ², from the Gibbs ensemble).
test("cooled saturated air drops its extra water", async ({ page }) => {
  test.setTimeout(300_000);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("pages/humidity/index.html");
  await page.evaluate(() => window.humidityTest!.ready);
  const r = await page.evaluate(() => window.humidityTest!.cool(0.44, 0.4, 120000));
  console.log(`humidity: vapour ${r.before.toFixed(4)} at kT 0.44 (saturated 0.058), ${r.after.toFixed(4)} after cooling to 0.40 (saturated 0.023)`);
  expect(Math.abs(r.before - 0.058)).toBeLessThan(0.015);
  expect(Math.abs(r.after - 0.023)).toBeLessThan(0.008);
  expect(errors).toEqual([]);
});
