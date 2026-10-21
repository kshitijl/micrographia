import { expect, test } from "@playwright/test";

// The charges measured from falls and rises are whole multiples of e (1.602 × 10⁻¹⁹ C), give or
// take the Brownian blur.
test("oil drops' charges come in whole multiples of e", async ({ page }) => {
  await page.goto("pages/millikan/index.html");
  const q = await page.evaluate(() => window.millikanTest!.measure(30));
  const off = q.map((x) => Math.abs(x / 1.602 - Math.round(x / 1.602)));
  console.log(`millikan: ${q.length} drops, charges/e: ${q.map((x) => (x / 1.602).toFixed(2)).join(" ")}`);
  expect(q.length).toBe(30);
  // Most within 0.2 e of a whole number.
  expect(off.filter((d) => d < 0.2).length).toBeGreaterThan(24);
});
