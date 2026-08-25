//! Project-v5 npm command package and independent carrier replay.

use std::path::Path;

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::ResolvedProgram;
use crate::project::{
    ProjectManifest, PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2, PROJECT_COMMAND_INPUT_V1,
    PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2, PROJECT_SCHEMA_V5,
};

use super::{
    command, data, package_error, payload_digest_artifacts_v4, render_carrier_artifacts,
    valid_package_name, valid_package_semver, valid_sha256_fact, NpmArtifact, NpmBuildIdentity,
    ProjectNpmBuild, PROJECT_NPM_BUILD_SCHEMA_V4,
};

pub(super) const USEFUL_DATA_COMMAND_V2_PACKAGE_PATHS: [&str; 7] =
    command::USEFUL_DATA_COMMAND_PACKAGE_PATHS;

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
    let command_id = command::validate_command(manifest, program)?;
    let exports = data::derive_exports(program, manifest.web_exports())?;
    let wasm = crate::wasm::emit_resolved_useful_data_command_v2(program, command_id)?;
    let recipe = super::render_semantic_recipe(program)?;
    let metadata = render_metadata(
        manifest.name(),
        version,
        command_id,
        &data::hex_sha256(&wasm),
    );
    let artifacts = render_package(
        manifest.name(),
        version,
        command_id,
        &wasm,
        &exports,
        metadata.as_bytes(),
    )?;
    let artifact_bytes = artifacts.iter().try_fold(0_usize, |total, item| {
        total
            .checked_add(item.bytes.len())
            .filter(|value| *value <= max_bytes)
            .ok_or_else(|| package_error("npm command v2 artifacts exceed the trusted limit"))
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
    let payload_digest = payload_digest_artifacts_v4(identity, &artifacts);
    let envelope = render_carrier_artifacts(
        PROJECT_NPM_BUILD_SCHEMA_V4,
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
    if !manifest.is_v5()
        || manifest.profile() != Some(PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2)
        || manifest.command_input() != Some(PROJECT_COMMAND_INPUT_V1)
        || !manifest
            .capabilities()
            .iter()
            .map(String::as_str)
            .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2)
    {
        return Err(package_error(
            "npm command v2 facade requires the useful-data-command.v2 Project v5 profile",
        ));
    }
    manifest
        .package_version()
        .ok_or_else(|| package_error("npm command v2 facade requires a package version"))
}

fn render_metadata(name: &str, version: &str, command: &str, wasm_sha256: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.useful-data-command.v2\",\"package\":{},\"version\":{},\"command\":{},\"input\":\"stdin-bytes+one-utf8-arg.v1\",\"capabilities\":[\"process.args.read\",\"process.stderr.write\",\"process.stdin.read\",\"process.stdout.write\"],\"stdout_transcript\":{{\"policy\":\"success-only.v1\",\"max_bytes\":65536,\"max_writes_per_path\":1}},\"result\":\"bool\",\"exits\":{{\"matched\":0,\"not_matched\":1,\"adapter_failure\":2}},\"wasm\":{{\"path\":\"app.wasm\",\"sha256\":{}}}}}\n",
        quote_json(name),
        quote_json(version),
        quote_json(command),
        quote_json(wasm_sha256),
    )
}

/// Render the v5-only Node facade. The frozen v4 renderer supplies the common
/// runtime, declarations, and package bytes, then this closed profile replaces
/// only the two authority-sensitive JavaScript leaves.
fn render_package(
    name: &str,
    version: &str,
    command_id: &str,
    wasm: &[u8],
    exports: &[data::DataExport],
    metadata: &[u8],
) -> Result<[NpmArtifact; 7], Diagnostic> {
    let mut artifacts =
        command::render_package_with_metadata(name, version, command_id, wasm, exports, metadata)?;
    let bindings = artifact_text(&artifacts, "semaprax.bindings.js")?;
    let bindings = replace_once(
        bindings,
        "takeTranscript, wasmSha256: EXPECTED_WASM_SHA256",
        "takeTranscript, discardTranscript, wasmSha256: EXPECTED_WASM_SHA256",
    )?;
    replace_artifact(
        &mut artifacts,
        "semaprax.bindings.js",
        bindings.into_bytes(),
    )?;
    replace_artifact(
        &mut artifacts,
        "semaprax.command.js",
        render_command_adapter(command_id).into_bytes(),
    )?;
    Ok(artifacts)
}

