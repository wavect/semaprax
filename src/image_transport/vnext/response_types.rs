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
    /// Dependency order: every referenced name precedes its use.
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
        active: BTreeSet::new(),
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
    let model = Model {
        definitions: builder.definitions,
        payloads,
    };
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
    active: BTreeSet<String>,
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
        if self.active.contains(&key) {
            return Err(invalid(
                "typed response recursive schemas are not supported",
            ));
        }
        if let Some(name) = self.names.get(&key) {
            return Ok(name.clone());
        }
        self.key_bytes = self.key_bytes.saturating_add(key.len());
        if self.key_bytes > 16 * 1024 * 1024 {
            return Err(capacity("typed response schema inventory exceeds 16 MiB"));
        }
        self.active.insert(key.clone());
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
            ) {
                return Err(invalid("typed response schema keyword is unsupported"));
            }
        }
        let shape = if let Some(reference) = fields.get("$ref") {
            if fields.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "$ref" | "$id" | "$schema" | "title" | "description"
                )
            }) {
                return Err(invalid("typed response reference has unsupported siblings"));
            }
            let reference = reference
                .as_str()
                .ok_or_else(|| invalid("typed response reference must be a string"))?;
            if let Some(document) = self.documents.get(reference) {
                Shape::Alias(self.schema(document, depth + 1)?)
            } else if self.unbundled.iter().any(|value| value == reference) {
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
        self.active.remove(&key);
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
    fn response_references_require_complete_acyclic_or_explicit_opaque_inventory() {
        let request = methods(json!({"$ref":"urn:outer"}));
        let cycle = BTreeMap::from([
            (
                "urn:outer".into(),
                json!({"type":"object","properties":{"next":{"$ref":"urn:inner"}},"additionalProperties":false}),
            ),
            ("urn:inner".into(), json!({"$ref":"urn:outer"})),
        ]);
        for language in ["rust", "python", "typescript"] {
            let errors = generate(language, &request, &cycle, &[]).err().unwrap();
            assert_eq!(errors[0].code, "SPX-G288");
            assert!(errors[0].message.contains("recursive"));
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
            active: BTreeSet::new(),
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
