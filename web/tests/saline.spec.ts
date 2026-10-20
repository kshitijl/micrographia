import { expect, test } from "@playwright/test";

test("cells burst in pure water, keep their size in saline and shrink in brine", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("pages/saline/index.html");
  await page.evaluate(() => window.salineTest!.ready);
  const water = await page.evaluate(() => window.salineTest!.run(0, 100000));
  const saline = await page.evaluate(() => window.salineTest!.run(0.35, 100000));
  const brine = await page.evaluate(() => window.salineTest!.run(0.9, 100000));
  console.log(`saline: water ${water.burst} burst; saline ${saline.burst} burst, area ${saline.area.toFixed(0)}; brine ${brine.burst} burst, area ${brine.area.toFixed(0)}`);
  expect(water.burst).toBeGreaterThanOrEqual(2);
  expect(saline.burst).toBe(0);
  expect(brine.burst).toBe(0);
  expect(saline.area).toBeGreaterThan(brine.area * 1.15);
});

test("the big stiff cell tears in pure water and holds in saline and brine", async ({ page }) => {
  test.setTimeout(180_000);
  await page.goto("pages/saline/index.html");
  await page.evaluate(() => window.salineTest!.ready);
  const r = [];
  for (const out of [0, 0.5, 1]) r.push(await page.evaluate((o) => window.salineTest!.runB(o, 30000), out));
  console.log(`saline (big cell): ${r.map((x) => `tension ${x.tension.toFixed(2)}${x.torn ? `, torn at ${x.tornAt} τ` : ""}`).join("; ")}`);
  // Measured: torn at 19.5 τ (tension 5.9); saline 4.15; brine 3.3; the limit is 5.4.
  expect(r[0].torn).toBe(true);
  expect(r[1].torn).toBe(false);
  expect(r[2].torn).toBe(false);
  expect(r[1].tension).toBeGreaterThan(r[2].tension);
});

test("big cell: taking salt out around it tears it; pouring salt in slackens it; neither heats the water", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/saline/index.html");
  await page.evaluate(() => window.salineTest!.ready);
  const fresh = await page.evaluate(() => window.salineTest!.dose(0.5, false, 12, 80000));
  const salty = await page.evaluate(() => window.salineTest!.dose(0.5, true, 4, 60000));
  const f = (x: number[]) => x.map((v) => v.toFixed(2)).join(" ");
  console.log(`saline (big cell), salt taken out: torn at ${fresh.tornAt} τ; tension ${f(fresh.trace)}; changed ${fresh.changed}`);
  console.log(`saline (big cell), salt poured in: tension ${f(salty.trace)}; kT ${salty.temps[0].toFixed(3)} → ${salty.temps[salty.temps.length - 1].toFixed(3)}`);
  // Measured: torn at 163 τ; with salt poured in the tension falls from 4.4 to ~3.6.
  expect(fresh.torn).toBe(true);
  expect(salty.torn).toBe(false);
  expect(salty.trace[salty.trace.length - 1]).toBeLessThan(salty.trace[0] - 0.4);
  for (const r of [fresh, salty]) expect(Math.abs(r.temps[r.temps.length - 1] / r.temps[0] - 1)).toBeLessThan(0.02);
});
