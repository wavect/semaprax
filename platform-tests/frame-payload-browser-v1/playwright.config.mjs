import { defineConfig } from "../wasm-scalar-browser-v1/node_modules/@playwright/test/index.mjs";
import { readFileSync } from "node:fs";

const installed = JSON.parse(readFileSync(new URL("../wasm-scalar-browser-v1/node_modules/@playwright/test/package.json", import.meta.url)));
if (installed.version !== "1.62.0") throw new Error("requires repository-pinned Playwright 1.62.0");

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  timeout: 30000,
  outputDir: "../wasm-scalar-browser-v1/test-results/frame-payload",
  use: { browserName: "chromium", trace: "retain-on-failure" },
});
