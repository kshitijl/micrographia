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
    // The method (molecular dynamics in 2D or 3D, Monte Carlo, lattice Boltzmann, a model), and
    // what holds or changes the energy.
    expect(text).toMatch(/Method: /);
    expect(text).toMatch(/Ensemble|ensemble|Heat bath|heat-bath|Energy:/);
    if (p === "heavy" || p === "arrhenius") console.log(`== ${p}\n${text}`);
  });
}
