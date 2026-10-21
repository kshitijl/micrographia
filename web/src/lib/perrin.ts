// The visitor's own Perrin table: pages that turn a measurement into Avogadro's number N record
// their latest value here (in this browser only), and the `perrin` page lists them beside
// Perrin's. A convenience: nothing breaks if storage is unavailable.

/** The true N (per mole). The simulations' units are mapped to the laboratory so that this is the
 * right answer; any error in a visitor's value is the measurement's. */
export const N_TRUE = 6.02214076e23;
/** The gas constant (J/mol/K) and the faraday (C/mol), as the chemists of 1900 had them. */
export const R = 8.314;
export const FARADAY = 96485;

export type Phenomenon = "viscosity" | "sedimentation" | "brownian" | "millikan" | "counting" | "blackbody";

export interface Count {
  n: number;
  /** How it was got, in a few words. */
  how: string;
  /** When (ms since 1970). */
  at: number;
}

const KEY = "micrographia-perrin";

export function recorded(): Partial<Record<Phenomenon, Count>> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}");
  } catch {
    return {};
  }
}

/** Record a value of N (ignored unless finite and positive). */
export function record(p: Phenomenon, n: number, how: string) {
  if (!(Number.isFinite(n) && n > 0)) return;
  try {
    const all = recorded();
    all[p] = { n, how, at: Date.now() };
    localStorage.setItem(KEY, JSON.stringify(all));
  } catch {
    // No storage (a private window, say): the page's own readout still shows the value.
  }
}

export function forget() {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // Nothing to forget.
  }
}

/** "6.1 × 10²³". */
export function formatN(n: number) {
  const e = Math.floor(Math.log10(n));
  const sup = String(e).replace(/\d/g, (d) => "⁰¹²³⁴⁵⁶⁷⁸⁹"[Number(d)]);
  return `${(n / 10 ** e).toFixed(1)} × 10${sup}`;
}
