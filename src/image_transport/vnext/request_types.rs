//! Recursive structural request types derived from selected compiler schemas.
//! These representations cannot establish lexical or semantic admission.
use super::{invalid, Result};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[path = "request_types_rust.rs"]
mod rust;
#[path = "request_types_script.rs"]
mod script;

const MAX_TYPES: usize = 4096;
const MAX_WORK: usize = 65_536;
const MAX_DEPTH: usize = 128;
const MAX_KEYS_BYTES: usize = 16 * 1024 * 1024;
const MAX_SOURCE_BYTES: usize = 900 * 1024;

pub(super) struct GeneratedTypes {
    pub(super) source: String,
    pub(super) params: BTreeMap<String, String>,
}
pub(super) struct Model {
    /// Reservation order; references may point forward or through structures.
    pub(super) definitions: Vec<Definition>,
    pub(super) params: BTreeMap<String, String>,
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
) -> Result<GeneratedTypes> {
    let model = build(methods, documents)?;
    let source = match language {
        "rust" => rust::emit(&model)?,
        "typescript" => script::typescript(&model)?,
        "python" => script::python(&model)?,
        _ => return Err(invalid("typed request language is unsupported")),
    };
    if source.len() > MAX_SOURCE_BYTES {
        return Err(capacity("typed request source exceeds 900 KiB"));
    }
    Ok(GeneratedTypes {
        source,
        params: model.params,
    })
}

fn build(methods: &[Value], documents: &BTreeMap<String, Value>) -> Result<Model> {
    for (id, document) in documents {
        if !id.starts_with("urn:") || id.contains('#') || document["$id"] != *id {
            return Err(invalid("typed request document identity is inconsistent"));
        }
    }
    let mut builder = Builder {
        documents,
        definitions: Vec::new(),
        names: BTreeMap::new(),
        object_guards: Vec::new(),
        work: 0,
        key_bytes: 0,
    };
    let mut params = BTreeMap::new();
    for descriptor in methods {
        let method = descriptor["method"]
            .as_str()
            .ok_or_else(|| invalid("typed request method identity is missing"))?;
        let schema = &descriptor["request_schema"]["properties"]["params"];
        if schema["type"] != "object" || schema["additionalProperties"] != false {
            return Err(invalid("typed request parameters must be a closed object"));
        }
        let ty = builder.schema(schema, "", 0, false)?;
        if params.insert(method.to_owned(), ty).is_some() {
            return Err(invalid("typed request method identities collide"));
        }
    }
    let definitions = builder
        .definitions
        .into_iter()
        .map(|definition| {
            definition.ok_or_else(|| invalid("typed request definition remained unresolved"))
        })
        .collect::<Result<Vec<_>>>()?;
    let model = Model {
        definitions,
        params,
    };
    let index = model
        .definitions
        .iter()
        .map(|definition| (definition.name.as_str(), &definition.shape))
        .collect::<BTreeMap<_, _>>();
    let mut work = builder.work;
    let mut done = BTreeSet::new();
    for definition in &model.definitions {
        productive(
            &definition.name,
            &index,
            &mut BTreeSet::new(),
            &mut done,
            0,
            &mut work,
        )?;
    }
    let mut object_results = BTreeMap::new();
    for guarded in builder.object_guards {
        if !object_shape(&guarded, &index, &mut object_results, 0, &mut work)? {
            return Err(invalid(
                "typed request reference does not preserve its object assertion",
            ));
        }
    }
    Ok(model)
}

