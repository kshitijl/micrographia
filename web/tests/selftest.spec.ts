import { expect, test } from "@playwright/test";

test("GPU self-checks pass in the browser", async ({ page }) => {
  page.on("console", (m) => console.log(`[page] ${m.text()}`));
  await page.goto("selftest/index.html");
  const out = await page.evaluate(() => window.selftestResults!);
  console.log(`adapter: ${out.adapter}`);
  for (const r of out.results) console.log(`${r.ok ? "pass" : "FAIL"} ${r.name}: ${r.message} (${r.ms.toFixed(0)} ms)`);
  expect(out.results.length).toBeGreaterThan(10);
  expect(out.results.filter((r) => !r.ok)).toEqual([]);
});
