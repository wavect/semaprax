//! Assurance Policy v1 (`semaprax.assurance-policy.v1`): a deterministic,
//! read-only classifier that decides, for one already-generated Assurance
//! Manifest v1 envelope (#183, [`crate::assurance_manifest::generate`]),
//! whether each obligation's current `classification` satisfies one of four
//! named policy profiles, and reports a fixed, documented remediation
//! suggestion when it does not.
//!
//! See [`docs/ASSURANCE-POLICY-V1.md`](../docs/ASSURANCE-POLICY-V1.md) for
//! the full specification: the four profiles and their precedence, the
//! per-obligation verdict shape, the runtime-guard retention rule, the
//! delta/CI policy, and the exact nonclaims.
//!
//! This module never re-derives obligations, never touches source or a
//! target artifact, and never removes anything itself. It reads an
//! envelope [`crate::assurance_manifest::verify_envelope`] already accepted
//! and answers one question per obligation ("does this classification meet
//! this profile?") plus one fixed retention rule for a `runtime_guarded`
//! method record. Every verdict is proof data: it grants no execution,
//! publication, signing, or automatic-repair authority, matching
//! AGENTS.md's "evidence capsules carry no authority."
//!
//! Diagnostics use the previously unused `SPX-Z6xx` family:
//! - `SPX-Z601`: invalid policy input (malformed profile token, malformed
//!   envelope/delta shape this module's own defense-in-depth parsing
//!   rejects independently of [`crate::assurance_manifest::verify_envelope`]).
//! - `SPX-Z602`: output byte-budget exhaustion; fail closed, never truncated.

use serde_json::{json, Value};

use crate::assurance_manifest::{self, AssuranceClass, ObligationKind};
use crate::diagnostic::Diagnostic;

pub const SCHEMA: &str = "semaprax.assurance-policy.v1";
pub const DELTA_POLICY_SCHEMA: &str = "semaprax.assurance-policy-delta.v1";
pub const MAX_POLICY_REPORT_BYTES: usize = 16 * 1024 * 1024;

const NONCLAIMS: &[&str] = &[
    "proof_data_not_authority",
    "evaluated_only_over_the_supplied_envelope_no_live_recheck_of_source_or_target",
    "grants_no_execution_publication_signing_or_repair_authority",
    "suggested_next_action_is_a_deterministic_label_not_a_verified_repair",
    "never_converts_an_open_obligation_to_assumed_on_its_own",
    "profile_precedence_is_caller_supplied_not_looked_up_from_project_or_deployment_configuration",
];

type Result<T> = std::result::Result<T, Diagnostic>;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-Z601", message.into())
}

fn capacity(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-Z602", message.into())
}

/// Closed, named assurance policy profile vocabulary. Each profile fixes
/// which [`AssuranceClass`] values it accepts as satisfying an obligation;
/// none of them ever narrows or widens the lattice itself.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PolicyProfile {
    /// Only a static discharge satisfies an obligation: `compiler_proved`,
    /// `smt_proved`, `model_checked`, or `theorem_proved`.
    RequireStatic,
    /// `RequireStatic`'s classes, plus `runtime_guarded`.
    AllowRuntimeGuard,
    /// `AllowRuntimeGuard`'s classes, plus `test_evidenced`.
    AllowTestEvidence,
    /// Every classification satisfies; this profile never fails a CI gate.
    /// It reports, and authorizes nothing.
    ReportOnly,
}

