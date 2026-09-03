//! Pure MCP tool descriptions derived from the exact selected v5 registry.
//! No output-schema claim is made for the deliberately opaque inner response.
#[cfg(test)]
use super::methods;
use super::{discovery, session_methods, VNextPolicy};
use crate::diagnostic::Diagnostic;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
const MAX_TOOLS: usize = 256;
const MAX_CATALOG_BYTES: usize = 16 * 1024 * 1024;
const MAX_PAGE_BYTES: usize = 900 * 1024;
const MAX_PAGE_TOOLS: usize = 8;
const MAX_SCHEMA_DEPTH: usize = 128;
const CURSOR_RESERVE: usize = 256;

pub(super) struct Catalog {
    tools: Vec<Value>,
    selected: BTreeMap<String, &'static str>,
    page_starts: Vec<usize>,
    identity: String,
}

impl Catalog {
    #[cfg(test)]
    pub(super) fn new(policy: &VNextPolicy, commit_enabled: bool) -> Result<Self> {
        Self::new_with_package(policy, commit_enabled, false, false, false)
    }
    pub(super) fn new_with_package(
        policy: &VNextPolicy,
        commit_enabled: bool,
        package_attached: bool,
        read_batch_selected: bool,
        candidate_archive_store_selected: bool,
    ) -> Result<Self> {
        let selected_methods = session_methods(
            policy,
            commit_enabled,
            package_attached,
            read_batch_selected,
            candidate_archive_store_selected,
        );
        if selected_methods.len() > MAX_TOOLS {
            return Err(capacity("MCP selected tool count exceeds 256"));
        }
        let schema_method = selected_methods
            .iter()
            .find(|method| method.name == "protocol/schemas")
            .ok_or_else(|| invalid("MCP catalogue lacks the v5 schema method"))?;
        let bundle = discovery::payload(
            schema_method,
            &Map::new(),
            &selected_methods,
            policy,
            commit_enabled,
        )?;
        let mut documents = BTreeMap::new();
        for document in bundle["documents"]
            .as_array()
            .ok_or_else(|| invalid("v5 schema bundle lacks documents"))?
        {
            let id = document["$id"]
                .as_str()
                .ok_or_else(|| invalid("v5 schema document lacks identity"))?;
            if documents.insert(id.to_owned(), document.clone()).is_some() {
                return Err(invalid("v5 schema document identities collide"));
            }
        }
        let descriptors = bundle["methods"]
            .as_array()
            .ok_or_else(|| invalid("v5 schema bundle lacks method descriptors"))?;
        let mut selected = BTreeMap::new();
        let mut tool_map = BTreeMap::new();
        let mut catalog_bytes = 2usize;
        for method in selected_methods {
            let name = tool_name(method.name)?;
            if selected.insert(name.clone(), method.name).is_some() {
                return Err(invalid("selected v5 methods collide as MCP tool names"));
            }
            let descriptor = descriptors
                .iter()
                .find(|descriptor| descriptor["method"] == method.name)
                .ok_or_else(|| invalid("selected v5 method has no descriptor"))?;
            let params = &descriptor["request_schema"]["properties"]["params"];
            if params["type"] != "object" || params["additionalProperties"] != false {
                return Err(invalid(
                    "selected v5 parameters are not a closed object schema",
                ));
            }
            let input_schema = self_contained(params, &documents)?;
            let tool = json!({
                "name":name,
                "title":method.name,
                "description":format!("Invoke the host-selected SEMAPRAX v5 method {}. Arguments are its exact params object. The result text is the complete v5 JSON-RPC response with inner id 0; existing revision checks and host grants remain authoritative.", method.name),
                "inputSchema":input_schema,
            });
            let bytes = encoded_len(&tool)?;
            if bytes + 1 + CURSOR_RESERVE > MAX_PAGE_BYTES {
                return Err(capacity("one MCP tool exceeds the bounded catalogue page"));
            }
            catalog_bytes = catalog_bytes
                .checked_add(bytes + 1)
                .ok_or_else(|| capacity("MCP catalogue size overflow"))?;
            if catalog_bytes > MAX_CATALOG_BYTES {
                return Err(capacity("MCP catalogue exceeds 16 MiB"));
            }
            tool_map.insert(name, tool);
        }
        let tools = tool_map.into_values().collect::<Vec<_>>();
        let mut hash = Sha256::new();
        hash.update(b"semaprax.mcp-tool-catalogue.v1\0MCP2025-11-25\0");
        hash.update(
            serde_json::to_vec(&tools).map_err(|_| invalid("MCP catalogue encoding failed"))?,
        );
        let identity = format!("{:x}", crate::digest_hex::LowerHex(hash.finalize()));
        let mut page_starts = vec![0];
        let mut page_bytes = CURSOR_RESERVE;
        let mut page_count = 0;
        for (index, tool) in tools.iter().enumerate() {
            let bytes = encoded_len(tool)? + 1;
            if page_count == MAX_PAGE_TOOLS || page_bytes + bytes > MAX_PAGE_BYTES {
                page_starts.push(index);
                page_bytes = CURSOR_RESERVE;
                page_count = 0;
            }
            page_bytes += bytes;
            page_count += 1;
        }
        Ok(Self {
            tools,
            selected,
            page_starts,
            identity,
        })
    }

