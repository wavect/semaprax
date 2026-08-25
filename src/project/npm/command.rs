//! Exact compiler-free Node command product for Project v4.

use std::path::Path;

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{OwnershipMode, ResolvedProgram, ResolvedType};
use crate::project::{
    ProjectManifest, PROJECT_COMMAND_STDOUT_CAPABILITY, PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1,
    PROJECT_SCHEMA_V4,
};

use super::{
    artifact, data, package_error, payload_digest_artifacts_v3, render_carrier_artifacts,
    valid_package_name, valid_package_semver, valid_sha256_fact, NpmArtifact, NpmBuildIdentity,
    ProjectNpmBuild, PROJECT_NPM_BUILD_SCHEMA_V3,
};

pub(super) const USEFUL_DATA_COMMAND_PACKAGE_PATHS: [&str; 7] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.command.json",
    "semaprax.command.js",
    "package.json",
];

pub(super) fn prepare(
    manifest: &ProjectManifest,
    program: &ResolvedProgram,
    project_revision: &str,
    workspace_revision: &str,
    project_graph_digest: &str,
    max_bytes: usize,
) -> Result<ProjectNpmBuild, Diagnostic> {
    let version = require_profile(manifest)?;
    super::validate_carrier_limit(0, max_bytes)?;
    let command = validate_command(manifest, program)?;
    let exports = data::derive_exports(program, manifest.web_exports())?;
    let wasm = crate::wasm::emit_resolved_module_with_byte_exports_and_stdout_transcript(
        program,
        manifest.web_exports(),
    )?;
    let recipe = super::render_semantic_recipe(program)?;
    let artifacts = render_package(manifest.name(), version, command, &wasm, &exports)?;
    let artifact_bytes = artifacts.iter().try_fold(0_usize, |total, item| {
        total
            .checked_add(item.bytes.len())
            .filter(|value| *value <= max_bytes)
            .ok_or_else(|| package_error("npm build artifacts exceed the trusted limit"))
    })?;
    let identity = NpmBuildIdentity {
        project_schema: manifest.schema(),
        package: manifest.name(),
        version,
        project_revision,
        workspace_revision,
        project_graph_digest,
        semantic_recipe: &recipe,
    };
    let payload_digest = payload_digest_artifacts_v3(identity, &artifacts);
    let envelope = render_carrier_artifacts(
        PROJECT_NPM_BUILD_SCHEMA_V3,
        identity,
        &artifacts,
        artifact_bytes,
        &payload_digest,
    );
    super::validate_carrier_limit(envelope.len(), max_bytes)?;
    let build = ProjectNpmBuild {
        envelope,
        payload_digest,
        artifact_bytes,
        max_bytes,
        trusted: super::trusted_binding(identity),
    };
    build.verify()?;
    Ok(build)
}

fn require_profile(manifest: &ProjectManifest) -> Result<&str, Diagnostic> {
    if !manifest.is_v4()
        || manifest.profile() != Some(PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1)
        || manifest.capabilities() != [PROJECT_COMMAND_STDOUT_CAPABILITY]
    {
        return Err(package_error(
            "npm command facade requires the useful-data-command.v1 Project v4 profile",
        ));
    }
    manifest
        .package_version()
        .ok_or_else(|| package_error("npm command facade requires a package version"))
}

pub(super) fn validate_command<'a>(
    manifest: &'a ProjectManifest,
    program: &ResolvedProgram,
) -> Result<&'a str, Diagnostic> {
    let command = manifest
        .command()
        .ok_or_else(|| package_error("npm command facade requires one command stable ID"))?;
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == command)
        .ok_or_else(|| package_error(format!("selected npm command `{command}` is absent")))?;
    let exact_parameters = function.params.len() == 2
        && function.params.iter().all(|parameter| {
            parameter.ty == ResolvedType::SliceU8 && parameter.ownership == OwnershipMode::Borrow
        });
    if !exact_parameters || function.return_type != ResolvedType::Bool {
        return Err(package_error(
            "selected npm command must have signature (borrow Slice<u8>, borrow Slice<u8>) -> bool",
        ));
    }
    for selected in manifest.web_exports() {
        let selected_function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == selected)
            .ok_or_else(|| package_error(format!("selected npm export `{selected}` is absent")))?;
        let expected: &[&str] = if selected == command {
            &[PROJECT_COMMAND_STDOUT_CAPABILITY]
        } else {
            &[]
        };
        if selected_function
            .effects
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != expected
        {
            return Err(package_error(
                "npm command Web closure effects exceed the exact stdout capability",
            ));
        }
    }
    Ok(command)
}

