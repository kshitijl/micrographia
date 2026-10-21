import { expect, test } from "@playwright/test";

// The CPU engine (WebAssembly SIMD) tracks the GPU engine's trajectories in the browser (to the
// last bits where their float divisions round alike); and the speeds of both, by size.
test("the CPU and GPU engines agree in the browser", async ({ page }) => {
  test.setTimeout(300_000);
  await page.goto("selftest/engines.html");
  const r = await page.evaluate(() => window.engineResults!);
  for (const row of r.rows) console.log(`engines: ${row.ok ? "pass" : "FAIL"} ${row.name}: ${row.message}`);
  expect(r.rows.filter((x) => !x.ok)).toEqual([]);
});