    /// Only names in the selected closed inventory can route a call.
    pub(super) fn method(&self, name: &str) -> Option<&'static str> {
        self.selected.get(name).copied()
    }

    pub(super) fn page(&self, cursor: Option<&str>) -> Result<Value> {
        let start = if let Some(cursor) = cursor {
            if cursor.len() > 128 {
                return Err(invalid("MCP catalogue cursor exceeds its bound"));
            }
            let prefix = format!("mcp1:{}:", self.identity);
            let offset = cursor
                .strip_prefix(&prefix)
                .ok_or_else(|| invalid("MCP cursor does not identify this selected catalogue"))?;
            let index = offset
                .parse::<usize>()
                .map_err(|_| invalid("MCP catalogue cursor offset is invalid"))?;
            if offset != index.to_string() || index == 0 {
                return Err(invalid("MCP catalogue cursor offset is not canonical"));
            }
            index
        } else {
            0
        };
        let page = self
            .page_starts
            .binary_search(&start)
            .map_err(|_| invalid("MCP catalogue cursor does not select a page boundary"))?;
        let end = self
            .page_starts
            .get(page + 1)
            .copied()
            .unwrap_or(self.tools.len());
        let mut result = json!({"tools":self.tools[start..end]});
        if end < self.tools.len() {
            result["nextCursor"] = json!(format!("mcp1:{}:{end}", self.identity));
        }
        if encoded_len(&result)? > MAX_PAGE_BYTES {
            return Err(capacity("MCP catalogue page exceeds 900 KiB"));
        }
        Ok(result)
    }
}

fn tool_name(method: &str) -> Result<String> {
    let name = method.replace('/', "__");
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(invalid(
            "selected v5 method cannot form a legal MCP tool name",
        ));
    }
    Ok(name)
}

/// Preserve recursion without external URNs or nested resource scopes. Only
/// schema positions are visited: `$ref` strings inside literal const/enum data
/// must not be interpreted as references.
fn self_contained(params: &Value, documents: &BTreeMap<String, Value>) -> Result<Value> {
    if params.get("$defs").is_some() || params.get("$id").is_some() {
        return Err(invalid(
            "v5 parameter schema unexpectedly owns a resource scope",
        ));
    }
    let mut reached = BTreeSet::new();
    let mut pending = references(params, "", documents)?;
    while let Some(id) = pending.pop_first() {
        if !reached.insert(id.clone()) {
            continue;
        }
        let document = documents
            .get(&id)
            .ok_or_else(|| invalid("MCP input schema reference is not bundled"))?;
        for dependency in references(document, &id, documents)? {
            if !reached.contains(&dependency) {
                pending.insert(dependency);
            }
        }
    }
    let aliases = reached
        .into_iter()
        .enumerate()
        .map(|(index, id)| (id, format!("d{index}")))
        .collect::<BTreeMap<_, _>>();
    let mut root = params.clone();
    rewrite(&mut root, "", documents, &aliases)?;
    let mut definitions = Map::new();
    for (id, alias) in &aliases {
        let mut document = documents[id].clone();
        rewrite(&mut document, id, documents, &aliases)?;
        definitions.insert(alias.clone(), document);
    }
    if !definitions.is_empty() {
        root["$defs"] = Value::Object(definitions);
    }
    root["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    Ok(root)
}

fn references(
    schema: &Value,
    scope: &str,
    documents: &BTreeMap<String, Value>,
) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    let mut copy = schema.clone();
    walk_schema(&mut copy, 0, &mut |object| {
        check_resource(object, scope)?;
        if let Some(reference) = object.get("$ref") {
            let reference = reference
                .as_str()
                .ok_or_else(|| invalid("schema reference is not a string"))?;
            ids.insert(resolve(reference, scope, documents)?.0.to_owned());
        }
        Ok(())
    })?;
    Ok(ids)
}

