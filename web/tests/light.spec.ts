import { expect, test } from "@playwright/test";

// The light in a hot box: quantum modes follow Planck's law and glow orange at 3,000 K; classical
// modes get kT each, violet as much as red. Grains of light knock out electrons with hν − W at
// most, none below the threshold however bright; brighter light gives more electrons, not faster.
test("light in lumps gives Planck's spectrum and the photoelectric effect", async ({ page }) => {
  await page.goto("pages/light/index.html");
  const q = await page.evaluate(() => window.lightTest!.box(3000, "quantum", 400));
  const c = await page.evaluate(() => window.lightTest!.box(3000, "classical", 400));
  console.log(`light: quantum / Planck ${q.planckRatio.map((x) => x.toFixed(3)).join(", ")}; per mode (kT) low ${q.perModeLow.toFixed(2)}, violet ${q.perModeViolet.toExponential(1)}; glow ${q.glow}`);
  console.log(`light: classical per mode (kT) low ${c.perModeLow.toFixed(2)}, violet ${c.perModeViolet.toFixed(2)}; glow ${c.glow}`);
  for (const r of q.planckRatio) expect(Math.abs(r - 1)).toBeLessThan(0.08);
  expect(q.perModeViolet).toBeLessThan(0.01);
  expect(Math.abs(c.perModeViolet - 1)).toBeLessThan(0.1);
  // Orange: more red than blue; classical: more blue than red.
  const rgb = (s: string) => s.match(/\d+/g)!.map(Number);
  expect(rgb(q.glow)[0]).toBeGreaterThan(rgb(q.glow)[2]);
  expect(rgb(c.glow)[2]).toBeGreaterThan(rgb(c.glow)[0]);

  const red = await page.evaluate(() => window.lightTest!.photo("grains", 650, 10, 20));
  const uv = await page.evaluate(() => window.lightTest!.photo("grains", 300, 1, 40));
  const uvBright = await page.evaluate(() => window.lightTest!.photo("grains", 300, 4, 40));
  const waveRed = await page.evaluate(() => window.lightTest!.photo("wave", 650, 4, 40));
  console.log(`light: red ×10 ${red.rate}/s; UV ×1 ${uv.rate.toFixed(1)}/s at most ${uv.keMax.toFixed(2)} eV, ×4 ${uvBright.rate.toFixed(1)}/s at most ${uvBright.keMax.toFixed(2)} eV (hν − W = ${(4.133 - 2.3).toFixed(2)}); wave theory, red ×4: ${waveRed.rate.toFixed(1)}/s`);
  expect(red.rate).toBe(0);
  expect(Math.abs(uv.keMax - 1.83)).toBeLessThan(0.05);
  expect(Math.abs(uvBright.keMax - 1.83)).toBeLessThan(0.05);
  expect(uvBright.rate / uv.rate).toBeGreaterThan(3);
  expect(uvBright.rate / uv.rate).toBeLessThan(5);
  expect(waveRed.rate).toBeGreaterThan(0);
  // Planck's count: h from two colours' fastest electrons, k from the 3,000 K spectrum's peak.
  await page.evaluate(() => window.lightTest!.box(3000, "quantum", 400));
  await page.evaluate(() => window.lightTest!.photo("grains", 250, 4, 40));
  await page.evaluate(() => window.lightTest!.photo("grains", 350, 4, 40));
  const nh = await page.locator("#nh").innerText();
  console.log(`light: ${nh}`);
  const n = Number(nh.match(/N = ([\d.]+) × 10²³/)![1]);
  expect(Math.abs(n - 6.02)).toBeLessThan(0.6);
});
