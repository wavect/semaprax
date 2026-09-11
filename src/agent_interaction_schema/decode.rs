//! The typed, bounded interaction value decoder.
//!
//! Untrusted bytes are validated as UTF-8, parsed as JSON, and checked
//! against the derived [`TypeGraph`] field by field: every declared field
//! must be present exactly once, no undeclared field may appear, every
//! variant tag must be one of the declared cases, and every scalar must fit
//! its declared representation and bound. The decoded value is then
//! re-rendered in the one canonical form the schema declares
//! (`"key_order":"declaration_order"`, closed objects) and required to equal
//! the input byte for byte. Because canonical rendering can only ever emit
//! one occurrence of each declared key, a source document that repeats any
//! key — declared or not — is strictly longer than its canonical rendering
//! and is therefore rejected by this exact-replay check: a duplicate key is
//! never silently collapsed into "whichever occurrence a generic map kept
//! last", it fails decode.
//!
//! A decoded value is data. It carries no authorization, no publication
//! token, and no capability, and decoding performs no effect.

use serde_json::{Map, Value};

use crate::diagnostic::quote_json;
use crate::diagnostic::Diagnostic;

use super::shape::{CaseRow, FieldRow, FieldType, Representation, TypeGraph, TypeShape};
use super::{
    decode_invariant, malformed, DOCUMENT_SCHEMA, MAX_BYTES_FIELD_BYTES, MAX_DEPTH,
    MAX_DOCUMENT_BYTES, MAX_STRING_FIELD_BYTES,
};

/// One decoded exact scalar value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarValue {
    Bool(bool),
    /// An exact signed integer inside its declared representation's bounds.
    Signed(i64),
    /// An exact unsigned integer inside its declared representation's bounds.
    Unsigned(u64),
    Text(String),
    Bytes(Vec<u8>),
}

/// One decoded field value: a leaf scalar or one fully decoded nested value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldValue {
    Scalar(ScalarValue),
    Nested(Box<TypedValue>),
}

/// One decoded field, addressed by its persistent stable identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedField {
    stable_id: String,
    value: FieldValue,
}

impl DecodedField {
    #[must_use]
    pub fn stable_id(&self) -> &str {
        &self.stable_id
    }

    #[must_use]
    pub fn value(&self) -> &FieldValue {
        &self.value
    }
}

/// One decoded record or variant value, at the root or nested inside a
/// parent field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypedValue {
    Record {
        fields: Vec<DecodedField>,
    },
    Variant {
        case: String,
        fields: Vec<DecodedField>,
    },
}

impl TypedValue {
    /// Returns the selected variant case identity, or `None` for a record.
    #[must_use]
    pub fn case(&self) -> Option<&str> {
        match self {
            Self::Variant { case, .. } => Some(case),
            Self::Record { .. } => None,
        }
    }

    #[must_use]
    pub fn fields(&self) -> &[DecodedField] {
        match self {
            Self::Record { fields } | Self::Variant { fields, .. } => fields,
        }
    }

    #[must_use]
    pub fn field(&self, stable_id: &str) -> Option<&FieldValue> {
        self.fields()
            .iter()
            .find(|field| field.stable_id == stable_id)
            .map(DecodedField::value)
    }
}

/// One decoded interaction document, bound to the exact schema it was
/// decoded against.
///
/// This value carries no authority: it is the checked reading of one
/// untrusted document and cannot construct an authorization or publication
/// token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInteractionValue {
    root_type_id: String,
    schema_digest: String,
    value: TypedValue,
    canonical_source: String,
}

impl DecodedInteractionValue {
    #[must_use]
    pub fn root_type_id(&self) -> &str {
        &self.root_type_id
    }

    #[must_use]
    pub fn schema_digest(&self) -> &str {
        &self.schema_digest
    }

    #[must_use]
    pub fn value(&self) -> &TypedValue {
        &self.value
    }

    /// Returns the exact admitted canonical document, including its
    /// terminal LF.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.canonical_source
    }
}

