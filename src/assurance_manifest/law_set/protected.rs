//! Host-selected specification boundary. Receipts and identities are never authority.
use super::{drift, invalid, wire, LawSet, Result};
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const PROTECTED_LAW_REVIEW_SCHEMA: &str = "semaprax.protected-law-review.v1";

/// Independently retained by the host, never recovered from candidate evidence.
/// All source declarations are specification dependencies except explicitly
/// selected implementation bodies. Their signatures/contracts remain protected.
#[derive(Clone, Debug)]
pub struct ProtectedLawBaseline {
    laws: LawSet,
    editable: BTreeSet<String>,
    specification: Value,
    digest: String,
}
impl ProtectedLawBaseline {
    pub fn new(base: &ProjectRevision, laws: LawSet, editable_bodies: Vec<String>) -> Result<Self> {
        LawSet::replay(base, &laws.payload.proof_profile, laws.to_json())?;
        if laws.payload.laws.is_empty() {
            return Err(invalid("protected law baseline must be nonempty"));
        }
        if editable_bodies.len() > super::MAX_LAWS {
            return Err(super::capacity());
        }
        for identity in &editable_bodies {
            super::text_id(identity)?;
        }
        let editable: BTreeSet<_> = editable_bodies.iter().cloned().collect();
        if editable.len() != editable_bodies.len() {
            return Err(invalid("duplicate editable implementation identity"));
        }
        let specification = specification(base, &editable)?;
        if specification["editable_present"] != json!(editable) {
            return Err(invalid(
                "editable implementation identity is not an admitted top-level function",
            ));
        }
        let digest = wire::digest(
            b"semaprax.protected-law-baseline.v1\0",
            &wire::canonical(&json!({
                "law_set": laws.digest(), "editable_bodies": editable, "specification": specification,
            }))?,
        );
        Ok(Self {
            laws,
            editable,
            specification,
            digest,
        })
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn base_revision(&self) -> &str {
        &self.laws.payload.project_revision
    }

    pub fn review(
        &self,
        base: &ProjectRevision,
        candidate: &ProjectRevision,
        laws: &LawSet,
        candidate_digest: &str,
    ) -> Result<ProtectedLawReview> {
        if base.project_revision() != self.base_revision() {
            return Err(drift(
                "protected baseline is not bound to this base revision",
            ));
        }
        LawSet::replay(base, &self.laws.payload.proof_profile, self.laws.to_json())?;
        LawSet::replay(candidate, &laws.payload.proof_profile, laws.to_json())?;
        super::text_id(candidate_digest)?;
        let current = specification(candidate, &self.editable)?;
        let mut deltas = Vec::new();
        let old: BTreeMap<_, _> = self
            .laws
            .payload
            .laws
            .iter()
            .map(|row| (&row.definition.law_id, row))
            .collect();
        let new: BTreeMap<_, _> = laws
            .payload
            .laws
            .iter()
            .map(|row| (&row.definition.law_id, row))
            .collect();
        for (id, prior) in &old {
            match new.get(id) {
                None => deltas.push(json!({"kind":"law_removed", "law_id":id})),
                Some(next) => {
                    if prior.definition.selector != next.definition.selector {
                        deltas.push(json!({"kind":"subject_or_proposition_changed", "law_id":id, "non_weakening":"unknown"}));
                    }
                    if prior.definition.assumption_ids != next.definition.assumption_ids
                        || prior.definition.requires_laws != next.definition.requires_laws
                    {
                        deltas.push(json!({"kind":"assumptions_or_lemmas_changed", "law_id":id}));
                    }
                    if prior.definition.evidence != next.definition.evidence {
                        deltas.push(json!({"kind":"evidence_requirement_changed", "law_id":id}));
                    }
                    if prior.module_id != next.module_id || prior.source_path != next.source_path {
                        deltas.push(json!({"kind":"law_owner_changed", "law_id":id}));
                    }
                }
            }
        }
        for id in new.keys().filter(|id| !old.contains_key(*id)) {
            deltas.push(json!({"kind":"law_added", "law_id":id}));
        }
        let assumptions = |set: &LawSet| -> Value {
            json!(set
                .payload
                .modules
                .iter()
                .map(|module| (&module.module_id, &module.assumptions))
                .collect::<BTreeMap<_, _>>())
        };
        if assumptions(&self.laws) != assumptions(laws) {
            deltas.push(json!({"kind":"module_assumptions_or_inventory_changed"}));
        }
        if self.laws.payload.proof_profile != laws.payload.proof_profile {
            deltas.push(json!({"kind":"backend_or_trust_profile_changed"}));
        }
        if self.specification != current {
            deltas.push(json!({"kind":"protected_specification_closure_changed", "non_weakening":"unknown"}));
        }
        let requires_review = !deltas.is_empty();
        let document = wire::canonical(&json!({
            "schema": PROTECTED_LAW_REVIEW_SCHEMA, "baseline_digest": self.digest,
            "base_revision": self.base_revision(), "candidate_revision": candidate.project_revision(),
            "candidate_digest": candidate_digest, "baseline_law_digest": self.laws.digest(),
            "candidate_law_digest": laws.digest(), "candidate_specification": current,
            "requires_specification_review": requires_review,
            "non_weakening": if requires_review { "unknown" } else { "canonical_equivalence" },
            "proof_work_invalidated": self.laws.payload.program_root != laws.payload.program_root,
            "deltas": deltas, "suggested_next_action": if requires_review { "request_specification_change" } else { "repair_implementation_or_supply_proof" },
            "authority": false,
        }))?;
        let digest = wire::digest(b"semaprax.protected-law-review.v1\0", &document);
        Ok(ProtectedLawReview {
            document,
            digest,
            requires_review,
        })
    }
}

/// Authority-free proposal, including exact baseline, candidate and law digests.
#[derive(Clone, Debug)]
pub struct ProtectedLawReview {
    document: String,
    digest: String,
    requires_review: bool,
}
impl ProtectedLawReview {
    pub fn to_json(&self) -> &str {
        &self.document
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn requires_specification_review(&self) -> bool {
        self.requires_review
    }
    pub fn require(&self, approval: Option<&SpecificationChangeApproval>) -> Result<()> {
        if let Some(approval) = approval {
            if approval.review_digest != self.digest {
                return Err(drift(
                    "specification approval names a different base, candidate, law set or policy",
                ));
            }
        }
        if self.requires_review && approval.is_none() {
            return Err(vec![Diagnostic::io(
                "SPX-LW120",
                "protected law intent changed; explicit host specification approval required",
            )]);
        }
        Ok(())
    }
}

/// Implemented and supplied by the embedding trusted host. The compiler does
/// not authenticate people or infer authority from proposer/reviewer strings.
/// Do not expose this callback implementation to an untrusted candidate client.
pub trait SpecificationChangeAuthority {
    fn approve_specification_change(&mut self, proposal: &ProtectedLawReview) -> bool;
}

/// Not serializable or constructible from a report; a separately authenticated
/// host callback must accept the exact proposal. Grants no filesystem authority.
#[derive(Debug)]
pub struct SpecificationChangeApproval {
    review_digest: String,
}
impl SpecificationChangeApproval {
    pub fn request(
        proposal: &ProtectedLawReview,
        authority: &mut dyn SpecificationChangeAuthority,
    ) -> Result<Self> {
        if !authority.approve_specification_change(proposal) {
            return Err(vec![Diagnostic::io(
                "SPX-LW121",
                "host denied specification change",
            )]);
        }
        Ok(Self {
            review_digest: proposal.digest.clone(),
        })
    }
}

fn specification(revision: &ProjectRevision, editable: &BTreeSet<String>) -> Result<Value> {
    let mut seen = BTreeSet::new();
    let mut sources = BTreeMap::new();
    for source in revision.sources() {
        // The retained Project loader owns normalization of non-executable law
        // sources. Do not rediscover selection from filenames or candidate data.
        if source.source_graph_schema() == "semaprax.native-law.v1" {
            sources.insert(
                source.path(),
                wire::digest(
                    b"semaprax.protected-specification-source.v1\0",
                    source.source(),
                ),
            );
            continue;
        }
        let mut program =
            crate::parse(source.source(), source.path()).map_err(|error| vec![error])?;
        for function in &mut program.functions {
            if editable.contains(&function.stable_id) {
                seen.insert(function.stable_id.clone());
                function.body.kind = crate::ast::ExprKind::Int(0);
            }
        }
        sources.insert(
            source.path(),
            wire::digest(
                b"semaprax.protected-specification-source.v1\0",
                &crate::format::canonical(&program),
            ),
        );
    }
    Ok(
        json!({"sources": sources, "editable_present": seen, "manifest": revision.manifest().to_canonical_toml()}),
    )
}
