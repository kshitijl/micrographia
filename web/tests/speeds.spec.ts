import { expect, test } from "@playwright/test";

// The energy and speed distributions of a 2D and a 3D gas: Boltzmann's and Maxwell's.
test("energies peak at zero in 2D and ½kT in 3D; speeds at a nonzero speed in both", async ({ page }) => {
  test.setTimeout(120_000);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("pages/speeds/index.html");
  await page.evaluate(() => window.speedsTest!.ready);
  const r = await page.evaluate(() => window.speedsTest!.run(4000));
  console.log(`speeds: 2D ${r.a.join(" | ")}; 3D ${r.b.join(" | ")}`);
  const num = (s: string) => Number(s.split(" ")[0]);
  // Average energy: kT in 2D, 1.5 kT in 3D (the readout is "E = x kT").
  expect(r.a[0]).toContain("= 1.00 kT");
  expect(r.b[0]).toContain("= 1.50 kT");
  // The most common energy: ~0 in 2D (the first bin), near ½kT in 3D.
  expect(num(r.a[1])).toBeLessThan(0.25);
  expect(num(r.b[1])).toBeGreaterThan(0.25);
  // The most common speed is well away from zero in both.
  expect(num(r.a[2])).toBeGreaterThan(0.6);
  expect(num(r.b[2])).toBeGreaterThan(0.9);
  expect(errors).toEqual([]);
});
