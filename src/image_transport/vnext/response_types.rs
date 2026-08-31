//! Concrete client response shapes from the same audited runtime schemas.
use super::{invalid, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[path = "response_types_rust.rs"]
mod rust;
#[path = "response_types_script.rs"]
mod script;

const MAX_TYPES: usize = 4096;
const MAX_WORK: usize = 65_536;
const MAX_DEPTH: usize = 128;
const MAX_SOURCE_BYTES: usize = 900 * 1024;

pub(super) struct GeneratedTypes {
    pub(super) source: String,
    pub(super) payloads: BTreeMap<String, String>,
}
pub(super) struct Model {
    /// Completion order. A guarded recursive edge may name a later definition.
    pub(super) definitions: Vec<Definition>,
    pub(super) payloads: BTreeMap<String, String>,
}
pub(super) struct Definition {
    pub(super) name: String,
    pub(super) shape: Shape,
}
pub(super) struct Field {
    pub(super) name: String,
    pub(super) ty: String,
    pub(super) required: bool,
}
pub(super) enum Shape {
    Any,
    Bool,
    Integer,
    String,
    Null,
    Literal(Value),
    Array(String),
    Tuple(Vec<String>),
    Object { fields: Vec<Field>, open: bool },
    Union(Vec<String>),
    Alias(String),
}

pub(super) fn generate(
    language: &str,
    methods: &[Value],
    documents: &BTreeMap<String, Value>,
    unbundled: &[Value],
) -> Result<GeneratedTypes> {
    let mut builder = Builder {
        documents,
        unbundled,
        definitions: Vec::new(),
        names: BTreeMap::new(),
        reservations: BTreeMap::new(),
        object_guards: Vec::new(),
        work: 0,
        key_bytes: 0,
    };
    let mut payloads = BTreeMap::new();
    for method in methods {
        let name = method["method"]
            .as_str()
            .ok_or_else(|| invalid("typed response method identity is missing"))?;
        let ty = builder.schema(
            &method["success_response_schema"]["properties"]["result"]["properties"]["payload"],
            0,
        )?;
        if payloads.insert(name.to_owned(), ty).is_some() {
            return Err(invalid("typed response method identities collide"));
        }
    }
    let mut model = Model {
        definitions: builder.definitions,
        payloads,
    };
    // Retain the established completion-order names for acyclic schemas. Only
    // recursive backedges use internal placeholders, resolved before emission.
    for definition in &mut model.definitions {
        visit_names_mut(&mut definition.shape, |name| {
            if let Some(resolved) = builder.reservations.get(name) {
                *name = resolved.clone();
            }
        });
    }
    let index = model
        .definitions
        .iter()
        .map(|definition| (definition.name.as_str(), &definition.shape))
        .collect::<BTreeMap<_, _>>();
    let mut done = BTreeSet::new();
    for definition in &model.definitions {
        productive(
            &definition.name,
            &index,
            &mut BTreeSet::new(),
            &mut done,
            0,
            &mut builder.work,
        )?;
    }
    for guarded in builder.object_guards {
        let name = builder.reservations.get(&guarded).unwrap_or(&guarded);
        if !object_shape(name, &index, &mut BTreeSet::new(), 0, &mut builder.work)? {
            return Err(invalid(
                "typed response reference does not preserve its object assertion",
            ));
        }
    }
    let source = match language {
        "rust" => rust::emit(&model)?,
        "typescript" => script::typescript(&model)?,
        "python" => script::python(&model)?,
        _ => return Err(invalid("typed response language is unsupported")),
    };
    if source.len() > MAX_SOURCE_BYTES {
        return Err(capacity("typed response source exceeds 900 KiB"));
    }
    Ok(GeneratedTypes {
        source,
        payloads: model.payloads,
    })
}

struct Builder<'a> {
    documents: &'a BTreeMap<String, Value>,
    unbundled: &'a [Value],
    definitions: Vec<Definition>,
    names: BTreeMap<String, String>,
    reservations: BTreeMap<String, String>,
    object_guards: Vec<String>,
    work: usize,
    key_bytes: usize,
}
impl Builder<'_> {
    fn charge(&mut self, depth: usize) -> Result<()> {
        self.work += 1;
        if self.work > MAX_WORK || depth > MAX_DEPTH {
            return Err(capacity(
                "typed response schema traversal exceeds its bound",
            ));
        }
        Ok(())
    }

    fn schema(&mut self, schema: &Value, depth: usize) -> Result<String> {
        self.charge(depth)?;
        let fields = schema
            .as_object()
            .ok_or_else(|| invalid("typed response schema must be an object"))?;
        let key = schema.to_string();
        if let Some(name) = self.names.get(&key) {
            return Ok(name.clone());
        }
        self.key_bytes = self.key_bytes.saturating_add(key.len());
        if self.key_bytes > 16 * 1024 * 1024 || self.names.len() >= MAX_TYPES {
            return Err(capacity("typed response schema inventory exceeds 16 MiB"));
        }
        let reserved = format!("ResponsePending{:04}", self.names.len());
        self.names.insert(key.clone(), reserved.clone());
        // The caller already audits validation keywords. Keep a second closed
        // structural boundary so future audit extensions cannot silently turn
        // unsupported schemas into permissive client types.
        for keyword in fields.keys() {
            if !matches!(
                keyword.as_str(),
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
                    | "uniqueItems"
                    | "not"
            ) {
                return Err(invalid("typed response schema keyword is unsupported"));
            }
        }
        audit_refinements(fields)?;
        let shape = if let Some(reference) = fields.get("$ref") {
            if fields.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "$ref" | "$id" | "$schema" | "title" | "description" | "type"
                )
            }) || fields.get("type").is_some_and(|value| value != "object")
            {
                return Err(invalid("typed response reference has unsupported siblings"));
            }
            let reference = reference
                .as_str()
                .ok_or_else(|| invalid("typed response reference must be a string"))?;
            if !reference.starts_with("urn:") || reference.contains('#') {
                return Err(invalid(
                    "typed response references require normalized absolute URNs",
                ));
            }
            if let Some(document) = self.documents.get(reference) {
                let target = self.schema(document, depth + 1)?;
                if fields.get("type").is_some() {
                    self.object_guards.push(target.clone());
                }
                Shape::Alias(target)
            } else if self.unbundled.iter().any(|value| value == reference) {
                if fields.get("type").is_some() {
                    return Err(invalid(
                        "opaque response reference cannot prove an object assertion",
                    ));
                }
                Shape::Any
            } else {
                return Err(invalid(
                    "typed response reference is not bundled or explicitly opaque",
                ));
            }
        } else if let Some(literal) = fields.get("const") {
            self.literal(literal, depth + 1)?
        } else if let Some(values) = fields.get("enum") {
            let values = values
                .as_array()
                .filter(|values| !values.is_empty())
                .ok_or_else(|| invalid("typed response enum must be nonempty"))?;
            let mut alternatives = Vec::new();
            for value in values {
                alternatives.push(self.schema(&serde_json::json!({"const":value}), depth + 1)?);
            }
            Shape::Union(alternatives)
        } else if fields.contains_key("oneOf") || fields.contains_key("anyOf") {
            if fields.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "oneOf" | "anyOf" | "$id" | "$schema" | "title" | "description"
                )
            }) || (fields.contains_key("oneOf") && fields.contains_key("anyOf"))
            {
                return Err(invalid(
                    "typed response alternatives require unsupported intersections",
                ));
            }
            let alternatives = fields
                .get("oneOf")
                .or_else(|| fields.get("anyOf"))
                .and_then(Value::as_array)
                .filter(|values| !values.is_empty())
                .ok_or_else(|| invalid("typed response alternatives must be nonempty"))?;
            let mut types = Vec::new();
            for alternative in alternatives {
                types.push(self.schema(alternative, depth + 1)?);
            }
            Shape::Union(types)
        } else {
            match fields.get("type").and_then(Value::as_str) {
                Some("boolean") => Shape::Bool,
                Some("integer") => Shape::Integer,
                Some("string") => Shape::String,
                Some("null") => Shape::Null,
                Some("array") => Shape::Array(self.schema(
                    fields.get("items").unwrap_or(&serde_json::json!({})),
                    depth + 1,
                )?),
                Some("object") => {
                    let empty = serde_json::Map::new();
                    let properties = match fields.get("properties") {
                        Some(value) => value.as_object().ok_or_else(|| {
                            invalid("typed response properties must be an object")
                        })?,
                        None => &empty,
                    };
                    let required = match fields.get("required") {
                        Some(value) => value
                            .as_array()
                            .ok_or_else(|| {
                                invalid("typed response required fields must be an array")
                            })?
                            .as_slice(),
                        None => &[],
                    };
                    for name in required {
                        if !name
                            .as_str()
                            .is_some_and(|name| properties.contains_key(name))
                        {
                            return Err(invalid("typed response requires an undescribed property"));
                        }
                    }
                    let mut members = Vec::new();
                    for (name, value) in properties {
                        members.push(Field {
                            name: name.clone(),
                            ty: self.schema(value, depth + 1)?,
                            required: required.iter().any(|value| value == name),
                        });
                    }
                    Shape::Object {
                        fields: members,
                        open: fields.get("additionalProperties") != Some(&Value::Bool(false)),
                    }
                }
                None if fields.keys().all(|key| {
                    matches!(key.as_str(), "$id" | "$schema" | "title" | "description")
                }) =>
                {
                    Shape::Any
                }
                _ => {
                    return Err(invalid(
                        "typed response schema lacks a supported concrete shape",
                    ))
                }
            }
        };
        if self.definitions.len() >= MAX_TYPES {
            return Err(capacity("typed response definition count exceeds 4096"));
        }
        let name = format!("ResponseType{:04}", self.definitions.len());
        self.definitions.push(Definition {
            name: name.clone(),
            shape,
        });
        self.reservations.insert(reserved, name.clone());
        self.names.insert(key, name.clone());
        Ok(name)
    }

    fn literal(&mut self, value: &Value, depth: usize) -> Result<Shape> {
        self.charge(depth)?;
        Ok(match value {
            Value::Object(values) => {
                let mut fields = Vec::new();
                for (name, value) in values {
                    fields.push(Field {
                        name: name.clone(),
                        ty: self.schema(&serde_json::json!({"const":value}), depth + 1)?,
                        required: true,
                    });
                }
                Shape::Object {
                    fields,
                    open: false,
                }
            }
            Value::Array(values) => {
                let mut items = Vec::new();
                for value in values {
                    items.push(self.schema(&serde_json::json!({"const":value}), depth + 1)?);
                }
                Shape::Tuple(items)
            }
            Value::Null => Shape::Null,
            Value::Number(number) if !(number.is_i64() || number.is_u64()) => {
                return Err(invalid(
                    "typed response literal number is outside the audited integer domain",
                ))
            }
            _ => Shape::Literal(value.clone()),
        })
    }
}

