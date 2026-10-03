//! Builder-owned LAW-09 publication evidence. This stays distinct from a
//! core Project diagnostic and does not by itself satisfy protected LawSet.

use super::{domain_digest, ProjectNativeRustSdkBundle};
use crate::diagnostic::Diagnostic;
use semaprax::assurance_manifest::law_set::{
    self,
    strict::{RequiredLawEvidence, StrictLawPolicy},
    LawPolicy, LawSet,
};
use semaprax::project::{ForeignCallerCertificate, ProjectRevision};
use serde_json::{json, Value};

fn mismatch(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-FL311", message)]
}

/// An exact conditional caller and guarded Project SDK publication retained
/// together. Only the guarded builder can populate the private bundle facts.
/// Foreign behavior remains the certificate's declared assumption.
#[derive(Clone, Debug)]
pub struct GuardedForeignCallerEvidence {
    bundle: ProjectNativeRustSdkBundle,
    caller: ForeignCallerCertificate,
}

impl ProjectNativeRustSdkBundle {
    pub fn bind_guarded_foreign_caller(
        &self,
        revision: &ProjectRevision,
        caller: ForeignCallerCertificate,
    ) -> Result<GuardedForeignCallerEvidence, Vec<Diagnostic>> {
        let Some(frontier) = &self.guarded_frontier else {
            return Err(mismatch(
                "Project SDK has no builder-retained foreign guard",
            ));
        };
        if frontier != caller.frontier()
            || self.project_revision != revision.project_revision()
            || self.workspace_revision != revision.workspace_revision()
            || self.sdk.manifest_digest() != caller.adapter_digest()
            || self.sdk.target_triple() != frontier.target()
        {
            return Err(mismatch(
                "guarded Project SDK publication differs from conditional caller",
            ));
        }
        caller.verify_published_guard(
            revision,
            self.sdk.output_directory(),
            self.sdk.manifest_digest(),
        )?;
        Ok(GuardedForeignCallerEvidence {
            bundle: self.clone(),
            caller,
        })
    }
}

impl GuardedForeignCallerEvidence {
    pub fn caller(&self) -> &ForeignCallerCertificate {
        &self.caller
    }

    pub fn manifest_digest(&self) -> &str {
        self.bundle.manifest_digest()
    }

    /// Recheck exact retained source and published package bytes at every
    /// consuming boundary; this grants no runtime or filesystem authority.
    pub fn replay(&self, revision: &ProjectRevision) -> Result<(), Vec<Diagnostic>> {
        let expected = self
            .bundle
            .bind_guarded_foreign_caller(revision, self.caller.clone())?;
        if expected.bundle != self.bundle || expected.caller != self.caller {
            return Err(mismatch("guarded foreign caller publication changed"));
        }
        Ok(())
    }