fn rewrite(
    schema: &mut Value,
    scope: &str,
    documents: &BTreeMap<String, Value>,
    aliases: &BTreeMap<String, String>,
) -> Result<()> {
    walk_schema(schema, 0, &mut |object| {
        check_resource(object, scope)?;
        object.remove("$id");
        if let Some(reference) = object.get("$ref") {
            let reference = reference
                .as_str()
                .ok_or_else(|| invalid("schema reference is not a string"))?;
            let (id, pointer) = resolve(reference, scope, documents)?;
            let alias = aliases
                .get(id)
                .ok_or_else(|| invalid("MCP input schema dependency was not collected"))?;
            object.insert("$ref".into(), json!(format!("#/$defs/{alias}{pointer}")));
        }
        Ok(())
    })
}

fn check_resource(object: &Map<String, Value>, scope: &str) -> Result<()> {
    if object
        .get("$id")
        .is_some_and(|id| id.as_str() != Some(scope))
        || [
            "$anchor",
            "$dynamicAnchor",
            "$dynamicRef",
            "$recursiveRef",
            "$recursiveAnchor",
        ]
        .iter()
        .any(|key| object.contains_key(*key))
    {
        return Err(invalid(
            "MCP input schema uses an unsupported nested resource or anchor",
        ));
    }
    Ok(())
}

fn resolve<'a>(
    reference: &'a str,
    scope: &'a str,
    documents: &BTreeMap<String, Value>,
) -> Result<(&'a str, &'a str)> {
    let (base, pointer) = reference.split_once('#').unwrap_or((reference, ""));
    let id = if base.is_empty() { scope } else { base };
    let document = documents
        .get(id)
        .ok_or_else(|| invalid("MCP input schema has an unresolved document reference"))?;
    if (!pointer.is_empty() && !pointer.starts_with('/')) || document.pointer(pointer).is_none() {
        return Err(invalid("MCP input schema has an unresolved JSON pointer"));
    }
    Ok((id, pointer))
}

fn walk_schema(
    schema: &mut Value,
    depth: usize,
    visit: &mut impl FnMut(&mut Map<String, Value>) -> Result<()>,
) -> Result<()> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(capacity("MCP schema traversal exceeds its depth bound"));
    }
    if schema.is_boolean() {
        return Ok(());
    }
    let object = schema
        .as_object_mut()
        .ok_or_else(|| invalid("MCP schema position is neither an object nor boolean"))?;
    visit(object)?;
    for key in [
        "$defs",
        "definitions",
        "properties",
        "patternProperties",
        "dependentSchemas",
    ] {
        if let Some(children) = object.get_mut(key) {
            for child in children
                .as_object_mut()
                .ok_or_else(|| invalid("MCP schema map is malformed"))?
                .values_mut()
            {
                walk_schema(child, depth + 1, visit)?;
            }
        }
    }
    for key in [
        "additionalProperties",
        "unevaluatedProperties",
        "propertyNames",
        "items",
        "contains",
        "additionalItems",
        "unevaluatedItems",
        "not",
        "if",
        "then",
        "else",
    ] {
        if let Some(child) = object.get_mut(key) {
            walk_schema(child, depth + 1, visit)?;
        }
    }
    for key in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(children) = object.get_mut(key) {
            for child in children
                .as_array_mut()
                .ok_or_else(|| invalid("MCP schema alternatives are malformed"))?
            {
                walk_schema(child, depth + 1, visit)?;
            }
        }
    }
    Ok(())
}

