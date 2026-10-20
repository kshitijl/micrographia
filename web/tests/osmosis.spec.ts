import { expect, test } from "@playwright/test";

test("the push on the membrane is close to van 't Hoff's c kT", async ({ page }) => {
  await page.goto("pages/osmosis/index.html");
  await page.evaluate(() => window.osmosisTest!.ready);
  const r = await page.evaluate(() => window.osmosisTest!.measure(60000));
  console.log(`osmosis: Π = ${r.pi.toFixed(4)}, c kT = ${(r.c * r.t).toFixed(4)}`);
  expect(r.pi / (r.c * r.t)).toBeGreaterThan(0.6);
  expect(r.pi / (r.c * r.t)).toBeLessThan(1.25);
});

test("a free membrane is pushed toward the pure water", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("pages/osmosis/index.html");
  await page.evaluate(() => window.osmosisDemos!.ready);
  const r = await page.evaluate(() => window.osmosisDemos!.membrane(100000));
  console.log(`osmosis: free membrane ${r.start.toFixed(1)} → ${r.end.toFixed(1)} σ`);
  // Native: 48 → 33 σ in 100k steps.
  expect(r.end).toBeLessThan(r.start - 6);
});

test("the solution climbs well above the pure water", async ({ page }) => {
  test.setTimeout(900_000);
  await page.goto("pages/osmosis/index.html");
  await page.evaluate(() => window.osmosisDemos!.ready);
  const r = await page.evaluate(() => window.osmosisDemos!.tube(400000));
  const late = r.dh.slice(r.dh.length / 2).sort((a, b) => a - b);
  const median = late[Math.floor(late.length / 2)];
  console.log(`osmosis: Δh (smoothed), median over the second half ${median.toFixed(1)} σ, van 't Hoff ${r.pred.toFixed(1)} σ`);
  // Native: rises to 8–17 σ within ~800 τ, then about half of van 't Hoff's ~20 σ on average
  // (crates/md/tests/osmosis_demos.rs). A qualitative check: it clearly rises, and not too far.
  expect(median).toBeGreaterThan(4);
  expect(median).toBeLessThan(1.4 * r.pred);
});

test("the classic U-tube: the solution climbs through the passage, with the energy strictly conserved", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("pages/osmosis/index.html");
  await page.evaluate(() => window.osmosisDemos!.ready);
  const r = await page.evaluate(() => window.osmosisDemos!.tube(200000, "C"));
  const late = r.dh.slice(r.dh.length / 2).sort((a, b) => a - b);
  const median = late[Math.floor(late.length / 2)];
  const b = r.book;
  console.log(`osmosis (U-tube): Δh median over the second half ${median.toFixed(1)} σ, van 't Hoff ${r.pred.toFixed(1)} σ; energy changes: lift ${b.lift.toFixed(1)}, heat ${b.heat.toFixed(1)}, interactions ${b.bonds.toFixed(1)} ε`);
  expect(median).toBeGreaterThan(4);
  expect(median).toBeLessThan(1.4 * r.pred);
  expect(Math.abs(b.lift + b.heat + b.bonds)).toBeLessThan(3);
});
