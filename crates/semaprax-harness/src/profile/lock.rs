//! `semaprax.harness.lock`: canonical JSON of the resolved identities. It holds
//! portable identities and digests only: no absolute paths, secrets or grants.

use super::config::{looks_like_absolute_path, looks_like_secret};
use super::resolve::{Binding, BindingState, ResolvedProfile, UpstreamBinding};
use crate::contract::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{self, parse_strict, JsonLimits};
use serde_json::{json, Map, Value};
use std::path::Path;

pub const LOCK_SCHEMA: &str = "semaprax.harness-lock.v1";
pub const LOCK_FILE: &str = "semaprax.harness.lock";

fn bad(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lock {
    pub config_digest: String,
    pub bindings: Vec<Binding>,
}

fn binding_json(b: &Binding) -> Value {
    let mut o = Map::new();
    o.insert("kind".into(), json!(b.kind.as_str()));
    o.insert("state".into(), json!(b.state.as_str()));
    if !b.provider_id.is_empty() {
        o.insert("provider_id".into(), json!(b.provider_id));
        o.insert("provider_version".into(), json!(b.provider_version));
        o.insert("adapter_version".into(), json!(b.adapter_version));
        o.insert("descriptor_digest".into(), json!(b.descriptor_digest));
        if let Some(u) = &b.upstream {
            o.insert(
                "upstream".into(),
                json!({"name": u.name, "version": u.version, "digest": u.digest}),
            );
        }
    }
    Value::Object(o)
}

fn lock_json(p: &ResolvedProfile) -> Value {
    let mut bindings: Vec<&Binding> = p.bindings.iter().collect();
    bindings.sort_by_key(|b| b.kind.as_str());
    json!({
        "schema": LOCK_SCHEMA,
        "config_digest": p.config_digest,
        "bindings": bindings.into_iter().map(binding_json).collect::<Vec<_>>(),
    })
}

/// Domain-separated digest of the lock rendering.
pub fn digest(p: &ResolvedProfile) -> String {
    json::digest(LOCK_SCHEMA, &lock_json(p))
}

fn scan(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if looks_like_absolute_path(s) || looks_like_secret(s) => Some(s.clone()),
        Value::Array(a) => a.iter().find_map(scan),
        Value::Object(m) => m.values().find_map(scan),
        _ => None,
    }
}

/// Canonical lock bytes (with a trailing LF). Refuses to render machine data.
pub fn render(p: &ResolvedProfile) -> HarnessResult<String> {
    let doc = lock_json(p);
    if scan(&doc).is_some() {
        return Err(bad(
            "SPX-HPB011",
            "refusing to write a lock that contains an absolute path or secret-looking value",
        ));
    }
    Ok(format!("{}\n", json::canonical(&doc)))
}

pub fn write(project: &Path, p: &ResolvedProfile) -> HarnessResult<()> {
    let text = render(p)?;
    std::fs::write(project.join(LOCK_FILE), text)
        .map_err(|e| bad("SPX-HPB010", format!("cannot write {LOCK_FILE}: {e}")))
}

/// Load the committed lock; `None` when absent.
pub fn load(project: &Path) -> HarnessResult<Option<Lock>> {
    match std::fs::read(project.join(LOCK_FILE)) {
        Ok(b) => parse(&b).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(bad("SPX-HPB010", format!("cannot read {LOCK_FILE}: {e}"))),
    }
}