/// Decodes one canonical `semaprax.agent-interaction-value.v1` document
/// against `graph`, bound to `schema_digest`.
///
/// `source` is untrusted response bytes, not yet known to be valid UTF-8.
pub(crate) fn decode(
    graph: &TypeGraph,
    schema_digest: &str,
    source: &[u8],
) -> Result<DecodedInteractionValue, Diagnostic> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(decode_invariant("document_bytes"));
    }
    let text = std::str::from_utf8(source).map_err(|_| decode_invariant("utf8"))?;
    let body = text.strip_suffix('\n').ok_or_else(malformed)?;
    if body.is_empty() || body.contains('\n') || body.contains('\r') || body.starts_with('\u{feff}')
    {
        return Err(malformed());
    }
    let value: Value = serde_json::from_str(body).map_err(|_| malformed())?;
    let top = value.as_object().ok_or_else(malformed)?;
    if !exact_keys(top, &["schema", "root_type_id", "schema_digest", "value"]) {
        return Err(malformed());
    }
    if string(top, "schema")? != DOCUMENT_SCHEMA {
        return Err(malformed());
    }
    if string(top, "root_type_id")? != graph.root_type_id {
        return Err(decode_invariant("root_type_id"));
    }
    if string(top, "schema_digest")? != schema_digest {
        return Err(decode_invariant("schema_digest"));
    }
    let root_value = top.get("value").ok_or_else(malformed)?;
    let decoded = decode_type(graph, &graph.root_type_id, root_value, 0)?;

    let rendered_value = render_typed_value(&decoded);
    let canonical_source = format!(
        "{{\"schema\":{},\"root_type_id\":{},\"schema_digest\":{},\"value\":{rendered_value}}}\n",
        quote_json(DOCUMENT_SCHEMA),
        quote_json(&graph.root_type_id),
        quote_json(schema_digest),
    );
    if canonical_source.as_bytes() != source {
        return Err(malformed());
    }
    Ok(DecodedInteractionValue {
        root_type_id: graph.root_type_id.clone(),
        schema_digest: schema_digest.to_owned(),
        value: decoded,
        canonical_source,
    })
}

fn decode_type(
    graph: &TypeGraph,
    type_id: &str,
    value: &Value,
    depth: u32,
) -> Result<TypedValue, Diagnostic> {
    if depth > MAX_DEPTH {
        return Err(decode_invariant("value.max_depth"));
    }
    let decl = graph
        .get(type_id)
        .expect("schema-internal type identity must resolve inside its own graph");
    let object = value.as_object().ok_or_else(malformed)?;
    match &decl.shape {
        TypeShape::Record { fields } => {
            if !exact_keys(object, &["fields"]) {
                return Err(malformed());
            }
            let fields_object = object
                .get("fields")
                .and_then(Value::as_object)
                .ok_or_else(malformed)?;
            Ok(TypedValue::Record {
                fields: decode_fields(graph, fields, fields_object, depth)?,
            })
        }
        TypeShape::Variant { cases } => {
            if !exact_keys(object, &["case", "fields"]) {
                return Err(malformed());
            }
            let selected = string(object, "case")?;
            let case: &CaseRow = cases
                .iter()
                .find(|case| case.stable_id == selected)
                .ok_or_else(|| decode_invariant("value.case"))?;
            let fields_object = object
                .get("fields")
                .and_then(Value::as_object)
                .ok_or_else(malformed)?;
            Ok(TypedValue::Variant {
                case: case.stable_id.clone(),
                fields: decode_fields(graph, &case.fields, fields_object, depth)?,
            })
        }
    }
}

fn decode_fields(
    graph: &TypeGraph,
    rows: &[FieldRow],
    object: &Map<String, Value>,
    depth: u32,
) -> Result<Vec<DecodedField>, Diagnostic> {
    for key in object.keys() {
        if !rows.iter().any(|row| &row.stable_id == key) {
            return Err(decode_invariant("value.fields.unknown"));
        }
    }
    let mut decoded = Vec::with_capacity(rows.len());
    for row in rows {
        let value = object
            .get(&row.stable_id)
            .ok_or_else(|| decode_invariant("value.fields.missing"))?;
        let field_value = match &row.ty {
            FieldType::Scalar(representation) => {
                FieldValue::Scalar(decode_scalar(*representation, value)?)
            }
            FieldType::Nested(nested_type_id) => FieldValue::Nested(Box::new(decode_type(
                graph,
                nested_type_id,
                value,
                depth + 1,
            )?)),
        };
        decoded.push(DecodedField {
            stable_id: row.stable_id.clone(),
            value: field_value,
        });
    }
    Ok(decoded)
}

