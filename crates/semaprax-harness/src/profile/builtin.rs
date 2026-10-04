//! Providers compiled into the host: they have no entry, no process and need no
//! trust grant. `model.generate` has no builtin (its fallback is "unavailable").

use crate::contract::{CapabilityKind, Descriptor};
use crate::json;
use serde_json::json;

pub const NATIVE_CONTEXT: &str = "semaprax/native-context";
pub const RAW_COMMAND: &str = "semaprax/raw-command";
pub const RULES_DECISION: &str = "semaprax/rules-decision";
pub const PLAIN_SKILLS: &str = "semaprax/plain-skills";

/// Builtin provider id for `kind`, when one exists.
pub fn provider_for(kind: CapabilityKind) -> Option<&'static str> {
    match kind {
        CapabilityKind::ContextRepository => Some(NATIVE_CONTEXT),
        CapabilityKind::CommandView => Some(RAW_COMMAND),
        CapabilityKind::DecisionEvaluate => Some(RULES_DECISION),
        CapabilityKind::SkillCatalog => Some(PLAIN_SKILLS),
        CapabilityKind::ModelGenerate => None,
    }
}

pub fn kind_of(provider_id: &str) -> Option<CapabilityKind> {
    CapabilityKind::ALL
        .into_iter()
        .find(|k| provider_for(*k) == Some(provider_id))
}

pub fn is_builtin(provider_id: &str) -> bool {
    kind_of(provider_id).is_some()
}

/// The runtime-builtin descriptor of a builtin provider.
pub fn descriptor(provider_id: &str) -> Option<Descriptor> {
    let kind = kind_of(provider_id)?;
    let doc = json!({
        "schema": "semaprax.harness-provider.v1",
        "provider": {"id": provider_id, "version": "1.0.0"},
        "adapter": {"runtime": "builtin", "version": "1.0.0"},
        "capabilities": [{"kind": kind.as_str(), "version": 1, "required": true, "operations": kind.operations()}],
        "platforms": ["macos-aarch64", "macos-x86_64", "linux-aarch64", "linux-x86_64"],
        "protocol": {"name": "semaprax.harness-rpc.v1", "min": 1, "max": 1},
        "permissions": {"read": [], "write": [], "network": [], "process": [], "secrets": []},
        "resources": {"handshake_timeout_ms": 1000, "invoke_timeout_ms": 30000, "max_frame_bytes": 1048576,
                      "max_concurrency": 1, "idle_shutdown_ms": 60000},
        "cancellation": "cooperative",
        "support": {"license": "Apache-2.0", "isolation": "in-process", "tested": []},
    });
    Some(Descriptor::parse(json::canonical(&doc).as_bytes()).expect("builtin descriptor is valid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_descriptor_parses() {
        for k in CapabilityKind::ALL {
            if let Some(id) = provider_for(k) {
                let d = descriptor(id).unwrap();
                assert_eq!(d.provider_id, id);
                assert_eq!(kind_of(id), Some(k));
            }
        }
        assert!(provider_for(CapabilityKind::ModelGenerate).is_none());
    }
}
