//! Project-v11 npm projection for bounded nested owned-record results.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::project::{
    NestedOwnedRecordApiDescriptor, NestedOwnedRecordFieldType, NestedOwnedRecordLeafType,
    PublicApiParameterType, PublicApiSubject, NESTED_OWNED_RECORD_PROJECT_SCHEMA,
};

use super::carrier::{
    artifact, payload_digest_artifacts_v10, render_carrier_artifacts, trusted_binding, NpmArtifact,
    NpmBuildIdentity,
};
use super::{package_error, validate_carrier_limit, ProjectNpmBuild};

pub const PACKAGE_PATHS: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.api.json",
    "package.json",
];

pub(super) fn prepare(
    program: &crate::hir::ResolvedProgram,
    descriptor: &NestedOwnedRecordApiDescriptor,
    package: &str,
    version: &str,
    max_bytes: usize,
) -> Result<ProjectNpmBuild, Diagnostic> {
    validate_carrier_limit(0, max_bytes)?;
    super::owned_data::validate_identity(package, version)?;
    let selected = descriptor
        .exports()
        .iter()
        .map(|e| e.stable_id().as_str().to_owned())
        .collect::<Vec<_>>();
    let subject = subject(descriptor);
    let replayed = crate::project::replay_nested_owned_record_api_descriptor(
        program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )?;
    if replayed != *descriptor {
        return Err(package_error("nested record descriptor replay disagrees"));
    }
    let recipe = super::render_owned_data_semantic_recipe(program)?;
    let replayed_program = super::semantic_recipe_v8::replay_against(program, &recipe)?;
    let replayed_descriptor = crate::project::replay_nested_owned_record_api_descriptor(
        &replayed_program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )?;
    let wasm =
        crate::wasm::emit_resolved_module_with_nested_owned_record_exports(program, descriptor)?;
    if wasm.is_empty()
        || wasm.len() > super::owned_data::MAX_WASM_BYTES
        || crate::wasm::emit_resolved_module_with_nested_owned_record_exports(
            &replayed_program,
            &replayed_descriptor,
        )? != wasm
    {
        return Err(package_error(
            "nested record Wasm disagrees with semantic replay or exceeds limit",
        ));
    }
    let roots = descriptor
        .exports()
        .iter()
        .map(|e| e.stable_id().clone())
        .collect::<Vec<_>>();
    let capacity = crate::wasm::owned_arena_capacity(program, &roots)?;
    let artifacts = render_package(package, version, descriptor, &wasm, capacity)?;
    let artifact_bytes = artifacts.iter().try_fold(0usize, |sum, artifact| {
        sum.checked_add(artifact.bytes().len())
            .filter(|n| *n <= max_bytes)
            .ok_or_else(|| package_error("nested record npm artifacts exceed limit"))
    })?;
    let identity = NpmBuildIdentity {
        project_schema: NESTED_OWNED_RECORD_PROJECT_SCHEMA,
        package,
        version,
        project_revision: descriptor.project_revision(),
        workspace_revision: descriptor.workspace_revision(),
        project_graph_digest: descriptor.project_graph_digest(),
        semantic_recipe: &recipe,
    };
    let payload_digest = payload_digest_artifacts_v10(identity, &artifacts);
    let envelope = render_carrier_artifacts(
        super::PROJECT_NPM_BUILD_SCHEMA_V10,
        identity,
        &artifacts,
        artifact_bytes,
        &payload_digest,
    );
    validate_carrier_limit(envelope.len(), max_bytes)?;
    let build = ProjectNpmBuild {
        envelope,
        payload_digest,
        artifact_bytes,
        max_bytes,
        trusted: trusted_binding(identity),
    };
    build.verify()?;
    Ok(build)
}

