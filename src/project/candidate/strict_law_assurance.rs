//! Explicit candidate strict-law join. This does not install a global policy.
use super::{publication, wire, ProjectCandidate};
use crate::assurance_manifest::{
    law_set::{
        native_proof::VerifiedLawProof,
        protected::{ProtectedLawBaseline, SpecificationChangeApproval},
        strict::{self, StrictLawPolicy},
        LawSet,
    },
    VerifiedProjectProof,
};
use crate::diagnostic::Diagnostic;
use crate::project::host_policy::{SelectedStrictLaw, StrictWorkspacePermit};
use std::path::Path;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub const STRICT_CANDIDATE_LAW_SCHEMA: &str = "semaprax.project-candidate-strict-law-assurance.v1";

/// Independently supplied host policy/evidence selection. No field is a proof
/// string or an execution capability; opaque approvals/proofs retain their owners.
pub struct StrictCandidateLawInputs<'a> {
    pub protection: &'a ProtectedLawBaseline,
    pub policy: &'a StrictLawPolicy,
    pub laws: &'a LawSet,
    pub proofs: &'a [VerifiedProjectProof],
    pub native_proofs: &'a [VerifiedLawProof],
    pub specification_approval: Option<&'a SpecificationChangeApproval>,
}
impl StrictCandidateLawInputs<'_> {
    fn require(&self, candidate: &ProjectCandidate) -> Result<()> {
        candidate
            .protected_law_review(self.protection, self.laws)?
            .require(self.specification_approval)?;
        let report = candidate.strict_law_assurance_with_native_proofs(
            candidate.candidate_digest(),
            self.laws,
            self.policy,
            self.proofs,
            self.native_proofs,
        )?;
        candidate.require_strict_law_assurance_with_native_proofs(
            &report,
            self.laws,
            self.policy,
            self.proofs,
            self.native_proofs,
        )
    }

    fn require_host_selection(
        &self,
        candidate: &ProjectCandidate,
        workspace_root: &Path,
    ) -> Result<(String, String)> {
        let Some(selection) = SelectedStrictLaw::open(workspace_root)? else {
            self.require(candidate)?;
            let report = candidate.strict_law_assurance_with_native_proofs(
                candidate.candidate_digest(),
                self.laws,
                self.policy,
                self.proofs,
                self.native_proofs,
            )?;
            let intent = candidate.protected_law_review(self.protection, self.laws)?;
            return Ok((report, intent.digest().to_owned()));
        };
        if selection.policy().digest() != self.policy.digest()
            || selection.protection().digest() != self.protection.digest()
        {
            return Err(vec![Diagnostic::io(
                "SPX-LW150",
                "candidate strict-law policy or editable intent scope differs from host selection",
            )]);
        }
        let current = LawSet::derive(
            candidate.revision(),
            selection.proof_profile(),
            candidate.revision().law_modules().to_vec(),
        )?;
        if current.to_json() != self.laws.to_json() {
            return Err(vec![Diagnostic::io(
                "SPX-LW150",
                "candidate law inventory differs from authenticated native law sources",
            )]);
        }
        let intent = selection.protection().review(
            selection.baseline(),
            candidate.revision(),
            &current,
            candidate.candidate_digest(),
        )?;
        intent.require(self.specification_approval)?;
        let report = strict::derive_with_native_proofs(
            candidate.revision(),
            &current,
            selection.policy(),
            self.proofs,
            self.native_proofs,
        )?;
        strict::require_with_native_proofs(
            &report,
            candidate.revision(),
            &current,
            selection.policy(),
            self.proofs,
            self.native_proofs,
        )?;
        let report: serde_json::Value =
            serde_json::from_str(&report).expect("derived strict report is JSON");
        let association = wire::render(
            serde_json::json!({
                "schema":STRICT_CANDIDATE_LAW_SCHEMA,
                "candidate_digest":candidate.candidate_digest(),
                "base_project_revision":candidate.base_revision().project_revision(),
                "host_baseline_project_revision":selection.baseline().project_revision(),
                "law_report":report,"publication_authority":false
            }),
            crate::assurance_manifest::law_set::MAX_BYTES,
        )?;
        Ok((association, intent.digest().to_owned()))
    }
}
impl ProjectCandidate {
    pub fn strict_law_assurance(
        &self,
        expected_candidate: &str,
        laws: &LawSet,
        policy: &StrictLawPolicy,
        proofs: &[VerifiedProjectProof],
    ) -> Result<String> {
        self.strict_law_assurance_with_native_proofs(expected_candidate, laws, policy, proofs, &[])
    }
    pub fn strict_law_assurance_with_native_proofs(
        &self,
        expected_candidate: &str,
        laws: &LawSet,
        policy: &StrictLawPolicy,
        proofs: &[VerifiedProjectProof],
        native_proofs: &[VerifiedLawProof],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if self.base_revision().project_revision() != policy.base_revision() {
            return Err(vec![Diagnostic::io(
                "SPX-LW104",
                "strict law policy belongs to a different candidate base revision",
            )]);
        }
        let report = strict::derive_with_native_proofs(
            self.revision(),
            laws,
            policy,
            proofs,
            native_proofs,
        )?;
        let report: serde_json::Value =
            serde_json::from_str(&report).expect("derived strict report is JSON");
        wire::render(
            serde_json::json!({"schema":STRICT_CANDIDATE_LAW_SCHEMA,"candidate_digest":self.candidate_digest(),"base_project_revision":self.base_revision().project_revision(),"law_report":report,"publication_authority":false}),
            crate::assurance_manifest::law_set::MAX_BYTES,
        )
    }
    pub fn require_strict_law_assurance(
        &self,
        document: &str,
        laws: &LawSet,
        policy: &StrictLawPolicy,
        proofs: &[VerifiedProjectProof],
    ) -> Result<()> {
        self.require_strict_law_assurance_with_native_proofs(document, laws, policy, proofs, &[])
    }
    pub fn require_strict_law_assurance_with_native_proofs(
        &self,
        document: &str,
        laws: &LawSet,
        policy: &StrictLawPolicy,
        proofs: &[VerifiedProjectProof],
        native_proofs: &[VerifiedLawProof],
    ) -> Result<()> {
        if document.len() > crate::assurance_manifest::law_set::MAX_BYTES {
            return Err(vec![Diagnostic::io(
                "SPX-LW102",
                "strict candidate law report exceeds its byte bound",
            )]);
        }
        let expected = self.strict_law_assurance_with_native_proofs(
            self.candidate_digest(),
            laws,
            policy,
            proofs,
            native_proofs,
        )?;
        if expected != document {
            return Err(vec![Diagnostic::io(
                "SPX-LW104",
                "candidate law report failed exact independent replay",
            )]);
        }
        let report: serde_json::Value =
            serde_json::from_str(&expected).expect("derived strict report is JSON");
        if report["law_report"]["accepted"] != true {
            return Err(vec![Diagnostic::io(
                "SPX-LW130",
                "candidate strict law requirements are not satisfied",
            )]);
        }
        Ok(())
    }
}

