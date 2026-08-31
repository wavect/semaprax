//! I/O-free request structs and response decoders from selected descriptors.
use super::{invalid, Result, VNEXT_PROTOCOL_SCHEMA, VNEXT_RESULT_SCHEMA};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

#[path = "request_types.rs"]
mod request_types;
#[path = "response_types.rs"]
mod response_types;

pub(super) fn generate(language: &str, bundle: &Value) -> Result<String> {
    let methods = bundle["methods"]
        .as_array()
        .ok_or_else(|| invalid("client descriptor list is missing"))?;
    let documents = response_documents(bundle)?;
    let typed = response_types::generate(
        language,
        methods,
        &documents,
        bundle["unbundled_payload_schemas"]
            .as_array()
            .ok_or_else(|| invalid("client opaque schema inventory missing"))?,
    )?;
    let typed_requests = request_types::generate(language, methods, &request_documents(bundle)?)?;
    let metadata = json!({"methods":methods.iter().map(|descriptor|(descriptor["method"].as_str().unwrap().to_owned(),json!({"params":descriptor["request_schema"]["properties"]["params"],"payload":descriptor["success_response_schema"]["properties"]["result"]["properties"]["payload"]}))).collect::<serde_json::Map<_,_>>(),
        "documents":documents,
        "unbundled":bundle["unbundled_payload_schemas"]});
    let encoded = serde_json::to_string(&metadata)
        .map_err(|_| invalid("client metadata serialization failed"))?;
    let mut source = match language {
        "typescript" => format!("// Generated selected-profile client. No I/O or capability changes.\nexport const PROTOCOL = {VNEXT_PROTOCOL_SCHEMA:?};\nexport const RESULT_SCHEMA = {VNEXT_RESULT_SCHEMA:?};\nconst META = JSON.parse({:?});\n{}\n",encoded,include_str!("client_typescript.txt")),
        "python" => format!("# Generated selected-profile client. No I/O or capability changes.\nimport json\nimport re\nfrom typing import Any, Literal, NotRequired, TypedDict, TypeAlias\nPROTOCOL = {VNEXT_PROTOCOL_SCHEMA:?}\nRESULT_SCHEMA = {VNEXT_RESULT_SCHEMA:?}\nMETA = json.loads({:?})\n{}\n",encoded,include_str!("client_python.txt")),
        "rust" => format!("// Generated selected-profile client. Requires serde(derive) + serde_json; no I/O.\nuse serde::{{Serialize, Deserialize}};\nuse serde_json::{{Value, json}};\npub const PROTOCOL: &str = {VNEXT_PROTOCOL_SCHEMA:?};\npub const RESULT_SCHEMA: &str = {VNEXT_RESULT_SCHEMA:?};\nconst METADATA: &str = {:?};\n{}\n",encoded,include_str!("client_rust.txt")),
        _ => return Err(invalid("unknown client language")),
    };
    source.push_str(&typed.source);
    source.push_str(&typed_requests.source);
    let mut public_names = BTreeSet::new();
    for descriptor in methods {
        let method = descriptor["method"].as_str().unwrap();
        let class = class_name(method);
        if !public_names.insert(class.clone()) {
            return Err(invalid("selected client method type names collide"));
        }
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
        let payload = typed
            .payloads
            .get(method)
            .ok_or_else(|| invalid("selected client method lacks a response type"))?;
        typed_decoder(&mut source, language, method, &class, &function, payload);
        let parameters = typed_requests
            .params
            .get(method)
            .ok_or_else(|| invalid("selected client method lacks a request type"))?;
        typed_request(&mut source, language, method, &class, &function, parameters);
    }
    Ok(source)
}

/// Additive structural request types retain ordinary outer validation and
/// compiler-owned admission. Neither static types nor serialization authorize
/// a semantic operation, widen the selected method set, or perform any I/O.
fn typed_request(
    source: &mut String,
    language: &str,
    method: &str,
    class: &str,
    function: &str,
    parameters: &str,
) {
    let alias = format!("{class}TypedParams");
    match language {
        "typescript" => writeln!(source, "export type {alias} = {parameters};\nexport function {function}_typed(id: RpcId, params: {alias}): string {{ return request(id, {method:?}, params); }}").unwrap(),
        "python" => writeln!(source, "{alias}: TypeAlias = {parameters}\ndef {function}_typed(request_id: RpcId, params: {alias}) -> str:\n    return request(request_id, {method:?}, dict(params))\n").unwrap(),
        _ => writeln!(source, "pub type {alias} = {parameters};\npub fn {function}_typed(id: RpcId, params: {alias}) -> Result<String, String> {{ request(id, {method:?}, serde_json::to_value(params).map_err(|e|e.to_string())?) }}").unwrap(),
    }
}