impl PolicyProfile {
    pub const ALL: [Self; 4] = [
        Self::RequireStatic,
        Self::AllowRuntimeGuard,
        Self::AllowTestEvidence,
        Self::ReportOnly,
    ];

    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::RequireStatic => "require-static",
            Self::AllowRuntimeGuard => "allow-runtime-guard",
            Self::AllowTestEvidence => "allow-test-evidence",
            Self::ReportOnly => "report-only",
        }
    }

    /// Parse one exact profile token. Unknown or case-folded names are
    /// rejected, never silently mapped onto the nearest known profile.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.token() == token)
    }

    /// Precedence rank: a strictly lower rank is a strictly stricter
    /// profile. Used only by [`Self::resolve_precedence`] to pick the
    /// strictest of several explicitly supplied profile sources; it is
    /// never used to compare a profile against an [`AssuranceClass`].
    #[must_use]
    pub const fn precedence_rank(self) -> u8 {
        match self {
            Self::RequireStatic => 0,
            Self::AllowRuntimeGuard => 1,
            Self::AllowTestEvidence => 2,
            Self::ReportOnly => 3,
        }
    }

    /// Resolve one effective profile across up to four explicit sources in
    /// the documented precedence order this issue names: source/module,
    /// Project, deployment/target, invocation/CI. The strictest
    /// (lowest [`Self::precedence_rank`]) profile among the ones actually
    /// supplied wins; an absent (`None`) source contributes nothing, so a
    /// caller that supplies zero sources gets `None` back rather than a
    /// silently assumed default. This function does not look up any of the
    /// four sources itself: it is a pure combinator over values the caller
    /// already read, by design (see the module nonclaim on this point), so
    /// a source that does not yet exist in this repository never silently
    /// downgrades the result.
    #[must_use]
    pub fn resolve_precedence(sources: &[Option<Self>]) -> Option<Self> {
        sources
            .iter()
            .copied()
            .flatten()
            .min_by_key(|profile| profile.precedence_rank())
    }

    #[must_use]
    fn accepts(self, class: AssuranceClass) -> bool {
        use AssuranceClass::{
            CompilerProved, ModelChecked, RuntimeGuarded, SmtProved, TestEvidenced, TheoremProved,
        };
        match self {
            Self::ReportOnly => true,
            Self::RequireStatic => matches!(
                class,
                CompilerProved | SmtProved | ModelChecked | TheoremProved
            ),
            Self::AllowRuntimeGuard => matches!(
                class,
                CompilerProved | SmtProved | ModelChecked | TheoremProved | RuntimeGuarded
            ),
            Self::AllowTestEvidence => matches!(
                class,
                CompilerProved
                    | SmtProved
                    | ModelChecked
                    | TheoremProved
                    | RuntimeGuarded
                    | TestEvidenced
            ),
        }
    }
}

/// One fixed, deterministic remediation label for an obligation that does
/// not satisfy `profile`. This is a suggestion until applied through a
/// typed change (rename/replace/add-contract/add-declaration); it is never
/// itself an edit, and it never proves the suggested change would verify.
#[must_use]
fn suggested_next_action(kind: ObligationKind, classification: AssuranceClass) -> &'static str {
    match (kind, classification) {
        (ObligationKind::Precondition, AssuranceClass::RuntimeGuarded) => "strengthen_precondition",
        (ObligationKind::Postcondition, AssuranceClass::RuntimeGuarded) => "weaken_postcondition",
        (ObligationKind::OwnershipParameter, _) => "add_invariant",
        (_, AssuranceClass::Open | AssuranceClass::AttemptInconclusive) => "add_runtime_guard",
        (_, AssuranceClass::TestEvidenced) => "split_function",
        _ => "mark_explicit_assumption",
    }
}

struct MethodView {
    class: AssuranceClass,
    target: Option<String>,
    artifact_digest: Option<String>,
}

struct ObligationView {
    id: String,
    declaration_id: String,
    kind: ObligationKind,
    classification: AssuranceClass,
    methods: Vec<MethodView>,
}

fn text_or_null(value: &Value, field: &str) -> Result<Option<String>> {
    if value.is_null() {
        return Ok(None);
    }
    value.as_str().map(str::to_owned).map(Some).ok_or_else(|| {
        invalid(format!(
            "assurance policy method `{field}` must be a string or null"
        ))
    })
}

fn parse_method(value: &Value) -> Result<MethodView> {
    let class_token = value["class"]
        .as_str()
        .ok_or_else(|| invalid("assurance policy method `class` must be a string"))?;
    let class = AssuranceClass::from_token(class_token).ok_or_else(|| {
        invalid(format!(
            "assurance policy method class `{class_token}` is outside the closed vocabulary"
        ))
    })?;
    Ok(MethodView {
        class,
        target: text_or_null(&value["target"], "target")?,
        artifact_digest: text_or_null(&value["artifact_digest"], "artifact_digest")?,
    })
}

