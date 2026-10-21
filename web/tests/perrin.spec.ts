import { expect, test } from "@playwright/test";

// Values of N measured on the Millikan and counting pages appear in the visitor's Perrin table,
// near the true 6.0 × 10²³.
test("measured values of N land in the Perrin table", async ({ page }) => {
  await page.goto("pages/perrin/index.html");
  await page.click("#forget");
  await page.goto("pages/millikan/index.html");
  await page.evaluate(() => window.millikanTest!.measure(12));
  const e = await page.locator("#e").innerText();
  await page.goto("pages/counting/index.html");
  await page.evaluate(() => window.countingTest!.run(600, 60));
  const c = await page.locator("#nav").innerText();
  console.log(`perrin: millikan ${e}; counting ${c}`);
  await page.goto("pages/perrin/index.html");
  for (const key of ["millikan", "counting"]) {
    const text = await page.locator(`tr[data-key="${key}"] td:nth-child(3)`).innerText();
    console.log(`perrin: ${key} row: ${text}`);
    const m = text.match(/([\d.]+) × 10²³/);
    expect(m).not.toBeNull();
    expect(Math.abs(Number(m![1]) - 6.02)).toBeLessThan(0.6);
  }
});
