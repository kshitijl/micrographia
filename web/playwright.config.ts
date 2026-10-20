import { defineConfig } from "@playwright/test";

// Runs against the production build (vite preview) in the installed Google Chrome, which has
// WebGPU on macOS. Set PW_CHANNEL to try another browser channel.
// PW_BROWSER=webkit or PW_BROWSER=firefox runs the suite in Playwright's builds of those
// engines (their WebGPU support varies); the default is the installed Google Chrome.
const browser = process.env.PW_BROWSER;

export default defineConfig({
  testDir: "tests",
  timeout: 120_000,
  reporter: [["list"]],
  use: browser
    ? {
        baseURL: "http://localhost:4173/",
        browserName: browser as "webkit" | "firefox",
        headless: true,
        launchOptions: browser === "firefox" ? { firefoxUserPrefs: { "dom.webgpu.enabled": true, "gfx.webgpu.ignore-blocklist": true } } : {},
      }
    : {
        baseURL: "http://localhost:4173/",
        channel: process.env.PW_CHANNEL ?? "chrome",
        headless: true,
        launchOptions: { args: ["--enable-unsafe-webgpu", "--enable-features=Vulkan"] },
      },
  webServer: {
    command: "npx vite preview",
    url: "http://localhost:4173/",
    reuseExistingServer: !process.env.CI,
  },
});
