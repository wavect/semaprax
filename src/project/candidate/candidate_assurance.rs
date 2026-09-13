//! Candidate-bound assurance summary and an independent acceptance boundary.
//!
//! See [`docs/PROJECT-CANDIDATE-ASSURANCE-ACCEPTANCE-V1.md`](../../../docs/PROJECT-CANDIDATE-ASSURANCE-ACCEPTANCE-V1.md)
//! for the full specification. This module composes existing evidence; it
//! never re-implements obligation derivation or the assurance lattice
//! ([`crate::assurance_manifest`] owns both) and it never executes a
//! target, discovers or runs project tests, or writes source.
//!
//! [`ProjectCandidate::candidate_assurance_summary`] independently verifies
//! caller-supplied `semaprax.assurance-manifest.v1` envelopes (one per
//! candidate source path) and rebinds each to this exact candidate's own
//! source bytes before joining their obligations into one report.
//! [`ProjectCandidate::grant_candidate_acceptance`] evaluates that summary
//! against a caller-chosen minimum assurance class and refuses outright when
//! the reviewer and proposer identities are the same string: a candidate
//! cannot accept itself.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::assurance_manifest::{self, AssuranceClass, ObligationKind};
use crate::diagnostic::Diagnostic;

use super::{wire, ProjectCandidate};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_ASSURANCE_SUMMARY_SCHEMA: &str =
    "semaprax.project-candidate-assurance-summary.v1";
pub const MAX_PROJECT_CANDIDATE_ASSURANCE_SUMMARY_BYTES: usize = 4 * 1024 * 1024;
/// A candidate cannot carry more source files than its own admission bound
/// allows, so an input set never needs to exceed this either.
pub const MAX_CANDIDATE_ASSURANCE_INPUTS: usize = 64;

pub const PROJECT_CANDIDATE_ACCEPTANCE_SCHEMA: &str = "semaprax.project-candidate-acceptance.v1";
pub const MAX_PROJECT_CANDIDATE_ACCEPTANCE_BYTES: usize = 65_536;
/// Caller-supplied identity strings (`proposer`/`reviewer`) are bounded so a
/// hostile string cannot inflate the acceptance record without limit.
pub const MAX_CANDIDATE_ACCEPTANCE_IDENTITY_BYTES: usize = 256;

/// The one reserved kind with no audited automatic producer. This list is
/// about producer coverage, not whether an individual source has an instance
/// of any other kind.
const KINDS_NOT_YET_DERIVED: [ObligationKind; 1] = [ObligationKind::ArchitectureLaw];

/// One caller-supplied `semaprax.assurance-manifest.v1` envelope, already
/// produced by `assurance_manifest::generate` over the exact bytes this
/// candidate carries at `path`. This module never calls `generate` itself:
/// composing an existing, independently verifiable evidence artifact is the
/// whole point, not regenerating a second copy of it.
#[derive(Clone, Copy, Debug)]
pub struct CandidateAssuranceInput<'a> {
    pub path: &'a str,
    pub envelope: &'a str,
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G930", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G931", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G932", message)]
}
fn refused(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G933", message)]
}

