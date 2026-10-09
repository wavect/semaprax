use super::super::{InternalStringModule, RUNTIME_SCHEMA, SCHEMA};
use crate::diagnostic::{quote_json, Diagnostic};
use sha2::{Digest, Sha256};

const PACKAGE: &str = "{\"private\":true,\"type\":\"module\",\"exports\":\"./semaprax.js\",\"types\":\"./semaprax.d.ts\"}\n";

fn digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

pub(super) fn artifacts(
    module_name: &str,
    source: &str,
    revision: &str,
    module: &InternalStringModule,
) -> Result<Vec<(&'static str, Vec<u8>)>, Diagnostic> {
    let declarations = declarations(module.descriptor())?;
    let facts: serde_json::Value = serde_json::from_str(module.descriptor())
        .map_err(|_| super::super::error("invalid emitted descriptor"))?;
    let toolkit = facts["profile"] == "text-toolkit-v1";
    let literal_format = facts["literal_format_v1"] == true;
    // Only compiler-derived hexadecimal and decimal constants enter this
    // executable template. Source names and stable identities never do.
    let mut app = include_str!("app.js")
        .replace(
            "__DESCRIPTOR_DIGEST__",
            &digest(module.descriptor().as_bytes())[7..],
        )
        .replace(
            "__DESCRIPTOR_BYTES__",
            &module.descriptor().len().to_string(),
        );
    if toolkit {
        app = app.replace("['schema','runtime_schema'", "['profile','capabilities','schema','runtime_schema'")
            .replace("value.schema!=='semaprax.wasm-internal-strings.v1'", "value.profile!=='text-toolkit-v1'||!Array.isArray(value.capabilities)||value.capabilities.length>1||value.capabilities.some(effect=>effect!=='fs.read')||value.schema!=='semaprax.wasm-text-toolkit.v1'")
            .replace("value.runtime_schema!=='semaprax.wasm-internal-strings.runtime.v1'", "value.runtime_schema!=='semaprax.wasm-text-toolkit.runtime.v1'");
    } else if literal_format {
        app = app.replace("['schema','runtime_schema'", "['profile','literal_format_v1','schema','runtime_schema'")
            .replace("value.schema!=='semaprax.wasm-internal-strings.v1'", "value.profile!=='literal-format-v1'||value.literal_format_v1!==true||value.schema!=='semaprax.wasm-literal-format.v1'")
            .replace("value.runtime_schema!=='semaprax.wasm-internal-strings.runtime.v1'", "value.runtime_schema!=='semaprax.wasm-literal-format.runtime.v1'");
    }
    if toolkit && literal_format {
        app = app.replace("['profile','capabilities','schema'", "['profile','capabilities','literal_format_v1','schema'")
            .replace("value.profile!=='text-toolkit-v1'", "value.literal_format_v1!==true||value.profile!=='text-toolkit-v1'");
    }
    let mut files = vec![
        ("app.wasm", module.wasm_bytes().to_vec()),
        ("semaprax.js", module.runtime_source().as_bytes().to_vec()),
        ("semaprax.d.ts", declarations.into_bytes()),
        (
            "semaprax.internal-strings.json",
            module.descriptor().as_bytes().to_vec(),
        ),
        ("package.json", PACKAGE.as_bytes().to_vec()),
        ("index.html", include_bytes!("index.html").to_vec()),
        ("app.js", app.into_bytes()),
    ];
    super::package_size(files.iter().map(|(_, bytes)| bytes.len()))?;
    let rows = files
        .iter()
        .map(|(path, bytes)| {
            format!(
                "{{\"path\":{},\"bytes\":{},\"sha256\":{}}}",
                quote_json(path),
                bytes.len(),
                quote_json(&digest(bytes))
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let manifest = format!("{{\"schema\":\"semaprax.web-internal-strings.v1\",\"module\":{},\"source_digest\":{},\"graph_revision\":{},\"compiler_schema\":{},\"runtime_schema\":{},\"capabilities\":[],\"artifacts\":[{}]}}\n", quote_json(module_name), quote_json(&digest(source.as_bytes())), quote_json(revision), quote_json(if toolkit { super::super::TOOLKIT_SCHEMA } else if literal_format { super::super::LITERAL_FORMAT_SCHEMA } else { SCHEMA }), quote_json(if toolkit { super::super::TOOLKIT_RUNTIME_SCHEMA } else if literal_format { super::super::LITERAL_FORMAT_RUNTIME_SCHEMA } else { RUNTIME_SCHEMA }), rows);
    let manifest = if toolkit {
        manifest
            .replace(
                "\"schema\":\"semaprax.web-internal-strings.v1\"",
                "\"schema\":\"semaprax.web-text-toolkit.v1\"",
            )
            .replace(
                "\"capabilities\":[]",
                &format!("\"capabilities\":{}", facts["capabilities"]),
            )
    } else if literal_format {
        manifest.replace("\"schema\":\"semaprax.web-internal-strings.v1\"", "\"schema\":\"semaprax.web-literal-format.v1\"")
    } else {
        manifest
    };
    files.insert(4, ("semaprax.manifest.json", manifest.into_bytes()));
    Ok(files)
}

fn declarations(descriptor: &str) -> Result<String, Diagnostic> {
    // The descriptor is freshly compiler-emitted and capped before this parse;
    // it is not external evidence or a separate authority admission API.
    let value: serde_json::Value = serde_json::from_str(descriptor)
        .map_err(|_| super::super::error("invalid emitted String descriptor"))?;
    let exports = value
        .get("exports")
        .and_then(|v| v.as_array())
        .filter(|v| (1..=32).contains(&v.len()))
        .ok_or_else(|| super::super::error("invalid emitted String exports"))?;
    let mut text = String::from("export type StringOutcome<T> = Readonly<{kind: 'success'; value: T}> | Readonly<{kind: 'failure'; domain: 'semaprax.arithmetic.v1'; code: 1|2|3|4|5|6|7|8}> | Readonly<{kind: 'failure'; domain: 'semaprax.contract.v1'; code: 1|2}> | Readonly<{kind: 'capacity'; cause: 'owners'|'value_bytes'|'live_bytes'|'cumulative_bytes'|'tokens'}>;\nexport interface StringFacade {\n");
    for export in exports {
        let id = export
            .get("stable_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| super::super::error("invalid emitted String identity"))?;
        let params = export
            .get("parameters")
            .and_then(|v| v.as_array())
            .filter(|v| v.len() <= 8)
            .ok_or_else(|| super::super::error("invalid emitted String parameters"))?;
        text.push_str(&format!("  call(id: {}", quote_json(id)));
        for (index, parameter) in params.iter().enumerate() {
            text.push_str(&format!(", arg{index}: {}", scalar(parameter)?));
        }
        let result = export
            .get("result")
            .ok_or_else(|| super::super::error("missing emitted String result"))?;
        text.push_str(&format!("): StringOutcome<{}>;\n", scalar(result)?));
    }
    text.push_str("}\nexport declare function instantiate(bytes: Uint8Array): Promise<Readonly<StringFacade>>;\n");
    if value["profile"] == "text-toolkit-v1" {
        text = text.replace("cause: 'owners'|", "cause: 'collection_owners'|'collection_bytes'|'collection_tokens'|'owners'|")
            .replace("export interface StringFacade", "export type ToolkitFailure = Readonly<{kind: 'failure'; domain: 'semaprax.text.v1'; code: 1|2|3}> | Readonly<{kind: 'failure'; domain: 'semaprax.convert.v1'; code: 1|2}> | Readonly<{kind: 'failure'; domain: 'semaprax.filesystem.v1'; code: 1|2|3|4|5|6|7}> | Readonly<{kind: 'failure'; domain: 'semaprax.map.v1'|'semaprax.map.v2'; code: 1|2|3|4}>;\nexport interface StringFacade")
            .replace("StringOutcome<bigint>;", "StringOutcome<bigint> | ToolkitFailure;")
            .replace("StringOutcome<boolean>;", "StringOutcome<boolean> | ToolkitFailure;")
            .replace("instantiate(bytes: Uint8Array)", "instantiate(bytes: Uint8Array, options?: Readonly<{maxOwnedCollections?: number; maxOwnedCollectionBytes?: number; fileReadText?: Readonly<{read(path: Uint8Array, maximum: number): Readonly<{ok: true; bytes: Uint8Array}> | Readonly<{ok: false; code: 1|2|3|4|5|6|7}>}>}>)");
    }
    if value["literal_format_v1"] == true {
        text = text.replace(
            "export interface StringFacade",
            "export type LiteralFormatFailure = Readonly<{kind: 'failure'; domain: 'semaprax.string-format.v1'; code: 1}>;\nexport interface StringFacade",
        );
        text = if value["profile"] == "text-toolkit-v1" {
            text.replace("| ToolkitFailure;", "| ToolkitFailure | LiteralFormatFailure;")
        } else {
            text.replace("StringOutcome<bigint>;", "StringOutcome<bigint> | LiteralFormatFailure;")
                .replace("StringOutcome<boolean>;", "StringOutcome<boolean> | LiteralFormatFailure;")
        };
    }
    Ok(text)
}

fn scalar(value: &serde_json::Value) -> Result<&'static str, Diagnostic> {
    match value.as_str() {
        Some("i64") => Ok("bigint"),
        Some("bool") => Ok("boolean"),
        _ => Err(super::super::error("invalid emitted String scalar")),
    }
}
