//! Project-v9 npm projection over the shared opaque owned-Bytes arena.

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::project::FlatOwnedRecordApiDescriptor;

use super::carrier::{
    artifact, payload_digest_artifacts_v8, render_carrier_artifacts, trusted_binding, NpmArtifact,
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
    descriptor: &FlatOwnedRecordApiDescriptor,
    package: &str,
    version: &str,
    max_bytes: usize,
) -> Result<ProjectNpmBuild, Diagnostic> {
    validate_carrier_limit(0, max_bytes)?;
    super::owned_data::validate_identity(package, version)?;
    let selected = descriptor
        .exports()
        .iter()
        .map(|export| export.stable_id().as_str().to_owned())
        .collect::<Vec<_>>();
    let subject = crate::project::PublicApiSubject {
        project_schema: crate::project::FLAT_OWNED_RECORD_PROJECT_SCHEMA,
        project_revision: descriptor.project_revision(),
        workspace_revision: descriptor.workspace_revision(),
        project_graph_digest: descriptor.project_graph_digest(),
    };
    let replayed = crate::project::replay_flat_owned_record_api_descriptor(
        program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )?;
    if &replayed != descriptor {
        return Err(package_error("flat record descriptor replay disagrees"));
    }
    let recipe = super::render_owned_data_semantic_recipe(program)?;
    let replayed_program = super::semantic_recipe_v8::replay_against(program, &recipe)?;
    let replayed_descriptor = crate::project::replay_flat_owned_record_api_descriptor(
        &replayed_program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )?;
    let wasm =
        crate::wasm::emit_resolved_module_with_flat_owned_record_exports(program, descriptor)?;
    if wasm.is_empty() || wasm.len() > super::owned_data::MAX_WASM_BYTES {
        return Err(package_error(
            "flat record npm Wasm exceeds the exact artifact limit",
        ));
    }
    if replayed_descriptor != *descriptor
        || crate::wasm::emit_resolved_module_with_flat_owned_record_exports(
            &replayed_program,
            &replayed_descriptor,
        )? != wasm
    {
        return Err(package_error(
            "flat record artifacts disagree with semantic replay",
        ));
    }
    let artifacts = render_package(package, version, descriptor, &wasm)?;
    let artifact_bytes = artifacts.iter().try_fold(0usize, |sum, artifact| {
        sum.checked_add(artifact.bytes.len())
            .filter(|value| *value <= max_bytes)
            .ok_or_else(|| package_error("flat record npm artifacts exceed limit"))
    })?;
    let identity = NpmBuildIdentity {
        project_schema: crate::project::FLAT_OWNED_RECORD_PROJECT_SCHEMA,
        package,
        version,
        project_revision: descriptor.project_revision(),
        workspace_revision: descriptor.workspace_revision(),
        project_graph_digest: descriptor.project_graph_digest(),
        semantic_recipe: &recipe,
    };
    let payload_digest = payload_digest_artifacts_v8(identity, &artifacts);
    let envelope = render_carrier_artifacts(
        super::PROJECT_NPM_BUILD_SCHEMA_V8,
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
    if identity.project_schema != crate::project::FLAT_OWNED_RECORD_PROJECT_SCHEMA {
        return Err(package_error("flat record carrier schema disagrees"));
    }
    let metadata = artifact_bytes(artifacts, "semaprax.api.json")?;
    let value: serde_json::Value = serde_json::from_slice(metadata)
        .map_err(|_| package_error("flat record metadata is invalid"))?;
    let object = value
        .as_object()
        .ok_or_else(|| package_error("flat record metadata is not an object"))?;
    let descriptor_bytes = object
        .get("descriptor")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error("flat record descriptor is absent"))?
        .as_bytes();
    let digest = object
        .get("descriptor_digest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| package_error("flat record digest is absent"))?;
    let program = super::semantic_recipe_v8::replay(identity.semantic_recipe)?;
    let descriptor_value: serde_json::Value = serde_json::from_slice(descriptor_bytes)
        .map_err(|_| package_error("flat record descriptor is invalid"))?;
    let selected = descriptor_value
        .get("exports")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| package_error("flat record exports are invalid"))?
        .iter()
        .map(|row| {
            row.get("stable_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| package_error("flat record export is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let subject = crate::project::PublicApiSubject {
        project_schema: identity.project_schema,
        project_revision: identity.project_revision,
        workspace_revision: identity.workspace_revision,
        project_graph_digest: identity.project_graph_digest,
    };
    let descriptor = crate::project::replay_flat_owned_record_api_descriptor(
        &program,
        &selected,
        subject,
        descriptor_bytes,
        digest,
    )?;
    let wasm = artifact_bytes(artifacts, "app.wasm")?;
    wasmparser::Validator::new()
        .validate_all(wasm)
        .map_err(|_| package_error("flat record Wasm is invalid"))?;
    if crate::wasm::emit_resolved_module_with_flat_owned_record_exports(&program, &descriptor)?
        != wasm
        || render_package(identity.package, identity.version, &descriptor, wasm)? != *artifacts
    {
        return Err(package_error("flat record package replay disagrees"));
    }
    Ok(())
}

fn render_package(
    package: &str,
    version: &str,
    descriptor: &FlatOwnedRecordApiDescriptor,
    wasm: &[u8],
) -> Result<[NpmArtifact; 6], Diagnostic> {
    super::owned_data::validate_identity(package, version)?;
    if wasm.is_empty() || wasm.len() > super::owned_data::MAX_WASM_BYTES {
        return Err(package_error(
            "flat record npm Wasm exceeds the exact artifact limit",
        ));
    }
    let wasm_digest = format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(wasm))
    );
    let mut runtime = super::owned_data::render_runtime_prelude(
        wasm_digest.strip_prefix("sha256:").unwrap_or(&wasm_digest),
    );
    runtime.push_str(&render_facade(descriptor));
    let declarations = format!("{}export interface SemapraxRuntime {{ readonly functions: Readonly<SemapraxApi>; call<I extends keyof SemapraxApi>(id:I,...args:Parameters<SemapraxApi[I]>):ReturnType<SemapraxApi[I]>; readonly wasmSha256:string; }}\nexport declare function instantiate(bytes:Uint8Array):Promise<SemapraxRuntime>;\nexport declare const exportIds:readonly(keyof SemapraxApi)[];\nexport default instantiate;\n", crate::project::render_flat_owned_record_typescript(descriptor));
    let metadata = crate::project::render_flat_owned_record_metadata(descriptor, &wasm_digest)?;
    crate::project::replay_flat_owned_record_metadata(descriptor, &wasm_digest, &metadata)?;
    let package_json = format!("{{\"name\":{},\"version\":{},\"type\":\"module\",\"sideEffects\":false,\"exports\":{{\".\":{{\"types\":\"./semaprax.bindings.d.ts\",\"import\":\"./semaprax.bindings.js\"}},\"./app.wasm\":\"./app.wasm\",\"./manifest\":\"./semaprax.api.json\"}},\"types\":\"./semaprax.bindings.d.ts\",\"files\":[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.api.json\"]}}\n", quote_json(package), quote_json(version));
    Ok([
        artifact("app.wasm", wasm),
        artifact("semaprax.js", runtime.as_bytes()),
        artifact(
            "semaprax.bindings.js",
            b"export { instantiate, exportIds, wasmSha256, default } from \"./semaprax.js\";\n",
        ),
        artifact("semaprax.bindings.d.ts", declarations.as_bytes()),
        artifact("semaprax.api.json", &metadata),
        artifact("package.json", package_json.as_bytes()),
    ])
}

fn render_facade(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let facts = descriptor.exports().iter().map(|export| {
        let parameters = export.parameters().iter().map(|(_,_,kind)| quote_json(kind.wire_name())).collect::<Vec<_>>().join(",");
        let fields = export.fields().iter().map(|field| format!("Object.freeze({{name:{},kind:{},offset:{}}})",quote_json(field.host_name()),quote_json(field.ty().wire_name()),field.ordinal()*8)).collect::<Vec<_>>().join(",");
        format!("[{},Object.freeze({{raw:{},params:Object.freeze([{parameters}]),fields:Object.freeze([{fields}]),size:{}}})]",quote_json(export.stable_id().as_str()),quote_json(&raw_symbol(export.stable_id().as_str())),export.fields().len()*8)
    }).collect::<Vec<_>>().join(",");
    format!(
        r#"const FACTS=new Map([{facts}]),IDS=Object.freeze(Array.from(FACTS.keys())),RESULT=65536,POISON=0xa5;const encoder=new TextEncoder();
function snapshot(value,type,label){{if(type==="borrow-str"){{if(typeof value!=="string")throw new TypeError(`${{label}} must be string`);for(let i=0;i<value.length;i++){{const unit=value.charCodeAt(i);if(unit>=0xd800&&unit<=0xdbff){{if(++i>=value.length||value.charCodeAt(i)<0xdc00||value.charCodeAt(i)>0xdfff)throw new TypeError(`${{label}} must contain Unicode scalar values`)}}else if(unit>=0xdc00&&unit<=0xdfff)throw new TypeError(`${{label}} must contain Unicode scalar values`)}}return encoder.encode(value)}}if(type==="borrow-slice-u8")return snapshotUint8(value,label);if(type==="i64"){{if(typeof value!=="bigint"||value<-(1n<<63n)||value>(1n<<63n)-1n)throw new TypeError(`${{label}} must be signed i64 bigint`);return value}}if(type==="bool"){{if(typeof value!=="boolean")throw new TypeError(`${{label}} must be boolean`);return value?1:0}}throw new Error("unknown parameter")}}
function facade(linked){{const e=linked.instance.exports,memory=e.memory;let busy=false,poisoned=false;function invoke(id,values){{if(poisoned||busy)throw new Error("SEMAPRAX flat-record runtime unavailable");const fact=FACTS.get(id);if(!fact||values.length!==fact.params.length)throw new TypeError("SEMAPRAX call identity disagrees");const snapshots=values.map((v,i)=>snapshot(v,fact.params[i],`argument ${{i}}`));let used=0;for(const v of snapshots)if(v instanceof Uint8Array){{if(v.byteLength>65536-used)throw new RangeError("borrowed input capacity");used+=v.byteLength}}busy=true;let began=false,settled=false,answer,primary=null;const bytes=new Uint8Array(memory.buffer),view=new DataView(memory.buffer);try{{let offset=0;const raw=[];for(const value of snapshots){{if(value instanceof Uint8Array){{linked.copyInto(bytes,value,offset);raw.push(offset,value.byteLength);offset+=value.byteLength}}else raw.push(value)}}bytes.fill(POISON,RESULT,RESULT+fact.size);linked.arena.begin();began=true;const status=e[fact.raw](...raw,RESULT);if(status!==0){{for(let i=0;i<fact.size;i++)if(bytes[RESULT+i]!==POISON)throw new Error("failure modified carrier");throw Object.assign(new Error(`SEMAPRAX failure ${{status}}`),{{status,semapraxSemantic:status<=10}})}}const values=Object.create(null);let ownedCarrier=null;for(const field of fact.fields){{if(field.kind==="owned-bytes")ownedCarrier=view.getBigInt64(RESULT+field.offset,true);else if(field.kind==="i64")values[field.name]=view.getBigInt64(RESULT+field.offset,true);else if(field.kind==="usize")values[field.name]=view.getBigUint64(RESULT+field.offset,true);else{{const value=view.getBigUint64(RESULT+field.offset,true);if(value>1n)throw new Error("bool invariant");values[field.name]=value===1n}}}}const owned=linked.arena.consume(ownedCarrier);linked.arena.settle();settled=true;for(const field of fact.fields)if(field.kind==="owned-bytes")values[field.name]=owned;answer=Object.freeze(values)}}catch(error){{primary=error;if(!(error instanceof RangeError)&&!(error instanceof TypeError)&&error?.semapraxSemantic!==true){{poisoned=true;linked.arena.poison()}}}}finally{{let failure=null;if(began&&!settled)try{{linked.arena.settle()}}catch(error){{poisoned=true;failure=error}}bytes.fill(0,0,used);bytes.fill(POISON,RESULT,RESULT+fact.size);busy=false;if(failure&&primary===null)primary=failure}}if(primary)throw primary;return answer}}const functions=Object.create(null);for(const id of IDS)Object.defineProperty(functions,id,{{value:(...v)=>invoke(id,v),enumerable:true}});return Object.freeze({{functions:Object.freeze(functions),call:(id,...v)=>invoke(id,v),wasmSha256}})}}export async function instantiate(bytes){{return facade(await instantiateCore(bytes))}}export const exportIds=IDS;export default instantiate;
"#
    )
}

fn raw_symbol(stable_id: &str) -> String {
    let mut value = String::from("spx_owned_v1_");
    for byte in stable_id.bytes() {
        use std::fmt::Write as _;
        write!(value, "{byte:02x}").unwrap();
    }
    value
}
fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 6], path: &str) -> Result<&'a [u8], Diagnostic> {
    artifacts
        .iter()
        .find(|artifact| artifact.path == path)
        .map(NpmArtifact::bytes)
        .ok_or_else(|| package_error("flat record artifact is absent"))
}