impl ProjectCandidate {
    /// Independently verify every supplied assurance-manifest envelope,
    /// rebind each to this exact candidate's current source bytes (never
    /// the envelope's own claimed path or a previously held revision), and
    /// join their obligations into one candidate-bound, source-bound report.
    ///
    /// Fails closed (`SPX-G932`) when a supplied envelope's `source.revision`
    /// does not match the revision independently recomputed from this exact
    /// candidate's source at `path` — a stale envelope from a previous
    /// candidate, a different base revision, or a mismatched file can never
    /// satisfy a current claim. A candidate source path with no supplied
    /// input is listed under `sources_not_observed`, never silently omitted.
    pub fn candidate_assurance_summary(
        &self,
        expected_candidate: &str,
        inputs: &[CandidateAssuranceInput<'_>],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if inputs.len() > MAX_CANDIDATE_ASSURANCE_INPUTS {
            return Err(capacity(
                "candidate assurance input count exceeds its bound",
            ));
        }

        let mut bound_paths: BTreeSet<String> = BTreeSet::new();
        let mut obligations: Vec<Value> = Vec::new();
        let mut by_class: BTreeMap<&'static str, u64> = AssuranceClass::ALL
            .into_iter()
            .map(|c| (c.token(), 0))
            .collect();
        let mut unsupported_formal_claims: BTreeSet<String> = BTreeSet::new();

        for input in inputs {
            if !bound_paths.insert(input.path.to_owned()) {
                return Err(invalid(
                    "candidate assurance inputs must name each source path at most once",
                ));
            }
            let source = self
                .revision
                .sources()
                .iter()
                .find(|source| source.path() == input.path)
                .ok_or_else(|| {
                    invalid("candidate assurance input path is not a source this candidate carries")
                })?;

            assurance_manifest::verify_envelope(input.envelope).map_err(|error| vec![error])?;
            let envelope: Value = serde_json::from_str(input.envelope).map_err(|_| {
                invalid("candidate assurance envelope is not valid JSON despite passing replay")
            })?;
            let payload = &envelope["payload"];
            let declared_revision = payload["source"]["revision"].as_str().ok_or_else(|| {
                invalid("candidate assurance envelope payload.source.revision must be a string")
            })?;

            // The independent rebind: recompute the exact same revision the
            // manifest producer itself would compute, from this candidate's
            // own held source bytes at this path -- never from anything the
            // envelope claims about itself. A mismatch means the envelope
            // was generated against different bytes: a previous candidate, a
            // sibling file, or drifted source. `payload.source.path` is
            // never compared: it is caller-supplied display text, while this
            // recomputation is the actual binding.
            let program =
                crate::parse(source.source(), source.path()).map_err(|error| vec![error])?;
            let recomputed_revision = crate::graph::revision(&program);
            if recomputed_revision != declared_revision {
                return Err(stale(
                    "candidate assurance envelope source revision does not match this exact \
                     candidate's current source; regenerate the envelope against current bytes",
                ));
            }

            let payload_obligations = payload["obligations"].as_array().ok_or_else(|| {
                invalid("candidate assurance envelope payload.obligations must be an array")
            })?;
            for obligation in payload_obligations {
                let id = obligation["id"]
                    .as_str()
                    .ok_or_else(|| invalid("candidate assurance obligation id must be a string"))?;
                let classification_token =
                    obligation["classification"].as_str().ok_or_else(|| {
                        invalid("candidate assurance obligation classification must be a string")
                    })?;
                let class = AssuranceClass::from_token(classification_token).ok_or_else(|| {
                    invalid(
                        "candidate assurance obligation classification is outside the closed vocabulary",
                    )
                })?;
                *by_class
                    .get_mut(class.token())
                    .expect("AssuranceClass::ALL is exhaustive") += 1;

                let methods = obligation["methods"].as_array().ok_or_else(|| {
                    invalid("candidate assurance obligation methods must be an array")
                })?;
                let is_formal_proof_class = matches!(
                    class,
                    AssuranceClass::SmtProved
                        | AssuranceClass::ModelChecked
                        | AssuranceClass::TheoremProved
                );
                let has_proof_ref = methods.iter().any(|method| method["proof_ref"].is_string());
                if is_formal_proof_class && !has_proof_ref {
                    unsupported_formal_claims.insert(id.to_owned());
                }

                obligations.push(json!({
                    "id": id,
                    "declaration_id": obligation["declaration_id"],
                    "kind": obligation["kind"],
                    "classification": classification_token,
                    "assumption_ids": obligation["assumption_ids"],
                    "source_path": input.path,
                }));
            }
        }

        obligations.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
        let mut seen_ids = BTreeSet::new();
        for obligation in &obligations {
            let id = obligation["id"].as_str().expect("id was validated above");
            if !seen_ids.insert(id.to_owned()) {
                return Err(invalid(
                    "candidate assurance obligation id collided across supplied inputs",
                ));
            }
        }

        let sources_not_observed: Vec<&str> = self
            .revision
            .sources()
            .iter()
            .map(|source| source.path())
            .filter(|path| !bound_paths.contains(*path))
            .collect();

        let by_class_value = Value::Object(
            by_class
                .into_iter()
                .map(|(class, count)| (class.to_owned(), json!(count)))
                .collect(),
        );

        let value = json!({
            "schema": PROJECT_CANDIDATE_ASSURANCE_SUMMARY_SCHEMA,
            "candidate_revision": self.candidate_digest(),
            "base_project_revision": self.base.project_revision(),
            "project_revision": self.revision.project_revision(),
            "sources_total": self.revision.sources().len(),
            "sources_bound": bound_paths.len(),
            "sources_not_observed": sources_not_observed,
            "obligations_total": obligations.len(),
            "by_class": by_class_value,
            "obligations": obligations,
            "unsupported_formal_claims": unsupported_formal_claims,
            "kinds_not_yet_derived": KINDS_NOT_YET_DERIVED.iter().map(|kind| kind.token()).collect::<Vec<_>>(),
            "candidate_retained": false,
            "publication_authority": false,
            "acceptance_authority": false,
            "nonclaims": [
                "no_target_execution",
                "no_project_test_discovery_or_execution",
                "not_multi_file_cross_reference_checked",
                "unsupported_or_unobserved_kinds_may_exist",
                "read_only_no_source_changes",
                "not_publication_or_execution_authority",
            ],
        });
        wire::render(value, MAX_PROJECT_CANDIDATE_ASSURANCE_SUMMARY_BYTES)
    }

