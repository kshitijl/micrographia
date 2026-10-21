import { expect, test } from "@playwright/test";

// Salt stops the freezing: just below the melting point the pure liquid freezes onto the
// crystal; with salt poured in, much less of it does in the same time.
test("salt poured into the liquid stops it freezing", async ({ page }) => {
  test.setTimeout(300_000);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("pages/freezing/index.html");
  await page.evaluate(() => window.freezingTest!.ready);
  const pure = await page.evaluate(() => window.freezingTest!.run(0.53, 0, 30000));
  const salty = await page.evaluate(() => window.freezingTest!.run(0.53, 4, 30000));
  console.log(`freezing at kT 0.53: pure ${Math.round(100 * pure.crystal)}% crystalline; with ${salty.salt} salt atoms ${Math.round(100 * salty.crystal)}% (kT measured ${salty.t})`);
  expect(salty.salt).toBeGreaterThan(400);
  expect(Math.abs(salty.t - 0.53)).toBeLessThan(0.03);
  expect(pure.crystal).toBeGreaterThan(salty.crystal + 0.15);
  expect(errors).toEqual([]);
});
