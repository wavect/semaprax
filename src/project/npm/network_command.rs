//! Project-v12 fixture-driven Language Network I/O v1 npm carrier.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::ResolvedProgram;
use crate::project::{
    ProjectManifest, PROJECT_LANGUAGE_COMMAND_INPUT_V1, PROJECT_NETWORK_COMMAND_CAPABILITIES_V1,
    PROJECT_PROFILE_NETWORK_COMMAND_IO_V1, PROJECT_SCHEMA_V12,
};

use super::{
    artifact, package_error, payload_digest_artifacts_v11, render_carrier_artifacts,
    valid_package_name, valid_package_semver, valid_sha256_fact, NpmArtifact, NpmBuildIdentity,
    ProjectNpmBuild, PROJECT_NPM_BUILD_SCHEMA_V11,
};

pub(super) const PACKAGE_PATHS: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.network.json",
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
    if !manifest.is_v12()
        || manifest.profile() != Some(PROJECT_PROFILE_NETWORK_COMMAND_IO_V1)
        || manifest.command_input() != Some(PROJECT_LANGUAGE_COMMAND_INPUT_V1)
        || !manifest
            .capabilities()
            .iter()
            .map(String::as_str)
            .eq(PROJECT_NETWORK_COMMAND_CAPABILITIES_V1)
        || manifest.command() != manifest.web_exports().first().map(String::as_str)
        || manifest.web_exports().len() != 1
    {
        return Err(package_error(
            "network npm package requires the exact Project v12 profile",
        ));
    }
    let version = manifest
        .package_version()
        .ok_or_else(|| package_error("network npm package requires a version"))?;
    super::validate_carrier_limit(0, max_bytes)?;
    let command = manifest
        .command()
        .ok_or_else(|| package_error("network npm package requires a command"))?;
    let wasm = crate::wasm::emit_resolved_language_network_io_v1(program, command)?;
    let recipe = super::render_semantic_recipe(program)?;
    let artifacts = render_package(manifest.name(), version, command, &wasm);
    let artifact_bytes = artifacts.iter().try_fold(0usize, |total, item| {
        total
            .checked_add(item.bytes.len())
            .filter(|value| *value <= max_bytes)
            .ok_or_else(|| package_error("network npm artifacts exceed the trusted limit"))
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
    let payload_digest = payload_digest_artifacts_v11(identity, &artifacts);
    let envelope = render_carrier_artifacts(
        PROJECT_NPM_BUILD_SCHEMA_V11,
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

fn render_package(name: &str, version: &str, command: &str, wasm: &[u8]) -> [NpmArtifact; 6] {
    let wasm_sha256 = format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(wasm)));
    let runtime = include_str!("network_runtime.mjs")
        .replace("__SPX_HASH__", &quote_json(&wasm_sha256))
        .replace("__SPX_COMMAND__", &quote_json(&raw_symbol(command)));
    let bindings =
        "export { createFixture, createInvocation, instantiate } from './semaprax.js';\n";
    let declarations = "export interface NetworkFixtureConnection { readonly host: string; readonly port: number; readonly recv?: readonly string[]; readonly expect_send?: string; readonly ready?: boolean; }\nexport interface NetworkFixtureDocument { readonly schema: 'semaprax.network-fixture.v1'; readonly connections: readonly NetworkFixtureConnection[]; }\nexport interface NetworkCommandResult { readonly result: boolean; readonly stdout: Uint8Array; readonly stderr: Uint8Array; }\nexport declare function createFixture(document: NetworkFixtureDocument): object;\nexport declare function createInvocation(argv: readonly string[], stdin: Uint8Array, fixture: object): object;\nexport declare function instantiate(wasm: Uint8Array, invocation: object): Promise<NetworkCommandResult>;\n";
    let metadata = format!(
        "{{\"schema\":\"semaprax.network-command-io.v1\",\"package\":{},\"version\":{},\"command\":{},\"input\":\"argv-utf8+stdin-bytes.v1\",\"provider\":\"fixture-only.v1\",\"capabilities\":[\"network.connect\",\"network.read\",\"network.write\",\"process.args.read\",\"process.stderr.write\",\"process.stdin.read\",\"process.stdout.write\"],\"wasm\":{{\"path\":\"app.wasm\",\"sha256\":{}}}}}\n",
        quote_json(name), quote_json(version), quote_json(command), quote_json(&wasm_sha256)
    );
    let package = format!(
        "{{\"name\":{},\"version\":{},\"type\":\"module\",\"sideEffects\":false,\"exports\":{{\".\":{{\"types\":\"./semaprax.bindings.d.ts\",\"import\":\"./semaprax.bindings.js\"}},\"./app.wasm\":\"./app.wasm\",\"./manifest\":\"./semaprax.network.json\"}}}}\n",
        quote_json(name), quote_json(version)
    );
    [
        artifact("app.wasm", wasm),
        artifact("semaprax.js", runtime.as_bytes()),
        artifact("semaprax.bindings.js", bindings.as_bytes()),
        artifact("semaprax.bindings.d.ts", declarations.as_bytes()),
        artifact("semaprax.network.json", metadata.as_bytes()),
        artifact("package.json", package.as_bytes()),
    ]
}

fn raw_symbol(stable_id: &str) -> String {
    let mut symbol = String::from("spx_data_");
    for byte in stable_id.bytes() {
        use std::fmt::Write as _;
        write!(symbol, "{byte:02x}").expect("String writes are infallible");
    }
    symbol
}

pub(super) fn validate_replayed(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact; 6],
) -> Result<(), Diagnostic> {
    if identity.project_schema != PROJECT_SCHEMA_V12
        || !valid_package_name(identity.package)
        || !valid_package_semver(identity.version)
        || !valid_sha256_fact(identity.project_revision)
        || !valid_sha256_fact(identity.workspace_revision)
        || !valid_sha256_fact(identity.project_graph_digest)
        || artifacts.iter().map(|item| item.path).ne(PACKAGE_PATHS)
    {
        return Err(package_error("network npm replay identity is invalid"));
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(artifact_bytes(artifacts, "semaprax.network.json")?)
            .map_err(|_| package_error("network npm metadata is invalid"))?;
    let command = metadata
        .get("command")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error("network npm command identity is absent"))?;
    let ast = crate::parse(
        identity.semantic_recipe,
        Path::new("semaprax-project-npm-network-command-recipe.spx"),
    )
    .map_err(|_| package_error("network npm semantic recipe does not parse"))?;
    let program = crate::hir::resolve(&ast)
        .map_err(|_| package_error("network npm semantic recipe does not resolve"))?;
    let wasm = crate::wasm::emit_resolved_language_network_io_v1(&program, command)?;
    let expected = render_package(identity.package, identity.version, command, &wasm);
    if artifacts != &expected {
        return Err(package_error(
            "network npm artifacts disagree with semantic replay",
        ));
    }
    Ok(())
}

fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 6], path: &str) -> Result<&'a [u8], Diagnostic> {
    artifacts
        .iter()
        .find(|artifact| artifact.path == path)
        .map(NpmArtifact::bytes)
        .ok_or_else(|| package_error(format!("network npm artifact `{path}` is absent")))
}
