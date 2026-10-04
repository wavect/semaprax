//! `semaprax.harness-rpc.v1` message construction and handshake validation.

use crate::contract::{ActiveCapability, CapabilityKind, Descriptor, ProjectBinding};
use crate::diag::HarnessDiagnostic;
use serde_json::{json, Value};

pub(crate) const PROTOCOL: &str = "semaprax.harness-rpc.v1";

pub(crate) fn initialize_params(
    d: &Descriptor,
    offered: &[ActiveCapability],
    project: &ProjectBinding,
) -> Value {
    json!({
        "protocol": PROTOCOL,
        "host_version": concat!("semaprax-harness/", env!("CARGO_PKG_VERSION")),
        "descriptor_digest": d.digest(),
        "offered": offered.iter().map(|c| json!({"kind": c.kind.as_str(), "version": c.version})).collect::<Vec<_>>(),
        "project": {"id": project.id, "worktree": project.worktree},
    })
}

/// Validate an initialize result: right protocol, `accepted` a subset of what
/// was offered (no extra kinds, versions or operations), no duplicates.
/// Violations are `SPX-HPC006` and quarantine the adapter.
pub(crate) fn parse_initialize(
    result: &Value,
    offered: &[ActiveCapability],
) -> Result<Vec<ActiveCapability>, HarnessDiagnostic> {
    let bad = |m: String| HarnessDiagnostic::new("SPX-HPC006", m);
    let obj = result
        .as_object()
        .ok_or_else(|| bad("initialize result is not an object".into()))?;
    if obj.get("protocol").and_then(Value::as_str) != Some(PROTOCOL) {
        return Err(bad(format!(
            "adapter answered protocol {:?}; this host speaks `{PROTOCOL}`",
            obj.get("protocol")
        )));
    }
    let list = obj
        .get("accepted")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("initialize result has no `accepted` list".into()))?;
    let mut out: Vec<ActiveCapability> = Vec::new();
    for item in list {
        let kind = item
            .get("kind")
            .and_then(Value::as_str)
            .and_then(CapabilityKind::parse);
        let version = item
            .get("version")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok());
        let (Some(kind), Some(version)) = (kind, version) else {
            return Err(bad(
                "accepted entry has an unknown kind or malformed version".into(),
            ));
        };
        let Some(o) = offered
            .iter()
            .find(|o| o.kind == kind && o.version == version)
        else {
            return Err(bad(format!(
                "adapter accepted {} v{version}, which the host did not offer",
                kind.as_str()
            )));
        };
        if out.iter().any(|a| a.kind == kind) {
            return Err(bad(format!("adapter accepted {} twice", kind.as_str())));
        }
        let ops: Vec<String> = item
            .get("operations")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .ok_or_else(|| bad("accepted entry has no operations list".into()))?;
        if let Some(extra) = ops.iter().find(|op| !o.operations.contains(op)) {
            return Err(bad(format!(
                "adapter accepted undeclared operation `{extra}` of {}",
                kind.as_str()
            )));
        }
        out.push(ActiveCapability {
            kind,
            version,
            operations: ops,
        });
    }
    Ok(out)
}
