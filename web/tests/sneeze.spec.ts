import { expect, test } from "@playwright/test";

// The same burst of heat adds about Q/T of entropy to each box: four times as much to the cold
// one (kT 0.5) as to the hot one (kT 2).
test("a sneeze adds Q/T of entropy: more to a cold box than a hot one", async ({ page }) => {
  test.setTimeout(300_000);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("pages/sneeze/index.html");
  await page.evaluate(() => window.sneezeTest!.ready);
  const c = await page.evaluate(() => window.sneezeTest!.sneeze("C", 300, 20000));
  const h = await page.evaluate(() => window.sneezeTest!.sneeze("H", 300, 20000));
  console.log(`sneeze: library ΔS ${c.ds.toFixed(0)} k (Q/T ${c.qt.toFixed(0)}); concert ΔS ${h.ds.toFixed(0)} k (Q/T ${h.qt.toFixed(0)})`);
  // Exactly, for an ideal gas: N k ln(1 + Q/E), 5% below Q/T for this 10% sneeze.
  expect(Math.abs(c.ds - 0.953 * c.qt)).toBeLessThan(0.06 * c.qt);
  expect(Math.abs(h.ds - 0.953 * h.qt)).toBeLessThan(0.15 * h.qt);
  expect(c.ds).toBeGreaterThan(2.5 * h.ds);
  expect(errors).toEqual([]);
});