fn decode_scalar(representation: Representation, value: &Value) -> Result<ScalarValue, Diagnostic> {
    match representation {
        Representation::Bool => value
            .as_bool()
            .map(ScalarValue::Bool)
            .ok_or_else(|| decode_invariant("value.representation")),
        Representation::Text => {
            let text = value
                .as_str()
                .ok_or_else(|| decode_invariant("value.representation"))?;
            if text.len() > MAX_STRING_FIELD_BYTES {
                return Err(decode_invariant("value.string_bytes"));
            }
            Ok(ScalarValue::Text(text.to_owned()))
        }
        Representation::Bytes => {
            let array = value
                .as_array()
                .ok_or_else(|| decode_invariant("value.representation"))?;
            if array.len() > MAX_BYTES_FIELD_BYTES {
                return Err(decode_invariant("value.bytes_length"));
            }
            let mut bytes = Vec::with_capacity(array.len());
            for element in array {
                let byte = element
                    .as_u64()
                    .filter(|value| *value <= u8::MAX as u64)
                    .ok_or_else(|| decode_invariant("value.bytes_element"))?;
                bytes.push(byte as u8);
            }
            Ok(ScalarValue::Bytes(bytes))
        }
        Representation::I32 | Representation::I64 | Representation::U8 | Representation::U64 => {
            let text = value
                .as_str()
                .ok_or_else(|| decode_invariant("value.representation"))?;
            decode_integer(representation, text)
        }
    }
}

/// Decodes one exact integer from its canonical decimal string.
///
/// Exact integers travel as decimal strings so every consumer preserves
/// values beyond the range a JSON number is guaranteed to carry. The
/// accepted form is exactly one canonical decimal: no sign except a leading
/// `-` on a negative value, no leading zero except the single digit `0`, no
/// `+`, no exponent, no fraction, and no surrounding whitespace.
fn decode_integer(representation: Representation, text: &str) -> Result<ScalarValue, Diagnostic> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let negative = text.starts_with('-');
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
        || (negative && digits == "0")
    {
        return Err(decode_invariant("value.integer"));
    }
    let (minimum, maximum) = representation
        .bounds()
        .expect("an integer representation always declares bounds");
    match representation {
        Representation::I32 | Representation::I64 => {
            let parsed: i64 = text
                .parse()
                .map_err(|_| decode_invariant("value.integer_range"))?;
            let low: i64 = minimum.parse().expect("declared bounds parse");
            let high: i64 = maximum.parse().expect("declared bounds parse");
            if parsed < low || parsed > high {
                return Err(decode_invariant("value.integer_range"));
            }
            Ok(ScalarValue::Signed(parsed))
        }
        Representation::U8 | Representation::U64 => {
            if negative {
                return Err(decode_invariant("value.integer_range"));
            }
            let parsed: u64 = text
                .parse()
                .map_err(|_| decode_invariant("value.integer_range"))?;
            let high: u64 = maximum.parse().expect("declared bounds parse");
            if parsed > high {
                return Err(decode_invariant("value.integer_range"));
            }
            Ok(ScalarValue::Unsigned(parsed))
        }
        Representation::Bool | Representation::Text | Representation::Bytes => {
            unreachable!("only integer representations reach integer decoding")
        }
    }
}

pub(crate) fn render_typed_value(value: &TypedValue) -> String {
    match value {
        TypedValue::Record { fields } => {
            format!("{{\"fields\":{}}}", render_fields(fields))
        }
        TypedValue::Variant { case, fields } => {
            format!(
                "{{\"case\":{},\"fields\":{}}}",
                quote_json(case),
                render_fields(fields)
            )
        }
    }
}

fn render_fields(fields: &[DecodedField]) -> String {
    let mut output = String::from("{");
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{}:{}",
            quote_json(&field.stable_id),
            render_field_value(&field.value)
        ));
    }
    output.push('}');
    output
}

fn render_field_value(value: &FieldValue) -> String {
    match value {
        FieldValue::Scalar(scalar) => render_scalar(scalar),
        FieldValue::Nested(nested) => render_typed_value(nested),
    }
}

fn render_scalar(value: &ScalarValue) -> String {
    match value {
        ScalarValue::Bool(true) => "true".to_owned(),
        ScalarValue::Bool(false) => "false".to_owned(),
        ScalarValue::Signed(value) => quote_json(&value.to_string()),
        ScalarValue::Unsigned(value) => quote_json(&value.to_string()),
        ScalarValue::Text(value) => quote_json(value),
        ScalarValue::Bytes(bytes) => {
            let mut output = String::from("[");
            for (index, byte) in bytes.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&byte.to_string());
            }
            output.push(']');
            output
        }
    }
}

fn exact_keys(object: &Map<String, Value>, keys: &[&str]) -> bool {
    object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(malformed)
}
