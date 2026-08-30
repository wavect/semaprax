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
    // Only compiler-derived hexadecimal and decimal constants enter this
    // executable template. Source names and stable identities never do.
    let app = include_str!("app.js")
        .replace(
            "__DESCRIPTOR_DIGEST__",
            &digest(module.descriptor().as_bytes())[7..],
        )
        .replace(
            "__DESCRIPTOR_BYTES__",
            &module.descriptor().len().to_string(),
        );
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
    let manifest = format!("{{\"schema\":\"semaprax.web-internal-strings.v1\",\"module\":{},\"source_digest\":{},\"graph_revision\":{},\"compiler_schema\":{},\"runtime_schema\":{},\"capabilities\":[],\"artifacts\":[{}]}}\n", quote_json(module_name), quote_json(&digest(source.as_bytes())), quote_json(revision), quote_json(SCHEMA), quote_json(RUNTIME_SCHEMA), rows);
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
    Ok(text)
}

fn scalar(value: &serde_json::Value) -> Result<&'static str, Diagnostic> {
    match value.as_str() {
        Some("i64") => Ok("bigint"),
        Some("bool") => Ok("boolean"),
        _ => Err(super::super::error("invalid emitted String scalar")),
    }
}
