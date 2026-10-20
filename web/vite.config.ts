import { defineConfig } from "vite";
import { resolve } from "node:path";
import { readdirSync, existsSync } from "node:fs";

// Every directory under pages/ with an index.html is a page.
const pages = Object.fromEntries(
  readdirSync(resolve(import.meta.dirname, "pages"))
    .filter((d: string) => existsSync(resolve(import.meta.dirname, "pages", d, "index.html")))
    .map((d: string) => [d, resolve(import.meta.dirname, "pages", d, "index.html")]),
);

export default defineConfig({
  base: "./",
  build: {
    target: "es2022",
    rollupOptions: {
      input: { index: resolve(import.meta.dirname, "index.html"), selftest: resolve(import.meta.dirname, "selftest/index.html"), ...pages },
    },
  },
  server: { port: 5173, strictPort: true },
  preview: { port: 4173, strictPort: true },
});