fn request_documents(bundle: &Value) -> Result<BTreeMap<String, Value>> {
    let mut documents = BTreeMap::new();
    for document in bundle["documents"]
        .as_array()
        .ok_or_else(|| invalid("client request schema documents missing"))?
    {
        let id = document["$id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| invalid("client request schema identity missing"))?;
        if documents.insert(id.to_owned(), document.clone()).is_some() {
            return Err(invalid("client request schema identities collide"));
        }
    }
    Ok(documents)
}

/// Additive typed helpers always enter through the existing method-bound
/// decoder. Static types cannot replace its envelope, revision or shape checks.
fn typed_decoder(
    source: &mut String,
    language: &str,
    method: &str,
    class: &str,
    function: &str,
    payload: &str,
) {
    match language {
        "typescript" => writeln!(
            source,
            "export type {class}Payload = {payload};\nexport type {class}Result = TypedResultEnvelope<{class}Payload>;\nexport function decode_{function}_typed(line: string, id: RpcId): {class}Result {{ return decode(line, {method:?}, id) as {class}Result; }}"
        ),
        "python" => writeln!(
            source,
            "{class}Payload: TypeAlias = {payload}\n{class}Result: TypeAlias = TypedResultEnvelope[{class}Payload]\ndef decode_{function}_typed(line: str, request_id: RpcId) -> {class}Result:\n    return cast({class}Result, decode(line, {method:?}, request_id))\n"
        ),
        _ => writeln!(
            source,
            "pub type {class}Payload = {payload};\npub type {class}Result = TypedResultEnvelope<{class}Payload>;\npub fn decode_{function}_typed(line: &str, id: &RpcId) -> Result<{class}Result, String> {{ typed_result(decode(line, {method:?}, id)?) }}"
        ),
    }
    .unwrap();
}

/// Only the schema assertion subset implemented by all three templates may
/// enter runtime metadata. Fail closed during generation, before a client can
/// silently skip an unsupported assertion. Constructor request interiors are
/// deliberately excluded, just as they are in each generated request builder.
fn audit_schema(schema: &Value, refs: &mut BTreeSet<String>, depth: usize) -> Result<()> {
    if depth > 128 {
        return Err(invalid("client schema depth exceeds supported traversal"));
    }
    let fields = schema
        .as_object()
        .ok_or_else(|| invalid("client schema must be an object"))?;
    for key in fields.keys() {
        if !matches!(
            key.as_str(),
            "$id"
                | "$schema"
                | "title"
                | "description"
                | "$ref"
                | "type"
                | "const"
                | "enum"
                | "oneOf"
                | "anyOf"
                | "properties"
                | "required"
                | "additionalProperties"
                | "items"
                | "minItems"
                | "maxItems"
                | "minimum"
                | "maximum"
                | "minLength"
                | "maxLength"
                | "x-max-utf8-bytes"
                | "pattern"
        ) {
            return Err(invalid(
                "client schema uses an unsupported validation keyword",
            ));
        }
    }
    if let Some(reference) = fields.get("$ref") {
        let reference = reference
            .as_str()
            .ok_or_else(|| invalid("client schema reference is not a string"))?;
        if !reference.starts_with("urn:")
            || reference.contains('#')
            || fields.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "$ref" | "$id" | "$schema" | "title" | "description"
                )
            })
        {
            return Err(invalid(
                "client schema requires unsupported reference semantics",
            ));
        }
        refs.insert(reference.to_owned());
    }
    if let Some(kind) = fields.get("type") {
        if !kind.as_str().is_some_and(|kind| {
            matches!(
                kind,
                "object" | "array" | "string" | "integer" | "boolean" | "null"
            )
        }) {
            return Err(invalid("client schema uses an unsupported value type"));
        }
    }
    for (kind, keywords) in [
        (
            "object",
            &["properties", "required", "additionalProperties"][..],
        ),
        ("array", &["items", "minItems", "maxItems"][..]),
        (
            "string",
            &["minLength", "maxLength", "x-max-utf8-bytes", "pattern"][..],
        ),
        ("integer", &["minimum", "maximum"][..]),
    ] {
        if keywords.iter().any(|key| fields.contains_key(*key)) && schema["type"] != kind {
            return Err(invalid(
                "client assertion requires an explicit matching type",
            ));
        }
    }
    if fields
        .get("additionalProperties")
        .is_some_and(|value| !value.is_boolean())
    {
        return Err(invalid(
            "client schema does not support schema-valued additional properties",
        ));
    }
    if let Some(pattern) = fields.get("pattern") {
        if pattern != "^sha256:[0-9a-f]{64}$" && pattern != r"^[^\u0000-\u001f\u007f-\u009f]+$" {
            return Err(invalid("client schema uses an unsupported pattern"));
        }
    }
    for keyword in ["oneOf", "anyOf"] {
        if let Some(branches) = fields.get(keyword) {
            let branches = branches
                .as_array()
                .ok_or_else(|| invalid("client schema alternatives must be an array"))?;
            if branches.is_empty() {
                return Err(invalid("client schema alternatives are empty"));
            }
            for branch in branches {
                audit_schema(branch, refs, depth + 1)?;
            }
        }
    }
    if let Some(properties) = fields.get("properties") {
        for property in properties
            .as_object()
            .ok_or_else(|| invalid("client schema properties must be an object"))?
            .values()
        {
            audit_schema(property, refs, depth + 1)?;
        }
    }
    if let Some(items) = fields.get("items") {
        audit_schema(items, refs, depth + 1)?;
    }
    Ok(())
}

