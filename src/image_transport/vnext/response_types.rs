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
#[path = "response_types/tests.rs"]
mod tests;