fn render_package(
    name: &str,
    version: &str,
    command: &str,
    wasm: &[u8],
    exports: &[data::DataExport],
) -> Result<[NpmArtifact; 7], Diagnostic> {
    if wasm.is_empty() || wasm.len() > 16 * 1024 * 1024 {
        return Err(package_error("npm command facade Wasm is not bounded"));
    }
    let digest = data::hex_sha256(wasm);
    let metadata = render_metadata(name, version, command, &digest);
    render_package_with_metadata(name, version, command, wasm, exports, metadata.as_bytes())
}

/// Render the frozen seven-file command facade around caller-authenticated
/// canonical metadata. Project v5 reuses the exact executable facade while
/// binding its wider fixed-adapter authority in a distinct metadata schema.
pub(super) fn render_package_with_metadata(
    name: &str,
    version: &str,
    command: &str,
    wasm: &[u8],
    exports: &[data::DataExport],
    metadata: &[u8],
) -> Result<[NpmArtifact; 7], Diagnostic> {
    if wasm.is_empty() || wasm.len() > 16 * 1024 * 1024 {
        return Err(package_error("npm command facade Wasm is not bounded"));
    }
    let digest = data::hex_sha256(wasm);
    let runtime = render_runtime(&digest)?;
    let bindings = render_bindings(exports, &digest)?;
    let declarations = data::render_declarations(exports);
    if exports.len() != 1 || exports[0].stable_id != command {
        return Err(package_error(
            "npm command facade requires exactly its one command export",
        ));
    }
    let adapter = render_command_adapter(command);
    let package = render_package_json(name, version);
    Ok([
        artifact("app.wasm", wasm),
        artifact("semaprax.js", runtime.as_bytes()),
        artifact("semaprax.bindings.js", bindings.as_bytes()),
        artifact("semaprax.bindings.d.ts", declarations.as_bytes()),
        artifact("semaprax.command.json", metadata),
        artifact("semaprax.command.js", adapter.as_bytes()),
        artifact("package.json", package.as_bytes()),
    ])
}

fn replace_once(source: String, from: &str, to: &str) -> Result<String, Diagnostic> {
    if source.matches(from).count() != 1 {
        return Err(package_error(
            "npm command runtime template binding drifted",
        ));
    }
    Ok(source.replacen(from, to, 1))
}

fn render_runtime(wasm_sha256: &str) -> Result<String, Diagnostic> {
    let runtime = data::render_runtime(wasm_sha256);
    replace_once(
        runtime,
        "candidate.buffer.byteLength !== 131072",
        "candidate.buffer.byteLength !== 196608",
    )
}

