import { isAbsolute } from "node:path";
import { defineConfig } from "@playwright/test";

function configuredFixtures() {
  const encoded = process.env.SEMAPRAX_CALCULATOR_FIXTURES;
  if (!encoded) {
    throw new Error("SEMAPRAX_CALCULATOR_FIXTURES must name all three calculator fixtures");
  }
  let fixtures;
  try {
    fixtures = JSON.parse(encoded);
  } catch {
    throw new Error("SEMAPRAX_CALCULATOR_FIXTURES must be valid fixture JSON");
  }
  const expectedNames = ["direct-source", "project-baseline", "project-renamed"];
  if (!Array.isArray(fixtures) || fixtures.length !== expectedNames.length) {
    throw new Error("SEMAPRAX_CALCULATOR_FIXTURES must contain exactly three fixtures");
  }
  const roots = new Set();
  for (const [index, fixture] of fixtures.entries()) {
    if (
      fixture === null ||
      typeof fixture !== "object" ||
      Array.isArray(fixture) ||
      Object.keys(fixture).join(",") !== "name,root" ||
      fixture.name !== expectedNames[index] ||
      typeof fixture.root !== "string" ||
      !isAbsolute(fixture.root) ||
      roots.has(fixture.root)
    ) {
      throw new Error("SEMAPRAX_CALCULATOR_FIXTURES contains an invalid ordered fixture");
    }
    roots.add(fixture.root);
  }
  return fixtures;
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
