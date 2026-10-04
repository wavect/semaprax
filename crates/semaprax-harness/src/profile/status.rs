//! Read-only views: `status`, `explain`, `inspect`.

use super::builtin;
use super::config::HarnessConfig;
use super::installations::LocalState;
use super::lock;
use super::resolve::{resolve_report, verdict_of, Resolution};
use crate::contract::{CapabilityKind, Descriptor};
use crate::json;
use serde_json::{json, Value};
use std::path::Path;

fn permissions_line(p: &crate::contract::PermissionRequest) -> String {
    let f = |n: &str, v: &Vec<String>| format!("{n}=[{}]", v.join(","));
    [
        f("read", &p.read),
        f("write", &p.write),
        f("network", &p.network),
        f("process", &p.process),
        f("secrets", &p.secrets),
    ]
    .join(" ")
}

fn lock_state(project: &Path, r: &Resolution) -> String {
    match lock::load(project) {
        Ok(None) => "absent".into(),
        Ok(Some(l)) => match lock::verify_frozen(&l, &r.profile) {
            Ok(()) => "matches".into(),
            Err(e) => format!("differs ({})", e.message),
        },
        Err(e) => format!("invalid ({})", e.message),
    }
}

fn binding_label(r: &Resolution, kind: CapabilityKind) -> &'static str {
    let b = r.profile.binding(kind).expect("every kind is bound");
    if r.profile.ambiguous.contains_key(&kind) {
        return "ambiguous";
    }
    match b.state {
        super::resolve::BindingState::Fallback => "fallback",
        s => s.as_str(),
    }
}

pub fn status(
    project: &Path,
    config: &HarnessConfig,
    state: &LocalState,
    as_json: bool,
) -> (String, bool) {
    let r = resolve_report(config, state);
    let ok = r.unmet.is_empty();
    let lock = lock_state(project, &r);
    if as_json {
        let bindings: Vec<Value> = r
            .profile
            .bindings
            .iter()
            .map(|b| {
                let cands: Vec<Value> = r
                    .profile
                    .candidates
                    .iter()
                    .filter(|c| c.kind == b.kind)
                    .map(|c| json!({"provider_id": c.provider_id, "verdict": c.verdict.as_str(), "detail": c.detail}))
                    .collect();
                json!({"kind": b.kind.as_str(), "state": binding_label(&r, b.kind), "provider_id": b.provider_id,
                       "provider_version": b.provider_version, "reason": b.reason, "candidates": cands})
            })
            .collect();
        let doc = json!({"schema": "semaprax.harness-status.v1", "config_digest": r.profile.config_digest, "lock": lock,
                         "inactive": r.profile.inactive, "ok": ok, "bindings": bindings});
        return (format!("{}\n", json::canonical(&doc)), ok);
    }
    let mut out = format!("config digest: {}\nlock: {lock}\n", r.profile.config_digest);
    for b in &r.profile.bindings {
        let who = if b.provider_id.is_empty() {
            "-".to_string()
        } else {
            format!("{}@{}", b.provider_id, b.provider_version)
        };
        out.push_str(&format!(
            "{:<19} {:<11} {who}\n    {}\n",
            b.kind.as_str(),
            binding_label(&r, b.kind),
            b.reason
        ));
        for c in r.profile.candidates.iter().filter(|c| c.kind == b.kind) {
            out.push_str(&format!(
                "    candidate {} {}: {}\n",
                c.provider_id,
                c.verdict.as_str(),
                c.detail
            ));
        }
    }
    for x in &r.profile.inactive {
        out.push_str(&format!(
            "inactive extension `{x}` (visible, never active)\n"
        ));
    }
    (out, ok)
}