struct Builder<'a> {
    documents: &'a BTreeMap<String, Value>,
    definitions: Vec<Option<Definition>>,
    names: BTreeMap<(String, String), String>,
    object_guards: Vec<String>,
    work: usize,
    key_bytes: usize,
}
impl Builder<'_> {
    fn schema(
        &mut self,
        schema: &Value,
        scope: &str,
        depth: usize,
        resource_root: bool,
    ) -> Result<String> {
        charge(depth, &mut self.work)?;
        let object = schema
            .as_object()
            .ok_or_else(|| invalid("typed request schema must be an object"))?;
        audit(object, scope, resource_root)?;
        let serialized = schema.to_string();
        let key = (scope.to_owned(), serialized);
        if let Some(name) = self.names.get(&key) {
            return Ok(name.clone());
        }
        self.key_bytes = self
            .key_bytes
            .saturating_add(key.0.len())
            .saturating_add(key.1.len());
        if self.key_bytes > MAX_KEYS_BYTES || self.definitions.len() >= MAX_TYPES {
            return Err(capacity("typed request schema inventory exceeds its bound"));
        }
        let ordinal = self.definitions.len();
        let name = format!("RequestType{ordinal:04}");
        self.definitions.push(None);
        self.names.insert(key, name.clone());
        let shape = if let Some(reference) = object.get("$ref") {
            if object
                .keys()
                .any(|key| !annotation(key) && !matches!(key.as_str(), "$ref" | "$defs" | "type"))
                || object.get("type").is_some_and(|value| value != "object")
            {
                return Err(invalid(
                    "typed request reference has unsupported assertion siblings",
                ));
            }
            let reference = reference
                .as_str()
                .ok_or_else(|| invalid("typed request reference must be a string"))?;
            let (document, pointer) = reference.split_once('#').unwrap_or((reference, ""));
            let id = if document.is_empty() { scope } else { document };
            if !id.starts_with("urn:")
                || id.contains('#')
                || (!pointer.is_empty() && !pointer.starts_with('/'))
            {
                return Err(invalid(
                    "typed request reference requires an absolute document or scoped JSON pointer",
                ));
            }
            let documents = self.documents;
            let document = documents
                .get(id)
                .ok_or_else(|| invalid("typed request reference document is missing"))?;
            let selected = document
                .pointer(pointer)
                .ok_or_else(|| invalid("typed request reference pointer is missing"))?;
            let target = self.schema(selected, id, depth + 1, pointer.is_empty())?;
            if object.contains_key("type") {
                self.object_guards.push(target.clone());
            }
            Shape::Alias(target)
        } else if let Some(value) = object.get("const") {
            literal_type(value, object.get("type"))?;
            if object.contains_key("enum")
                || object.contains_key("oneOf")
                || object.contains_key("anyOf")
            {
                return Err(invalid(
                    "typed request constant has unsupported intersecting choices",
                ));
            }
            self.literal(value, scope, depth + 1)?
        } else if let Some(values) = object.get("enum") {
            if object.contains_key("oneOf") || object.contains_key("anyOf") {
                return Err(invalid(
                    "typed request enum has unsupported intersecting choices",
                ));
            }
            let values = values
                .as_array()
                .filter(|values| !values.is_empty())
                .ok_or_else(|| invalid("typed request enum must be nonempty"))?;
            let mut variants = Vec::new();
            for value in values {
                literal_type(value, object.get("type"))?;
                variants.push(self.schema(
                    &serde_json::json!({"const":value}),
                    scope,
                    depth + 1,
                    false,
                )?);
            }
            Shape::Union(variants)
        } else if object.contains_key("oneOf") || object.contains_key("anyOf") {
            if object
                .keys()
                .any(|key| !annotation(key) && !matches!(key.as_str(), "$defs" | "oneOf" | "anyOf"))
                || (object.contains_key("oneOf") && object.contains_key("anyOf"))
            {
                return Err(invalid(
                    "typed request alternatives have unsupported intersections",
                ));
            }
            let values = object
                .get("oneOf")
                .or_else(|| object.get("anyOf"))
                .and_then(Value::as_array)
                .filter(|values| !values.is_empty())
                .ok_or_else(|| invalid("typed request alternatives must be nonempty"))?;
            let mut variants = Vec::new();
            for value in values {
                variants.push(self.schema(value, scope, depth + 1, false)?);
            }
            Shape::Union(variants)
        } else {
            match object.get("type").and_then(Value::as_str) {
                Some("boolean") => Shape::Bool,
                Some("integer") => Shape::Integer,
                Some("string") => Shape::String,
                Some("null") => Shape::Null,
                Some("array") => Shape::Array(self.schema(
                    object.get("items").unwrap_or(&serde_json::json!({})),
                    scope,
                    depth + 1,
                    false,
                )?),
                Some("object") => {
                    let empty = Map::new();
                    let properties = object
                        .get("properties")
                        .map(|value| {
                            value.as_object().ok_or_else(|| {
                                invalid("typed request properties must be an object")
                            })
                        })
                        .transpose()?
                        .unwrap_or(&empty);
                    let required = object
                        .get("required")
                        .map(|value| {
                            value.as_array().ok_or_else(|| {
                                invalid("typed request required fields must be an array")
                            })
                        })
                        .transpose()?
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let mut seen = BTreeSet::new();
                    for field in required {
                        let field = field.as_str().ok_or_else(|| {
                            invalid("typed request required field must be a string")
                        })?;
                        if !properties.contains_key(field) || !seen.insert(field) {
                            return Err(invalid(
                                "typed request requires an undescribed or repeated property",
                            ));
                        }
                    }
                    let mut fields = Vec::new();
                    for (field, value) in properties {
                        fields.push(Field {
                            name: field.clone(),
                            ty: self.schema(value, scope, depth + 1, false)?,
                            required: seen.contains(field.as_str()),
                        });
                    }
                    Shape::Object {
                        fields,
                        open: object.get("additionalProperties") != Some(&Value::Bool(false)),
                    }
                }
                None if object.keys().all(|key| annotation(key) || key == "$defs") => Shape::Any,
                _ => {
                    return Err(invalid(
                        "typed request schema lacks a supported structural shape",
                    ))
                }
            }
        };
        self.definitions[ordinal] = Some(Definition {
            name: name.clone(),
            shape,
        });
        Ok(name)
    }

    fn literal(&mut self, value: &Value, scope: &str, depth: usize) -> Result<Shape> {
        charge(depth, &mut self.work)?;
        Ok(match value {
            Value::Object(values) => {
                let mut fields = Vec::new();
                for (name, value) in values {
                    fields.push(Field {
                        name: name.clone(),
                        ty: self.schema(
                            &serde_json::json!({"const":value}),
                            scope,
                            depth + 1,
                            false,
                        )?,
                        required: true,
                    });
                }
                Shape::Object {
                    fields,
                    open: false,
                }
            }
            Value::Array(values) => {
                let mut types = Vec::new();
                for value in values {
                    types.push(self.schema(
                        &serde_json::json!({"const":value}),
                        scope,
                        depth + 1,
                        false,
                    )?);
                }
                Shape::Tuple(types)
            }
            Value::Null => Shape::Null,
            Value::Number(number) if !(number.is_i64() || number.is_u64()) => {
                return Err(invalid(
                    "typed request literal is outside the supported integer domain",
                ))
            }
            _ => Shape::Literal(value.clone()),
        })
    }
}

