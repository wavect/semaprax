//! I/O-free request structs and response decoders from selected descriptors.
use super::{invalid, Result, VNEXT_PROTOCOL_SCHEMA, VNEXT_RESULT_SCHEMA};
use serde_json::{json, Value};
use std::fmt::Write;

pub(super) fn generate(language: &str, bundle: &Value) -> Result<String> {
    let methods = bundle["methods"]
        .as_array()
        .ok_or_else(|| invalid("client descriptor list is missing"))?;
    let metadata = json!({"methods":methods.iter().map(|descriptor|(descriptor["method"].as_str().unwrap().to_owned(),json!({"params":descriptor["request_schema"]["properties"]["params"],"payload":descriptor["success_response_schema"]["properties"]["result"]["properties"]["payload"]}))).collect::<serde_json::Map<_,_>>(),
        "documents":bundle["documents"].as_array().unwrap().iter().filter(|document| {
            let id=document["$id"].as_str().unwrap_or("");
            !id.contains("image-v5.") && !matches!(id,"urn:semaprax.typed-expression.v1"|"urn:semaprax.semantic-change-intent.v1"|"urn:semaprax.semantic-change.v1"|"urn:semaprax.project-candidate-recovery.v1")
        }).map(|document|(document["$id"].as_str().unwrap().to_owned(),document.clone())).collect::<serde_json::Map<_,_>>(),
        "unbundled":bundle["unbundled_payload_schemas"]});
    let encoded = serde_json::to_string(&metadata)
        .map_err(|_| invalid("client metadata serialization failed"))?;
    let mut source = match language {
        "typescript" => format!("// Generated selected-profile client. No I/O or capability changes.\nexport const PROTOCOL = {VNEXT_PROTOCOL_SCHEMA:?};\nexport const RESULT_SCHEMA = {VNEXT_RESULT_SCHEMA:?};\nconst META = JSON.parse({:?});\n{}\n",encoded,include_str!("client_typescript.txt")),
        "python" => format!("# Generated selected-profile client. No I/O or capability changes.\nimport json\nimport re\nfrom typing import Any, Literal, NotRequired, TypedDict, TypeAlias\nPROTOCOL = {VNEXT_PROTOCOL_SCHEMA:?}\nRESULT_SCHEMA = {VNEXT_RESULT_SCHEMA:?}\nMETA = json.loads({:?})\n{}\n",encoded,include_str!("client_python.txt")),
        "rust" => format!("// Generated selected-profile client. Requires serde(derive) + serde_json; no I/O.\nuse serde::{{Serialize, Deserialize}};\nuse serde_json::{{Value, json}};\npub const PROTOCOL: &str = {VNEXT_PROTOCOL_SCHEMA:?};\npub const RESULT_SCHEMA: &str = {VNEXT_RESULT_SCHEMA:?};\nconst METADATA: &str = {:?};\n{}\n",encoded,include_str!("client_rust.txt")),
        _ => return Err(invalid("unknown client language")),
    };
    for descriptor in methods {
        let method = descriptor["method"].as_str().unwrap();
        let class = class_name(method);
        let function = function_name(method);
        let params = &descriptor["request_schema"]["properties"]["params"];
        let fields = params["properties"].as_object().unwrap();
        let required = params["required"].as_array().unwrap();
        match language {
            "typescript" => {
                writeln!(source, "export interface {class}Params {{").unwrap();
                for (name, schema) in fields {
                    let optional = if required.iter().any(|value| value == name) {
                        ""
                    } else {
                        "?"
                    };
                    writeln!(source, "  {name}{optional}: {};", ts_type(schema)).unwrap();
                }
                writeln!(source,"}}\nexport function {function}(id: RpcId, params: {class}Params): string {{ return request(id, {method:?}, params); }}\nexport function decode_{function}(line: string, id: RpcId): ResultEnvelope {{ return decode(line, {method:?}, id); }}").unwrap();
            }
            "python" => {
                writeln!(source, "class {class}Params(TypedDict):").unwrap();
                if fields.is_empty() {
                    writeln!(source, "    pass").unwrap();
                }
                for (name, schema) in fields {
                    let ty = py_type(schema);
                    let ty = if required.iter().any(|value| value == name) {
                        ty
                    } else {
                        format!("NotRequired[{ty}]")
                    };
                    writeln!(source, "    {name}: {ty}").unwrap();
                }
                writeln!(source,"\ndef {function}(request_id: RpcId, params: {class}Params) -> str:\n    return request(request_id, {method:?}, dict(params))\ndef decode_{function}(line: str, request_id: RpcId) -> ResultEnvelope:\n    return decode(line, {method:?}, request_id)\n").unwrap();
            }
            _ => {
                let mut field_types = Vec::new();
                for (name, schema) in fields {
                    let ty = if let Some(values) = schema["enum"].as_array() {
                        let ty = format!("{class}{}", class_name(name));
                        writeln!(
                            source,
                            "#[derive(Clone, Debug, Serialize, Deserialize)]\npub enum {ty} {{"
                        )
                        .unwrap();
                        for (index, value) in values.iter().enumerate() {
                            writeln!(
                                source,
                                "    #[serde(rename = {:?})] Choice{index},",
                                value.as_str().unwrap()
                            )
                            .unwrap();
                        }
                        writeln!(source, "}}").unwrap();
                        ty
                    } else {
                        match schema["type"].as_str() {
                            Some("integer") => "u64",
                            Some("object") => "serde_json::Map<String, Value>",
                            _ => "String",
                        }
                        .into()
                    };
                    field_types.push((name, ty, required.iter().any(|value| value == name)));
                }
                writeln!(source,"#[derive(Clone, Debug, Serialize, Deserialize)]\n#[serde(deny_unknown_fields)]\npub struct {class}Params {{").unwrap();
                for (name, ty, required) in field_types {
                    if !required {
                        writeln!(
                            source,
                            "    #[serde(default, skip_serializing_if = \"Option::is_none\")]"
                        )
                        .unwrap();
                    }
                    writeln!(
                        source,
                        "    pub r#{name}: {},",
                        if required {
                            ty
                        } else {
                            format!("Option<{ty}>")
                        }
                    )
                    .unwrap();
                }
                writeln!(source,"}}\npub fn {function}(id: RpcId, params: {class}Params) -> Result<String, String> {{ request(id, {method:?}, serde_json::to_value(params).map_err(|e|e.to_string())?) }}\npub fn decode_{function}(line: &str, id: &RpcId) -> Result<ResultEnvelope, String> {{ decode(line, {method:?}, id) }}").unwrap();
            }
        }
    }
    Ok(source)
}
fn class_name(value: &str) -> String {
    value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            format!(
                "{}{}",
                chars.next().unwrap().to_ascii_uppercase(),
                chars.as_str()
            )
        })
        .collect()
}
fn function_name(value: &str) -> String {
    format!("request_{}", value.replace(['/', '-'], "_"))
}
fn ts_type(schema: &Value) -> String {
    if let Some(values) = schema["enum"].as_array() {
        return values
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join(" | ");
    }
    match schema["type"].as_str() {
        Some("integer") => "number".into(),
        Some("object") => "JsonObject".into(),
        _ if schema["pattern"] == "^sha256:[0-9a-f]{64}$" => "Digest".into(),
        _ => "string".into(),
    }
}
fn py_type(schema: &Value) -> String {
    if let Some(values) = schema["enum"].as_array() {
        return format!(
            "Literal[{}]",
            values
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    match schema["type"].as_str() {
        Some("integer") => "int",
        Some("object") => "dict[str, Any]",
        _ => "str",
    }
    .into()
}