pub fn parse(bytes: &[u8]) -> HarnessResult<Lock> {
    let limits = JsonLimits {
        max_bytes: 1024 * 1024,
        max_depth: 8,
        max_nodes: 10_000,
    };
    let doc = parse_strict(bytes, &limits)
        .map_err(|e| bad("SPX-HPB010", format!("{LOCK_FILE}: {}", e.message)))?;
    let m = doc
        .as_object()
        .ok_or_else(|| bad("SPX-HPB010", format!("{LOCK_FILE} must be an object")))?;
    if let Some(k) = m
        .keys()
        .find(|k| !["schema", "config_digest", "bindings"].contains(&k.as_str()))
    {
        return Err(bad(
            "SPX-HPB010",
            format!("{LOCK_FILE}: unknown member `{k}`"),
        ));
    }
    if m.get("schema").and_then(Value::as_str) != Some(LOCK_SCHEMA) {
        return Err(bad(
            "SPX-HPB010",
            format!("{LOCK_FILE}: expected schema `{LOCK_SCHEMA}`"),
        ));
    }
    let s = |o: &Map<String, Value>, k: &str| -> HarnessResult<String> {
        o.get(k)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| bad("SPX-HPB010", format!("{LOCK_FILE}: `{k}` must be a string")))
    };
    let config_digest = s(m, "config_digest")?;
    let list = m.get("bindings").and_then(Value::as_array).ok_or_else(|| {
        bad(
            "SPX-HPB010",
            format!("{LOCK_FILE}: `bindings` must be an array"),
        )
    })?;
    let mut bindings = Vec::new();
    for v in list {
        let o = v.as_object().ok_or_else(|| {
            bad(
                "SPX-HPB010",
                format!("{LOCK_FILE}: a binding must be an object"),
            )
        })?;
        const KNOWN: [&str; 7] = [
            "kind",
            "state",
            "provider_id",
            "provider_version",
            "adapter_version",
            "descriptor_digest",
            "upstream",
        ];
        if let Some(k) = o.keys().find(|k| !KNOWN.contains(&k.as_str())) {
            return Err(bad(
                "SPX-HPB010",
                format!("{LOCK_FILE}: unknown binding member `{k}`"),
            ));
        }
        let kind_name = s(o, "kind")?;
        let kind = CapabilityKind::parse(&kind_name).ok_or_else(|| {
            bad(
                "SPX-HPB010",
                format!("{LOCK_FILE}: unknown capability kind `{kind_name}`"),
            )
        })?;
        let state_name = s(o, "state")?;
        let state = BindingState::parse(&state_name).ok_or_else(|| {
            bad(
                "SPX-HPB010",
                format!("{LOCK_FILE}: unknown state `{state_name}`"),
            )
        })?;
        let has_id = o.contains_key("provider_id");
        let (provider_id, provider_version, adapter_version, descriptor_digest) = if has_id {
            (
                s(o, "provider_id")?,
                s(o, "provider_version")?,
                s(o, "adapter_version")?,
                s(o, "descriptor_digest")?,
            )
        } else {
            Default::default()
        };
        let upstream = match o.get("upstream") {
            None => None,
            Some(u) => {
                let u = u.as_object().ok_or_else(|| {
                    bad(
                        "SPX-HPB010",
                        format!("{LOCK_FILE}: `upstream` must be an object"),
                    )
                })?;
                Some(UpstreamBinding {
                    name: s(u, "name")?,
                    version: s(u, "version")?,
                    digest: s(u, "digest")?,
                })
            }
        };
        bindings.push(Binding {
            kind,
            provider_id,
            provider_version,
            adapter_version,
            descriptor_digest,
            upstream,
            state,
            reason: String::new(),
        });
    }
    Ok(Lock {
        config_digest,
        bindings,
    })
}

fn identity(b: &Binding) -> String {
    if b.provider_id.is_empty() {
        return format!("<{}>", b.state.as_str());
    }
    format!(
        "{}@{} (adapter {}, descriptor {})",
        b.provider_id, b.provider_version, b.adapter_version, b.descriptor_digest
    )
}

/// Frozen check: the current resolution must equal the lock exactly. Never
/// substitutes another provider; names the locked identity that is missing.
pub fn verify_frozen(lock: &Lock, current: &ResolvedProfile) -> HarnessResult<()> {
    if lock.config_digest != current.config_digest {
        return Err(bad("SPX-HPB013", format!("{} no longer matches the configuration it was locked from; run `semaprax harness resolve`", LOCK_FILE)));
    }
    for want in &lock.bindings {
        let name = want.kind.as_str();
        let got = current.binding(want.kind);
        let same = got.is_some_and(|g| {
            g.state == want.state
                && g.provider_id == want.provider_id
                && g.provider_version == want.provider_version
                && g.adapter_version == want.adapter_version
                && g.descriptor_digest == want.descriptor_digest
                && g.upstream == want.upstream
        });
        if !same {
            let have = got.map_or_else(
                || "nothing".to_string(),
                |g| format!("{} {} ({})", g.state.as_str(), identity(g), g.reason),
            );
            return Err(bad(
                "SPX-HPB014",
                format!("frozen resolution refused: lock requires {} for `{name}` but this machine resolves {have}", identity(want)),
            ));
        }
    }
    if let Some(extra) = current
        .bindings
        .iter()
        .find(|b| !lock.bindings.iter().any(|l| l.kind == b.kind))
    {
        return Err(bad(
            "SPX-HPB014",
            format!(
                "frozen resolution refused: `{}` is not in the lock",
                extra.kind.as_str()
            ),
        ));
    }
    Ok(())
}