fn render_bindings(exports: &[data::DataExport], wasm_sha256: &str) -> Result<String, Diagnostic> {
    let bindings = replace_once(
        data::render_bindings(exports, wasm_sha256),
        "e.memory.buffer.byteLength !== 131072",
        "e.memory.buffer.byteLength !== 196608",
    )?;
    let bindings = replace_once(
        bindings,
        "let busy = false, poisoned = false;",
        "let busy = false, poisoned = false, transcriptAvailable = false;\n  function discardTranscript() { new Uint8Array(e.memory.buffer).fill(0, 131072, 196608); transcriptAvailable = false; }\n  function takeTranscript() {\n    if (!transcriptAvailable) throw new Error(\"SEMAPRAX stdout transcript was not sealed\");\n    const length = globalNumber(e.__spx_stdout_length_v1, \"stdout length\"), transcriptBase = globalNumber(e.__spx_stdout_base_v1, \"stdout base\"), transcriptCapacity = globalNumber(e.__spx_stdout_capacity_v1, \"stdout capacity\");\n    if (transcriptBase !== 131072 || transcriptCapacity !== 65536 || length > transcriptCapacity) { discardTranscript(); throw new Error(\"SEMAPRAX stdout transcript metadata is invalid\"); }\n    const memory = new Uint8Array(e.memory.buffer), value = memory.slice(transcriptBase, transcriptBase + length); discardTranscript(); return value;\n  }",
    )?;
    let bindings = replace_once(
        bindings,
        "if (busy) throw new SemapraxDataError(12);",
        "if (busy) throw new SemapraxDataError(12); discardTranscript();",
    )?;
    let bindings = replace_once(
        bindings,
        "let primaryError = null, began = false, memory = null;",
        "let primaryError = null, resultValue, succeeded = false, began = false, memory = null;",
    )?;
    let bindings = replace_once(
        bindings,
        "else return scalarResult(value, fact.result);",
        "else { resultValue = scalarResult(value, fact.result); succeeded = true; }",
    )?;
    let bindings = replace_once(
        bindings,
        "if (settlementError !== null) throw settlementError;",
        "if (settlementError !== null) { discardTranscript(); throw settlementError; }",
    )?;
    let bindings = replace_once(
        bindings,
        "throw primaryError;",
        "if (primaryError !== null) { discardTranscript(); throw primaryError; } if (!succeeded) { discardTranscript(); throw new Error(\"SEMAPRAX command result was not settled\"); } transcriptAvailable = true; return resultValue;",
    )?;
    replace_once(
        bindings,
        "return Object.freeze({ functions: Object.freeze(functions), call: (id, ...values) => invoke(id, values), wasmSha256: EXPECTED_WASM_SHA256 });",
        "return Object.freeze({ functions: Object.freeze(functions), call: (id, ...values) => invoke(id, values), takeTranscript, wasmSha256: EXPECTED_WASM_SHA256 });",
    )
}

fn render_metadata(name: &str, version: &str, command: &str, wasm_sha256: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.useful-data-command.v1\",\"package\":{},\"version\":{},\"command\":{},\"capabilities\":[\"process.stdout.write\"],\"input\":{{\"stdin\":\"slice-u8\",\"argv\":[\"utf8-slice-u8\"],\"cumulative_max_bytes\":65536}},\"result\":\"bool\",\"wasm\":{{\"path\":\"app.wasm\",\"sha256\":{}}}}}\n",
        quote_json(name), quote_json(version), quote_json(command), quote_json(wasm_sha256)
    )
}

fn render_command_adapter(command: &str) -> String {
    format!(
        r#"#!/usr/bin/env node
import {{ readFile }} from "node:fs/promises";
import {{ stdin, stdout, stderr, argv }} from "node:process";
import {{ fileURLToPath }} from "node:url";
import {{ instantiate }} from "./semaprax.bindings.js";
const flush = (stream, bytes) => new Promise((resolve, reject) => stream.write(bytes, error => error ? reject(error) : resolve()));
const fail = async () => {{ try {{ await flush(stderr, "spxgrep: command failed\n"); }} catch {{}} finally {{ process.exitCode = 2; }} }};
try {{
  if (argv.length !== 3) throw new Error("usage");
  const storage = new Uint8Array(65536), encoder = new TextEncoder();
  const encoded = encoder.encodeInto(argv[2], storage);
  if (encoded.read !== argv[2].length) throw new Error("argument exceeds bound");
  const needle = storage.subarray(0, encoded.written); let used = encoded.written;
  for await (const chunk of stdin) {{
    if (!(chunk instanceof Uint8Array) || chunk.byteLength > 65536 - used) throw new Error("stdin exceeds bound");
    storage.set(chunk, used); used += chunk.byteLength;
  }}
  const input = storage.subarray(encoded.written, used);
  const wasm = new Uint8Array(await readFile(fileURLToPath(new URL("./app.wasm", import.meta.url))));
  const runtime = await instantiate(wasm);
  const matched = runtime.call({command}, input, needle);
  const transcript = runtime.takeTranscript();
  await flush(stdout, transcript); process.exitCode = matched ? 0 : 1;
}} catch {{ await fail(); }}
"#,
        command = quote_json(command),
    )
}

