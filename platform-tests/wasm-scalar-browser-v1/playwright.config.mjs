import { isAbsolute } from "node:path";
import { defineConfig } from "@playwright/test";

function configuredFixtures() {
  const encoded = process.env.SEMAPRAX_CALCULATOR_FIXTURES;
  if (encoded) {
    let fixtures;
    try {
      fixtures = JSON.parse(encoded);
    } catch {
      throw new Error("SEMAPRAX_CALCULATOR_FIXTURES must be valid fixture JSON");
    }
    if (!Array.isArray(fixtures) || fixtures.length !== 2) {
      throw new Error("SEMAPRAX_CALCULATOR_FIXTURES must contain exactly two fixtures");
    }
    const names = new Set();
    const roots = new Set();
    for (const fixture of fixtures) {
      if (
        fixture === null ||
        typeof fixture !== "object" ||
        Array.isArray(fixture) ||
        Object.keys(fixture).sort().join(",") !== "name,root" ||
        typeof fixture.name !== "string" ||
        !/^[a-z][a-z0-9-]{0,31}$/.test(fixture.name) ||
        names.has(fixture.name) ||
        typeof fixture.root !== "string" ||
        !isAbsolute(fixture.root) ||
        roots.has(fixture.root)
      ) {
        throw new Error("SEMAPRAX_CALCULATOR_FIXTURES contains an invalid fixture");
      }
      names.add(fixture.name);
      roots.add(fixture.root);
    }
    return fixtures;
  }

  const root = process.env.SEMAPRAX_CALCULATOR_ROOT;
  if (!root) {
    throw new Error("SEMAPRAX_CALCULATOR_ROOT must name the generated calculator directory");
  }
  return [{ name: "direct-source", root }];
}

const fixtures = configuredFixtures();
const projects = fixtures.map((fixture, index) => ({
  name: fixture.name,
  use: { baseURL: `http://127.0.0.1:${4173 + index}` },
}));
const webServer = fixtures.map((fixture, index) => ({
  command: "node ./serve.mjs",
  env: {
    ...process.env,
    SEMAPRAX_CALCULATOR_ROOT: fixture.root,
    SEMAPRAX_CALCULATOR_PORT: String(4173 + index),
  },
  url: `http://127.0.0.1:${4173 + index}/index.html`,
  reuseExistingServer: false,
  timeout: 30_000,
}));

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  timeout: 30_000,
  use: {
    browserName: "chromium",
    trace: "retain-on-failure",
  },
  projects,
  webServer,
});
