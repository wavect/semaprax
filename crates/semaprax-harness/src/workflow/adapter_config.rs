//! Adapter configuration plumbing (HN-10): `[capability."<kind>".config]`
//! values of `semaprax.harness.toml`, validated against the selected
//! descriptor's `config.fields`, reach the adapter as host-set environment
//! `SEMAPRAX_HARNESS_CFG_<FIELD>` (field name upper-cased). The prefix is
//! host-reserved, so an adapter can trust it; the adapter's own legacy names
//! stay as aliases. Secrets and paths outside the project are refused here
//! and by the configuration parser.

use crate::contract::Descriptor;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::profile::config::{looks_like_absolute_path, looks_like_secret, CapabilityConfig};
use serde_json::Value;
use std::collections::BTreeMap;

/// Environment prefix of forwarded adapter config (host-reserved namespace).
pub const CFG_PREFIX: &str = "SEMAPRAX_HARNESS_CFG_";

fn bad(msg: String) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPB004", msg)
}

fn plain_path(s: &str) -> bool {
    !looks_like_absolute_path(s) && !s.contains('\\') && !s.split('/').any(|seg| seg == "..")
}

/// Validate `cfg.config` against `descriptor.config_fields` and render the
/// adapter environment. Unknown fields, wrong types, secrets and paths outside
/// the project are refused; defaults are not forwarded (the adapter owns them).
pub fn config_env(
    descriptor: &Descriptor,
    cfg: &CapabilityConfig,
) -> HarnessResult<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for (name, v) in &cfg.config {
        let field = descriptor.config_fields.get(name).ok_or_else(|| {
            bad(format!(
                "config field `{name}` is not declared by provider `{}`",
                descriptor.provider_id
            ))
        })?;
        if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(bad(format!("config field `{name}` is not a valid name")));
        }
        let strings: Vec<&str> = match (field.ty.as_str(), v) {
            ("string", Value::String(s)) => vec![s],
            ("string-list", Value::Array(a)) if a.iter().all(Value::is_string) => {
                a.iter().filter_map(Value::as_str).collect()
            }
            ("bool", Value::Bool(_)) | ("int", Value::Number(_)) => vec![],
            (ty, _) => {
                return Err(bad(format!(
                    "config field `{name}` must be a {ty} for provider `{}`",
                    descriptor.provider_id
                )))
            }
        };
        for s in strings {
            if looks_like_secret(s) {
                return Err(bad(format!(
                    "config field `{name}` holds a secret-looking value"
                )));
            }
            if !plain_path(s) {
                return Err(bad(format!(
                    "config field `{name}` must stay inside the project (no absolute, home or `..` path)"
                )));
            }
        }
        let text = match v {
            Value::String(s) => s.clone(),
            Value::Array(a) => a
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","),
            other => other.to_string(),
        };
        out.insert(format!("{CFG_PREFIX}{}", name.to_ascii_uppercase()), text);
    }
    Ok(out)
}