pub(super) fn validate_replayed(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact; 6],
) -> Result<(), Diagnostic> {
    if identity.project_schema != NESTED_OWNED_RECORD_PROJECT_SCHEMA {
        return Err(package_error("nested record carrier schema disagrees"));
    }
    let metadata = artifact_bytes(artifacts, "semaprax.api.json")?;
    let value: serde_json::Value = serde_json::from_slice(metadata)
        .map_err(|_| package_error("nested record metadata is invalid"))?;
    let descriptor_bytes = value
        .get("descriptor")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error("nested record descriptor is absent"))?
        .as_bytes();
    let digest = value
        .get("descriptor_digest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error("nested record digest is absent"))?;
    let selected = serde_json::from_slice::<serde_json::Value>(descriptor_bytes)
        .ok()
        .and_then(|v| v.get("exports")?.as_array().cloned())
        .ok_or_else(|| package_error("nested record exports are invalid"))?
        .into_iter()
        .map(|row| {
            row.get("stable_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| package_error("nested record export is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let program = super::semantic_recipe_v8::replay(identity.semantic_recipe)?;
    let subject = PublicApiSubject {
        project_schema: identity.project_schema,
        project_revision: identity.project_revision,
        workspace_revision: identity.workspace_revision,
        project_graph_digest: identity.project_graph_digest,
    };
    let descriptor = crate::project::replay_nested_owned_record_api_descriptor(
        &program,
        &selected,
        subject,
        descriptor_bytes,
        digest,
    )?;
    let wasm = artifact_bytes(artifacts, "app.wasm")?;
    wasmparser::Validator::new()
        .validate_all(wasm)
        .map_err(|_| package_error("nested record Wasm is invalid"))?;
    let roots = descriptor
        .exports()
        .iter()
        .map(|e| e.stable_id().clone())
        .collect::<Vec<_>>();
    let capacity = crate::wasm::owned_arena_capacity(&program, &roots)?;
    if crate::wasm::emit_resolved_module_with_nested_owned_record_exports(&program, &descriptor)?
        != wasm
        || render_package(
            identity.package,
            identity.version,
            &descriptor,
            wasm,
            capacity,
        )? != *artifacts
    {
        return Err(package_error("nested record package replay disagrees"));
    }
    Ok(())
}

fn render_package(
    package: &str,
    version: &str,
    descriptor: &NestedOwnedRecordApiDescriptor,
    wasm: &[u8],
    capacity: u32,
) -> Result<[NpmArtifact; 6], Diagnostic> {
    let digest = format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(wasm))
    );
    let facts = render_facts(descriptor)?;
    let mut runtime =
        render_runtime_prelude(digest.strip_prefix("sha256:").unwrap_or(&digest), capacity);
    runtime.push_str(include_str!("owned_nested_invocation/result.js"));
    runtime.push_str(include_str!("owned_nested_invocation/call.js"));
    runtime.push_str(
        &include_str!("owned_nested_invocation/facade.js")
            .replace("__SPX_MEMORY_BYTES__", "131072")
            .replace("__SPX_FACTS__", &facts),
    );
    let declarations = render_typescript(descriptor)?;
    let metadata = format!("{{\"schema\":\"semaprax.nested-owned-record-api.v1\",\"package\":{},\"version\":{},\"descriptor\":{},\"descriptor_digest\":{},\"wasm\":{{\"path\":\"app.wasm\",\"sha256\":{}}},\"artifacts\":[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.api.json\",\"package.json\"]}}\n", quote_json(package), quote_json(version), quote_json(&String::from_utf8(descriptor.canonical_bytes()).expect("canonical JSON is UTF-8")), quote_json(&descriptor.digest()), quote_json(&digest));
    let package_json = format!("{{\"name\":{},\"version\":{},\"type\":\"module\",\"sideEffects\":false,\"exports\":{{\".\":{{\"types\":\"./semaprax.bindings.d.ts\",\"import\":\"./semaprax.bindings.js\"}},\"./app.wasm\":\"./app.wasm\",\"./manifest\":\"./semaprax.api.json\"}},\"types\":\"./semaprax.bindings.d.ts\",\"files\":[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.api.json\"]}}\n", quote_json(package), quote_json(version));
    Ok([
        artifact("app.wasm", wasm),
        artifact("semaprax.js", runtime.as_bytes()),
        artifact(
            "semaprax.bindings.js",
            b"export { instantiate, exportIds, wasmSha256, default } from \"./semaprax.js\";\n",
        ),
        artifact("semaprax.bindings.d.ts", declarations.as_bytes()),
        artifact("semaprax.api.json", metadata.as_bytes()),
        artifact("package.json", package_json.as_bytes()),
    ])
}

fn render_runtime_prelude(wasm_digest: &str, capacity: u32) -> String {
    format!(
        "const EXPECTED_WASM_SHA256 = {};\n{}{}{}",
        quote_json(wasm_digest),
        include_str!("owned_data_input_v8.js"),
        include_str!("owned_nested_invocation/arena.js")
            .replace("__SPX_CAPACITY__", &capacity.to_string()),
        include_str!("owned_invocation/core.js"),
    )
}

fn render_facts(descriptor: &NestedOwnedRecordApiDescriptor) -> Result<String, Diagnostic> {
    let names = descriptor
        .records()
        .iter()
        .flat_map(|record| {
            record
                .fields()
                .iter()
                .map(|field| (field.stable_id().clone(), field.host_name()))
        })
        .collect::<BTreeMap<_, _>>();
    descriptor.exports().iter().map(|export| {
        let params = export.parameters().iter().map(|(_,_,ty)| quote_json(parameter_wire(*ty))).collect::<Vec<_>>().join(",");
        let leaves = export.leaves().iter().map(|leaf| {
            let path = leaf.field_path().iter().map(|id| names.get(id).map(|name| quote_json(name)).ok_or_else(|| package_error("nested record leaf path is absent from descriptor records"))).collect::<Result<Vec<_>, _>>()?.join(",");
            Ok(format!("Object.freeze({{path:Object.freeze([{path}]),kind:{},offset:{}}})", quote_json(leaf_wire(leaf.ty())), leaf.ordinal()*8))
        }).collect::<Result<Vec<_>, Diagnostic>>()?.join(",");
        Ok(format!("[{},Object.freeze({{raw:{},params:Object.freeze([{params}]),leaves:Object.freeze([{leaves}]),size:{}}})]", quote_json(export.typescript_name()), quote_json(&raw_symbol(export.stable_id().as_str())), export.leaves().len()*8))
    }).collect::<Result<Vec<_>, Diagnostic>>().map(|rows| rows.join(","))
}

fn render_typescript(descriptor: &NestedOwnedRecordApiDescriptor) -> Result<String, Diagnostic> {
    let mut output = String::new();
    for record in descriptor.records() {
        output.push_str(&format!("export interface {} {{\n", record.host_name()));
        for field in record.fields() {
            let ty = match field.ty() {
                NestedOwnedRecordFieldType::I64 | NestedOwnedRecordFieldType::Usize => {
                    "bigint".to_owned()
                }
                NestedOwnedRecordFieldType::Bool => "boolean".to_owned(),
                NestedOwnedRecordFieldType::OwnedBytes => "Uint8Array".to_owned(),
                NestedOwnedRecordFieldType::Record(id) => descriptor
                    .records()
                    .iter()
                    .find(|r| r.stable_id() == id)
                    .map(|r| r.host_name().to_owned())
                    .ok_or_else(|| package_error("nested record TypeScript reference is absent"))?,
            };
            output.push_str(&format!(
                "  readonly {}: {};\n",
                quote_json(field.host_name()),
                ty
            ));
        }
        output.push_str("}\n");
    }
    output.push_str("export interface SemapraxApi {\n");
    for export in descriptor.exports() {
        let params = export
            .parameters()
            .iter()
            .enumerate()
            .map(|(i, (_, _, ty))| format!("arg{i}: {}", parameter_ts(*ty)))
            .collect::<Vec<_>>()
            .join(", ");
        let result = descriptor
            .records()
            .iter()
            .find(|r| r.stable_id() == export.result_record_id())
            .map(|r| r.host_name())
            .ok_or_else(|| package_error("nested record result type is absent"))?;
        output.push_str(&format!(
            "  readonly {}: ({params}) => {result};\n",
            quote_json(export.typescript_name())
        ));
    }
    output.push_str("}\nexport interface SemapraxRuntime { readonly functions: Readonly<SemapraxApi>; call<I extends keyof SemapraxApi>(id:I,...args:Parameters<SemapraxApi[I]>):ReturnType<SemapraxApi[I]>; readonly wasmSha256:string; }\nexport declare function instantiate(bytes:Uint8Array):Promise<SemapraxRuntime>;\nexport declare const exportIds:readonly(keyof SemapraxApi)[];\nexport default instantiate;\n");
    Ok(output)
}

fn subject(descriptor: &NestedOwnedRecordApiDescriptor) -> PublicApiSubject<'_> {
    PublicApiSubject {
        project_schema: NESTED_OWNED_RECORD_PROJECT_SCHEMA,
        project_revision: descriptor.project_revision(),
        workspace_revision: descriptor.workspace_revision(),
        project_graph_digest: descriptor.project_graph_digest(),
    }
}
fn parameter_wire(value: PublicApiParameterType) -> &'static str {
    match value {
        PublicApiParameterType::I64 => "i64",
        PublicApiParameterType::Bool => "bool",
        PublicApiParameterType::BorrowStr => "borrow-str",
        PublicApiParameterType::BorrowSliceU8 => "borrow-slice-u8",
    }
}
fn parameter_ts(value: PublicApiParameterType) -> &'static str {
    match value {
        PublicApiParameterType::I64 => "bigint",
        PublicApiParameterType::Bool => "boolean",
        PublicApiParameterType::BorrowStr => "string",
        PublicApiParameterType::BorrowSliceU8 => "Uint8Array",
    }
}
fn leaf_wire(value: NestedOwnedRecordLeafType) -> &'static str {
    match value {
        NestedOwnedRecordLeafType::I64 => "i64",
        NestedOwnedRecordLeafType::Bool => "bool",
        NestedOwnedRecordLeafType::Usize => "usize",
        NestedOwnedRecordLeafType::OwnedBytes => "owned-bytes",
    }
}
fn raw_symbol(id: &str) -> String {
    let mut out = String::from("spx_owned_v1_");
    for byte in id.bytes() {
        use std::fmt::Write as _;
        write!(out, "{byte:02x}").unwrap();
    }
    out
}
fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 6], path: &str) -> Result<&'a [u8], Diagnostic> {
    artifacts
        .iter()
        .find(|a| a.path == path)
        .map(NpmArtifact::bytes)
        .ok_or_else(|| package_error("nested record artifact is absent"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn v11_runtime_has_one_preflight_copy_commit_and_post_settlement_publication() {
        let arena = include_str!("owned_nested_invocation/arena.js");
        let result = include_str!("owned_nested_invocation/result.js");
        let call = include_str!("owned_nested_invocation/call.js");
        assert!(arena.contains("function consumeMany(carriers)"));
        assert!(arena.contains("if(tokens.has(value.token))"));
        assert!(arena.contains("total>65536"));
        assert!(
            arena.find("const copies=[]").unwrap()
                < arena.find("entries.delete(item.token)").unwrap()
        );
        assert!(result.contains("consumeMany(owned.map(leaf=>leaf.value))"));
        assert!(!result.contains("arena.consume("));
        assert!(call.find("arena.settle()").unwrap() < call.find("answer=complete()").unwrap());
        assert!(!include_str!("owned_invocation/arena.js").contains("consumeMany"));
    }

    #[test]
    fn v11_package_replays_two_owned_occurrences_without_legacy_widening() {
        let source = r#"module nested.npm;
@id("nested.payload") record Payload { @id("nested.payload.bytes") bytes: Bytes, @id("nested.payload.size") size: usize, }
@id("nested.envelope") record Envelope { @id("nested.envelope.left") left: Payload, @id("nested.envelope.right") right: Payload, }
@id("nested.build") fn build(input: borrow Slice<u8>) -> Envelope { Envelope { left: Payload { bytes: bytes_copy(input), size: byte_len(input) }, right: Payload { bytes: bytes_copy(input), size: byte_len(input) } } }
@id("nested.main") fn main() -> i64 { 0 }
"#;
        let checked = crate::check(source, Path::new("nested-npm.spx")).unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let subject = PublicApiSubject {
            project_schema: NESTED_OWNED_RECORD_PROJECT_SCHEMA,
            project_revision:
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            workspace_revision:
                "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            project_graph_digest:
                "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        };
        let descriptor = crate::project::derive_nested_owned_record_api_descriptor(
            &program,
            &["nested.build".to_owned()],
            subject,
        )
        .unwrap();
        let build = prepare(
            &program,
            &descriptor,
            "nested-npm",
            "1.0.0",
            4 * 1024 * 1024,
        )
        .unwrap();
        build.verify().unwrap();
        let artifacts = match super::super::carrier::decode_carrier_artifacts(
            build.envelope(),
            build.max_bytes(),
        )
        .unwrap()
        {
            super::super::carrier::ReplayedNpmArtifacts::NestedOwnedRecord(value) => value,
            _ => panic!("v11 carrier replay selected a legacy artifact family"),
        };
        let runtime =
            std::str::from_utf8(artifact_bytes(&artifacts, "semaprax.js").unwrap()).unwrap();
        assert!(runtime.contains("consumeMany"));
        assert_eq!(runtime.matches("kind:\"owned-bytes\"").count(), 2);
    }
}
