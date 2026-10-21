import { expect, test } from "@playwright/test";

// The depletion force: two big disks held 0.3 σ apart among small ones are pushed together about
// as Asakura and Oosawa say; let go, they stay close.
test("a crowd pushes two disks together", async ({ page }) => {
  test.setTimeout(600_000);
  await page.goto("pages/depletion/index.html");
  await page.evaluate(() => window.depletionTest!.ready);
  const r = await page.evaluate(() => window.depletionTest!.measure(3000));
  console.log(`depletion: gap ${r.gap.toFixed(2)}: push ${r.force.toFixed(3)} (AO ${r.ao.toFixed(3)}) at kT ${r.kt.toFixed(3)}`);
  expect(Math.abs(r.force / r.ao - 1)).toBeLessThan(0.3);
  const f = await page.evaluate(() => window.depletionTest!.free(40000));
  const close = f.gaps.filter((g) => g < 1).length / f.gaps.length;
  console.log(`depletion: let go, close (gap < 1 σ) ${(100 * close).toFixed(0)}% of the time; gaps ${f.gaps.slice(0, 10).map((g) => g.toFixed(1)).join(" ")} …`);
  // At random they would be this close 1.5% of the time (the ring 6–7 σ from one centre, in the box).
  expect(close).toBeGreaterThan(0.1);
});
