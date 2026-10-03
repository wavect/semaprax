import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./test",
  testMatch: "browser-acceptance.spec.mjs",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  timeout: 30_000,
  use: {
    browserName: "chromium",
    trace: "retain-on-failure"
  }
});
