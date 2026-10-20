/** Read a CSS custom property as linear-free sRGB floats [r, g, b, a] in 0..1. */
export function cssColor(name: string, el: Element = document.documentElement): [number, number, number, number] {
  const raw = getComputedStyle(el).getPropertyValue(name).trim();
  const probe = document.createElement("canvas").getContext("2d")!;
  probe.fillStyle = raw || "#000";
  const norm = probe.fillStyle as string; // "#rrggbb" or "rgba(r, g, b, a)"
  if (norm.startsWith("#")) {
    const n = parseInt(norm.slice(1), 16);
    return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255, 1];
  }
  const m = norm.match(/[\d.]+/g)!.map(Number);
  return [m[0] / 255, m[1] / 255, m[2] / 255, m[3] ?? 1];
}

/** Call `f` now and whenever the colour scheme changes. */
export function onThemeChange(f: () => void): void {
  f();
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", f);
}
