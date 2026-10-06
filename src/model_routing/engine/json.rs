//! Canonical JSON rendering and domain-separated SHA-256 digests. These bytes
//! are bound into cache keys, evidence keys, frozen plans and the v2 rendered
//! digest, so every consumer must compute them from this one implementation.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Canonical rendering: sorted keys, no whitespace.
pub fn canonical(value: &Value) -> String {
    let mut out = String::new();
    render(value, &mut out);
    out
}

fn render(v: &Value, out: &mut String) {
    match v {
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                render(x, out);
            }
            out.push(']');
        }
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                render(&m[k], out);
            }
            out.push('}');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// `sha256:<64 hex>` over `domain` + NUL + canonical JSON.
pub fn digest(domain: &str, value: &Value) -> String {
    sha256_labeled(domain, canonical(value).as_bytes())
}

/// Same domain separation over raw bytes.
pub fn sha256_labeled(domain: &str, bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(domain.as_bytes());
    h.update([0u8]);
    h.update(bytes);
    format!("sha256:{}", hex(&h.finalize()))
}

/// Plain `sha256:<hex>` of bytes.
pub fn sha256_plain(bytes: &[u8]) -> String {
    format!("sha256:{}", hex(&Sha256::digest(bytes)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