pub fn explain(config: &HarnessConfig, state: &LocalState, kind: CapabilityKind) -> String {
    let r = resolve_report(config, state);
    let cc = config.capability(kind);
    let b = r.profile.binding(kind).expect("every kind is bound");
    let mut out = format!("{}\nmode: {}\n", kind.as_str(), cc.mode.as_str());
    out.push_str(&format!(
        "project pin: {}\n",
        cc.provider.as_deref().unwrap_or("none")
    ));
    out.push_str(&format!(
        "user preference: {}\n",
        state.preferences.get(&kind).map_or("none", String::as_str)
    ));
    out.push_str(&format!(
        "builtin fallback: {}\n",
        builtin::provider_for(kind).unwrap_or("none (unavailable)")
    ));
    out.push_str("precedence: disabled > project pin > user preference > single compatible trusted installation > builtin fallback\n");
    for c in r.profile.candidates.iter().filter(|c| c.kind == kind) {
        out.push_str(&format!(
            "candidate {} {}: {}\n",
            c.provider_id,
            c.verdict.as_str(),
            c.detail
        ));
    }
    if let Some(ids) = r.profile.ambiguous.get(&kind) {
        out.push_str(&format!("ambiguous between: {}\n", ids.join(", ")));
    }
    let who = if b.provider_id.is_empty() {
        "no provider".to_string()
    } else {
        format!("{}@{}", b.provider_id, b.provider_version)
    };
    out.push_str(&format!(
        "result: {} ({who}): {}\n",
        binding_label(&r, kind),
        b.reason
    ));
    out
}

fn summary(d: &Descriptor) -> String {
    let caps: Vec<String> = d
        .capabilities
        .iter()
        .map(|c| format!("{}/v{}", c.kind_name, c.version))
        .collect();
    format!(
        "provider: {}@{}\nruntime: {} (adapter {})\ncapabilities: {}\nplatforms: {}\nrequested permissions: {}\n",
        d.provider_id, d.provider_version, d.runtime.as_str(), d.adapter_version, caps.join(", "), d.platforms.join(", "), permissions_line(&d.permissions)
    )
}

pub fn inspect(
    state: &LocalState,
    provider_id: &str,
) -> Result<String, crate::diag::HarnessDiagnostic> {
    if let Some(d) = builtin::descriptor(provider_id) {
        return Ok(format!(
            "{}descriptor digest: {}\nbuiltin: compiled in; no process, no trust grant needed\n",
            summary(&d),
            d.digest()
        ));
    }
    let inst = state.installations.get(provider_id).ok_or_else(|| {
        crate::diag::HarnessDiagnostic::new("SPX-HPB023", format!("provider `{provider_id}` is not adopted on this machine; run `semaprax harness adopt <descriptor>`"))
    })?;
    let mut out = String::new();
    match inst.inspect() {
        Ok(i) => {
            out.push_str(&summary(&i.descriptor));
            out.push_str(&format!(
                "descriptor digest: {}\n",
                i.current.descriptor_digest
            ));
            out.push_str(&format!(
                "entry digest: {}\n",
                i.current.entry_digest.as_deref().unwrap_or("none")
            ));
            if let Some(u) = &i.descriptor.upstream {
                out.push_str(&format!(
                    "upstream: {} {} supported [{}]\n",
                    u.name,
                    u.package,
                    u.versions.join(", ")
                ));
            }
            out.push_str(&format!(
                "upstream digest: {}\n",
                i.current.upstream_digest.as_deref().unwrap_or("none")
            ));
        }
        Err(e) => out.push_str(&format!("unreadable: {}\n", e.message)),
    }
    if let Some(u) = &inst.upstream {
        out.push_str(&format!(
            "adopted upstream: {} version {} compatible={}\n",
            u.path.display(),
            u.version.as_deref().unwrap_or("unidentified"),
            u.compatible
        ));
    }
    match state.trust.get(provider_id) {
        Some(t) => out.push_str(&format!(
            "granted permissions: {}\n",
            permissions_line(&crate::contract::PermissionRequest {
                read: t.granted.read.clone(),
                write: t.granted.write.clone(),
                network: t.granted.network.clone(),
                process: t.granted.process.clone(),
                secrets: t.granted.secrets.clone(),
            })
        )),
        None => out.push_str("granted permissions: none (not trusted)\n"),
    }
    let (v, detail) = verdict_of(inst, state);
    out.push_str(&format!(
        "compatibility and trust: {}: {detail}\n",
        v.as_str()
    ));
    Ok(out)
}