    /// Evaluate this exact candidate's assurance summary against
    /// `minimum_class` and record the decision as authority-free evidence.
    ///
    /// Regenerates the summary itself from `inputs` via
    /// [`Self::candidate_assurance_summary`] rather than accepting a
    /// caller-handed summary document, so nothing the candidate's own
    /// source (including any test file it can edit) writes ever reaches
    /// this decision except through that one independently bound and
    /// verified path.
    ///
    /// Refuses outright (`SPX-G933`) when `reviewer == proposer`: a
    /// candidate cannot accept itself. This is a structural, string-identity
    /// check, not an authentication system; a real deployment must still
    /// bind `proposer`/`reviewer` to distinct authenticated principals
    /// outside this library, matching every other evidence artifact in this
    /// codebase (`OwnedWorkflowApproval`, `analysis_*evidence`): the
    /// returned record carries `acceptance_authority: false` and grants no
    /// execution or publication authority of its own.
    pub fn grant_candidate_acceptance(
        &self,
        expected_candidate: &str,
        inputs: &[CandidateAssuranceInput<'_>],
        minimum_class: AssuranceClass,
        proposer: &str,
        reviewer: &str,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        validate_identity(proposer)?;
        validate_identity(reviewer)?;
        if proposer == reviewer {
            return Err(refused(
                "candidate acceptance reviewer must be distinct from the proposer; \
                 a candidate cannot accept itself",
            ));
        }

        let summary_text = self.candidate_assurance_summary(expected_candidate, inputs)?;
        let summary: Value = serde_json::from_str(&summary_text)
            .expect("candidate_assurance_summary always renders valid JSON");

        let mut unmet_obligations = Vec::new();
        for obligation in summary["obligations"]
            .as_array()
            .expect("candidate_assurance_summary always renders an obligations array")
        {
            let class = AssuranceClass::from_token(
                obligation["classification"]
                    .as_str()
                    .expect("candidate_assurance_summary always renders a closed classification"),
            )
            .expect("candidate_assurance_summary only ever renders closed-vocabulary classes");
            let meets_minimum =
                class == minimum_class || assurance_manifest::dominates(class, minimum_class);
            if !meets_minimum {
                unmet_obligations.push(obligation["id"].clone());
            }
        }

        let sources_not_observed = summary["sources_not_observed"].clone();
        let unsupported_formal_claims = summary["unsupported_formal_claims"].clone();
        let granted = unmet_obligations.is_empty()
            && sources_not_observed
                .as_array()
                .is_some_and(std::vec::Vec::is_empty)
            && unsupported_formal_claims
                .as_array()
                .is_some_and(std::vec::Vec::is_empty);

        let value = json!({
            "schema": PROJECT_CANDIDATE_ACCEPTANCE_SCHEMA,
            "candidate_revision": self.candidate_digest(),
            "base_project_revision": self.base.project_revision(),
            "proposer": proposer,
            "reviewer": reviewer,
            "minimum_class": minimum_class.token(),
            "granted": granted,
            "unmet_obligations": unmet_obligations,
            "sources_not_observed": sources_not_observed,
            "unsupported_formal_claims": unsupported_formal_claims,
            "assurance_summary_obligations_total": summary["obligations_total"],
            "acceptance_authority": false,
            "publication_authority": false,
            "execution_authority": false,
            "nonclaims": [
                "not_publication_or_execution_authority",
                "not_a_cryptographic_identity_attestation",
                "does_not_rerun_or_trust_candidate_declared_tests",
                "self_granted_acceptance_structurally_refused",
            ],
        });
        wire::render(value, MAX_PROJECT_CANDIDATE_ACCEPTANCE_BYTES)
    }
}

fn validate_identity(identity: &str) -> Result<()> {
    if identity.is_empty() || identity.len() > MAX_CANDIDATE_ACCEPTANCE_IDENTITY_BYTES {
        return Err(invalid(
            "candidate acceptance proposer/reviewer identity must be non-empty and bounded",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assurance_manifest::{
        AssuranceManifestOptions, ExternalRecords, MethodRecord, Obligation,
    };
    use crate::project::{with_authenticated_project, ProjectRevision};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    // `assurance_manifest::generate` parses and verifies exactly the file it
    // is pointed at, standalone (no project-linked imports, and requiring a
    // `main`), matching `capability_manifest`/`region_report`. Within a
    // project, only the entry module and a listed `tests` module may declare
    // `main` (SPX-G172 rejects it on any other source), so the one candidate
    // source this suite binds assurance envelopes to is always the entry
    // module itself, carrying both the contracted `divide` function and its
    // own `main`.
    const APP_WITH_CONTRACTS: &str = "module assurance.app;\n\
@id(\"assurance.divide\") fn divide(left: i64, right: i64) -> i64\n\
    requires right != 0\n\
{\n    left / right\n}\n\
@id(\"assurance.main\") fn main() -> i64 { divide(4, 2) }\n";

    impl Fixture {
        fn new(app: &str, tests_body: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "spx-candidate-assurance-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(root.join("src")).unwrap();
            let root = root.canonicalize().unwrap();
            std::fs::write(
                root.join("semaprax.toml"),
                r#"schema = "semaprax.project.v8"
name = "candidate-assurance"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "assurance.app"
sources = ["src/app.spx", "src/tests.spx"]
web_exports = []
tests = ["assurance.tests"]
"#,
            )
            .unwrap();
            for (path, text) in [
                ("src/app.spx", app.to_owned()),
                (
                    "src/tests.spx",
                    format!("module assurance.tests;\n{tests_body}\n"),
                ),
            ] {
                let parsed = crate::parse(&text, path).unwrap();
                std::fs::write(root.join(path), crate::format::canonical(&parsed)).unwrap();
            }
            Self(root)
        }

        fn app_path(&self) -> PathBuf {
            self.0.join("src/app.spx")
        }

        fn tests_path(&self) -> PathBuf {
            self.0.join("src/tests.spx")
        }

        fn revision(&self) -> Arc<ProjectRevision> {
            with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
                Ok(snapshot.retain_revision())
            })
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn open(revision: &Arc<ProjectRevision>) -> ProjectCandidate {
        ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap()
    }

    fn manifest_for(path: &std::path::Path) -> String {
        assurance_manifest::generate(path, &AssuranceManifestOptions::default()).unwrap()
    }

    const PLAIN_TESTS_BODY: &str = "@id(\"assurance.check\") fn main()->i64 {0}";

    #[test]
    fn joins_and_binds_a_genuine_envelope_to_the_exact_candidate() {
        let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
        let revision = fixture.revision();
        let candidate = open(&revision);
        let envelope = manifest_for(&fixture.app_path());
        let inputs = [CandidateAssuranceInput {
            path: "src/app.spx",
            envelope: &envelope,
        }];

        let summary = candidate
            .candidate_assurance_summary(candidate.candidate_digest(), &inputs)
            .unwrap();
        assert!(summary.len() <= MAX_PROJECT_CANDIDATE_ASSURANCE_SUMMARY_BYTES);
        let value: Value = serde_json::from_str(&summary).unwrap();
        assert_eq!(value["schema"], PROJECT_CANDIDATE_ASSURANCE_SUMMARY_SCHEMA);
        assert_eq!(
            value["candidate_revision"],
            json!(candidate.candidate_digest())
        );
        // One precondition, two parameter obligations, and result ownership
        // for both `divide` and `main`.
        assert_eq!(value["obligations_total"], json!(5));
        assert_eq!(value["publication_authority"], json!(false));
        assert_eq!(value["acceptance_authority"], json!(false));
        let not_observed = value["sources_not_observed"].as_array().unwrap();
        let not_observed: Vec<&str> = not_observed.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(not_observed, vec!["src/tests.spx"]);
        let kinds_not_yet_derived = value["kinds_not_yet_derived"].as_array().unwrap();
        assert_eq!(
            kinds_not_yet_derived.as_slice(),
            &[json!("architecture_law")]
        );
    }

    #[test]
    fn stale_envelope_after_tampering_with_its_declared_revision_is_rejected() {
        let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
        let revision = fixture.revision();
        let candidate = open(&revision);
        let genuine_envelope = manifest_for(&fixture.app_path());

        // Hand-forge an envelope claiming a revision this candidate never
        // held, re-signing its own digest/bytes so structural replay
        // (`verify_envelope`) still accepts it; only the independent rebind
        // this module performs should reject it.
        let mut tampered: Value = serde_json::from_str(&genuine_envelope).unwrap();
        tampered["payload"]["source"]["revision"] =
            json!("sha256:0000000000000000000000000000000000000000000000000000000000000000");
        let payload = tampered["payload"].to_string();
        let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
        sha2::Digest::update(&mut hasher, b"semaprax.assurance-manifest.payload.v1\0");
        sha2::Digest::update(&mut hasher, (payload.len() as u64).to_le_bytes());
        sha2::Digest::update(&mut hasher, payload.as_bytes());
        let digest = format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(sha2::Digest::finalize(hasher))
        );
        let rebuilt = format!(
            "{{\"schema\":\"semaprax.assurance-manifest.v1\",\"digest\":\"{digest}\",\"bytes\":{},\"payload\":{payload}}}",
            payload.len(),
        );
        assurance_manifest::verify_envelope(&rebuilt).expect("hand-rebuilt envelope must replay");

        let inputs = [CandidateAssuranceInput {
            path: "src/app.spx",
            envelope: &rebuilt,
        }];
        let error = candidate
            .candidate_assurance_summary(candidate.candidate_digest(), &inputs)
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G932");
    }

    #[test]
    fn envelope_for_a_path_outside_the_candidate_is_rejected() {
        let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
        let revision = fixture.revision();
        let candidate = open(&revision);
        let envelope = manifest_for(&fixture.app_path());
        let inputs = [CandidateAssuranceInput {
            path: "src/does-not-exist.spx",
            envelope: &envelope,
        }];
        let error = candidate
            .candidate_assurance_summary(candidate.candidate_digest(), &inputs)
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G930");
    }

    #[test]
    fn duplicate_input_paths_are_rejected() {
        let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
        let revision = fixture.revision();
        let candidate = open(&revision);
        let envelope = manifest_for(&fixture.app_path());
        let inputs = [
            CandidateAssuranceInput {
                path: "src/app.spx",
                envelope: &envelope,
            },
            CandidateAssuranceInput {
                path: "src/app.spx",
                envelope: &envelope,
            },
        ];
        let error = candidate
            .candidate_assurance_summary(candidate.candidate_digest(), &inputs)
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G930");
    }

    #[test]
    fn unsupported_formal_proof_claim_without_a_proof_ref_is_flagged() {
        let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
        let revision = fixture.revision();
        let candidate = open(&revision);

        let method = MethodRecord::new(AssuranceClass::TheoremProved, "adversarial-claim", "0");
        assert!(method.proof_ref.is_none());
        let obligation = Obligation::new(
            ObligationKind::ArchitectureLaw,
            "assurance.divide",
            "law:no-real-proof",
        )
        .with_method(method);
        let mut records = ExternalRecords::default();
        records.obligations.push(obligation);
        let options = AssuranceManifestOptions::default().with_external_records(records);
        let envelope = assurance_manifest::generate(&fixture.app_path(), &options).unwrap();

        let inputs = [CandidateAssuranceInput {
            path: "src/app.spx",
            envelope: &envelope,
        }];
        let summary = candidate
            .candidate_assurance_summary(candidate.candidate_digest(), &inputs)
            .unwrap();
        let value: Value = serde_json::from_str(&summary).unwrap();
        let unsupported = value["unsupported_formal_claims"].as_array().unwrap();
        assert_eq!(unsupported.len(), 1);

        // An unsupported formal claim, and the unobserved tests source, both
        // keep acceptance from being granted even at the lowest bar.
        let granted = candidate
            .grant_candidate_acceptance(
                candidate.candidate_digest(),
                &inputs,
                AssuranceClass::Open,
                "agent-proposer",
                "human-reviewer",
            )
            .unwrap();
        let granted: Value = serde_json::from_str(&granted).unwrap();
        assert_eq!(granted["granted"], json!(false));
    }

    #[test]
    fn a_candidate_cannot_accept_itself() {
        let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
        let revision = fixture.revision();
        let candidate = open(&revision);
        let app_envelope = manifest_for(&fixture.app_path());
        let tests_envelope = manifest_for(&fixture.tests_path());
        let inputs = [
            CandidateAssuranceInput {
                path: "src/app.spx",
                envelope: &app_envelope,
            },
            CandidateAssuranceInput {
                path: "src/tests.spx",
                envelope: &tests_envelope,
            },
        ];

        // Same identity for proposer and reviewer: structurally refused
        // regardless of how strong the underlying assurance summary is.
        let error = candidate
            .grant_candidate_acceptance(
                candidate.candidate_digest(),
                &inputs,
                AssuranceClass::Open,
                "agent-x",
                "agent-x",
            )
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G933");

        // A genuinely distinct reviewer can still evaluate the exact same
        // inputs and grant acceptance.
        let granted = candidate
            .grant_candidate_acceptance(
                candidate.candidate_digest(),
                &inputs,
                AssuranceClass::Open,
                "agent-x",
                "human-reviewer",
            )
            .unwrap();
        let granted: Value = serde_json::from_str(&granted).unwrap();
        assert_eq!(granted["granted"], json!(true));
    }

    #[test]
    fn editing_the_visible_test_file_cannot_alter_the_production_obligations_or_acceptance() {
        let honest = Fixture::new(
            APP_WITH_CONTRACTS,
            "@id(\"assurance.check\") fn main()->i64 { 1 }",
        );
        let lying = Fixture::new(
            APP_WITH_CONTRACTS,
            "@id(\"assurance.check\") fn main()->i64 { 0 }",
        );

        let honest_candidate = open(&honest.revision());
        let lying_candidate = open(&lying.revision());
        let honest_envelope = manifest_for(&honest.app_path());
        let lying_envelope = manifest_for(&lying.app_path());
        // The app.spx bytes are byte-identical in both fixtures, so their
        // assurance-manifest obligations over that one file are identical
        // (the envelopes' `source.path` differ only because each fixture
        // uses its own temporary directory).
        let obligations_of = |envelope: &str| -> Value {
            serde_json::from_str::<Value>(envelope).unwrap()["payload"]["obligations"].clone()
        };
        assert_eq!(
            obligations_of(&honest_envelope),
            obligations_of(&lying_envelope)
        );

        let honest_inputs = [CandidateAssuranceInput {
            path: "src/app.spx",
            envelope: &honest_envelope,
        }];
        let lying_inputs = [CandidateAssuranceInput {
            path: "src/app.spx",
            envelope: &lying_envelope,
        }];

        let honest_summary: Value = serde_json::from_str(
            &honest_candidate
                .candidate_assurance_summary(honest_candidate.candidate_digest(), &honest_inputs)
                .unwrap(),
        )
        .unwrap();
        let lying_summary: Value = serde_json::from_str(
            &lying_candidate
                .candidate_assurance_summary(lying_candidate.candidate_digest(), &lying_inputs)
                .unwrap(),
        )
        .unwrap();

        // Candidate revisions differ (the tests.spx text differs), but the
        // production obligations joined from src/app.spx are identical: a
        // test file the candidate is free to edit has no path into them.
        assert_ne!(
            honest_summary["candidate_revision"],
            lying_summary["candidate_revision"]
        );
        assert_eq!(honest_summary["obligations"], lying_summary["obligations"]);
        assert_eq!(honest_summary["by_class"], lying_summary["by_class"]);
    }

    #[test]
    fn identity_bounds_are_enforced() {
        let fixture = Fixture::new(APP_WITH_CONTRACTS, PLAIN_TESTS_BODY);
        let revision = fixture.revision();
        let candidate = open(&revision);
        let envelope = manifest_for(&fixture.app_path());
        let inputs = [CandidateAssuranceInput {
            path: "src/app.spx",
            envelope: &envelope,
        }];
        let error = candidate
            .grant_candidate_acceptance(
                candidate.candidate_digest(),
                &inputs,
                AssuranceClass::Open,
                "",
                "reviewer",
            )
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G930");
    }
}
