import init from "../wasm/sm_wasm.js";
export * from "../wasm/sm_wasm.js";

let ready: Promise<unknown> | null = null;

/** Load and instantiate the wasm module once per page. */
export function loadWasm(): Promise<unknown> {
  ready ??= init();
  return ready;
}

/** Explain why the page cannot run, or return null if WebGPU looks available. */
export async function webgpuProblem(): Promise<string | null> {
  if (!("gpu" in navigator)) {
    return "This page needs WebGPU, which this browser does not provide. Try a recent Chrome, Edge or Safari.";
  }
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) {
    return "WebGPU is present but no GPU adapter is available (it may be disabled or blocklisted).";
  }
  return null;
}
