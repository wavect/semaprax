import { defineConfig } from "@playwright/test";

const calculatorRoot = process.env.SEMAPRAX_CALCULATOR_ROOT;
if (!calculatorRoot) {
  throw new Error("SEMAPRAX_CALCULATOR_ROOT must name the generated calculator directory");
}

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  timeout: 30_000,
  use: {
    baseURL: "http://127.0.0.1:4173",
    browserName: "chromium",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "node ./serve.mjs",
    env: {
      ...process.env,
      SEMAPRAX_CALCULATOR_ROOT: calculatorRoot,
    },
    url: "http://127.0.0.1:4173/index.html",
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
