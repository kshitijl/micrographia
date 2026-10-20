import { expect, test } from "@playwright/test";

test("the buffer holds most added protons, obeying mass action", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("pages/buffer/index.html");
  await page.evaluate(() => window.bufferTest!.ready);
  const r = await page.evaluate(() => window.bufferTest!.run(300));
  console.log(`buffer: K ${r.k.toFixed(4)} per column ${r.ks.map((k) => k.toFixed(4)).join(" ")}`);
  console.log(`buffer: free H buffered ${r.free.map((m) => m[0].toFixed(2)).join(" ")}; water ${r.free.map((m) => m[3].toFixed(1)).join(" ")}`);
  // Mass action: K is the same in every column up to the equivalence point (40 protons).
  const ks = r.ks.slice(1, 6);
  expect(Math.max(...ks) / Math.min(...ks)).toBeLessThan(1.8);
  // At 24 protons per box the buffer leaves far fewer free than water does.
  expect(r.free[3][0]).toBeLessThan(0.25 * r.free[3][3]);
  // Henderson–Hasselbalch from the median K matches each column within 0.15 in −log₁₀[H].
  for (let c = 1; c < r.free.length; c++) expect(Math.abs(Math.log10(r.free[c][0] / r.predicted[c]))).toBeLessThan(0.15);
});