#[cfg(test)]
mod hostile_source_tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn v9_facade_closes_i64_and_utf16_before_busy_or_arena_effects() {
        let source = r#"
module flat.hostile;
@id("flat.packet") record Packet { @id("flat.bytes") bytes: Bytes, @id("flat.flag") flag: bool }
@id("flat.make") fn make(text: borrow Str, value: i64) -> Packet { Packet { bytes: bytes_copy(str_as_bytes(text)), flag: value == 0 } }
@id("flat.main") fn main() -> i64 { 0 }
"#;
        let checked = crate::check(source, Path::new("flat-hostile.spx")).unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let subject = crate::project::PublicApiSubject {
            project_schema: crate::project::FLAT_OWNED_RECORD_PROJECT_SCHEMA,
            project_revision:
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            workspace_revision:
                "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            project_graph_digest:
                "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        };
        let descriptor = crate::project::derive_flat_owned_record_api_descriptor(
            &program,
            &["flat.make".to_owned()],
            subject,
        )
        .unwrap();
        let facade = render_facade(&descriptor);
        let snapshot = facade.find("const snapshots=").unwrap();
        assert!(facade[..snapshot].contains("unit>=0xd800&&unit<=0xdbff"));
        assert!(facade[..snapshot].contains("unit>=0xdc00&&unit<=0xdfff"));
        assert!(facade[..snapshot].contains("++i>=value.length"));
        assert!(facade[..snapshot].contains("return encoder.encode(value)"));
        assert!(facade[..snapshot].contains("value<-(1n<<63n)"));
        assert!(facade[..snapshot].contains("value>(1n<<63n)-1n"));
        assert!(snapshot < facade.find("busy=true").unwrap());
        assert!(snapshot < facade.find("arena.begin()").unwrap());
    }
}