fn annotation(key: &str) -> bool {
    matches!(key, "$id" | "$schema" | "title" | "description") || key.starts_with("x-")
}

fn literal_type(value: &Value, kind: Option<&Value>) -> Result<()> {
    let Some(kind) = kind else { return Ok(()) };
    let agrees = match kind.as_str() {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("integer") => value.is_i64() || value.is_u64(),
        Some("boolean") => value.is_boolean(),
        Some("null") => value.is_null(),
        _ => false,
    };
    if agrees {
        Ok(())
    } else {
        Err(invalid(
            "typed request literal contradicts its explicit value type",
        ))
    }
}

/// Static shape generation recognizes these constraints without claiming that
/// a host language's type system proves them. Compiler request admission still
/// checks bounds, identifier patterns, excluded names, uniqueness and budgets.
fn audit(object: &Map<String, Value>, scope: &str, resource_root: bool) -> Result<()> {
    for key in object.keys() {
        if !annotation(key)
            && !matches!(
                key.as_str(),
                "$defs"
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
                    | "pattern"
                    | "not"
            )
        {
            return Err(invalid("typed request schema keyword is unsupported"));
        }
    }
    if object.get("type").is_some_and(|value| {
        !value.as_str().is_some_and(|kind| {
            matches!(
                kind,
                "object" | "array" | "string" | "integer" | "boolean" | "null"
            )
        })
    }) {
        return Err(invalid("typed request schema value type is unsupported"));
    }
    if object
        .get("$id")
        .is_some_and(|id| !resource_root || id.as_str() != Some(scope))
    {
        return Err(invalid(
            "typed request nested resource scope is unsupported",
        ));
    }
    if object.get("$defs").is_some_and(|value| !value.is_object()) {
        return Err(invalid("typed request definitions must be an object"));
    }
    if object
        .get("additionalProperties")
        .is_some_and(|value| !value.is_boolean())
        || object
            .get("uniqueItems")
            .is_some_and(|value| !value.is_boolean())
    {
        return Err(invalid(
            "typed request object or array constraint is malformed",
        ));
    }
    for (kind, keys) in [
        (
            "object",
            &["properties", "required", "additionalProperties"][..],
        ),
        (
            "array",
            &["items", "minItems", "maxItems", "uniqueItems"][..],
        ),
        ("string", &["minLength", "maxLength", "pattern", "not"][..]),
        ("integer", &["minimum", "maximum"][..]),
    ] {
        if keys.iter().any(|key| object.contains_key(*key))
            && object.get("type").and_then(Value::as_str) != Some(kind)
        {
            return Err(invalid(
                "typed request constraint requires its explicit matching type",
            ));
        }
    }
    for key in ["minItems", "maxItems", "minLength", "maxLength"] {
        if object
            .get(key)
            .is_some_and(|value| value.as_u64().is_none())
        {
            return Err(invalid(
                "typed request size bound must be an unsigned integer",
            ));
        }
    }
    for key in ["minimum", "maximum"] {
        if object
            .get(key)
            .is_some_and(|value| !(value.is_i64() || value.is_u64()))
        {
            return Err(invalid("typed request integer bound is malformed"));
        }
    }
    if object
        .get("pattern")
        .is_some_and(|value| !value.is_string())
    {
        return Err(invalid("typed request pattern must be a string"));
    }
    if let Some(not) = object.get("not") {
        let excluded = not
            .as_object()
            .ok_or_else(|| invalid("typed request exclusion must be a closed enum"))?;
        if excluded.len() != 1
            || !excluded
                .get("enum")
                .and_then(Value::as_array)
                .is_some_and(|values| !values.is_empty() && values.iter().all(Value::is_string))
        {
            return Err(invalid(
                "typed request exclusion requires a finite string enum",
            ));
        }
    }
    Ok(())
}