fn render_package_json(name: &str, version: &str) -> String {
    format!(
        "{{\"name\":{},\"version\":{},\"type\":\"module\",\"sideEffects\":false,\"bin\":{{\"spxgrep\":\"./semaprax.command.js\"}},\"exports\":{{\".\":{{\"types\":\"./semaprax.bindings.d.ts\",\"import\":\"./semaprax.bindings.js\"}},\"./app.wasm\":\"./app.wasm\",\"./manifest\":\"./semaprax.command.json\"}},\"types\":\"./semaprax.bindings.d.ts\",\"files\":[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.command.json\",\"semaprax.command.js\"],\"engines\":{{\"node\":\">=22\"}}}}\n",
        quote_json(name), quote_json(version)
    )
}

pub(super) fn validate_replayed(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact; 7],
) -> Result<(), Diagnostic> {
    if identity.project_schema != PROJECT_SCHEMA_V4
        || !valid_package_name(identity.package)
        || !valid_package_semver(identity.version)
        || !valid_sha256_fact(identity.project_revision)
        || !valid_sha256_fact(identity.workspace_revision)
        || !valid_sha256_fact(identity.project_graph_digest)
    {
        return Err(package_error(
            "npm command build identity facts are not canonical",
        ));
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(artifact_bytes(artifacts, "semaprax.command.json")?)
            .map_err(|_| package_error("npm command metadata is not valid JSON"))?;
    let command = metadata
        .get("command")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error("npm command metadata command is invalid"))?;
    let ast = crate::parse(
        identity.semantic_recipe,
        Path::new("semaprax-project-npm-command-recipe.spx"),
    )
    .map_err(|_| package_error("npm command semantic recipe does not parse"))?;
    let program = crate::hir::resolve(&ast)
        .map_err(|_| package_error("npm command semantic recipe does not resolve"))?;
    let selected = metadata_selected_exports(&metadata)?;
    let manifest = replay_manifest(identity, command, &selected)?;
    validate_command(&manifest, &program)?;
    let exports = data::derive_exports(&program, &selected)?;
    let wasm = crate::wasm::emit_resolved_module_with_byte_exports_and_stdout_transcript(
        &program, &selected,
    )?;
    if artifact_bytes(artifacts, "app.wasm")? != wasm {
        return Err(package_error(
            "npm command app.wasm disagrees with semantic replay",
        ));
    }
    let expected = render_package(identity.package, identity.version, command, &wasm, &exports)?;
    if artifacts != &expected {
        return Err(package_error(
            "npm command artifacts disagree with semantic replay",
        ));
    }
    Ok(())
}

fn metadata_selected_exports(value: &serde_json::Value) -> Result<Vec<String>, Diagnostic> {
    let command = value
        .get("command")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error("npm command metadata command is invalid"))?;
    Ok(vec![command.to_owned()])
}

fn replay_manifest(
    identity: NpmBuildIdentity<'_>,
    command: &str,
    selected: &[String],
) -> Result<ProjectManifest, Diagnostic> {
    let source = format!(
        "schema = \"semaprax.project.v4\"\nname = {}\nversion = {}\nprofile = \"useful-data-command.v1\"\nentry = \"replay.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [{}]\ncommand = {}\ncapabilities = [\"process.stdout.write\"]\ntests = [\"replay.tests\"]\n",
        quote_json(identity.package), quote_json(identity.version), selected.iter().map(|id| quote_json(id)).collect::<Vec<_>>().join(", "), quote_json(command)
    );
    ProjectManifest::parse(&source)
        .map_err(|_| package_error("npm command metadata identity is invalid"))
}

fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 7], path: &str) -> Result<&'a [u8], Diagnostic> {
    artifacts
        .iter()
        .find(|artifact| artifact.path == path)
        .map(NpmArtifact::bytes)
        .ok_or_else(|| package_error(format!("npm command artifact `{path}` is absent")))
}
