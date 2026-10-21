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
  // The most common energy: ~0 in 2D (the first bins), ½kT in 3D (kT = 0.95 here).
  expect(num(r.a[1])).toBeLessThan(0.15);
  expect(Math.abs(num(r.b[1]) - 0.48)).toBeLessThan(0.12);
  // The most common speed, √(kT/m) in 2D and √(2kT/m) in 3D.
  expect(Math.abs(num(r.a[2]) - 1.0)).toBeLessThan(0.12);
  expect(Math.abs(num(r.b[2]) - 1.38)).toBeLessThan(0.15);
  expect(errors).toEqual([]);
});
