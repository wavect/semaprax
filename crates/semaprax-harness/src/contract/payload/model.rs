//! `model.generate/v1`: host-side wrapper over the existing `ProviderAdapter`
//! SDK. The payload is opaque base64 bytes plus a logical model id.

use super::*;

pub fn validate(dir: Direction, v: &Value) -> HarnessResult<()> {
    match dir {
        Direction::Request => {
            let m = shape(
                v,
                "model request",
                &["model", "input_base64", "max_output_bytes"],
                &["max_output_tokens", "reasoning_effort", "segments"],
            )?;
            logical_id(m)?;
            base64(m, "input_base64")?;
            uint_of(m, "max_output_bytes")?;
            if m.contains_key("max_output_tokens") && uint_of(m, "max_output_tokens")? == 0 {
                return Err(e("SPX-HPA040", "`max_output_tokens` must be at least 1"));
            }
            if let Some(seg) = m.get("segments") {
                segments(seg, str_of(m, "input_base64", 1 << 24)?)?;
            }
            if m.contains_key("reasoning_effort")
                && !matches!(
                    m["reasoning_effort"].as_str(),
                    Some("minimal" | "low" | "medium" | "high")
                )
            {
                return Err(e(
                    "SPX-HPA040",
                    "`reasoning_effort` must be minimal, low, medium or high",
                ));
            }
        }
        Direction::Result => {
            let m = shape(
                v,
                "model result",
                &["model", "output_base64", "usage"],
                &["receipt"],
            )?;
            logical_id(m)?;
            base64(m, "output_base64")?;
            let u = shape(&m["usage"], "usage", &["input_bytes", "output_bytes"], &[])?;
            uint_of(u, "input_bytes")?;
            uint_of(u, "output_bytes")?;
            if let Some(r) = m.get("receipt") {
                receipt(r)?;
            }
        }
    }
    Ok(())
}

/// Optional ordered-prompt `segments` (TC-04): closed, bounded, and
/// consistent with the encoded input (the item sizes sum to the decoded
/// length), so a boundary can never point outside the request.
fn segments(v: &Value, input_b64: &str) -> HarnessResult<()> {
    let m = shape(
        v,
        "segments",
        &[
            "renderer",
            "prefix_identity",
            "items",
            "cache_boundary_after",
        ],
        &[],
    )?;
    str_of(m, "renderer", 32)?;
    let id = str_of(m, "prefix_identity", 80)?;
    if !id.starts_with("sha256:") {
        return Err(e("SPX-HPA040", "`prefix_identity` must be a sha256 digest"));
    }
    let items = array_of(m, "items", 8)?;
    if items.is_empty() {
        return Err(e("SPX-HPA040", "`segments.items` must not be empty"));
    }
    let (mut total, mut ids) = (0u64, Vec::new());
    for it in items {
        let im = shape(it, "segment item", &["id", "bytes"], &[])?;
        ids.push(str_of(im, "id", 32)?);
        total += uint_of(im, "bytes")?;
    }
    let body = input_b64.trim_end_matches('=').len() as u64;
    if total != body * 3 / 4 {
        return Err(e(
            "SPX-HPA040",
            "`segments` sizes do not add up to the input length",
        ));
    }
    if !ids.contains(&str_of(m, "cache_boundary_after", 32)?) {
        return Err(e(
            "SPX-HPA040",
            "`cache_boundary_after` must name a segment item",
        ));
    }
    Ok(())
}

/// Typed provider receipt (`semaprax.harness-model-receipt.v1`): bounded,
/// closed, and data only. Usage objects stay provider-native; the host
/// normalizes them.
fn receipt(v: &Value) -> HarnessResult<()> {
    let m = shape(
        v,
        "receipt",
        &[],
        &[
            "schema",
            "protocol",
            "model",
            "request_id",
            "finish_reason",
            "usage",
            "usage_events",
            "provider_cost_micros",
            "controls",
        ],
    )?;
    for k in ["schema", "protocol", "model", "request_id", "finish_reason"] {
        if m.contains_key(k) {
            str_of(m, k, 128)?;
        }
    }
    if m.contains_key("provider_cost_micros") {
        uint_of(m, "provider_cost_micros")?;
    }
    if m.get("usage").is_some_and(|u| !u.is_object()) {
        return Err(e("SPX-HPA040", "receipt `usage` must be an object"));
    }
    if m.contains_key("usage_events") {
        let ev = array_of(m, "usage_events", 256)?;
        if ev.iter().any(|x| !x.is_object()) {
            return Err(e("SPX-HPA040", "receipt `usage_events` must be objects"));
        }
    }
    if m.get("controls").is_some_and(|c| !c.is_object()) {
        return Err(e("SPX-HPA040", "receipt `controls` must be an object"));
    }
    Ok(())
}

fn logical_id(m: &Map<String, Value>) -> HarnessResult<()> {
    let id = str_of(m, "model", 64)?;
    let ok = id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-' | b'/')
        });
    if ok {
        Ok(())
    } else {
        Err(e("SPX-HPA045", format!("`{id}` is not a logical model id")))
    }
}

fn base64(m: &Map<String, Value>, key: &str) -> HarnessResult<()> {
    let s = str_of(m, key, 1 << 24)?;
    let body = s.trim_end_matches('=');
    if s.len() - body.len() > 2
        || !body
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
    {
        return Err(e("SPX-HPA040", format!("`{key}` is not base64")));
    }
    Ok(())
}

pub fn check_against_request(request: &Value, result: &Value) -> HarnessResult<()> {
    if request.get("model") != result.get("model") {
        return Err(e(
            "SPX-HPA045",
            "result model id differs from the requested model",
        ));
    }
    Ok(())
}