fn replace_once(source: String, from: &str, to: &str) -> Result<String, Diagnostic> {
    if source.matches(from).count() != 1 {
        return Err(package_error(
            "npm command v2 facade template binding drifted",
        ));
    }
    Ok(source.replacen(from, to, 1))
}

fn artifact_text(artifacts: &[NpmArtifact; 7], path: &str) -> Result<String, Diagnostic> {
    String::from_utf8(artifact_bytes(artifacts, path)?.to_vec())
        .map_err(|_| package_error(format!("npm command v2 artifact `{path}` is not UTF-8")))
}

fn replace_artifact(
    artifacts: &mut [NpmArtifact; 7],
    path: &str,
    bytes: Vec<u8>,
) -> Result<(), Diagnostic> {
    let artifact = artifacts
        .iter_mut()
        .find(|artifact| artifact.path == path)
        .ok_or_else(|| package_error(format!("npm command v2 artifact `{path}` is absent")))?;
    artifact.bytes = bytes;
    Ok(())
}

fn render_command_adapter(command: &str) -> String {
    format!(
        r#"#!/usr/bin/env node
import {{ readFile }} from "node:fs/promises";
import {{ stdin, stdout, stderr, argv }} from "node:process";
import {{ fileURLToPath }} from "node:url";
import {{ instantiate }} from "./semaprax.bindings.js";
const flush = (stream, bytes) => new Promise((resolve, reject) => {{
  let settled = false;
  const settle = error => {{ if (settled) return; settled = true; error ? reject(error) : resolve(); }};
  const onError = error => settle(error);
  stream.once("error", onError);
  try {{
    stream.write(bytes, error => {{
      if (error) {{ settle(error); setImmediate(() => stream.off("error", onError)); }}
      else {{ stream.off("error", onError); settle(); }}
    }});
  }} catch (error) {{ stream.off("error", onError); settle(error); }}
}});
const fail = async () => {{ try {{ await flush(stderr, "spxgrep: command failed\n"); }} catch {{}} finally {{ process.exitCode = 2; }} }};
const rejectLoneSurrogate = value => {{
  for (let index = 0; index < value.length; index += 1) {{
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {{
      if (index + 1 >= value.length) throw new Error("argument contains an unpaired surrogate");
      const tail = value.charCodeAt(index + 1);
      if (tail < 0xdc00 || tail > 0xdfff) throw new Error("argument contains an unpaired surrogate");
      index += 1;
    }} else if (unit >= 0xdc00 && unit <= 0xdfff) {{
      throw new Error("argument contains an unpaired surrogate");
    }}
  }}
}};
try {{
  if (argv.length !== 3) throw new Error("usage");
  rejectLoneSurrogate(argv[2]);
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
  if (!matched) {{ runtime.discardTranscript(); process.exitCode = 1; }}
  else {{ const transcript = runtime.takeTranscript(); await flush(stdout, transcript); process.exitCode = 0; }}
}} catch {{ await fail(); }}
"#,
        command = quote_json(command),
    )
}

