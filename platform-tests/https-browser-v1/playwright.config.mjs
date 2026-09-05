import { isAbsolute } from "node:path";
import { defineConfig } from "@playwright/test";

const generatedRoot = process.env.SEMAPRAX_HTTPS_PACKAGE_ROOT;
if (!generatedRoot || !isAbsolute(generatedRoot)) {
  throw new Error("SEMAPRAX_HTTPS_PACKAGE_ROOT must be an absolute generated-package path");
}
const port = 4189;

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  timeout: 30_000,
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    browserName: "chromium",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "node ./serve.mjs",
    env: {
      ...process.env,
      SEMAPRAX_HTTPS_BROWSER_PORT: String(port),
      SEMAPRAX_HTTPS_PACKAGE_ROOT: generatedRoot,
    },
    url: `http://127.0.0.1:${port}/index.html`,
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
