import { defineConfig } from "@playwright/test";

// Runs against the production build (vite preview) in the installed Google Chrome, which has
// WebGPU on macOS. Set PW_CHANNEL to try another browser channel.
export default defineConfig({
  testDir: "tests",
  timeout: 120_000,
  reporter: [["list"]],
  use: {
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