fn visit_names_mut(shape: &mut Shape, mut visit: impl FnMut(&mut String)) {
    match shape {
        Shape::Alias(name) | Shape::Array(name) => visit(name),
        Shape::Tuple(names) | Shape::Union(names) => names.iter_mut().for_each(visit),
        Shape::Object { fields, .. } => fields.iter_mut().for_each(|field| visit(&mut field.ty)),
        _ => {}
    }
}

fn names(shape: &Shape) -> Vec<&str> {
    match shape {
        Shape::Alias(name) | Shape::Array(name) => vec![name],
        Shape::Tuple(names) | Shape::Union(names) => names.iter().map(String::as_str).collect(),
        Shape::Object { fields, .. } => fields.iter().map(|field| field.ty.as_str()).collect(),
        _ => Vec::new(),
    }
}

fn charge(depth: usize, work: &mut usize) -> Result<()> {
    *work += 1;
    if depth > MAX_DEPTH || *work > MAX_WORK {
        return Err(capacity("typed response graph traversal exceeds its bound"));
    }
    Ok(())
}

fn productive<'a>(
    name: &'a str,
    index: &BTreeMap<&'a str, &'a Shape>,
    active: &mut BTreeSet<&'a str>,
    done: &mut BTreeSet<&'a str>,
    depth: usize,
    work: &mut usize,
) -> Result<()> {
    charge(depth, work)?;
    if done.contains(name) {
        return Ok(());
    }
    if !active.insert(name) {
        return Err(invalid(
            "typed response has unproductive alias or union recursion",
        ));
    }
    match index
        .get(name)
        .ok_or_else(|| invalid("typed response names an absent definition"))?
    {
        Shape::Alias(next) => productive(next, index, active, done, depth + 1, work)?,
        Shape::Union(alternatives) => {
            for next in alternatives {
                productive(next, index, active, done, depth + 1, work)?;
            }
        }
        _ => {}
    }
    active.remove(name);
    done.insert(name);
    Ok(())
}

