import { expect, test } from "@playwright/test";

// A flashing ratchet walks; with the grip always on, always off, or with no thermal noise, it
// does not; a load slows it.
test("a flashing ratchet turns noise into walking, and only then", async ({ page }) => {
  await page.goto("pages/motor/index.html");
  const run = (mode: "flashing" | "on" | "off", kT: number, load: number) => page.evaluate(([m, t, f]) => window.motorTest!.run(m as "flashing", t as number, f as number, 20), [mode, kT, load] as const);
  const flash = await run("flashing", 1, 0);
  const on = await run("on", 1, 0);
  const off = await run("off", 1, 0);
  const cold = await run("flashing", 0.02, 0);
  const loaded = await run("flashing", 1, 2);
  console.log(`motor: speed flashing ${flash.speed.toFixed(2)} (fuel ${flash.fuelPerStep.toFixed(1)} kT a step), on ${on.speed.toFixed(3)}, off ${off.speed.toFixed(3)}, cold ${cold.speed.toFixed(3)}, under load 2 ${loaded.speed.toFixed(2)}`);
  expect(flash.speed).toBeGreaterThan(0.3);
  expect(Math.abs(on.speed)).toBeLessThan(0.1);
  expect(Math.abs(off.speed)).toBeLessThan(0.3);
  expect(Math.abs(cold.speed)).toBeLessThan(0.1);
  expect(loaded.speed).toBeLessThan(flash.speed);
});