fn encoded_len(value: &Value) -> Result<usize> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|_| invalid("MCP catalogue encoding failed"))
}
fn invalid(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G350", message)]
}
fn capacity(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G351", message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursive_documents_resolve_locally_without_rewriting_literals() {
        let documents = BTreeMap::from([
            (
                "urn:a".into(),
                json!({"$id":"urn:a", "$ref":"#/$defs/node", "$defs":{
                    "node":{"type":"object","properties":{"next":{"$ref":"#/$defs/node"},"other":{"$ref":"urn:b"}}}
                }}),
            ),
            (
                "urn:b".into(),
                json!({"$id":"urn:b","const":{"$ref":"literal-not-a-schema-reference"}}),
            ),
        ]);
        let params = json!({"type":"object","additionalProperties":false,"properties":{"value":{"$ref":"urn:a"}}});
        let resolved = self_contained(&params, &documents).unwrap();
        assert_eq!(resolved["properties"]["value"]["$ref"], "#/$defs/d0");
        assert_eq!(resolved["$defs"]["d0"]["$ref"], "#/$defs/d0/$defs/node");
        assert_eq!(
            resolved["$defs"]["d0"]["$defs"]["node"]["properties"]["next"]["$ref"],
            "#/$defs/d0/$defs/node"
        );
        assert_eq!(
            resolved["$defs"]["d1"]["const"]["$ref"],
            "literal-not-a-schema-reference"
        );
        let mut copy = resolved.clone();
        walk_schema(&mut copy, 0, &mut |schema| {
            assert!(!schema.contains_key("$id"));
            if let Some(reference) = schema.get("$ref") {
                let pointer = reference.as_str().unwrap().strip_prefix('#').unwrap();
                assert!(resolved.pointer(pointer).is_some());
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn missing_schema_dependencies_fail_closed() {
        let params = json!({"type":"object","properties":{"value":{"$ref":"urn:missing"}}});
        let errors = self_contained(&params, &BTreeMap::new()).unwrap_err();
        assert_eq!(errors[0].code, "SPX-G350");
    }

    #[test]
    fn selected_catalogue_pages_are_complete_bound_and_grant_specific() {
        let policy = VNextPolicy {
            candidate_prepare: true,
            diagnostics: true,
            build_enabled: true,
            test_policy: None,
        };
        let catalog = Catalog::new(&policy, true).unwrap();
        let mut cursor = None;
        let mut names = Vec::new();
        let mut first_cursor = None;
        loop {
            let page = catalog.page(cursor.as_deref()).unwrap();
            assert!(encoded_len(&page).unwrap() <= MAX_PAGE_BYTES);
            let tools = page["tools"].as_array().unwrap();
            assert!(!tools.is_empty() && tools.len() <= MAX_PAGE_TOOLS);
            for tool in tools {
                assert!(tool.get("outputSchema").is_none());
                assert_eq!(tool["inputSchema"]["additionalProperties"], false);
                let input = &tool["inputSchema"];
                let mut copy = input.clone();
                walk_schema(&mut copy, 0, &mut |schema| {
                    assert!(!schema.contains_key("$id"));
                    if let Some(reference) = schema.get("$ref") {
                        let pointer = reference.as_str().unwrap().strip_prefix('#').unwrap();
                        assert!(input.pointer(pointer).is_some());
                    }
                    Ok(())
                })
                .unwrap();
                names.push(tool["name"].as_str().unwrap().to_owned());
            }
            cursor = page["nextCursor"].as_str().map(str::to_owned);
            if first_cursor.is_none() {
                first_cursor = cursor.clone();
            }
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(names.len(), methods(&policy, true).len());
        assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            catalog.method("candidate__apply-intent"),
            Some("candidate/apply-intent")
        );
        assert_eq!(catalog.method("candidate/apply-intent"), None);
        let readonly = Catalog::new(&VNextPolicy::default(), false).unwrap();
        assert_eq!(readonly.method("candidate__apply-intent"), None);
        assert_eq!(
            readonly.page(first_cursor.as_deref()).unwrap_err()[0].code,
            "SPX-G350"
        );
        assert_eq!(
            catalog
                .page(Some(&format!("mcp1:{}:01", catalog.identity)))
                .unwrap_err()[0]
                .code,
            "SPX-G350"
        );
    }
}
