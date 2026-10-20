import { expect, test } from "@playwright/test";

test("strands melt at a temperature that rises with length; single pairs barely hold", async ({ page }) => {
  test.setTimeout(900_000);
  await page.goto("pages/dna/index.html");
  await page.evaluate(() => window.dnaTest!.ready);
  const r = await page.evaluate(() => window.dnaTest!.sweep(50));
  console.log(`dna: Tm ${r.tm.map((t) => t.toFixed(2)).join(" ")}; ${r.curves.map((c) => `${c.t}: ${c.share.map((s) => s.toFixed(2)).join("/")}`).join(", ")}`);
  console.log(`dna: length-8 histogram at kT 0.9: ${r.hist.join(" ")}`);
  // Native runs: Tm ≈ 0.72 (4 beads), 0.95 (8 beads) (crates/md/tests/dna.rs).
  expect(r.tm[1]).toBeGreaterThan(r.tm[0] + 0.12);
  for (const c of r.curves) expect(c.share[0]).toBeLessThan(0.2);
  expect(r.curves[0].share[2]).toBeGreaterThan(0.9);
});