    /// Bounded builder-owned strict route for exactly one protected foreign
    /// law with one exact source owner. The ordinary core strict route stays open.
    /// This accepts named foreign assumptions and a retained runtime guard,
    /// not a theorem about the Rust implementation or an execution receipt.
    pub fn derive_conditional_strict_law_report(
        &self,
        revision: &ProjectRevision,
        laws: &LawSet,
        policy: &StrictLawPolicy,
    ) -> Result<String, Vec<Diagnostic>> {
        self.replay(revision)?;
        let held = LawSet::replay(revision, laws.proof_profile(), laws.to_json())?;
        if held.digest() != laws.digest() || held.digest() != policy.baseline().digest() {
            return Err(mismatch(
                "conditional foreign law baseline differs from retained inventory",
            ));
        }
        let mut owner = None;
        for source in revision.sources() {
            let program = semaprax::parse(source.source(), std::path::Path::new(source.path()))
                .map_err(|_| mismatch("conditional foreign law source cannot be reparsed"))?;
            if program
                .functions
                .iter()
                .any(|function| function.stable_id == self.caller.caller_id())
            {
                if owner.replace(source).is_some() {
                    return Err(mismatch(
                        "conditional foreign caller has multiple source owners",
                    ));
                }
            }
        }
        let source =
            owner.ok_or_else(|| mismatch("conditional foreign caller source is absent"))?;
        let law_id = self.caller.law_id();
        let Some(RequiredLawEvidence::ForeignConditionalGuard {
            adapter_digest,
            summary_digest,
            accepted_conditions,
        }) = policy.requirements().get(law_id)
        else {
            return Err(mismatch("conditional foreign law requirement is absent"));
        };
        if policy.requirements().len() != 1
            || adapter_digest != self.manifest_digest()
            || summary_digest != self.caller.frontier().summary_digest()
        {
            return Err(mismatch("conditional foreign law policy identity differs"));
        }
        let mut actual_conditions = self.caller.conditions().to_vec();
        actual_conditions.sort();
        actual_conditions.dedup();
        let mut accepted = accepted_conditions.clone();
        accepted.sort();
        accepted.dedup();
        if actual_conditions.len() != self.caller.conditions().len()
            || accepted.len() != accepted_conditions.len()
            || accepted != actual_conditions
        {
            return Err(mismatch(
                "conditional foreign law assumptions are not exactly accepted",
            ));
        }
        let document: Value = serde_json::from_str(held.to_json())
            .map_err(|_| mismatch("conditional foreign law inventory is malformed"))?;
        let Some(rows) = document["payload"]["laws"].as_array() else {
            return Err(mismatch("conditional foreign law rows are absent"));
        };
        let Some(modules) = document["payload"]["modules"].as_array() else {
            return Err(mismatch("conditional foreign law modules are absent"));
        };
        if rows.len() != 1 || modules.len() != 1 {
            return Err(mismatch(
                "conditional foreign law profile requires one protected law",
            ));
        }
        let row = &rows[0];
        let module = &modules[0];
        let (minimum, maximum) = self
            .caller
            .guarded_i64_range()
            .ok_or_else(|| mismatch("conditional foreign law guard is absent"))?;
        if row["definition"]["law_id"] != law_id
            || row["definition"]["selector"]
                != json!({"kind":"foreign_guarded_caller","caller_id":self.caller.caller_id(),"import_id":self.caller.import_id(),"minimum":minimum,"maximum":maximum})
            || row["definition"]["assumption_ids"] != json!(actual_conditions)
            || row["definition"]["requires_laws"] != json!([])
            || row["definition"]["evidence"] != "runtime_guarded"
            || row["source_path"] != source.path()
            || module["source_path"] != source.path()
            || row["source_digest"] != source.source_digest()
            || module["assumptions"] != json!(actual_conditions)
            || held.semantic_digest(law_id) != row["semantic_digest"].as_str()
        {
            return Err(mismatch(
                "conditional foreign law scope or source owner differs",
            ));
        }
        let inventory = law_set::derive_report(
            revision,
            &held,
            &LawPolicy::strict(policy.baseline().clone())?,
        )?;
        let inventory_value: Value = serde_json::from_str(&inventory)
            .map_err(|_| mismatch("conditional foreign law report is malformed"))?;
        let inventory_payload = &inventory_value["payload"];
        if inventory_payload["counts"]["required"] != 1
            || inventory_payload["accepted"] != false
            || inventory_payload["laws"][0]["law_id"] != law_id
            || inventory_payload["laws"][0]["status"] != "awaiting_evidence"
            || inventory_payload["laws"][0]["reason"]
                != "builder_authenticated_foreign_attachment_unavailable"
        {
            return Err(mismatch(
                "ordinary foreign law frontier did not remain open",
            ));
        }
        let report = json!({
            "schema":"semaprax.builder-foreign-conditional-strict-law.v1",
            "project_revision":revision.project_revision(),
            "program_root":inventory_payload["program_root"],
            "policy_digest":policy.digest(),
            "law_digest":held.digest(),
            "law_id":law_id,
            "semantic_digest":held.semantic_digest(law_id),
            "inventory_report_digest":domain_digest(b"semaprax.builder-foreign-law-inventory.v1\0",inventory.as_bytes()),
            "adapter_digest":self.manifest_digest(),
            "summary_digest":self.caller.frontier().summary_digest(),
            "accepted_conditions":actual_conditions,
            "accepted":true,
            "source_route_proved":true,
            "builder_guard_authenticated":true,
            "foreign_internals_proved":false,
            "runtime_call_observed":false,
            "source_authority":false,
            "execution_authority":false,
            "publication_authority":false,
        });
        let mut encoded = serde_json::to_string(&report)
            .map_err(|_| mismatch("conditional foreign law report cannot be encoded"))?;
        encoded.push('\n');
        if encoded.len() > law_set::MAX_BYTES {
            return Err(mismatch("conditional foreign law report exceeds bound"));
        }
        Ok(encoded)
    }

    /// Exact rederivation prevents a submitted report from granting coverage.
    pub fn require_conditional_strict_law_report(
        &self,
        document: &str,
        revision: &ProjectRevision,
        laws: &LawSet,
        policy: &StrictLawPolicy,
    ) -> Result<(), Vec<Diagnostic>> {
        let expected = self.derive_conditional_strict_law_report(revision, laws, policy)?;
        if document != expected {
            return Err(mismatch(
                "conditional foreign strict law report differs from replay",
            ));
        }
        Ok(())
    }
}