pub(super) fn validate_replayed(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact; 7],
) -> Result<(), Diagnostic> {
    if identity.project_schema != PROJECT_SCHEMA_V5
        || !valid_package_name(identity.package)
        || !valid_package_semver(identity.version)
        || !valid_sha256_fact(identity.project_revision)
        || !valid_sha256_fact(identity.workspace_revision)
        || !valid_sha256_fact(identity.project_graph_digest)
    {
        return Err(package_error(
            "npm command v2 build identity facts are not canonical",
        ));
    }
    let metadata_bytes = artifact_bytes(artifacts, "semaprax.command.json")?;
    let metadata: serde_json::Value = serde_json::from_slice(metadata_bytes)
        .map_err(|_| package_error("npm command v2 metadata is not valid JSON"))?;
    let object = metadata
        .as_object()
        .ok_or_else(|| package_error("npm command v2 metadata must be one JSON object"))?;
    super::require_exact_keys(
        object,
        &[
            "schema",
            "package",
            "version",
            "command",
            "input",
            "capabilities",
            "stdout_transcript",
            "result",
            "exits",
            "wasm",
        ],
    )?;
    let command_id = object
        .get("command")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error("npm command v2 metadata command is invalid"))?;
    let ast = crate::parse(
        identity.semantic_recipe,
        Path::new("semaprax-project-npm-command-v2-recipe.spx"),
    )
    .map_err(|_| package_error("npm command v2 semantic recipe does not parse"))?;
    let program = crate::hir::resolve(&ast)
        .map_err(|_| package_error("npm command v2 semantic recipe does not resolve"))?;
    let manifest = replay_manifest(identity, command_id)?;
    command::validate_command(&manifest, &program)?;
    let selected = vec![command_id.to_owned()];
    let exports = data::derive_exports(&program, &selected)?;
    let wasm = crate::wasm::emit_resolved_useful_data_command_v2(&program, command_id)?;
    if artifact_bytes(artifacts, "app.wasm")? != wasm {
        return Err(package_error(
            "npm command v2 app.wasm disagrees with semantic replay",
        ));
    }
    let expected_metadata = render_metadata(
        identity.package,
        identity.version,
        command_id,
        &data::hex_sha256(&wasm),
    );
    let expected = render_package(
        identity.package,
        identity.version,
        command_id,
        &wasm,
        &exports,
        expected_metadata.as_bytes(),
    )?;
    if artifacts != &expected {
        return Err(package_error(
            "npm command v2 artifacts disagree with semantic replay",
        ));
    }
    Ok(())
}

fn replay_manifest(
    identity: NpmBuildIdentity<'_>,
    command: &str,
) -> Result<ProjectManifest, Diagnostic> {
    let source = format!(
        "schema = \"semaprax.project.v5\"\nname = {}\nversion = {}\nprofile = \"useful-data-command.v2\"\nentry = \"replay.app\"\nsources = [\"a.spx\", \"b.spx\"]\nweb_exports = [{}]\ncommand = {}\ninput = \"stdin-bytes+one-utf8-arg.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"replay.tests\"]\n",
        quote_json(identity.package),
        quote_json(identity.version),
        quote_json(command),
        quote_json(command),
    );
    ProjectManifest::parse(&source)
        .map_err(|_| package_error("npm command v2 metadata identity is invalid"))
}

fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 7], path: &str) -> Result<&'a [u8], Diagnostic> {
    artifacts
        .iter()
        .find(|artifact| artifact.path == path)
        .map(NpmArtifact::bytes)
        .ok_or_else(|| package_error(format!("npm command v2 artifact `{path}` is absent")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_v2_is_one_exact_canonical_line() {
        assert_eq!(
            render_metadata(
                "spxgrep",
                "0.1.0",
                "spxgrep.contains",
                "0123456789abcdef"
            ),
            "{\"schema\":\"semaprax.useful-data-command.v2\",\"package\":\"spxgrep\",\"version\":\"0.1.0\",\"command\":\"spxgrep.contains\",\"input\":\"stdin-bytes+one-utf8-arg.v1\",\"capabilities\":[\"process.args.read\",\"process.stderr.write\",\"process.stdin.read\",\"process.stdout.write\"],\"stdout_transcript\":{\"policy\":\"success-only.v1\",\"max_bytes\":65536,\"max_writes_per_path\":1},\"result\":\"bool\",\"exits\":{\"matched\":0,\"not_matched\":1,\"adapter_failure\":2},\"wasm\":{\"path\":\"app.wasm\",\"sha256\":\"0123456789abcdef\"}}\n"
        );
    }
}
