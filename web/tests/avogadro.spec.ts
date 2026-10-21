import { expect, test } from "@playwright/test";

// Gay-Lussac's volumes: H₂ + Cl₂ (two atoms each) react to 2 volumes of HCl, the same as they
// started; Dalton's single atoms, H + Cl → HCl, would give 1.
test("hydrogen and chlorine molecules give two volumes of HCl; Dalton's atoms would give one", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("pages/avogadro/index.html");
  await page.evaluate(() => window.avogadroTest!.ready);
  const r = await page.evaluate(() => window.avogadroTest!.react(20_000, 100_000, 40_000, 40_000));
  for (const k of ["A", "D"] as const) console.log(`avogadro ${k}: volume ${r[k].before.toFixed(2)} → ${r[k].after.toFixed(2)}; H₂ ${r[k].h2}, Cl₂ ${r[k].cl2}, HCl ${r[k].hcl}, free ${r[k].free}`);
  expect(r.A.hcl).toBeGreaterThan(0.85 * 300);
  expect(r.D.hcl).toBeGreaterThan(0.85 * 150);
  expect(Math.abs(r.A.after / r.A.before - 1)).toBeLessThan(0.12);
  expect(Math.abs(r.D.after / r.D.before - 0.5)).toBeLessThan(0.1);
});