fn object_shape<'a>(
    name: &'a str,
    index: &BTreeMap<&'a str, &'a Shape>,
    active: &mut BTreeSet<&'a str>,
    depth: usize,
    work: &mut usize,
) -> Result<bool> {
    charge(depth, work)?;
    if !active.insert(name) {
        return Ok(false);
    }
    let result = match index.get(name) {
        Some(Shape::Object { .. }) => true,
        Some(Shape::Alias(next)) => object_shape(next, index, active, depth + 1, work)?,
        Some(Shape::Union(alternatives)) => {
            let mut all = !alternatives.is_empty();
            for next in alternatives {
                all &= object_shape(next, index, active, depth + 1, work)?;
            }
            all
        }
        _ => false,
    };
    active.remove(name);
    Ok(result)
}

/// Linear bounded SCC analysis identifies exactly the definitions requiring
/// finite Rust recursion carriers. Acyclic generated representations stay put.
pub(super) fn recursive_names(model: &Model) -> Result<BTreeSet<String>> {
    let lookup = model
        .definitions
        .iter()
        .enumerate()
        .map(|(index, item)| (item.name.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut edges = vec![Vec::new(); model.definitions.len()];
    let mut reverse = vec![Vec::new(); model.definitions.len()];
    let mut work = 0;
    for (index, definition) in model.definitions.iter().enumerate() {
        charge(0, &mut work)?;
        for target in names(&definition.shape) {
            charge(0, &mut work)?;
            let target = *lookup
                .get(target)
                .ok_or_else(|| invalid("typed response names an absent definition"))?;
            edges[index].push(target);
            reverse[target].push(index);
        }
    }
    let mut seen = vec![false; edges.len()];
    let mut order = Vec::new();
    for start in 0..edges.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![(start, 0)];
        while let Some((node, next)) = stack.last_mut() {
            charge(0, &mut work)?;
            if let Some(child) = edges[*node].get(*next).copied() {
                *next += 1;
                if !seen[child] {
                    seen[child] = true;
                    stack.push((child, 0));
                    if stack.len() > MAX_DEPTH {
                        return Err(capacity(
                            "typed response dependency depth exceeds its bound",
                        ));
                    }
                }
            } else {
                order.push(*node);
                stack.pop();
            }
        }
    }
    seen.fill(false);
    let mut result = BTreeSet::new();
    for start in order.into_iter().rev() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut component = Vec::new();
        let mut pending = vec![start];
        while let Some(node) = pending.pop() {
            charge(0, &mut work)?;
            component.push(node);
            for child in &reverse[node] {
                charge(0, &mut work)?;
                if !seen[*child] {
                    seen[*child] = true;
                    pending.push(*child);
                }
            }
        }
        if component.len() > 1 || edges[start].contains(&start) {
            result.extend(
                component
                    .into_iter()
                    .map(|index| model.definitions[index].name.clone()),
            );
        }
    }
    Ok(result)
}

