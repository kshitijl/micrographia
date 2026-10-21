import { expect, test } from "@playwright/test";

// The countercurrent multiplier: 200 mOsm steps make urine four times saltier than blood; no ADH,
// dilute urine; a longer loop or stronger pumps, saltier.
test("the loop of Henle multiplies small steps into concentrated urine", async ({ page }) => {
  await page.goto("pages/kidney/index.html");
  const run = (o: { pump: number; adh: number; len: number; wash: number }) => page.evaluate((o) => window.kidneyTest!.run(3000, o), o);
  const human = await run({ pump: 200, adh: 1, len: 8, wash: 0.02 });
  const noAdh = await run({ pump: 200, adh: 0, len: 8, wash: 0.02 });
  const weak = await run({ pump: 100, adh: 1, len: 8, wash: 0.02 });
  const short = await run({ pump: 200, adh: 1, len: 4, wash: 0.02 });
  const long = await run({ pump: 200, adh: 1, len: 24, wash: 0.02 });
  console.log(`kidney: urine ${human.urine.toFixed(0)} (no ADH ${noAdh.urine.toFixed(0)}, weak pumps ${weak.urine.toFixed(0)}, short loop ${short.urine.toFixed(0)}, long loop ${long.urine.toFixed(0)})`);
  expect(human.urine).toBeGreaterThan(1000);
  expect(human.urine).toBeLessThan(1500);
  expect(noAdh.urine).toBeLessThan(200);
  expect(weak.urine).toBeLessThan(0.8 * human.urine);
  expect(long.urine).toBeGreaterThan(1.1 * human.urine);
  expect(short.urine).toBeLessThan(0.8 * human.urine);
});