fn parse_obligation(value: &Value) -> Result<ObligationView> {
    let id = value["id"]
        .as_str()
        .ok_or_else(|| invalid("assurance policy obligation `id` must be a string"))?
        .to_owned();
    let declaration_id = value["declaration_id"]
        .as_str()
        .ok_or_else(|| invalid("assurance policy obligation `declaration_id` must be a string"))?
        .to_owned();
    let kind_token = value["kind"]
        .as_str()
        .ok_or_else(|| invalid("assurance policy obligation `kind` must be a string"))?;
    let kind = ObligationKind::from_token(kind_token).ok_or_else(|| {
        invalid(format!(
            "assurance policy obligation kind `{kind_token}` is outside the closed vocabulary"
        ))
    })?;
    let classification_token = value["classification"]
        .as_str()
        .ok_or_else(|| invalid("assurance policy obligation `classification` must be a string"))?;
    let classification = AssuranceClass::from_token(classification_token).ok_or_else(|| {
        invalid(format!(
            "assurance policy obligation classification `{classification_token}` is outside the closed vocabulary"
        ))
    })?;
    let methods = value["methods"]
        .as_array()
        .ok_or_else(|| invalid("assurance policy obligation `methods` must be an array"))?
        .iter()
        .map(parse_method)
        .collect::<Result<Vec<_>>>()?;
    Ok(ObligationView {
        id,
        declaration_id,
        kind,
        classification,
        methods,
    })
}

fn payload_of(envelope_json: &str) -> Result<Value> {
    let value: Value = serde_json::from_str(envelope_json)
        .map_err(|_| invalid("assurance policy envelope is not valid JSON"))?;
    value
        .get("payload")
        .cloned()
        .ok_or_else(|| invalid("assurance policy envelope has no `payload`"))
}

fn parse_obligations(payload: &Value) -> Result<Vec<ObligationView>> {
    payload["obligations"]
        .as_array()
        .ok_or_else(|| invalid("assurance policy payload `obligations` must be an array"))?
        .iter()
        .map(parse_obligation)
        .collect()
}

const STATIC_CLASSES: [AssuranceClass; 4] = [
    AssuranceClass::CompilerProved,
    AssuranceClass::SmtProved,
    AssuranceClass::ModelChecked,
    AssuranceClass::TheoremProved,
];

/// One `runtime_guarded` method's exact retention verdict: item 4 of the
/// issue's implementation sequence, "a guard can be removed only when the
/// exact target/artifact obligation is statically discharged under an
/// accepted policy; otherwise retain it." `target`/`artifact_digest` must
/// both be present and byte-equal between the guard and a sibling static
/// method for a removal verdict; two absent (`None`) target/artifact pairs
/// are never treated as an equal, unspecified binding, so an unbound guard
/// is always retained rather than guessed removable.
fn runtime_guard_verdict(
    profile: PolicyProfile,
    guard: &MethodView,
    siblings: &[MethodView],
) -> Value {
    if matches!(profile, PolicyProfile::ReportOnly) {
        return json!({
            "artifact_digest": guard.artifact_digest,
            "reason": "report_only_profile_grants_no_removal_authority",
            "removable": false,
            "target": guard.target,
        });
    }
    let exact_match = guard.target.is_some()
        && guard.artifact_digest.is_some()
        && siblings.iter().any(|sibling| {
            STATIC_CLASSES.contains(&sibling.class)
                && profile.accepts(sibling.class)
                && sibling.target == guard.target
                && sibling.artifact_digest == guard.artifact_digest
        });
    let (removable, reason) = if exact_match {
        (true, "statically_discharged_for_exact_target_and_artifact")
    } else if guard.target.is_none() || guard.artifact_digest.is_none() {
        (false, "guard_target_or_artifact_not_bound")
    } else {
        (
            false,
            "no_accepted_static_discharge_for_exact_target_and_artifact",
        )
    };
    json!({
        "artifact_digest": guard.artifact_digest,
        "reason": reason,
        "removable": removable,
        "target": guard.target,
    })
}