fn charge(depth: usize, work: &mut usize) -> Result<()> {
    *work += 1;
    if depth > MAX_DEPTH || *work > MAX_WORK {
        return Err(capacity("typed request traversal exceeds its bound"));
    }
    Ok(())
}

/// Reject every recursion cycle that can recur without consuming an object,
/// array or tuple boundary. Merely having some terminating union branch is not
/// enough for safe generated recursive aliases or untagged deserialization.
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
            "typed request has unproductive alias or union recursion",
        ));
    }
    match index
        .get(name)
        .ok_or_else(|| invalid("typed request reference type is absent"))?
    {
        Shape::Alias(target) => productive(target, index, active, done, depth + 1, work)?,
        Shape::Union(types) => {
            for target in types {
                productive(target, index, active, done, depth + 1, work)?;
            }
        }
        _ => {}
    }
    active.remove(name);
    done.insert(name);
    Ok(())
}

fn object_shape(
    name: &str,
    index: &BTreeMap<&str, &Shape>,
    memo: &mut BTreeMap<String, bool>,
    depth: usize,
    work: &mut usize,
) -> Result<bool> {
    charge(depth, work)?;
    if let Some(result) = memo.get(name) {
        return Ok(*result);
    }
    let result = match index
        .get(name)
        .ok_or_else(|| invalid("typed request asserted type is absent"))?
    {
        Shape::Object { .. } => true,
        Shape::Alias(target) => object_shape(target, index, memo, depth + 1, work)?,
        Shape::Union(types) => {
            let mut all = true;
            for target in types {
                all &= object_shape(target, index, memo, depth + 1, work)?;
            }
            all
        }
        _ => false,
    };
    memo.insert(name.to_owned(), result);
    Ok(result)
}

