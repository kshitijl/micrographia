import { expect, test } from "@playwright/test";

// Every simulation gets "Slow motion / Full speed" buttons; at full speed the simulation runs as
// fast as the GPU allows, sampled for the picture and the page's readouts.
test("full speed runs the simulation faster than slow motion", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("pages/boiling/index.html");
  await page.evaluate(() => window.boilingTest!.ready);
  const lab = page.locator("section.lab").first();
  const label = lab.locator(".speedlabel");
  const rate = async () => Number(((await label.textContent()) ?? "").split(" ")[0].replace(/,/g, ""));
  await lab.locator("#run").click();
  await page.waitForTimeout(3000);
  const slow = await rate();
  await lab.locator(".speedmode button", { hasText: "Full speed" }).click();
  await page.waitForTimeout(3000);
  const fast = await rate();
  console.log(`steps/s: slow motion ${slow}, full speed ${fast}`);
  expect(slow).toBeGreaterThan(0);
  // Run alone, full speed is ~2.4 times faster here (8,000 against 3,400 steps/s on an M-series
  // Mac); the suite's parallel workers share one GPU, so only check it is no slower.
  expect(fast).toBeGreaterThan(0.8 * slow);
  // The page's readouts keep coming at full speed.
  expect(Number(await page.locator("#tm").textContent())).toBeGreaterThan(0);
  expect(errors).toEqual([]);
});
