import { expect, test } from "@playwright/test";

// Perrin's grains settle into an exponential haze whose height gives kT = m g H.
test("grains under gravity thin exponentially, with kT = m g H", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/sedimentation/index.html");
  await page.evaluate(() => window.sedimentationTest!.ready);
  const r = await page.evaluate(() => window.sedimentationTest!.run(200000, 400000));
  console.log(`sedimentation: scale height ${r.scale.toFixed(1)} σ (12 at kT 1), kT from the grains ${r.kt}, thermometer ${r.thermometer}`);
  expect(Math.abs(r.kt / r.thermometer - 1)).toBeLessThan(0.25);
});
