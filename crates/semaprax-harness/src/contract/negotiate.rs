//! Deterministic capability negotiation, identity and downgrade checks.
//! Negotiation is pure: it never launches a process and its output carries
//! no permission grant.

use super::descriptor::{parse_version, Descriptor};
use super::kind::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use std::collections::BTreeMap;

/// Kinds and versions the host implements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSupport {
    kinds: BTreeMap<CapabilityKind, Vec<u32>>,
}

impl HostSupport {
    /// The five first-wave kinds at their supported versions.
    pub fn first_wave() -> Self {
        Self {
            kinds: CapabilityKind::ALL
                .into_iter()
                .map(|k| (k, k.supported_versions().to_vec()))
                .collect(),
        }
    }

    /// Replace one kind's implemented versions (empty removes the kind).
    pub fn with(mut self, kind: CapabilityKind, versions: &[u32]) -> Self {
        if versions.is_empty() {
            self.kinds.remove(&kind);
        } else {
            self.kinds.insert(kind, versions.to_vec());
        }
        self
    }

    pub fn versions(&self, kind: CapabilityKind) -> &[u32] {
        self.kinds.get(&kind).map_or(&[], Vec::as_slice)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveCapability {
    pub kind: CapabilityKind,
    pub version: u32,
    pub operations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InactiveCapability {
    pub kind_name: String,
    pub version: u32,
    pub reason: String,
}

/// Result of negotiation. Declaration order is preserved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Negotiation {
    pub active: Vec<ActiveCapability>,
    pub inactive: Vec<InactiveCapability>,
}

/// Decide, before any launch, which declared capabilities become active.
/// A required capability of an unknown kind (`SPX-HPA019`) or an unsupported
/// version (`SPX-HPA023`) is an error; optional ones and every extension are
/// reported inactive.
pub fn negotiate(descriptor: &Descriptor, host: &HostSupport) -> HarnessResult<Negotiation> {
    let mut n = Negotiation {
        active: Vec::new(),
        inactive: Vec::new(),
    };
    for c in &descriptor.capabilities {
        let inactive = |reason: &str| InactiveCapability {
            kind_name: c.kind_name.clone(),
            version: c.version,
            reason: reason.to_string(),
        };
        match c.kind {
            None if c.required => {
                return Err(HarnessDiagnostic::new(
                    "SPX-HPA019",
                    format!(
                        "required capability kind `{}` is unknown to this host",
                        c.kind_name
                    ),
                ))
            }
            None => n.inactive.push(inactive("unknown capability kind")),
            Some(k) if host.versions(k).contains(&c.version) => n.active.push(ActiveCapability {
                kind: k,
                version: c.version,
                operations: c.operations.clone(),
            }),
            Some(k) if c.required => {
                return Err(HarnessDiagnostic::new(
                    "SPX-HPA023",
                    format!(
                        "required capability {} v{} is not supported (host supports {:?})",
                        k.as_str(),
                        c.version,
                        host.versions(k)
                    ),
                ))
            }
            Some(_) => n.inactive.push(inactive("unsupported capability version")),
        }
    }
    for x in &descriptor.extensions {
        n.inactive.push(InactiveCapability {
            kind_name: x.kind.clone(),
            version: x.version,
            reason: "extension capabilities are advertised metadata only".into(),
        });
    }
    Ok(n)
}

/// Two descriptors with the same provider id in one resolution are refused.
pub fn check_duplicate_identities(descriptors: &[Descriptor]) -> HarnessResult<()> {
    let mut seen: Vec<&str> = Vec::new();
    for d in descriptors {
        if seen.contains(&d.provider_id.as_str()) {
            return Err(HarnessDiagnostic::new(
                "SPX-HPA020",
                format!(
                    "duplicate identity: provider `{}` appears more than once",
                    d.provider_id
                ),
            ));
        }
        seen.push(&d.provider_id);
    }
    Ok(())
}

/// Identity and versions recorded in a lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockedIdentity {
    pub provider_id: String,
    pub provider_version: String,
    pub adapter_version: String,
}

/// Refuse a candidate whose provider or adapter version is lower than the
/// locked one, or whose identity differs (`SPX-HPA021`).
pub fn refuse_downgrade(locked: &LockedIdentity, candidate: &Descriptor) -> HarnessResult<()> {
    let fail = |m: String| Err(HarnessDiagnostic::new("SPX-HPA021", m));
    if locked.provider_id != candidate.provider_id {
        return fail(format!(
            "candidate `{}` is not the locked provider `{}`",
            candidate.provider_id, locked.provider_id
        ));
    }
    for (what, old, new) in [
        (
            "provider",
            &locked.provider_version,
            &candidate.provider_version,
        ),
        (
            "adapter",
            &locked.adapter_version,
            &candidate.adapter_version,
        ),
    ] {
        let (Some(o), Some(n)) = (parse_version(old), parse_version(new)) else {
            return fail(format!(
                "{what} version cannot be compared: `{old}` vs `{new}`"
            ));
        };
        if cmp(&n, &o) == std::cmp::Ordering::Less {
            return fail(format!(
                "{what} version {new} is a downgrade from locked {old}"
            ));
        }
    }
    Ok(())
}

fn cmp(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    (0..a.len().max(b.len()))
        .map(|i| a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0)))
        .find(|o| o.is_ne())
        .unwrap_or(std::cmp::Ordering::Equal)
}
