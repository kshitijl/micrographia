import { test } from "@playwright/test";

// Screenshots of every page after a short run, for visual review (not assertions).
const pages = ["reversal", "corner", "boltzmann", "heavy", "heatflow", "oilwater", "osmosis", "compress", "rubber", "atmosphere", "carnot", "boiling", "lipids", "balloon", "smell", "fep", "saline", "arrhenius", "buffer"];

for (const scheme of ["light", "dark"] as const) {
  for (const p of pages) {
    test(`screenshot ${p} (${scheme})`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await page.setViewportSize({ width: 1280, height: 1000 });
      await page.goto(`pages/${p}/index.html`);
      await page.locator("#overlay, #overlay1").first().waitFor({ state: "hidden", timeout: 30_000 });
      const start = page.locator("#run, #down, #run1").first();
      await start.click();
      await page.waitForTimeout(p === "buffer" ? 8000 : 2500);
      await page.screenshot({ path: `test-results/screens/${p}-${scheme}.png` });
    });
  }
}

test("screenshot index", async ({ page }) => {
  await page.goto("index.html");
  await page.screenshot({ path: "test-results/screens/index.png", fullPage: true });
});
