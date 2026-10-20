import { expect, test } from "@playwright/test";
import { readdirSync } from "node:fs";

// Every page says what kind of simulation it runs, from the engine's own description.
const pages = readdirSync(new URL("../pages", import.meta.url)).filter((p) => !p.startsWith("."));

for (const p of pages) {
  test(`${p} describes its simulation`, async ({ page }) => {
    await page.goto(`pages/${p}/index.html`);
    const about = page.locator("section.about-sim").first();
    await about.waitFor({ timeout: 30_000 });
    const text = await about.innerText();
    expect(text).toContain("molecular dynamics in 2D");
    expect(text).toMatch(/Ensemble|Heat bath|heat-bath/);
    if (p === "heavy" || p === "arrhenius") console.log(`== ${p}\n${text}`);
  });
}
