import { spawnSync } from "node:child_process";
import { readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const roots = process.argv.slice(2);
if (roots.length !== 2) {
  throw new Error("usage: npm run test:fixtures -- <direct-source-root> <project-root>");
}

const expectedIds = [
  "calculator.add",
  "calculator.divide",
  "calculator.is-negative",
  "calculator.multiply",
  "calculator.not",
  "calculator.subtract",
];
const expectedSchemas = ["semaprax.web.v4", "semaprax.web-project.v1"];
const fixtures = roots.map((root, index) => {
  const resolved = resolve(root);
  if (!statSync(resolved).isDirectory()) {
    throw new Error(`calculator fixture is not a directory: ${resolved}`);
  }
  for (const relative of [
    "index.html",
    "app.js",
    "generated/app.wasm",
    "generated/semaprax.bindings.js",
  ]) {
    if (!statSync(resolve(resolved, relative)).isFile()) {
      throw new Error(`calculator fixture is missing ${relative}: ${resolved}`);
    }
  }
  const manifest = JSON.parse(
    readFileSync(resolve(resolved, "generated/semaprax.scalar-exports.json"), "utf8"),
  );
  if (manifest.schema !== expectedSchemas[index]) {
    throw new Error(`calculator fixture has the wrong package schema: ${resolved}`);
  }
  const observedIds = manifest.scalar_abi?.functions?.map((entry) => entry.stable_id);
  if (JSON.stringify(observedIds) !== JSON.stringify(expectedIds)) {
    throw new Error(`calculator fixture has the wrong stable-ID inventory: ${resolved}`);
  }
  return {
    name: index === 0 ? "direct-source" : "project",
    root: resolved,
  };
});
if (fixtures[0].root === fixtures[1].root) {
  throw new Error("direct-source and Project calculator roots must be distinct");
}

const cli = fileURLToPath(new URL("./node_modules/@playwright/test/cli.js", import.meta.url));
const result = spawnSync(process.execPath, [cli, "test", "--workers=1", "--retries=0"], {
  cwd: fileURLToPath(new URL(".", import.meta.url)),
  env: {
    ...process.env,
    SEMAPRAX_CALCULATOR_FIXTURES: JSON.stringify(fixtures),
  },
  stdio: "inherit",
});
if (result.error) throw result.error;
if (result.signal) throw new Error(`Playwright terminated by ${result.signal}`);
process.exitCode = result.status ?? 1;
