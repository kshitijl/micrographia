import { expect, test } from "@playwright/test";

// Haemoglobin (the MWC model): loads in the lungs and unloads about a quarter in the body; more
// when exercising; carbon monoxide takes seats by Haldane's ratio and cuts the delivery more than
// the seats it takes.
test("haemoglobin delivers oxygen cooperatively, and CO spoils it", async ({ page }) => {
  await page.goto("pages/blood/index.html");
  const run = (o: object) => page.evaluate((x) => window.bloodTest!.run(x, 8000), o);
  const rest = await run({});
  const ex = await run({ exercise: true });
  const co = await run({ co: 100 });
  console.log(`blood: rest: lungs ${rest.lungs.toFixed(3)}, body ${rest.body.toFixed(3)}, delivered ${rest.delivered.toFixed(2)}; exercise: delivered ${ex.delivered.toFixed(2)}; 100 ppm CO: seats ${(100 * co.cohb).toFixed(1)}%, lungs ${co.lungs.toFixed(3)}, delivered ${co.delivered.toFixed(2)}`);
  expect(Math.abs(rest.lungs - 0.944)).toBeLessThan(0.04);
  expect(Math.abs(rest.body - 0.75)).toBeLessThan(0.05);
  expect(ex.delivered).toBeGreaterThan(1.5 * rest.delivered);
  // Haldane: CO/O₂ seats = 220 × 0.0713 / 100 ≈ 0.157.
  expect(Math.abs(co.cohb / co.lungs - 0.157)).toBeLessThan(0.05);
  expect(co.delivered).toBeLessThan(0.9 * rest.delivered);
});
