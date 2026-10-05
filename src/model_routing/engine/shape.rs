//! Closed-shape JSON member accessors for payload validation. Every refusal is
//! `SPX-HPA040`; the harness payload validators use these same helpers.

use super::diag::{DecisionResult, Diagnostic};
use serde_json::{Map, Value};

pub fn e(code: &'static str, msg: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, msg)
}

/// Require an object whose members are exactly `required` plus a subset of `optional`.
pub fn shape<'a>(
    v: &'a Value,
    what: &str,
    required: &[&str],
    optional: &[&str],
) -> DecisionResult<&'a Map<String, Value>> {
    let m = v
        .as_object()
        .ok_or_else(|| e("SPX-HPA040", format!("{what} must be an object")))?;
    for r in required {
        if !m.contains_key(*r) {
            return Err(e("SPX-HPA040", format!("{what} is missing `{r}`")));
        }
    }
    for k in m.keys() {
        if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
            return Err(e(
                "SPX-HPA040",
                format!("{what} has unexpected member `{k}`"),
            ));
        }
    }
    Ok(m)
}

pub fn str_of<'a>(m: &'a Map<String, Value>, key: &str, max: usize) -> DecisionResult<&'a str> {
    let s = m
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| e("SPX-HPA040", format!("`{key}` must be a string")))?;
    if s.len() > max {
        return Err(e("SPX-HPA040", format!("`{key}` exceeds {max} bytes")));
    }
    Ok(s)
}

pub fn uint_of(m: &Map<String, Value>, key: &str) -> DecisionResult<u64> {
    m.get(key).and_then(Value::as_u64).ok_or_else(|| {
        e(
            "SPX-HPA040",
            format!("`{key}` must be a non-negative integer"),
        )
    })
}

pub fn bool_of(m: &Map<String, Value>, key: &str) -> DecisionResult<bool> {
    m.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| e("SPX-HPA040", format!("`{key}` must be a boolean")))
}

pub fn array_of<'a>(
    m: &'a Map<String, Value>,
    key: &str,
    max: usize,
) -> DecisionResult<&'a Vec<Value>> {
    let a = m
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| e("SPX-HPA040", format!("`{key}` must be an array")))?;
    if a.len() > max {
        return Err(e(
            "SPX-HPA040",
            format!("`{key}` has more than {max} entries"),
        ));
    }
    Ok(a)
}