fn obligation_verdict(profile: PolicyProfile, obligation: &ObligationView) -> Value {
    let satisfied = profile.accepts(obligation.classification);
    let suggested_next_action = if satisfied {
        None
    } else {
        Some(suggested_next_action(
            obligation.kind,
            obligation.classification,
        ))
    };
    let runtime_guards: Vec<Value> = obligation
        .methods
        .iter()
        .filter(|method| method.class == AssuranceClass::RuntimeGuarded)
        .map(|guard| runtime_guard_verdict(profile, guard, &obligation.methods))
        .collect();
    json!({
        "classification": obligation.classification.token(),
        "declaration_id": obligation.declaration_id,
        "id": obligation.id,
        "kind": obligation.kind.token(),
        "runtime_guards": runtime_guards,
        "satisfied": satisfied,
        "suggested_next_action": suggested_next_action,
    })
}

fn render(mut value: Value, maximum: usize) -> Result<String> {
    value.sort_all_objects();
    let mut output = serde_json::to_string(&value)
        .map_err(|_| invalid("assurance policy JSON cannot be rendered"))?;
    output.push('\n');
    if output.len() > maximum {
        return Err(capacity("assurance policy output exceeds its byte limit"));
    }
    Ok(output)
}

/// Evaluate one already-generated, already-verified Assurance Manifest v1
/// envelope against `profile`. Independently replays the envelope with
/// [`crate::assurance_manifest::verify_envelope`] before trusting any of
/// its fields (fail closed on a malformed or forged envelope), then reports
/// one verdict per obligation plus a fixed-shape summary. Read-only: never
/// touches source, a target artifact, or the envelope's own bytes.
pub fn evaluate(envelope_json: &str, profile: PolicyProfile) -> Result<String> {
    assurance_manifest::verify_envelope(envelope_json)?;
    let payload = payload_of(envelope_json)?;
    let obligations = parse_obligations(&payload)?;
    let obligations_total = obligations.len();
    let satisfied_total = obligations
        .iter()
        .filter(|obligation| profile.accepts(obligation.classification))
        .count();
    let verdicts: Vec<Value> = obligations
        .iter()
        .map(|obligation| obligation_verdict(profile, obligation))
        .collect();
    let ci_status =
        if matches!(profile, PolicyProfile::ReportOnly) || satisfied_total == obligations_total {
            "pass"
        } else {
            "fail"
        };
    render(
        json!({
            "ci_status": ci_status,
            "counts": {
                "obligations_total": obligations_total,
                "satisfied": satisfied_total,
                "unsatisfied": obligations_total - satisfied_total,
            },
            "nonclaims": NONCLAIMS,
            "obligations": verdicts,
            "profile": profile.token(),
            "schema": SCHEMA,
            "source": payload["source"],
        }),
        MAX_POLICY_REPORT_BYTES,
    )
}

fn classification_of_id<'a>(
    obligations: &'a [ObligationView],
    id: &str,
) -> Option<&'a ObligationView> {
    obligations.iter().find(|obligation| obligation.id == id)
}

