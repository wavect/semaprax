//! Dependency-complete logical index for the replayed LAW-04 law inventory.
//!
//! Its task digests exclude Project/source bytes; those remain mandatory in
//! each freshly minted evidence record. Only checked normalized law meanings,
//! declared assumption owners and transitive prerequisite laws contribute to
//! logical reuse. No law proof is created by this index.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{invalid, wire, LawRow, LawSet, Result};

pub const SCHEMA: &str = "semaprax.law-proof-dependency-index.v1";
const DIGEST_DOMAIN: &[u8] = b"semaprax.law-proof-dependency.v1\0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyRow {
    pub semantic_digest: String,
    pub logical_digest: String,
    pub assumption_ids: Vec<String>,
    pub requires_laws: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyIndex {
    pub schema: String,
    pub proof_profile: String,
    /// Current association only; it is deliberately outside logical digests.
    pub project_revision: String,
    pub program_root: String,
    pub laws: BTreeMap<String, DependencyRow>,
}

impl DependencyIndex {
    pub fn logical_digest(&self, law_id: &str) -> Option<&str> {
        self.laws.get(law_id).map(|row| row.logical_digest.as_str())
    }

    pub fn to_json(&self) -> Result<String> {
        wire::canonical(self)
    }

    /// Select exact transitive prerequisites and return them in canonical
    /// dependency order. Reject missing and cyclic edges even for a caller
    /// constructed index; no solver work should start from an incomplete plan.
    pub fn ordered_closure(&self, selected: &[String]) -> Result<Vec<String>> {
        if selected.is_empty() || selected.len() > super::MAX_LAWS {
            return Err(invalid(
                "selected law dependency closure is empty or over capacity",
            ));
        }
        let mut pending = selected.iter().cloned().collect::<BTreeSet<_>>();
        let mut closure = BTreeSet::new();
        while let Some(id) = pending.pop_first() {
            if !closure.insert(id.clone()) {
                continue;
            }
            if closure.len() > super::MAX_LAWS {
                return Err(invalid("selected law dependency closure exceeds capacity"));
            }
            let row = self
                .laws
                .get(&id)
                .ok_or_else(|| invalid("selected law dependency is absent"))?;
            pending.extend(row.requires_laws.iter().cloned());
        }
        let mut order = Vec::with_capacity(closure.len());
        while !closure.is_empty() {
            let ready = closure
                .iter()
                .filter(|id| {
                    self.laws[*id]
                        .requires_laws
                        .iter()
                        .all(|required| order.contains(required))
                })
                .cloned()
                .collect::<Vec<_>>();
            if ready.is_empty() {
                return Err(invalid("selected law dependency is cyclic or absent"));
            }
            for id in ready {
                closure.remove(&id);
                order.push(id);
            }
        }
        Ok(order)
    }
}

/// Recompute a deterministic topological index from the checked LawSet. A
/// missing or cyclic edge is refused even if a caller supplies a stale row.
pub fn derive(laws: &LawSet) -> Result<DependencyIndex> {
    let mut assumption_owners = BTreeMap::<&str, &str>::new();
    for module in &laws.payload.modules {
        for assumption in &module.assumptions {
            if assumption_owners
                .insert(assumption.as_str(), module.module_id.as_str())
                .is_some()
            {
                return Err(invalid(
                    "duplicate assumption owner in law dependency index",
                ));
            }
        }
    }
    let mut source = BTreeMap::<&str, &LawRow>::new();
    for row in &laws.payload.laws {
        if source.insert(row.definition.law_id.as_str(), row).is_some() {
            return Err(invalid("duplicate law in dependency index"));
        }
    }
    let mut pending = source.keys().copied().collect::<BTreeSet<_>>();
    let mut indexed = BTreeMap::<String, DependencyRow>::new();
    while !pending.is_empty() {
        let ready = pending
            .iter()
            .copied()
            .filter(|id| {
                source[*id]
                    .definition
                    .requires_laws
                    .iter()
                    .all(|required| indexed.contains_key(required))
            })
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err(invalid(
                "missing or cyclic law dependency in checked inventory",
            ));
        }
        for id in ready {
            let row = source[id];
            let assumptions = row
                .definition
                .assumption_ids
                .iter()
                .map(|assumption| {
                    assumption_owners
                        .get(assumption.as_str())
                        .map(|owner| json!([assumption, owner]))
                        .ok_or_else(|| invalid("law assumption owner is missing"))
                })
                .collect::<Result<Vec<_>>>()?;
            let required = row
                .definition
                .requires_laws
                .iter()
                .map(|required| {
                    indexed
                        .get(required)
                        .map(|dependency| json!([required, dependency.logical_digest]))
                        .ok_or_else(|| invalid("prerequisite law is missing"))
                })
                .collect::<Result<Vec<_>>>()?;
            let logical_digest = wire::digest(
                DIGEST_DOMAIN,
                &wire::canonical(&json!({
                    "proof_profile": laws.payload.proof_profile,
                    "module_id": row.module_id,
                    "semantic_digest": row.semantic_digest,
                    "assumptions": assumptions,
                    "requires_laws": required,
                }))?,
            );
            indexed.insert(
                id.into(),
                DependencyRow {
                    semantic_digest: row.semantic_digest.clone(),
                    logical_digest,
                    assumption_ids: row.definition.assumption_ids.clone(),
                    requires_laws: row.definition.requires_laws.clone(),
                },
            );
            pending.remove(id);
        }
    }
    Ok(DependencyIndex {
        schema: SCHEMA.into(),
        proof_profile: laws.payload.proof_profile.clone(),
        project_revision: laws.payload.project_revision.clone(),
        program_root: laws.payload.program_root.clone(),
        laws: indexed,
    })
}
