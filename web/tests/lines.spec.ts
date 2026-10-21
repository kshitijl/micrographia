import { expect, test } from "@playwright/test";

// Doppler widths: λ √(kT/m)/c, so hydrogen's line (m = 1) is √16 times wider, relative to its
// wavelength, than oxygen's (m = 16); the widths weigh the atoms.
test("spectral lines are as wide as Doppler says, and weigh the atoms that glow", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/lines/index.html");
  await page.evaluate(() => window.linesTest!.ready);
  for (const scale of [1, 1.5]) {
    const r = await page.evaluate((s) => window.linesTest!.measure(s, 20000), scale);
    console.log(`lines: ${r.t.toFixed(0)} K: widths ${r.widths.map((w) => w.toFixed(2)).join(", ")} pm (Doppler ${r.doppler.map((w) => w.toFixed(2)).join(", ")}); masses ${r.masses.map((m) => m.toFixed(2)).join(", ")}`);
    for (let k = 0; k < 2; k++) expect(Math.abs(r.widths[k] / r.doppler[k] - 1)).toBeLessThan(0.08);
    expect(Math.abs(r.masses[0] - 1)).toBeLessThan(0.15);
    expect(Math.abs(r.masses[1] - 16)).toBeLessThan(2.5);
  }
});