/// Apply a CI fail policy to a base/candidate delta
/// ([`crate::assurance_manifest::delta`]): fail when any obligation
/// weakened, any candidate assumption is stale as of the caller's `as_of`,
/// or any newly `added` obligation's candidate classification does not
/// satisfy `profile`. `PolicyProfile::ReportOnly` never fails: it always
/// reports `ci_status: "pass"`, matching its name.
pub fn evaluate_delta(
    base_envelope: &str,
    candidate_envelope: &str,
    as_of: Option<&str>,
    profile: PolicyProfile,
) -> Result<String> {
    let delta_json = assurance_manifest::delta(base_envelope, candidate_envelope, as_of)?;
    let delta_value: Value = serde_json::from_str(&delta_json)
        .map_err(|_| invalid("assurance policy delta is not valid JSON"))?;
    let candidate_payload = payload_of(candidate_envelope)?;
    let candidate_obligations = parse_obligations(&candidate_payload)?;

    let added: Vec<String> = delta_value["added"]
        .as_array()
        .ok_or_else(|| invalid("assurance policy delta `added` must be an array"))?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    let weakened_count = delta_value["weakened"]
        .as_array()
        .ok_or_else(|| invalid("assurance policy delta `weakened` must be an array"))?
        .len();
    let stale: Vec<String> = delta_value["stale"]
        .as_array()
        .ok_or_else(|| invalid("assurance policy delta `stale` must be an array"))?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();

    let mut new_open_obligations = Vec::new();
    for id in &added {
        if let Some(obligation) = classification_of_id(&candidate_obligations, id) {
            if !profile.accepts(obligation.classification) {
                new_open_obligations.push(id.clone());
            }
        }
    }

    let report_only = matches!(profile, PolicyProfile::ReportOnly);
    let ci_status = if report_only
        || (weakened_count == 0 && stale.is_empty() && new_open_obligations.is_empty())
    {
        "pass"
    } else {
        "fail"
    };

    render(
        json!({
            "ci_status": ci_status,
            "delta": {
                "added": delta_value["added"],
                "assumption_changed": delta_value["assumption_changed"],
                "reclassified": delta_value["reclassified"],
                "removed": delta_value["removed"],
                "stale": delta_value["stale"],
                "strengthened": delta_value["strengthened"],
                "weakened": delta_value["weakened"],
            },
            "new_open_obligations": new_open_obligations,
            "nonclaims": NONCLAIMS,
            "profile": profile.token(),
            "schema": DELTA_POLICY_SCHEMA,
        }),
        MAX_POLICY_REPORT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assurance_manifest::{
        AssuranceManifestOptions, ExternalRecords, MethodRecord, Obligation,
    };
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    // `assurance_manifest::generate` parses and verifies exactly the file it
    // is pointed at, standalone, and requires a `main` (SPX-G172), matching
    // `capability_manifest`/`region_report`. This fixture's `main` takes no
    // parameters and carries no contract, so it derives zero automatic
    // obligations of its own: every obligation a test below sees comes only
    // from the `ExternalRecords` it supplies, through the same public
    // extension point a future SMT/model-checking/proof-kernel producer
    // uses (see "Obligation derivation" in docs/ASSURANCE-MANIFEST-V1.md).
    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "spx-assurance-policy-{}-{}.spx",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::write(
                &path,
                "module policy.fixture;\n\n@id(\"policy.main\") fn main() -> i64 { 0 }\n",
            )
            .unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn envelope(obligations: &[Obligation]) -> String {
        let fixture = Fixture::new();
        let options = AssuranceManifestOptions::default().with_external_records(ExternalRecords {
            obligations: obligations.to_vec(),
            assumptions: Vec::new(),
        });
        assurance_manifest::generate(&fixture.0, &options).unwrap()
    }

    #[test]
    fn every_profile_token_round_trips() {
        for profile in PolicyProfile::ALL {
            assert_eq!(PolicyProfile::from_token(profile.token()), Some(profile));
        }
        assert_eq!(PolicyProfile::from_token("unknown"), None);
    }

    #[test]
    fn precedence_resolves_the_strictest_supplied_source() {
        let resolved = PolicyProfile::resolve_precedence(&[
            Some(PolicyProfile::ReportOnly),
            Some(PolicyProfile::AllowTestEvidence),
            None,
            Some(PolicyProfile::RequireStatic),
        ]);
        assert_eq!(resolved, Some(PolicyProfile::RequireStatic));
        assert_eq!(PolicyProfile::resolve_precedence(&[None, None]), None);
    }

    #[test]
    fn require_static_rejects_a_runtime_guarded_obligation() {
        let obligation = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let envelope = envelope(&[obligation]);
        let report = evaluate(&envelope, PolicyProfile::RequireStatic).unwrap();
        assert!(report.contains("\"ci_status\":\"fail\""));
        assert!(report.contains("\"suggested_next_action\":\"strengthen_precondition\""));
    }

    #[test]
    fn allow_runtime_guard_accepts_the_same_obligation() {
        let obligation = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let envelope = envelope(&[obligation]);
        let report = evaluate(&envelope, PolicyProfile::AllowRuntimeGuard).unwrap();
        assert!(report.contains("\"ci_status\":\"pass\""));
        assert!(report.contains("\"satisfied\":true"));
    }

    #[test]
    fn report_only_always_passes_and_never_authorizes_removal() {
        let obligation = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let envelope = envelope(&[obligation]);
        let report = evaluate(&envelope, PolicyProfile::ReportOnly).unwrap();
        assert!(report.contains("\"ci_status\":\"pass\""));
        assert!(report.contains("\"reason\":\"report_only_profile_grants_no_removal_authority\""));
        assert!(report.contains("\"removable\":false"));
    }

    #[test]
    fn a_guard_is_removable_only_for_the_exact_target_and_artifact() {
        let matching = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method({
                let mut method = MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1");
                method.target = Some("native64".to_owned());
                method.artifact_digest = Some("sha256:aa".to_owned());
                method
            })
            .with_method({
                let mut method = MethodRecord::new(AssuranceClass::CompilerProved, "t", "1");
                method.target = Some("native64".to_owned());
                method.artifact_digest = Some("sha256:aa".to_owned());
                method
            });
        let mismatched = Obligation::new(ObligationKind::Precondition, "app.g", "require:0")
            .with_method({
                let mut method = MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1");
                method.target = Some("native64".to_owned());
                method.artifact_digest = Some("sha256:aa".to_owned());
                method
            })
            .with_method({
                let mut method = MethodRecord::new(AssuranceClass::CompilerProved, "t", "1");
                method.target = Some("wasm32".to_owned());
                method.artifact_digest = Some("sha256:bb".to_owned());
                method
            });
        let envelope = envelope(&[matching, mismatched]);
        let report = evaluate(&envelope, PolicyProfile::AllowRuntimeGuard).unwrap();
        assert!(report.contains(
            "\"reason\":\"statically_discharged_for_exact_target_and_artifact\",\"removable\":true"
        ));
        assert!(report.contains(
            "\"reason\":\"no_accepted_static_discharge_for_exact_target_and_artifact\",\"removable\":false"
        ));
    }

    #[test]
    fn delta_policy_fails_on_a_weakened_obligation() {
        let before = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::CompilerProved, "t", "1"));
        let after = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let base = envelope(&[before]);
        let candidate = envelope(&[after]);
        let report =
            evaluate_delta(&base, &candidate, None, PolicyProfile::AllowRuntimeGuard).unwrap();
        assert!(report.contains("\"ci_status\":\"fail\""));
    }

    #[test]
    fn delta_policy_flags_a_new_open_obligation_under_require_static() {
        let kept = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::CompilerProved, "t", "1"));
        let added = Obligation::new(ObligationKind::Precondition, "app.h", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let base = envelope(std::slice::from_ref(&kept));
        let candidate = envelope(&[kept, added]);
        let report = evaluate_delta(&base, &candidate, None, PolicyProfile::RequireStatic).unwrap();
        assert!(report.contains("\"ci_status\":\"fail\""));
        assert!(report.contains("\"new_open_obligations\":[\"semaprax.obligation.v1"));
    }

    #[test]
    fn delta_policy_report_only_never_fails() {
        let before = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::CompilerProved, "t", "1"));
        let after = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let base = envelope(&[before]);
        let candidate = envelope(&[after]);
        let report = evaluate_delta(&base, &candidate, None, PolicyProfile::ReportOnly).unwrap();
        assert!(report.contains("\"ci_status\":\"pass\""));
    }

    #[test]
    fn a_malformed_envelope_is_rejected_before_any_profile_is_applied() {
        let error = evaluate("not json", PolicyProfile::ReportOnly).unwrap_err();
        assert_eq!(error.code, "SPX-Z103");
    }

    #[test]
    fn an_unrecognized_profile_token_does_not_parse() {
        assert_eq!(PolicyProfile::from_token("REQUIRE-STATIC"), None);
        assert_eq!(PolicyProfile::from_token("require_static"), None);
    }
}
