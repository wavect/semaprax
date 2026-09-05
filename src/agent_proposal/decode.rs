//! The typed proposal decoder.
//!
//! A proposal is untrusted model output. Decoding is total validation against
//! the derived closed schema and produces data only: no `Authorized<T>`, no
//! publication token, no capability, and no effect.

use serde_json::{Map, Value};

use crate::diagnostic::{quote_json, Diagnostic};

use super::shape::{FieldRow, Representation, Shape};
use super::{
    malformed, proposal_invariant, MAX_PROPOSAL_BYTES, MAX_STRING_FIELD_BYTES, PROPOSAL_SCHEMA,
};

/// One decoded exact scalar value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalValue {
    Bool(bool),
    /// An exact signed integer inside its declared representation's bounds.
    Signed(i64),
    /// An exact unsigned integer inside its declared representation's bounds.
    Unsigned(u64),
    Text(String),
}

/// One decoded proposal field, addressed by its persistent stable identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedField {
    stable_id: String,
    value: ProposalValue,
}

impl DecodedField {
    /// Returns the field's persistent stable identity.
    #[must_use]
    pub fn stable_id(&self) -> &str {
        &self.stable_id
    }

    /// Returns the decoded exact value.
    #[must_use]
    pub fn value(&self) -> &ProposalValue {
        &self.value
    }
}

/// One decoded proposal.
///
/// This value carries no authority. It is the checked reading of one untrusted
/// model document and cannot construct an authorization or publication token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProposal {
    agent_id: String,
    proposal_schema_digest: String,
    case: Option<String>,
    fields: Vec<DecodedField>,
    canonical_source: String,
}

impl DecodedProposal {
    /// Returns the agent identity the proposal was decoded against.
    #[must_use]
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Returns the schema digest the proposal is bound to.
    #[must_use]
    pub fn proposal_schema_digest(&self) -> &str {
        &self.proposal_schema_digest
    }

    /// Returns the selected variant case identity, or `None` for a record.
    #[must_use]
    pub fn case(&self) -> Option<&str> {
        self.case.as_deref()
    }

    /// Returns the decoded fields in declaration order.
    #[must_use]
    pub fn fields(&self) -> &[DecodedField] {
        &self.fields
    }

    /// Returns the decoded value of one field by its stable identity.
    #[must_use]
    pub fn field(&self, stable_id: &str) -> Option<&ProposalValue> {
        self.fields
            .iter()
            .find(|field| field.stable_id == stable_id)
            .map(DecodedField::value)
    }

    /// Returns the exact admitted canonical document, including its terminal LF.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.canonical_source
    }
}

/// Decodes one canonical proposal document against a derived schema.
pub(crate) fn decode(
    agent_id: &str,
    schema_digest: &str,
    shape: &Shape,
    source: &str,
) -> Result<DecodedProposal, Diagnostic> {
    if source.len() > MAX_PROPOSAL_BYTES {
        return Err(proposal_invariant("proposal_bytes"));
    }
    let Some(body) = source.strip_suffix('\n') else {
        return Err(malformed());
    };
    if body.is_empty() || body.contains('\n') || body.contains('\r') || body.starts_with('\u{feff}')
    {
        return Err(malformed());
    }
    let value: Value = serde_json::from_str(body).map_err(|_| malformed())?;
    let top = value.as_object().ok_or_else(malformed)?;
    if !exact_keys(
        top,
        &["schema", "agent_id", "proposal_schema_digest", "value"],
    ) {
        return Err(malformed());
    }
    if string(top, "schema")? != PROPOSAL_SCHEMA {
        return Err(malformed());
    }
    if string(top, "agent_id")? != agent_id {
        return Err(proposal_invariant("agent_id"));
    }
    if string(top, "proposal_schema_digest")? != schema_digest {
        return Err(proposal_invariant("proposal_schema_digest"));
    }
    let body_value = top
        .get("value")
        .and_then(Value::as_object)
        .ok_or_else(malformed)?;

    let (case, fields) = match shape {
        Shape::Record { fields } => {
            if !exact_keys(body_value, &["fields"]) {
                return Err(malformed());
            }
            (None, decode_fields(body_value, fields)?)
        }
        Shape::Variant { cases } => {
            if !exact_keys(body_value, &["case", "fields"]) {
                return Err(malformed());
            }
            let selected = string(body_value, "case")?;
            let case = cases
                .iter()
                .find(|case| case.stable_id == selected)
                .ok_or_else(|| proposal_invariant("value.case"))?;
            (
                Some(case.stable_id.clone()),
                decode_fields(body_value, &case.fields)?,
            )
        }
    };

    let decoded = DecodedProposal {
        agent_id: agent_id.to_owned(),
        proposal_schema_digest: schema_digest.to_owned(),
        case,
        fields,
        canonical_source: String::new(),
    };
    let canonical_source = render(&decoded);
    if canonical_source != source {
        return Err(malformed());
    }
    Ok(DecodedProposal {
        canonical_source,
        ..decoded
    })
}

