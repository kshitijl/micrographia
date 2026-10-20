import { expect, test } from "@playwright/test";

test("the alchemical prediction agrees with direct counting", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("pages/fep/index.html");
  await page.evaluate(() => window.fepTest!.ready);
  const fep = await page.evaluate(() => window.fepTest!.fep());
  const direct = await page.evaluate(() => window.fepTest!.direct(400));
  console.log(`fep: TI ${fep.ti.toFixed(2)} kT, FEP ${fep.fep.toFixed(2)} kT, direct ${direct.toFixed(2)} kT`);
  expect(Math.abs(fep.ti - fep.fep)).toBeLessThan(0.15);
  expect(Math.abs(fep.ti - direct)).toBeLessThan(0.5);
  expect(fep.ti).toBeLessThan(-0.7);
});
