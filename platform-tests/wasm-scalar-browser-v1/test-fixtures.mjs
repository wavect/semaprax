import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync, realpathSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const roots = process.argv.slice(2);
if (roots.length !== 3) {
  throw new Error(
    "usage: npm run test:fixtures -- <direct-source-root> <project-baseline-root> <project-renamed-root>",
  );
}

const expectedIds = [
  "calculator.add",
  "calculator.divide",
  "calculator.is-negative",
  "calculator.multiply",
  "calculator.not",
  "calculator.subtract",
];
const expectedFunctions = [
  { stable_id: "calculator.add", parameters: ["i64", "i64"], result: "i64" },
  { stable_id: "calculator.divide", parameters: ["i64", "i64"], result: "i64" },
  { stable_id: "calculator.is-negative", parameters: ["i64"], result: "bool" },
  { stable_id: "calculator.multiply", parameters: ["i64", "i64"], result: "i64" },
  { stable_id: "calculator.not", parameters: ["bool"], result: "bool" },
  { stable_id: "calculator.subtract", parameters: ["i64", "i64"], result: "i64" },
].map(({ stable_id, parameters, result }) => ({
  stable_id,
  wasm_export: `spx_scalar_${Buffer.from(stable_id).toString("hex")}`,
  parameters,
  result,
}));
const fixtureSpecs = [
  { name: "direct-source", schema: "semaprax.web.v4" },
  {
    name: "project-baseline",
    schema: "semaprax.web-project.v1",
    project_revision: "sha256:8576caa566cb7f0d265354927c5bc7b481146f05e616f76917f340b4af26f053",
    workspace_revision: "sha256:f0454397a2b339677bc49c9ccd8e8491917426202c6aba2475221879e02ae3f6",
    project_graph_digest: "sha256:92dbd747c206def786979a298d9b6d81f13768dac93a9c6465c839c6be9f96d9",
  },
  {
    name: "project-renamed",
    schema: "semaprax.web-project.v1",
    project_revision: "sha256:afa7b35b6b057eaa1cbf89c68ccd1e19a8d988f4168049f70717f80c28218fb7",
    workspace_revision: "sha256:8fcf973950f10bf9393ff5597484333b178e5f93c5d2a1847f6ccc18d6185f71",
    project_graph_digest: "sha256:822f5d61f8f6578341da4dad98ec59cac2a3447fa4cc5458cf8d4a2da1052bf2",
  },
];
const expectedArtifacts = [
  "app.wasm",
  "index.html",
  "package.json",
  "semaprax.bindings.d.ts",
  "semaprax.bindings.js",
  "semaprax.js",
];
const shellFiles = ["app.js", "consumer.ts", "index.html", "package.json"];
const directManifestKeys = [
  "schema",
  "module",
  "graph_revision",
  "capabilities",
  "artifacts",
  "scalar_abi",
];
const projectManifestKeys = [
  "schema",
  "project_schema",
  "project",
  "project_revision",
  "workspace_revision",
  "project_graph_digest",
  "entry_module",
  "capabilities",
  "artifacts",
  "scalar_abi",
];
const committedShellRoot = fileURLToPath(
  new URL("../../examples/calculator-web/", import.meta.url),
);

function exactKeys(value, expected, subject) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    JSON.stringify(Object.keys(value)) !== JSON.stringify(expected)
  ) {
    throw new Error(`${subject} has the wrong canonical key order`);
  }
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function shaRevision(value) {
  return typeof value === "string" && /^sha256:[0-9a-f]{64}$/.test(value);
}

function requireRegularFile(path, subject) {
  if (!lstatSync(path).isFile()) {
    throw new Error(`${subject} is not a regular file: ${path}`);
  }
}

