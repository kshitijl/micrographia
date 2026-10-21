import { expect, test } from "@playwright/test";

test("weak cross-attraction separates at low temperature; equal attraction stays mixed", async ({ page }) => {
  await page.goto("pages/oilwater/index.html");
  await page.evaluate(() => window.oilTest!.ready);
  const sep = await page.evaluate(() => window.oilTest!.run(40000, 0.2, 0.7));
  await page.locator("#stage").screenshot({ path: "test-results/oilwater-separated.png" });
  await page.reload();
  await page.evaluate(() => window.oilTest!.ready);
  const mixed = await page.evaluate(() => window.oilTest!.run(20000, 1.0, 0.7));
  console.log(`oil/water mixing after 200 τ: cross 0.2 → ${sep.toFixed(2)}, cross 1.0 → ${mixed.toFixed(2)}`);
  expect(sep).toBeLessThan(0.7);
  expect(mixed).toBeGreaterThan(0.85);
});

test("with gravity on, the lighter oil collects on top", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/oilwater/index.html");
  await page.evaluate(() => window.oilTest!.ready);
  // Native: 0.67 of the oil in the top half after 1200 τ at g = 0.03 (crates/md/tests/oilwater.rs).
  const top = await page.evaluate(() => window.oilTest!.oilOnTop(240000, 0.03));
  console.log(`oil/water: share of the oil in the top half with gravity: ${top.toFixed(2)}`);
  expect(top).toBeGreaterThan(0.6);
});

test("soap keeps a shaken emulsion as many small droplets; without it they merge", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/oilwater/index.html");
  const none = await page.evaluate(() => window.oilTest!.soap(0, 160000));
  const soapy = await page.evaluate(() => window.oilTest!.soap(0.2, 160000));
  const last = (r: { drops: number }[]) => r[r.length - 1].drops;
  console.log(`emulsion: droplets after 800 τ, no soap ${last(none)}, plenty of soap ${last(soapy)} (soap at a boundary ${Math.round(100 * soapy[soapy.length - 1].edge)}%)`);
  // Measured: 30 against 57, with 88% of the soap at a boundary.
  expect(last(soapy)).toBeGreaterThan(1.4 * last(none));
  expect(soapy[soapy.length - 1].edge).toBeGreaterThan(0.6);
});

// Dropping soap in by hand: each click switches on 40 dormant soap molecules where it lands.
test("the soap dropper adds soap where you click", async ({ page }) => {
  await page.goto("pages/oilwater/index.html");
  await page.evaluate(() => window.oilTest!.ready);
  await page.waitForTimeout(3000);
  await page.locator("#dropS").click();
  for (let k = 0; k < 3; k++) {
    await page.locator("#canvasS").click({ position: { x: 100 + 80 * k, y: 120 } });
    await page.waitForTimeout(400);
  }
  const text = await page.locator("#addedS").textContent();
  console.log(`soap dropper: ${text}`);
  expect(text).toContain("120 molecules");
});