fn response_documents(bundle: &Value) -> Result<BTreeMap<String, Value>> {
    let available = bundle["documents"]
        .as_array()
        .ok_or_else(|| invalid("client schema documents missing"))?
        .iter()
        .map(|document| (document["$id"].as_str().unwrap_or("").to_owned(), document))
        .collect::<BTreeMap<_, _>>();
    let unbundled = bundle["unbundled_payload_schemas"]
        .as_array()
        .ok_or_else(|| invalid("client opaque schema inventory missing"))?;
    let mut pending = BTreeSet::new();
    for method in bundle["methods"]
        .as_array()
        .ok_or_else(|| invalid("client method inventory missing"))?
    {
        audit_schema(
            &method["success_response_schema"]["properties"]["result"]["properties"]["payload"],
            &mut pending,
            0,
        )?;
        let mut params = method["request_schema"]["properties"]["params"].clone();
        for field in params["properties"]
            .as_object_mut()
            .ok_or_else(|| invalid("client parameter fields missing"))?
            .values_mut()
        {
            field
                .as_object_mut()
                .ok_or_else(|| invalid("client parameter schema malformed"))?
                .remove("$ref");
        }
        audit_schema(&params, &mut pending, 0)?;
    }
    let mut visited = BTreeSet::new();
    let mut included = BTreeMap::new();
    while let Some(id) = pending.pop_first() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if let Some(document) = available.get(&id) {
            audit_schema(document, &mut pending, 0)?;
            included.insert(id, (*document).clone());
        } else if !unbundled.iter().any(|value| value == &id) {
            return Err(invalid(
                "client response schema has an unclassified missing reference",
            ));
        }
    }
    Ok(included)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_response_assertions_fail_before_client_generation() {
        for unsupported in [
            json!({"allOf":[{"type":"string"}]}),
            json!({"type":"object","additionalProperties":{"type":"string"}}),
            json!({"$ref":"#/$defs/x"}),
            json!({"$ref":"urn:example","type":"string"}),
            json!({"pattern":"not_a_supported_pattern"}),
            json!({"type":"array","uniqueItems":true}),
            json!({"minimum":1}),
        ] {
            assert!(
                audit_schema(&unsupported, &mut BTreeSet::new(), 0).is_err(),
                "{unsupported}"
            );
        }
        // Constant JSON is data, including keys which resemble schema keywords.
        audit_schema(
            &json!({"const":{"allOf":[],"$ref":"#not_a_schema_reference"}}),
            &mut BTreeSet::new(),
            0,
        )
        .unwrap();
    }

    #[test]
    fn metadata_contains_only_transitive_selected_response_documents() {
        let bundle = json!({"methods":[{"request_schema":{"properties":{"params":{"type":"object","properties":{}}}},"success_response_schema":{"properties":{"result":{"properties":{"payload":{"$ref":"urn:outer"}}}}}}],
            "documents":[{"$id":"urn:outer","type":"object","properties":{"child":{"$ref":"urn:inner"}}},{"$id":"urn:inner","type":"string"},{"$id":"urn:unselected","allOf":[]}],"unbundled_payload_schemas":[]});
        let docs = response_documents(&bundle).unwrap();
        assert_eq!(
            docs.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["urn:inner", "urn:outer"]
        );
        let mut changed = bundle;
        changed["documents"][1]["allOf"] = json!([]);
        assert!(response_documents(&changed).is_err());
    }

    #[test]
    fn typed_public_names_reject_colliding_method_spellings() {
        let methods = ["sample/value", "sample-value"]
            .map(|method| {
                json!({
                    "method":method,
                    "request_schema":{"properties":{"params":{
                        "type":"object","properties":{},"required":[],"additionalProperties":false
                    }}},
                    "success_response_schema":{"properties":{"result":{"properties":{
                        "payload":{"type":"object","properties":{},"required":[],"additionalProperties":false}
                    }}}}
                })
            });
        let bundle = json!({"methods":methods,"documents":[],"unbundled_payload_schemas":[]});
        for language in ["typescript", "python", "rust"] {
            let errors = generate(language, &bundle).unwrap_err();
            assert_eq!(errors[0].code, "SPX-G288");
            assert!(errors[0].message.contains("names collide"));
        }
    }
}
