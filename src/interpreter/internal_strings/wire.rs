//! Exact encoding check after the shared closed-field/type replay succeeds.

use super::super::{consistency_error, Diagnostic, OUTCOME_FUEL_EXHAUSTED};
use serde_json::Value;

pub(crate) fn verify_canonical(
    envelope: &str,
    value: &Value,
    max_bytes: u64,
) -> Result<(), Diagnostic> {
    if envelope.len() as u64 > max_bytes {
        return Err(consistency_error(
            "internal String envelope exceeds its declared max_bytes".to_owned(),
        ));
    }
    let payload = &value["payload"];
    if payload["fuel"]["exhausted"].as_bool()
        != Some(payload["outcome"]["kind"].as_str() == Some(OUTCOME_FUEL_EXHAUSTED))
    {
        return Err(consistency_error(
            "internal String fuel and outcome disagree".to_owned(),
        ));
    }
    let digest = payload["source"]["sha256"].as_str().unwrap_or_default();
    if !digest.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) {
        return Err(consistency_error(
            "internal String source digest must be sha256: plus 64 lowercase hex digits".to_owned(),
        ));
    }
    let mut output = Canonical {
        text: String::new(),
        limit: envelope.len(),
    };
    output.value(value, Shape::Envelope)?;
    if output.text != envelope {
        return Err(consistency_error(
            "internal String envelope is not the exact canonical encoding".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Shape {
    Envelope,
    Payload,
    Source,
    Function,
    Arguments,
    Argument,
    Limits,
    Fuel,
    Outcome,
    Status,
    Leaf,
}

struct Canonical {
    text: String,
    limit: usize,
}

impl Canonical {
    fn append(&mut self, text: &str) -> Result<(), Diagnostic> {
        if text.len() > self.limit.saturating_sub(self.text.len()) {
            return Err(consistency_error(
                "internal String canonical encoding exceeds its carrier".to_owned(),
            ));
        }
        self.text.push_str(text);
        Ok(())
    }

    // Match diagnostic::quote_json exactly without allocating an intermediate
    // escaped clone. The output cannot grow beyond the already bounded input.
    fn quoted(&mut self, text: &str) -> Result<(), Diagnostic> {
        self.append("\"")?;
        for character in text.chars() {
            match character {
                '"' => self.append("\\\"")?,
                '\\' => self.append("\\\\")?,
                '\n' => self.append("\\n")?,
                '\r' => self.append("\\r")?,
                '\t' => self.append("\\t")?,
                control if control.is_control() => {
                    self.append(&format!("\\u{:04x}", control as u32))?
                }
                other => self.append(other.encode_utf8(&mut [0; 4]))?,
            }
        }
        self.append("\"")
    }

    fn value(&mut self, value: &Value, shape: Shape) -> Result<(), Diagnostic> {
        if let Value::Object(object) = value {
            let keys: &[&str] = match shape {
                Shape::Envelope => &["schema", "digest", "bytes", "payload"],
                Shape::Payload => &[
                    "schema",
                    "source",
                    "function",
                    "arguments",
                    "limits",
                    "fuel",
                    "outcome",
                    "nonclaims",
                ],
                Shape::Source => &["path", "revision", "sha256"],
                Shape::Function => &["stable_id", "name"],
                Shape::Argument => &["index", "name", "type", "value"],
                Shape::Limits => &["max_bytes", "max_steps"],
                Shape::Fuel => &["steps_used", "budget", "exhausted"],
                Shape::Outcome => match value["kind"].as_str() {
                    Some("returned") => &["kind", "type", "value"],
                    Some("failed") => &["kind", "status"],
                    _ => &["kind"],
                },
                Shape::Status => &["schema", "domain_id", "code", "class", "retryable"],
                _ => return Err(consistency_error("unexpected canonical object".to_owned())),
            };
            self.append("{")?;
            for (index, key) in keys.iter().enumerate() {
                if index != 0 {
                    self.append(",")?;
                }
                self.quoted(key)?;
                self.append(":")?;
                let child = match *key {
                    "payload" => Shape::Payload,
                    "source" => Shape::Source,
                    "function" => Shape::Function,
                    "arguments" => Shape::Arguments,
                    "limits" => Shape::Limits,
                    "fuel" => Shape::Fuel,
                    "outcome" => Shape::Outcome,
                    "status" => Shape::Status,
                    _ => Shape::Leaf,
                };
                self.value(&object[*key], child)?;
            }
            self.append("}")
        } else if let Value::Array(items) = value {
            self.append("[")?;
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    self.append(",")?;
                }
                self.value(
                    item,
                    if matches!(shape, Shape::Arguments) {
                        Shape::Argument
                    } else {
                        Shape::Leaf
                    },
                )?;
            }
            self.append("]")
        } else if let Value::String(text) = value {
            self.quoted(text)
        } else {
            self.append(&value.to_string())
        }
    }
}