fn decode_fields(
    body: &Map<String, Value>,
    rows: &[FieldRow],
) -> Result<Vec<DecodedField>, Diagnostic> {
    let fields = body
        .get("fields")
        .and_then(Value::as_object)
        .ok_or_else(malformed)?;
    for key in fields.keys() {
        if !rows.iter().any(|row| &row.stable_id == key) {
            return Err(proposal_invariant("value.fields.unknown"));
        }
    }
    let mut decoded = Vec::with_capacity(rows.len());
    for row in rows {
        let value = fields
            .get(&row.stable_id)
            .ok_or_else(|| proposal_invariant("value.fields.missing"))?;
        decoded.push(DecodedField {
            stable_id: row.stable_id.clone(),
            value: decode_scalar(row.representation, value)?,
        });
    }
    Ok(decoded)
}

fn decode_scalar(
    representation: Representation,
    value: &Value,
) -> Result<ProposalValue, Diagnostic> {
    match representation {
        Representation::Bool => value
            .as_bool()
            .map(ProposalValue::Bool)
            .ok_or_else(|| proposal_invariant("value.representation")),
        Representation::Text => {
            let text = value
                .as_str()
                .ok_or_else(|| proposal_invariant("value.representation"))?;
            if text.len() > MAX_STRING_FIELD_BYTES {
                return Err(proposal_invariant("value.string_bytes"));
            }
            Ok(ProposalValue::Text(text.to_owned()))
        }
        Representation::I32 | Representation::I64 | Representation::U8 | Representation::U64 => {
            let text = value
                .as_str()
                .ok_or_else(|| proposal_invariant("value.representation"))?;
            decode_integer(representation, text)
        }
    }
}

/// Decodes one exact integer from its canonical decimal string.
///
/// Exact integers travel as decimal strings so every consumer preserves values
/// beyond the range a JSON number is guaranteed to carry. The accepted form is
/// exactly one canonical decimal: no sign except a leading `-` on a negative
/// value, no leading zero except the single digit `0`, no `+`, no exponent, no
/// fraction, and no surrounding whitespace.
fn decode_integer(representation: Representation, text: &str) -> Result<ProposalValue, Diagnostic> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let negative = text.starts_with('-');
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
        || (negative && digits == "0")
    {
        return Err(proposal_invariant("value.integer"));
    }
    let (minimum, maximum) = representation
        .bounds()
        .expect("an integer representation always declares bounds");
    match representation {
        Representation::I32 | Representation::I64 => {
            let parsed: i64 = text
                .parse()
                .map_err(|_| proposal_invariant("value.integer_range"))?;
            let low: i64 = minimum.parse().expect("declared bounds parse");
            let high: i64 = maximum.parse().expect("declared bounds parse");
            if parsed < low || parsed > high {
                return Err(proposal_invariant("value.integer_range"));
            }
            Ok(ProposalValue::Signed(parsed))
        }
        Representation::U8 | Representation::U64 => {
            if negative {
                return Err(proposal_invariant("value.integer_range"));
            }
            let parsed: u64 = text
                .parse()
                .map_err(|_| proposal_invariant("value.integer_range"))?;
            let high: u64 = maximum.parse().expect("declared bounds parse");
            if parsed > high {
                return Err(proposal_invariant("value.integer_range"));
            }
            Ok(ProposalValue::Unsigned(parsed))
        }
        Representation::Bool | Representation::Text => {
            unreachable!("only integer representations reach integer decoding")
        }
    }
}

fn render(decoded: &DecodedProposal) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},\"proposal_schema_digest\":{},\"value\":{{",
        quote_json(PROPOSAL_SCHEMA),
        quote_json(&decoded.agent_id),
        quote_json(&decoded.proposal_schema_digest)
    );
    if let Some(case) = &decoded.case {
        output.push_str(&format!("\"case\":{},", quote_json(case)));
    }
    output.push_str("\"fields\":{");
    for (index, field) in decoded.fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{}:{}",
            quote_json(&field.stable_id),
            render_scalar(&field.value)
        ));
    }
    output.push_str("}}}\n");
    output
}

fn render_scalar(value: &ProposalValue) -> String {
    match value {
        ProposalValue::Bool(true) => "true".to_owned(),
        ProposalValue::Bool(false) => "false".to_owned(),
        ProposalValue::Signed(value) => quote_json(&value.to_string()),
        ProposalValue::Unsigned(value) => quote_json(&value.to_string()),
        ProposalValue::Text(value) => quote_json(value),
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