const fixtures = roots.map((root, index) => {
  const spec = fixtureSpecs[index];
  const resolved = realpathSync(resolve(root));
  if (!statSync(resolved).isDirectory()) {
    throw new Error(`calculator fixture is not a directory: ${resolved}`);
  }
  for (const relative of shellFiles) {
    requireRegularFile(resolve(resolved, relative), `calculator fixture shell ${relative}`);
    if (
      !readFileSync(resolve(resolved, relative)).equals(
        readFileSync(resolve(committedShellRoot, relative)),
      )
    ) {
      throw new Error(`calculator fixture changed committed shell ${relative}: ${resolved}`);
    }
  }
  const generatedRoot = resolve(resolved, "generated");
  if (!lstatSync(generatedRoot).isDirectory()) {
    throw new Error(`calculator fixture generated path is not a directory: ${resolved}`);
  }
  const expectedGeneratedInventory = [...expectedArtifacts, "semaprax.scalar-exports.json"].sort();
  if (
    JSON.stringify(readdirSync(generatedRoot).sort()) !==
    JSON.stringify(expectedGeneratedInventory)
  ) {
    throw new Error(`calculator fixture has a foreign or missing generated artifact: ${resolved}`);
  }
  for (const relative of expectedGeneratedInventory) {
    requireRegularFile(
      resolve(generatedRoot, relative),
      `calculator generated artifact ${relative}`,
    );
  }
  const manifestSource = readFileSync(
    resolve(generatedRoot, "semaprax.scalar-exports.json"),
    "utf8",
  );
  const manifest = JSON.parse(manifestSource);
  if (`${JSON.stringify(manifest)}\n` !== manifestSource) {
    throw new Error(`calculator fixture manifest is not canonical JSON: ${resolved}`);
  }
  exactKeys(
    manifest,
    index === 0 ? directManifestKeys : projectManifestKeys,
    `${spec.name} manifest`,
  );
  if (manifest.schema !== spec.schema) {
    throw new Error(`calculator fixture has the wrong package schema: ${resolved}`);
  }
  if (!Array.isArray(manifest.capabilities) || manifest.capabilities.length !== 0) {
    throw new Error(`calculator fixture widened capability authority: ${resolved}`);
  }
  if (index === 0) {
    if (manifest.module !== "examples.calculator" || !shaRevision(manifest.graph_revision)) {
      throw new Error(`direct-source fixture has the wrong authenticated subject: ${resolved}`);
    }
  } else if (
    manifest.project_schema !== "semaprax.project.v1" ||
    manifest.project !== "calculator" ||
    manifest.entry_module !== "calculator.app" ||
    manifest.project_revision !== spec.project_revision ||
    manifest.workspace_revision !== spec.workspace_revision ||
    manifest.project_graph_digest !== spec.project_graph_digest
  ) {
    throw new Error(`Project fixture does not match its exact rename known-answer subject: ${resolved}`);
  }
  if (!Array.isArray(manifest.artifacts) || manifest.artifacts.length !== expectedArtifacts.length) {
    throw new Error(`calculator fixture has the wrong artifact inventory: ${resolved}`);
  }
  for (const [artifactIndex, artifact] of manifest.artifacts.entries()) {
    exactKeys(artifact, ["path", "sha256"], `${spec.name} artifact ${artifactIndex}`);
    const expectedPath = expectedArtifacts[artifactIndex];
    const bytes = readFileSync(resolve(resolved, "generated", expectedPath));
    if (artifact.path !== expectedPath || artifact.sha256 !== sha256(bytes)) {
      throw new Error(`calculator fixture artifact authentication failed: ${resolved}`);
    }
  }
  exactKeys(manifest.scalar_abi, ["schema", "functions"], `${spec.name} scalar ABI`);
  if (manifest.scalar_abi.schema !== "semaprax.wasm-scalar.v1") {
    throw new Error(`calculator fixture has the wrong scalar ABI schema: ${resolved}`);
  }
  if (!Array.isArray(manifest.scalar_abi.functions)) {
    throw new Error(`calculator fixture scalar ABI functions are not an array: ${resolved}`);
  }
  for (const [functionIndex, entry] of manifest.scalar_abi.functions.entries()) {
    exactKeys(
      entry,
      ["stable_id", "wasm_export", "parameters", "result"],
      `${spec.name} scalar ABI function ${functionIndex}`,
    );
  }
  const observedIds = manifest.scalar_abi?.functions?.map((entry) => entry.stable_id);
  if (JSON.stringify(observedIds) !== JSON.stringify(expectedIds)) {
    throw new Error(`calculator fixture has the wrong stable-ID inventory: ${resolved}`);
  }
  if (JSON.stringify(manifest.scalar_abi.functions) !== JSON.stringify(expectedFunctions)) {
    throw new Error(`calculator fixture has the wrong exact six-function scalar API: ${resolved}`);
  }
  return {
    name: spec.name,
    root: resolved,
    manifest,
  };
});
if (new Set(fixtures.map((fixture) => fixture.root)).size !== fixtures.length) {
  throw new Error("direct-source, Project baseline, and Project renamed roots must be distinct");
}
const [direct, baseline, renamed] = fixtures;
if (
  JSON.stringify(direct.manifest.scalar_abi) !== JSON.stringify(baseline.manifest.scalar_abi) ||
  JSON.stringify(baseline.manifest.scalar_abi) !== JSON.stringify(renamed.manifest.scalar_abi)
) {
  throw new Error("direct and Project calculator scalar ABIs are not exactly equal");
}
if (
  baseline.manifest.project_revision === renamed.manifest.project_revision ||
  baseline.manifest.workspace_revision === renamed.manifest.workspace_revision ||
  baseline.manifest.project_graph_digest === renamed.manifest.project_graph_digest
) {
  throw new Error("Project display rename did not change every revision-bound subject fact");
}
for (const artifact of expectedArtifacts) {
  const baselineBytes = readFileSync(resolve(baseline.root, "generated", artifact));
  const renamedBytes = readFileSync(resolve(renamed.root, "generated", artifact));
  if (!baselineBytes.equals(renamedBytes)) {
    throw new Error(`Project display rename changed stable-ID artifact ${artifact}`);
  }
}

const cli = fileURLToPath(new URL("./node_modules/@playwright/test/cli.js", import.meta.url));
const result = spawnSync(process.execPath, [cli, "test", "--workers=1", "--retries=0"], {
  cwd: fileURLToPath(new URL(".", import.meta.url)),
  env: {
    ...process.env,
    SEMAPRAX_CALCULATOR_FIXTURES: JSON.stringify(
      fixtures.map(({ name, root }) => ({ name, root })),
    ),
  },
  stdio: "inherit",
});
if (result.error) throw result.error;
if (result.signal) throw new Error(`Playwright terminated by ${result.signal}`);
process.exitCode = result.status ?? 1;