fn audit_refinements(fields: &serde_json::Map<String, Value>) -> Result<()> {
    let ty = fields.get("type").and_then(Value::as_str);
    for (kind, keys) in [
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
        if keys.iter().any(|key| fields.contains_key(*key)) && ty != Some(kind) {
            return Err(invalid(
                "typed response constraint requires its matching explicit type",
            ));
        }
    }
    if fields
        .get("uniqueItems")
        .is_some_and(|value| !value.is_boolean())
        || fields
            .get("additionalProperties")
            .is_some_and(|value| !value.is_boolean())
    {
        return Err(invalid("typed response boolean constraint is malformed"));
    }
    if let Some(not) = fields.get("not") {
        if !not.as_object().is_some_and(|object| {
            object.len() == 1
                && object
                    .get("enum")
                    .and_then(Value::as_array)
                    .is_some_and(|values| !values.is_empty() && values.iter().all(Value::is_string))
        }) {
            return Err(invalid(
                "typed response exclusion must be a finite string enum",
            ));
        }
    }
    if let Some(ty) = ty {
        let matches = |value: &Value| match ty {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "integer" => value.is_i64() || value.is_u64(),
            "null" => value.is_null(),
            _ => false,
        };
        if fields.get("const").is_some_and(|value| !matches(value))
            || fields
                .get("enum")
                .and_then(Value::as_array)
                .is_some_and(|values| values.iter().any(|value| !matches(value)))
        {
            return Err(invalid(
                "typed response literal disagrees with its explicit type",
            ));
        }
    }
    Ok(())
}

