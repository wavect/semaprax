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
                | "uniqueItems"
                | "minimum"
                | "maximum"
                | "minLength"
                | "maxLength"
                | "x-max-utf8-bytes"
                | "pattern"
                | "not"
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
        (
            "array",
            &["items", "minItems", "maxItems", "uniqueItems"][..],
        ),
        (
            "string",
            &[
                "minLength",
                "maxLength",
                "x-max-utf8-bytes",
                "pattern",
                "not",
            ][..],
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
        if !matches!(
            pattern.as_str(),
            Some(
                "^sha256:[0-9a-f]{64}$"
                    | r"^[^\u0000-\u001f\u007f-\u009f]+$"
                    | "^[A-Za-z_][A-Za-z0-9_]*$"
                    | "^[A-Za-z0-9_.:-]+$"
                    | "^[a-z0-9._-]+$"
                    | "^[0-9a-f]{8}$"
                    | "^[0-9a-f]{16}$"
            )
        ) {
            return Err(invalid("client schema uses an unsupported pattern"));
        }
    }
    if let Some(excluded) = fields.get("not") {
        let object = excluded
            .as_object()
            .ok_or_else(|| invalid("client exclusion must be a closed finite string enum"))?;
        let values = object
            .get("enum")
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty() && values.len() <= 4096)
            .ok_or_else(|| invalid("client exclusion requires a bounded nonempty enum"))?;
        let mut unique = BTreeSet::new();
        if object.len() != 1
            || values
                .iter()
                .any(|value| value.as_str().is_none_or(|value| !unique.insert(value)))
        {
            return Err(invalid("client exclusion must contain unique strings only"));
        }
    }
    if let Some(unique) = fields.get("uniqueItems") {
        if !unique.is_boolean()
            || (unique == true
                && (schema["items"]["type"] != "string"
                    || !schema["maxItems"]
                        .as_u64()
                        .is_some_and(|bound| bound <= 4096)))
        {
            return Err(invalid(
                "client uniqueness requires a bounded direct string array",
            ));
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

/// Lift only root-local compiler definitions. Schema positions are visited
/// explicitly so literal objects containing `$ref` remain literal data.
struct ResponseNormalizer {
    work: usize,
    bytes: usize,
}
impl ResponseNormalizer {
    fn document(&mut self, id: &str, source: &Value) -> Result<BTreeMap<String, Value>> {
        self.bytes = self.bytes.saturating_add(source.to_string().len());
        if self.bytes > 16 * 1024 * 1024
            || !id.starts_with("urn:")
            || id.contains('#')
            || id.len() > 4096
        {
            return Err(invalid(
                "client response schema identity or inventory exceeds bounds",
            ));
        }
        let mut root = source.clone();
        let object = root
            .as_object_mut()
            .ok_or_else(|| invalid("client schema document is not an object"))?;
        if object.get("$id").and_then(Value::as_str) != Some(id) {
            return Err(invalid(
                "client schema document identity does not match its registry key",
            ));
        }
        let definitions = match object.remove("$defs") {
            Some(Value::Object(definitions)) => definitions,
            Some(_) => return Err(invalid("client schema definitions must be an object")),
            None => serde_json::Map::new(),
        };
        if definitions.len() > 4096 {
            return Err(invalid("client schema definition count exceeds its bound"));
        }
        let mut names = BTreeMap::new();
        for name in definitions.keys() {
            if name.is_empty()
                || name.len() > 128
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            {
                return Err(invalid("client schema definition name is unsupported"));
            }
            names.insert(name.clone(), format!("{id}:response-def:{name}"));
        }
        let mut result = BTreeMap::new();
        self.schema(&mut root, &names, true, 0)?;
        result.insert(id.to_owned(), root);
        for (name, mut definition) in definitions {
            self.schema(&mut definition, &names, false, 0)?;
            let name = names[&name].clone();
            definition
                .as_object_mut()
                .unwrap()
                .insert("$id".into(), json!(name));
            result.insert(name, definition);
        }
        Ok(result)
    }

    fn schema(
        &mut self,
        schema: &mut Value,
        names: &BTreeMap<String, String>,
        root: bool,
        depth: usize,
    ) -> Result<()> {
        self.work = self.work.saturating_add(1);
        if self.work > 65536 || depth > 128 {
            return Err(invalid(
                "client response normalization exceeds its traversal bound",
            ));
        }
        let object = schema
            .as_object_mut()
            .ok_or_else(|| invalid("client normalization requires a schema object"))?;
        if object.contains_key("$defs")
            || (!root && (object.contains_key("$id") || object.contains_key("$schema")))
        {
            return Err(invalid(
                "client schema has an unsupported nested identity or definition scope",
            ));
        }
        object.retain(|key, _| !response_annotation(key));
        if let Some(reference) = object.get_mut("$ref") {
            let value = reference
                .as_str()
                .ok_or_else(|| invalid("client schema reference must be text"))?;
            if let Some(name) = value.strip_prefix("#/$defs/") {
                let absolute = names.get(name).ok_or_else(|| {
                    invalid("client schema local reference is dangling or unsupported")
                })?;
                *reference = json!(absolute);
            } else if !value.starts_with("urn:") || value.contains('#') {
                return Err(invalid(
                    "client schema reference is not an exact registry identity",
                ));
            }
        }
        if let Some(properties) = object.get_mut("properties") {
            for property in properties
                .as_object_mut()
                .ok_or_else(|| invalid("client schema properties are malformed"))?
                .values_mut()
            {
                self.schema(property, names, false, depth + 1)?;
            }
        }
        for keyword in ["oneOf", "anyOf"] {
            if let Some(branches) = object.get_mut(keyword) {
                for branch in branches
                    .as_array_mut()
                    .ok_or_else(|| invalid("client schema alternatives are malformed"))?
                {
                    self.schema(branch, names, false, depth + 1)?;
                }
            }
        }
        for keyword in ["items", "not"] {
            if let Some(child) = object.get_mut(keyword) {
                self.schema(child, names, false, depth + 1)?;
            }
        }
        Ok(())
    }
}

fn response_annotation(key: &str) -> bool {
    matches!(
        key,
        "x-base-and-fields-depth-increment"
            | "x-base-depth-increment"
            | "x-body-scope"
            | "x-counts-toward-expression-node-budget"
            | "x-evaluation-order"
            | "x-field-coverage"
            | "x-implicit-field-place-node-basis"
            | "x-implicit-field-place-nodes"
            | "x-implicit-if-block-nodes"
            | "x-implicit-let-nodes"
            | "x-implicit-match-nodes"
            | "x-implicit-project-node-basis"
            | "x-implicit-project-nodes"
            | "x-implicit-update-nodes"
            | "x-initializer-scope"
            | "x-max-combined-identities"
            | "x-max-expression-depth"
            | "x-max-expression-nodes"
            | "x-order"
            | "x-pattern-node-charge"
            | "x-requires-exact-declared-arity"
            | "x-requires-exact-exhaustive-case-and-field-coverage"
            | "x-requires-exact-field-identity-coverage"
            | "x-requires-exact-owner-and-field-admission"
            | "x-root-depth-increment"
            | "x-root-selection"
            | "x-sorted"
            | "x-total-payload-binders-maximum"
            | "x-value-and-body-depth-increment"
    )
}

fn response_documents(bundle: &Value) -> Result<BTreeMap<String, Value>> {
    let mut available = BTreeMap::new();
    for document in bundle["documents"]
        .as_array()
        .ok_or_else(|| invalid("client schema documents missing"))?
    {
        let id = document["$id"]
            .as_str()
            .ok_or_else(|| invalid("client schema identity is absent"))?;
        if available.insert(id.to_owned(), document).is_some() {
            return Err(invalid("client schema identities collide"));
        }
    }
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
    let mut lifted = BTreeMap::new();
    let mut normalizer = ResponseNormalizer { work: 0, bytes: 0 };
    while let Some(id) = pending.pop_first() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if let Some(document) = lifted.remove(&id) {
            audit_schema(&document, &mut pending, 0)?;
            included.insert(id, document);
        } else if let Some(document) = available.get(&id) {
            let normalized = normalizer.document(&id, document)?;
            for (name, schema) in normalized {
                if name == id {
                    audit_schema(&schema, &mut pending, 0)?;
                    included.insert(name, schema);
                } else if available.contains_key(&name)
                    || included.contains_key(&name)
                    || lifted.insert(name, schema).is_some()
                {
                    return Err(invalid("client normalized definition identities collide"));
                }
            }
        } else if !unbundled.iter().any(|value| value == &id) {
            return Err(invalid(
                "client response schema has an unclassified missing reference",
            ));
        }
        if included.len().saturating_add(lifted.len()) > 4096 {
            return Err(invalid(
                "client normalized schema inventory exceeds its bound",
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
        for (pattern, length) in [("^[0-9a-f]{8}$", 8), ("^[0-9a-f]{16}$", 16)] {
            audit_schema(
                &json!({"type":"string","minLength":length,"maxLength":length,"pattern":pattern}),
                &mut BTreeSet::new(),
                0,
            )
            .unwrap();
        }
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
    fn rust_client_fits_the_actual_serialized_discovery_payload() {
        use super::super::super::VNextPolicy;
        for policy in [
            VNextPolicy::default(),
            VNextPolicy {
                candidate_prepare: true,
                ..VNextPolicy::default()
            },
            VNextPolicy {
                candidate_prepare: true,
                diagnostics: true,
                ..VNextPolicy::default()
            },
            VNextPolicy {
                candidate_prepare: true,
                diagnostics: true,
                build_enabled: true,
                ..VNextPolicy::default()
            },
        ] {
            for batch_selected in [false, true] {
                let methods =
                    super::super::super::session_methods(&policy, false, false, batch_selected);
                let method = methods
                    .iter()
                    .find(|method| method.name == "protocol/client")
                    .unwrap();
                let params = serde_json::Map::from_iter([("language".to_owned(), json!("rust"))]);
                // Exercise the production payload builder and its unchanged cap:
                // the JSON source string escapes quotes/newlines and is larger
                // than the generated source's own byte count.
                let report =
                    super::super::payload(method, &params, &methods, &policy, false).unwrap();
                assert!(
                    serde_json::to_vec(&report).unwrap().len() <= super::super::MAX_DISCOVERY_BYTES
                );
                let source = report["source"].as_str().unwrap();
                assert!(source.contains("response_literal!"));
                assert_eq!(source.matches("macro_rules! response_literal").count(), 1);
                assert!(source.contains("response literal mismatch"));
                assert_eq!(
                    source.contains("pub fn request_workspace_read_batch("),
                    batch_selected
                );
            }
        }
    }

    #[test]
    fn selected_recursive_definitions_normalize_without_erasing_assertions() {
        let root = json!({"$id":"urn:recursive","$ref":"#/$defs/expression","$defs":{
            "expression":{"oneOf":[
                {"type":"string","minLength":1,"x-max-utf8-bytes":128,"pattern":"^[A-Za-z_][A-Za-z0-9_]*$","not":{"enum":["let"]}},
                {"type":"array","items":{"$ref":"#/$defs/expression"},"maxItems":2,"x-max-expression-nodes":4096},
                {"const":{"$ref":"#literal-data","$id":"literal-data"}}
            ]},
            "unused":{"type":"string"}
        }});
        let bundle = json!({"methods":[{"request_schema":{"properties":{"params":{"type":"object","properties":{}}}},"success_response_schema":{"properties":{"result":{"properties":{"payload":{"$ref":"urn:recursive"}}}}}}],
            "documents":[root],"unbundled_payload_schemas":[]});
        let docs = response_documents(&bundle).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs["urn:recursive"]["$ref"],
            "urn:recursive:response-def:expression"
        );
        let cases = &docs["urn:recursive:response-def:expression"]["oneOf"];
        assert_eq!(cases[0]["not"], json!({"enum":["let"]}));
        assert_eq!(cases[0]["x-max-utf8-bytes"], 128);
        assert_eq!(
            cases[1]["items"]["$ref"],
            "urn:recursive:response-def:expression"
        );
        assert!(cases[1].get("x-max-expression-nodes").is_none());
        assert_eq!(cases[2]["const"]["$ref"], "#literal-data");
        for hostile in [
            json!({"$ref":"#/$defs/missing"}),
            json!({"$ref":"https://example.invalid/schema"}),
            json!({"$ref":"urn:other#/$defs/x"}),
            json!({"$id":"urn:nested","type":"string"}),
            json!({"type":"string","x-unknown-validation":true}),
            json!({"type":"string","not":{"type":"string"}}),
            json!({"type":"string","not":{"enum":["let","let"]}}),
            json!({"type":"string","allOf":[{"minLength":1}]}),
        ] {
            let mut changed = bundle.clone();
            changed["documents"][0]["$defs"]["expression"]["oneOf"][0] = hostile;
            assert!(response_documents(&changed).is_err());
        }
        let mut collision = bundle;
        collision["documents"]
            .as_array_mut()
            .unwrap()
            .push(json!({"$id":"urn:recursive:response-def:expression","type":"string"}));
        assert!(response_documents(&collision).is_err());
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
