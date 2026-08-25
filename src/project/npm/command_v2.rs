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
    let artifacts = command::render_package_with_metadata(
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
    let expected = command::render_package_with_metadata(
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