pub const STRICT_LAW_PUBLICATION_SCHEMA: &str = "semaprax.strict-law-publication.v1";

/// Authority-free exact publication proposal, including the strict policy and
/// checked law report. It cannot select its own baseline, proofs or host rights.
pub struct StrictLawPublication {
    document: String,
}
impl StrictLawPublication {
    pub fn to_json(&self) -> &str {
        &self.document
    }
}

fn publication_document(
    candidate: &ProjectCandidate,
    inputs: &StrictCandidateLawInputs<'_>,
    workspace_root: &Path,
    publication: &str,
) -> Result<String> {
    let (law_report, intent_digest) = inputs.require_host_selection(candidate, workspace_root)?;
    wire::render(
        serde_json::json!({
            "schema":STRICT_LAW_PUBLICATION_SCHEMA,"candidate_digest":candidate.candidate_digest(),
            "protected_intent_review_digest":intent_digest,"law_assurance":law_report,
            "publication":publication,"publication_authority":false
        }),
        publication::MAX_PROJECT_CANDIDATE_PUBLICATION_BYTES,
    )
}

/// Strict evidence is replayed after the ordinary host lock is acquired and
/// before any proposal is returned; the proposal still grants no authority.
pub fn prepare_strict_law_publication(
    candidate: &ProjectCandidate,
    inputs: &StrictCandidateLawInputs<'_>,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
) -> Result<StrictLawPublication> {
    let publication = publication::prepare_with_selected_law_gate(
        candidate,
        approved_candidate_digest,
        workspace_root,
        project_manifest,
        expected_workspace_revision,
        || {
            inputs
                .require_host_selection(candidate, workspace_root)
                .map(|_| ())
        },
    )?;
    Ok(StrictLawPublication {
        document: publication_document(candidate, inputs, workspace_root, publication.to_json())?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn apply_strict_law_publication(
    candidate: &ProjectCandidate,
    inputs: &StrictCandidateLawInputs<'_>,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
    submitted_publication: &[u8],
) -> Result<String> {
    apply_strict_law_publication_with_hook(
        candidate,
        inputs,
        approved_candidate_digest,
        workspace_root,
        project_manifest,
        expected_workspace_revision,
        submitted_publication,
        |_| Ok(()),
    )
}

/// Internal only: lets the owning unit regression move a retained source at a
/// named managed-Workspace final boundary. No caller outside this module can
/// supply a hook.
#[allow(clippy::too_many_arguments)]
fn apply_strict_law_publication_with_hook(
    candidate: &ProjectCandidate,
    inputs: &StrictCandidateLawInputs<'_>,
    approved_candidate_digest: &str,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
    submitted_publication: &[u8],
    hook: impl FnMut(crate::workspace::SemanticChangeApplyPoint) -> std::io::Result<()>,
) -> Result<String> {
    if submitted_publication.len() > publication::MAX_PROJECT_CANDIDATE_PUBLICATION_BYTES {
        return Err(vec![Diagnostic::io(
            "SPX-LW102",
            "strict publication proposal exceeds its byte bound",
        )]);
    }
    // Extract untrusted bytes only. The ordinary publication route replays its
    // own exact artifact, and the strict gate rederives the whole outer envelope
    // after authority acquisition and before staging.
    let submitted: serde_json::Value =
        serde_json::from_slice(submitted_publication).map_err(|_| {
            vec![Diagnostic::io(
                "SPX-LW104",
                "invalid strict publication proposal",
            )]
        })?;
    let publication = submitted["publication"].as_str().ok_or_else(|| {
        vec![Diagnostic::io(
            "SPX-LW104",
            "strict publication artifact missing",
        )]
    })?;
    publication::apply_with_selected_law_gate_with_hook(
        candidate,
        approved_candidate_digest,
        workspace_root,
        project_manifest,
        expected_workspace_revision,
        publication.as_bytes(),
        hook,
        || {
            inputs.require_host_selection(candidate, workspace_root)?;
            if publication_document(candidate, inputs, workspace_root, publication)?.as_bytes()
                != submitted_publication
            {
                return Err(vec![Diagnostic::io(
                    "SPX-LW104",
                    "strict publication policy, proof or intent association changed",
                )]);
            }
            Ok(SelectedStrictLaw::open(workspace_root)?
                .map(|_| {
                    StrictWorkspacePermit::after_strict_gate(
                        workspace_root,
                        inputs.policy,
                        inputs.protection,
                    )
                })
                .transpose()?)
        },
    )
}

#[cfg(test)]
mod law14_final_boundary_tests {
    use super::*;
    use crate::assurance_manifest::law_set::protected::{
        ProtectedLawReview, SpecificationChangeAuthority,
    };
    use crate::assurance_manifest::law_set::{
        strict::RequiredLawEvidence, EvidenceRequirement, LawDefinition, LawModule, LawSelector,
    };
    use crate::project::{with_authenticated_project, SemanticChange};
    use crate::workspace::SemanticChangeApplyPoint;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    fn architecture_module() -> LawModule {
        LawModule {
            module_id: "calculator.laws".into(),
            source_path: "src/core.spx".into(),
            assumptions: vec![],
            laws: vec![LawDefinition {
                law_id: "calculator.architecture".into(),
                selector: LawSelector::ForbidReaches {
                    claim_id: "no-divide".into(),
                    from: "calculator.is-negative".into(),
                    to: "calculator.divide".into(),
                },
                assumption_ids: vec![],
                requires_laws: vec![],
                evidence: EvidenceRequirement::CompilerProved,
            }],
        }
    }

    struct Approve;
    impl SpecificationChangeAuthority for Approve {
        fn approve_specification_change(&mut self, _: &ProtectedLawReview) -> bool {
            true
        }
    }

    #[test]
    fn law14_strict_publication_final_boundary_source_race_refuses_before_active() {
        let root = std::env::temp_dir().join(format!(
            "spx-law14-final-boundary-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        let root = root.canonicalize().unwrap();
        let manifest = root.join("semaprax.toml");
        let paths = root.join("paths.json");
        std::fs::write(&paths, "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"src/app.spx\"},{\"path\":\"src/core.spx\"},{\"path\":\"src/tests.spx\"}]}\n").unwrap();
        let workspace = crate::semantic_workspace::initialize(&root, &paths).unwrap();
        let base = with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
            .unwrap();
        let baseline = LawSet::derive(&base, "law14-race-v1", vec![architecture_module()]).unwrap();
        let policy = StrictLawPolicy::new(
            baseline.clone(),
            BTreeMap::from([(
                "calculator.architecture".into(),
                RequiredLawEvidence::CompilerStatic,
            )]),
        )
        .unwrap();
        let protection = ProtectedLawBaseline::new(&base, baseline, vec![]).unwrap();
        let candidate = ProjectCandidate::open(base.clone(), base.project_revision()).unwrap();
        let change = SemanticChange::new(
            base.project_revision(),
            &serde_json::json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":"unused","type":"i64","argument":{"kind":"i64","value":0}}]}),
        )
        .unwrap();
        let candidate = candidate
            .apply(candidate.candidate_digest(), &change)
            .unwrap();
        let laws = LawSet::derive(
            candidate.revision(),
            "law14-race-v1",
            vec![architecture_module()],
        )
        .unwrap();
        let intent = candidate.protected_law_review(&protection, &laws).unwrap();
        let approval = SpecificationChangeApproval::request(&intent, &mut Approve).unwrap();
        let inputs = StrictCandidateLawInputs {
            protection: &protection,
            policy: &policy,
            laws: &laws,
            proofs: &[],
            native_proofs: &[],
            specification_approval: Some(&approval),
        };
        let proposal = prepare_strict_law_publication(
            &candidate,
            &inputs,
            candidate.candidate_digest(),
            &root,
            &manifest,
            &workspace,
        )
        .unwrap();
        let active = root.join(".semaprax-workspace/ACTIVE");
        let before = std::fs::read(&active).unwrap();
        let source = root.join("src/core.spx");
        let original = std::fs::read_to_string(&source).unwrap();
        let points = std::cell::RefCell::new(Vec::new());
        let error = apply_strict_law_publication_with_hook(
            &candidate,
            &inputs,
            candidate.candidate_digest(),
            &root,
            &manifest,
            &workspace,
            proposal.to_json().as_bytes(),
            |point| {
                points.borrow_mut().push(point);
                if point == SemanticChangeApplyPoint::BeforeActiveReplace {
                    std::fs::write(&source, format!("{original}\n"))?;
                }
                Ok(())
            },
        )
        .expect_err("final strict-law source drift must refuse publication");
        assert!(points
            .borrow()
            .contains(&SemanticChangeApplyPoint::BeforeActiveReplace));
        assert!(
            error.iter().any(|diagnostic| diagnostic.code == "SPX-J102"),
            "final-boundary source drift must retain SPX-J102: {error:?}"
        );
        assert_eq!(std::fs::read(&active).unwrap(), before);
        assert_ne!(std::fs::read_to_string(&source).unwrap(), original);
        std::fs::remove_dir_all(root).unwrap();
    }
}