fn capacity(message: &'static str) -> Vec<crate::diagnostic::Diagnostic> {
    vec![crate::diagnostic::Diagnostic::io("SPX-G289", message)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn method(name: &str, field: Value) -> Value {
        json!({"method":name,"request_schema":{"properties":{"params":{
            "type":"object","additionalProperties":false,"required":["value"],"properties":{"value":field}
        }}}})
    }

    #[test]
    fn actual_recursive_constructor_and_recovery_documents_have_concrete_types() {
        let bundle: Value =
            serde_json::from_str(&crate::project::SemanticChange::constructor_schemas().unwrap())
                .unwrap();
        let documents = bundle["documents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|document| {
                (
                    document["$id"].as_str().unwrap().to_owned(),
                    document.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let methods = [
            method(
                "candidate/apply-intent",
                json!({"type":"object","$ref":"urn:semaprax.semantic-change-intent.v1"}),
            ),
            method(
                "hole/fill",
                json!({"type":"object","$ref":"urn:semaprax.typed-expression.v1"}),
            ),
            method(
                "candidate/recovery-restore",
                json!({"type":"object","$ref":"urn:semaprax.project-candidate-recovery.v1"}),
            ),
        ];
        let model = build(&methods, &documents).unwrap();
        assert_eq!(model.params.len(), 3);
        assert!(model
            .definitions
            .iter()
            .any(|definition| matches!(&definition.shape, Shape::Alias(_))));
        assert!(model
            .definitions
            .iter()
            .any(|definition| matches!(&definition.shape, Shape::Union(_))));
        assert!(!model
            .definitions
            .iter()
            .any(|definition| matches!(&definition.shape, Shape::Any)));
        for language in ["rust", "typescript", "python"] {
            let generated = generate(language, &methods, &documents).unwrap();
            assert!(generated.source.len() <= MAX_SOURCE_BYTES);
            assert_eq!(generated.params.len(), 3);
        }
    }

    #[test]
    fn alias_and_union_only_recursion_rejects_even_with_a_terminating_branch() {
        for definition in [
            json!({"$ref":"#/$defs/node"}),
            json!({"anyOf":[{"$ref":"#/$defs/node"},{"type":"string"}]}),
        ] {
            let documents = BTreeMap::from([(
                "urn:cycle".into(),
                json!({"$id":"urn:cycle","$ref":"#/$defs/node","$defs":{"node":definition}}),
            )]);
            let errors = build(&[method("probe", json!({"$ref":"urn:cycle"}))], &documents)
                .err()
                .unwrap();
            assert_eq!(errors[0].code, "SPX-G288");
            assert!(errors[0].message.contains("unproductive"));
        }
        let documents = BTreeMap::from([(
            "urn:guarded".into(),
            json!({"$id":"urn:guarded","type":"object","additionalProperties":false,"properties":{"next":{"$ref":"urn:guarded"}}}),
        )]);
        assert!(build(
            &[method("probe", json!({"$ref":"urn:guarded"}))],
            &documents
        )
        .is_ok());
    }

    #[test]
    fn local_references_keep_document_scope_and_literal_refs_are_plain_data() {
        let documents = BTreeMap::from([
            (
                "urn:left".into(),
                json!({"$id":"urn:left","$ref":"#/$defs/item","$defs":{"item":{"const":"left"}}}),
            ),
            (
                "urn:right".into(),
                json!({"$id":"urn:right","$ref":"#/$defs/item","$defs":{"item":{"const":"right"}}}),
            ),
        ]);
        let methods = [
            method("left", json!({"$ref":"urn:left"})),
            method("right", json!({"$ref":"urn:right"})),
            method("literal", json!({"const":{"$ref":"urn:missing"}})),
        ];
        let model = build(&methods, &documents).unwrap();
        for expected in ["left", "right", "urn:missing"] {
            assert!(model.definitions.iter().any(
                |definition| matches!(&definition.shape,Shape::Literal(value) if value==expected)
            ));
        }
        let mut missing = documents.clone();
        missing.get_mut("urn:right").unwrap()["$defs"] = json!({});
        let errors = build(&methods, &missing).err().unwrap();
        assert_eq!(errors[0].code, "SPX-G288");
        assert!(errors[0].message.contains("pointer is missing"));
        let errors = build(
            &[method("unscoped", json!({"$ref":"#/$defs/item"}))],
            &documents,
        )
        .err()
        .unwrap();
        assert_eq!(errors[0].code, "SPX-G288");
    }

    #[test]
    fn reference_object_assertions_and_unknown_shapes_fail_closed() {
        let documents = BTreeMap::from([(
            "urn:scalar".into(),
            json!({"$id":"urn:scalar","type":"string"}),
        )]);
        let errors = build(
            &[method(
                "probe",
                json!({"type":"object","$ref":"urn:scalar"}),
            )],
            &documents,
        )
        .err()
        .unwrap();
        assert_eq!(errors[0].code, "SPX-G288");
        assert!(errors[0].message.contains("object assertion"));
        for schema in [
            json!({"allOf":[]}),
            json!({"type":"number"}),
            json!({"type":"integer","const":"not an integer"}),
            json!({"type":"string","enum":["accepted",false]}),
            json!({"$ref":"urn:absent"}),
            json!({"type":"string","not":{"type":"string"}}),
        ] {
            assert_eq!(
                build(&[method("probe", schema)], &documents).err().unwrap()[0].code,
                "SPX-G288"
            );
        }
    }
}