fn capacity(message: &'static str) -> Vec<crate::diagnostic::Diagnostic> {
    vec![crate::diagnostic::Diagnostic::io("SPX-G289", message)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn methods(payload: Value) -> Vec<Value> {
        vec![
            json!({"method":"probe/read","success_response_schema":{"properties":{
                "result":{"properties":{"payload":payload}}
            }}}),
        ]
    }

    #[test]
    fn response_references_reject_unguarded_cycles_and_require_explicit_opaque_inventory() {
        let request = methods(json!({"$ref":"urn:outer"}));
        let cycle = BTreeMap::from([
            ("urn:outer".into(), json!({"$ref":"urn:inner"})),
            ("urn:inner".into(), json!({"$ref":"urn:outer"})),
        ]);
        for language in ["rust", "python", "typescript"] {
            let errors = generate(language, &request, &cycle, &[]).err().unwrap();
            assert_eq!(errors[0].code, "SPX-G288");
            assert!(errors[0].message.contains("recursion"));
            let errors = generate(language, &request, &BTreeMap::new(), &[])
                .err()
                .unwrap();
            assert_eq!(errors[0].code, "SPX-G288");
            assert!(errors[0].message.contains("not bundled"));
            let result =
                generate(language, &request, &BTreeMap::new(), &[json!("urn:outer")]).unwrap();
            assert_eq!(result.payloads.len(), 1);
        }
        let errors = generate("rust", &methods(json!({"allOf":[]})), &BTreeMap::new(), &[])
            .err()
            .unwrap();
        assert_eq!(errors[0].code, "SPX-G288");
    }

    #[test]
    fn literal_reference_keys_remain_typed_data_without_schema_lookup() {
        let request = methods(
            json!({"const":{"$ref":"urn:not-a-schema","required":null,"items":[true,"x"]}}),
        );
        let generated = generate("rust", &request, &BTreeMap::new(), &[]).unwrap();
        assert!(generated.source.contains("#[serde(rename = \"$ref\")]"));
        assert!(generated.source.contains("urn:not-a-schema"));
        assert!(generated.source.contains("response literal mismatch"));
        assert!(generated.source.contains("#[serde(deny_unknown_fields)]"));
    }

    #[test]
    fn guarded_recursive_objects_and_arrays_have_finite_named_client_representations() {
        let request = methods(json!({"$ref":"urn:node"}));
        let documents = BTreeMap::from([(
            "urn:node".into(),
            json!({
                "type":"object","additionalProperties":false,"required":["value"],
                "properties":{"value":{"type":"integer"},
                    "parent":{"$ref":"urn:node"},
                    "next":{"anyOf":[{"$ref":"urn:node"},{"type":"null"}]},
                    "children":{"type":"array","items":{"$ref":"urn:node"}}}
            }),
        )]);
        for language in ["rust", "python", "typescript"] {
            let generated = generate(language, &request, &documents, &[]).unwrap();
            assert!(!generated.source.contains("ResponsePending"));
            assert_eq!(
                generated.source,
                generate(language, &request, &documents, &[])
                    .unwrap()
                    .source
            );
            assert_eq!(generated.payloads.len(), 1);
            match language {
                "rust" => {
                    assert!(generated.source.contains("#[serde(transparent)]"));
                    assert!(generated.source.contains("Presence<Box<ResponseType"));
                    assert!(generated.source.contains("Vec<ResponseType"));
                    assert!(generated.source.contains("Signed(i64), Unsigned(u64)"));
                }
                "python" => {
                    assert!(generated.source.contains("NotRequired[\"ResponseType"));
                    assert!(generated.source.contains("list[\"ResponseType"));
                }
                "typescript" => assert!(generated.source.contains("Array<ResponseType")),
                _ => unreachable!(),
            }
        }
        let array = BTreeMap::from([(
            "urn:node".into(),
            json!({"type":"array","items":{"$ref":"urn:node"}}),
        )]);
        let generated = generate("rust", &request, &array, &[]).unwrap();
        assert!(generated.source.contains("(pub Vec<ResponseType"));
        assert!(generated.source.contains("(pub Box<ResponseType"));
    }

    #[test]
    fn terminating_union_branch_does_not_authorize_unguarded_recursion() {
        let documents = BTreeMap::from([
            (
                "urn:a".into(),
                json!({"anyOf":[{"$ref":"urn:b"},{"type":"integer"}]}),
            ),
            ("urn:b".into(), json!({"$ref":"urn:a"})),
        ]);
        for language in ["rust", "python", "typescript"] {
            let error = generate(language, &methods(json!({"$ref":"urn:a"})), &documents, &[])
                .err()
                .unwrap();
            assert_eq!(error[0].code, "SPX-G288");
            assert!(error[0].message.contains("unproductive"));
        }
        let documents = BTreeMap::from([(
            "urn:a".into(),
            json!({"type":"array","items":{"type":"integer"}}),
        )]);
        assert!(generate(
            "rust",
            &methods(json!({"type":"object","$ref":"urn:a"})),
            &documents,
            &[]
        )
        .is_err());
        assert!(generate(
            "rust",
            &methods(json!({"type":"integer","const":"wrong"})),
            &BTreeMap::new(),
            &[]
        )
        .is_err());
    }

    #[test]
    fn recursive_component_analysis_includes_cross_edges_to_finished_branches() {
        let model = Model {
            payloads: BTreeMap::new(),
            definitions: vec![
                Definition {
                    name: "A".into(),
                    shape: Shape::Object {
                        open: false,
                        fields: vec![
                            Field {
                                name: "b".into(),
                                ty: "B".into(),
                                required: true,
                            },
                            Field {
                                name: "d".into(),
                                ty: "D".into(),
                                required: false,
                            },
                        ],
                    },
                },
                Definition {
                    name: "B".into(),
                    shape: Shape::Alias("C".into()),
                },
                Definition {
                    name: "C".into(),
                    shape: Shape::Alias("A".into()),
                },
                Definition {
                    name: "D".into(),
                    shape: Shape::Alias("C".into()),
                },
                Definition {
                    name: "Leaf".into(),
                    shape: Shape::Integer,
                },
            ],
        };
        assert_eq!(
            recursive_names(&model).unwrap(),
            ["A", "B", "C", "D"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
    }

    #[test]
    fn recursive_rust_union_checks_discriminants_before_children_and_bounds_retries() {
        let documents = BTreeMap::from([(
            "urn:expr".into(),
            json!({"oneOf":[
                {"type":"object","additionalProperties":false,"required":["kind","arguments"],
                    "properties":{"arguments":{"type":"array","items":{"$ref":"urn:expr"}},
                        "kind":{"const":"call"}}},
                {"type":"object","additionalProperties":false,"required":["kind","target","arguments"],
                    "properties":{"arguments":{"type":"array","items":{"$ref":"urn:expr"}},
                        "kind":{"const":"builtin_call"},"target":{"const":"core.bytes.len"}}},
                {"type":"integer"}
            ]}),
        )]);
        let generated = generate(
            "rust",
            &methods(json!({"$ref":"urn:expr"})),
            &documents,
            &[],
        )
        .unwrap();
        let dispatch_and_helpers = generated
            .source
            .split("        for branch in 0..3 {")
            .nth(1)
            .unwrap();
        let (dispatcher, helpers) = dispatch_and_helpers
            .split_once("\nimpl ResponseType")
            .unwrap();
        let charge = dispatcher
            .find("ResponseTypeDecodeGuard::charge()")
            .unwrap();
        let selection = dispatcher
            .find("let convert:Option<fn(&Value)->Result<Self,serde_json::Error>>=match branch")
            .unwrap();
        let conversion = dispatcher.find("convert(&value)").unwrap();
        assert!(charge < selection && selection < conversion);
        assert_eq!(dispatcher.matches("convert(&value)").count(), 1);
        assert!(!dispatcher.contains("serde_json::from_value"));
        assert!(!dispatcher.contains("value.clone()"));
        let sticky = dispatcher.find("ResponseTypeDecodeGuard::check()").unwrap();
        assert!(conversion < sticky && sticky < dispatcher.find("return Ok(parsed)").unwrap());
        assert_eq!(
            dispatcher
                .matches("ResponseTypeDecodeGuard::check()")
                .count(),
            2
        );
        // Both recursive object alternatives inspect their discriminants before
        // choosing an outlined conversion, including the builtin's target.
        for index in 0..2 {
            let arm = dispatcher
                .split(&format!("{index} if value.is_object()"))
                .nth(1)
                .unwrap()
                .split('\n')
                .next()
                .unwrap();
            let constant = arm.find("value.get(\"kind\")").unwrap();
            let selected = arm
                .find(&format!("Some(Self::__response_decode_choice_{index})"))
                .unwrap();
            assert!(constant < selected);
            if index == 1 {
                assert!(arm.find("value.get(\"target\")").unwrap() < selected);
            }
        }
        // Debug builds must not reserve all branch-specific serde temporaries
        // in the recursive dispatcher. Keep each conversion in its own frame.
        for index in 0..3 {
            let signature = format!("    #[inline(never)]\n    fn __response_decode_choice_{index}(value:&Value)->Result<Self,serde_json::Error> {{");
            let helper = helpers
                .split(&signature)
                .nth(1)
                .unwrap()
                .split("\n    }")
                .next()
                .unwrap();
            assert_eq!(
                helper
                    .matches(" as Deserialize>::deserialize(value)")
                    .count(),
                1
            );
            assert!(!helper.contains("serde_json::from_value"));
            assert!(!helper.contains("value.clone()"));
            assert!(helper.contains(&format!(".map(Self::Choice{index})")));
        }
        assert!(generated
            .source
            .contains("literal.as_str()==Some(\"core.bytes.len\")"));
        assert!(generated
            .source
            .contains("let _budget=ResponseTypeDecodeGuard::enter()?;"));
        assert!(generated.source.contains("remaining=65_536"));
        assert!(generated.source.contains("depth>=128"));
        assert!(generated
            .source
            .contains("state.set((remaining,depth,true))"));
        assert!(generated
            .source
            .contains("if depth==0 { remaining=65_536; failed=false; }"));
        assert!(generated
            .source
            .contains("ResponseTypeDecodeGuard::check()?;\n    let payload=payload.map_err"));
    }

    #[test]
    fn optional_nullable_fields_keep_presence_separate_from_required_null() {
        let nullable = json!({"anyOf":[{"type":"string"},{"type":"null"}]});
        let schema = json!({"type":"object","additionalProperties":false,
            "required":["must"],"properties":{"must":nullable,"maybe":nullable}});
        let documents = BTreeMap::new();
        let mut builder = Builder {
            documents: &documents,
            unbundled: &[],
            definitions: Vec::new(),
            names: BTreeMap::new(),
            reservations: BTreeMap::new(),
            object_guards: Vec::new(),
            work: 0,
            key_bytes: 0,
        };
        let root = builder.schema(&schema, 0).unwrap();
        let object = builder
            .definitions
            .iter()
            .find(|definition| definition.name == root)
            .unwrap();
        let Shape::Object {
            fields,
            open: false,
        } = &object.shape
        else {
            panic!("closed response object expected")
        };
        let must = fields.iter().find(|field| field.name == "must").unwrap();
        let maybe = fields.iter().find(|field| field.name == "maybe").unwrap();
        assert!(must.required);
        assert!(!maybe.required);
        assert_eq!(must.ty, maybe.ty);
        let generated = generate("rust", &methods(schema), &documents, &[]).unwrap();
        assert!(generated
            .source
            .contains(&format!("pub r#must: {},", must.ty)));
        assert!(generated
            .source
            .contains(&format!("pub r#maybe: Presence<{}>,", maybe.ty)));
        assert!(generated.source.contains("impl<T> Default for Presence<T>"));
        assert!(generated
            .source
            .contains("T::deserialize(deserializer).map(Self::Present)"));
    }

    #[test]
    fn large_literal_arrays_keep_each_position_and_full_integer_extremes() {
        let mut values = (0..32).map(|value| json!(value)).collect::<Vec<_>>();
        values.extend([json!(i64::MIN), json!(u64::MAX)]);
        let generated = generate(
            "rust",
            &methods(json!({"const":values})),
            &BTreeMap::new(),
            &[],
        )
        .unwrap();
        assert!(generated.source.contains("pub item_33:"));
        assert!(generated
            .source
            .contains("deserializer.deserialize_tuple(34,SequenceVisitor)"));
        assert!(generated
            .source
            .contains("sequence.next_element::<serde::de::IgnoredAny>()?"));
        assert!(generated
            .source
            .contains("serde::ser::SerializeTuple::serialize_element"));
        assert!(generated.source.contains("-9223372036854775808"));
        assert!(generated.source.contains("18446744073709551615"));
        assert!(generated.source.contains("Signed(i64), Unsigned(u64)"));
        let thirteen = (0..13).map(|value| json!(value)).collect::<Vec<_>>();
        let nested = generate(
            "rust",
            &methods(json!({"const":{"items":thirteen}})),
            &BTreeMap::new(),
            &[],
        )
        .unwrap();
        assert!(nested.source.contains("pub item_12:"));
        assert!(nested
            .source
            .contains("deserializer.deserialize_tuple(13,SequenceVisitor)"));
    }
}
