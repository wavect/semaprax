//! Canonical, deterministic JSON rendering for
//! `semaprax.assurance-manifest.v1`.
//!
//! See [`docs/ASSURANCE-MANIFEST-V1.md`](../../docs/ASSURANCE-MANIFEST-V1.md)
//! "Canonical envelope" for the exact key order and digest domains this
//! module implements.

use sha2::{Digest as _, Sha256};

use crate::bounded_output::BudgetedJoin as _;
use crate::diagnostic::quote_json;

use super::lattice::{classification_of, AssuranceClass};
use super::obligation::{AssumptionRecord, MethodRecord, Obligation};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.assurance-manifest.v1";

const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.assurance-manifest.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.assurance-manifest.payload.v1\0";

const NONCLAIMS_JSON: &str = "\"no_smt_solver_invoked\",\
\"no_model_checker_invoked\",\
\"no_proof_kernel_invoked\",\
\"no_project_test_discovery_or_execution\",\
\"no_target_execution\",\
\"no_native_or_wasm_runtime_execution\",\
\"not_human_approval_or_policy\",\
\"not_signature_or_publication_authority\",\
\"not_safe_compatible_or_target_conformant\",\
\"no_repository_or_multi_file_analysis\",\
\"no_architecture_law_derivation_yet\",\
\"read_only_no_source_changes\"";

pub(super) fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

pub(super) fn source_digest(source: &str) -> String {
    domain_digest(SOURCE_DIGEST_DOMAIN, source.as_bytes())
}

pub(super) fn payload_digest(payload_bytes: &[u8]) -> String {
    domain_digest(PAYLOAD_DIGEST_DOMAIN, payload_bytes)
}

/// Every input [`render`] needs. Kept as one struct so `generate` cannot
/// accidentally pass source/revision/digest positionally out of order.
pub(super) struct RenderInput<'a> {
    pub source_path_text: &'a str,
    pub revision: &'a str,
    pub source_sha256: &'a str,
    pub obligations: &'a [Obligation],
    pub assumptions: &'a [AssumptionRecord],
    pub max_bytes: usize,
    pub max_obligations: usize,
}

fn opt_json(value: &Option<String>) -> String {
    match value {
        Some(text) => quote_json(text),
        None => "null".to_owned(),
    }
}

fn render_method(method: &MethodRecord) -> String {
    let inputs = method
        .inputs
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let assumption_ids = method
        .assumption_ids
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let test_refs = method
        .test_refs
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    bformat!(
        "{{\"class\":{},\"tool\":{},\"tool_version\":{},\"inputs\":[{}],\"bounds\":{},\
\"assumption_ids\":[{}],\"proof_ref\":{},\"counterexample_ref\":{},\"runtime_fallback\":{},\
\"test_refs\":[{}],\"target\":{},\"artifact_digest\":{},\"detail\":{}}}",
        quote_json(method.class.token()),
        quote_json(&method.tool),
        quote_json(&method.tool_version),
        inputs,
        opt_json(&method.bounds),
        assumption_ids,
        opt_json(&method.proof_ref),
        opt_json(&method.counterexample_ref),
        method.runtime_fallback,
        test_refs,
        opt_json(&method.target),
        opt_json(&method.artifact_digest),
        opt_json(&method.detail),
    )
}

fn render_obligation(
    obligation: &Obligation,
    class_counts: &mut [(AssuranceClass, usize)],
) -> String {
    let classes: Vec<AssuranceClass> = obligation
        .methods
        .iter()
        .map(|method| method.class)
        .collect();
    let classification = classification_of(&classes);
    if let Some(slot) = class_counts
        .iter_mut()
        .find(|(class, _)| *class == classification)
    {
        slot.1 += 1;
    }
    let methods_json = obligation
        .methods
        .iter()
        .map(render_method)
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let mut assumption_ids: Vec<&str> = obligation
        .methods
        .iter()
        .flat_map(|method| method.assumption_ids.iter().map(String::as_str))
        .collect();
    assumption_ids.sort_unstable();
    assumption_ids.dedup();
    let assumption_ids_json = assumption_ids
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    bformat!(
        "{{\"id\":{},\"declaration_id\":{},\"kind\":{},\"classification\":{},\"methods\":[{}],\
\"assumption_ids\":[{}]}}",
        quote_json(&obligation.id),
        quote_json(&obligation.declaration_id),
        quote_json(obligation.kind.token()),
        quote_json(classification.token()),
        methods_json,
        assumption_ids_json,
    )
}

fn render_assumption(record: &AssumptionRecord) -> String {
    let dependents = record
        .dependents
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    bformat!(
        "{{\"id\":{},\"owner\":{},\"rationale\":{},\"scope\":{},\"review_by\":{},\"dependents\":[{}]}}",
        quote_json(&record.id),
        quote_json(&record.owner),
        quote_json(&record.rationale),
        quote_json(&record.scope),
        opt_json(&record.review_by),
        dependents,
    )
}

/// Render the canonical envelope. `input.obligations`/`input.assumptions`
/// need not already be sorted; this function sorts both by `id` in
/// ascending byte order before rendering, independent of the order the
/// caller assembled them in.
pub(super) fn render(input: &RenderInput<'_>) -> String {
    let mut obligations: Vec<&Obligation> = input.obligations.iter().collect();
    obligations.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    let mut assumptions: Vec<&AssumptionRecord> = input.assumptions.iter().collect();
    assumptions.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));

    let mut class_counts: Vec<(AssuranceClass, usize)> = AssuranceClass::ALL
        .into_iter()
        .map(|class| (class, 0usize))
        .collect();
    let obligation_entries: Vec<String> = obligations
        .iter()
        .map(|obligation| render_obligation(obligation, &mut class_counts))
        .collect();
    let assumption_entries: Vec<String> = assumptions
        .iter()
        .map(|record| render_assumption(record))
        .collect();

    let by_class_json = class_counts
        .iter()
        .map(|(class, count)| format!("{}:{count}", quote_json(class.token())))
        .collect::<Vec<_>>()
        .join(",");

    let source_json = bformat!(
        "{{\"path\":{},\"revision\":{},\"sha256\":{}}}",
        quote_json(input.source_path_text),
        quote_json(input.revision),
        quote_json(input.source_sha256),
    );
    let limits_json = bformat!(
        "{{\"max_bytes\":{},\"max_obligations\":{}}}",
        input.max_bytes,
        input.max_obligations,
    );
    let counts_json = bformat!(
        "{{\"obligations_total\":{},\"assumptions_total\":{},\"by_class\":{{{}}}}}",
        obligations.len(),
        assumptions.len(),
        by_class_json,
    );

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{},\"limits\":{},\"counts\":{},\"obligations\":[{}],\
\"assumptions\":[{}],\"nonclaims\":[{}]}}",
        SCHEMA,
        source_json,
        limits_json,
        counts_json,
        obligation_entries.budgeted_join(","),
        assumption_entries.budgeted_join(","),
        NONCLAIMS_JSON,
    );
    bformat!(
        "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        SCHEMA,
        quote_json(&payload_digest(payload.as_bytes())),
        payload.len(),
        payload,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assurance_manifest::obligation::{MethodRecord, Obligation, ObligationKind};

    fn sample_input<'a>(
        obligations: &'a [Obligation],
        assumptions: &'a [AssumptionRecord],
    ) -> RenderInput<'a> {
        RenderInput {
            source_path_text: "examples/meaning.spx",
            revision: "revision",
            source_sha256:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            obligations,
            assumptions,
            max_bytes: 65_536,
            max_obligations: 1024,
        }
    }

    #[test]
    fn rendering_is_deterministic() {
        let obligation =
            Obligation::new(ObligationKind::Precondition, "app.f", "require:0").with_method(
                MethodRecord::new(AssuranceClass::RuntimeGuarded, "tool", "1.0"),
            );
        let first = render(&sample_input(std::slice::from_ref(&obligation), &[]));
        let second = render(&sample_input(&[obligation], &[]));
        assert_eq!(first, second);
    }

    #[test]
    fn obligations_render_in_ascending_id_order_regardless_of_input_order() {
        let a = Obligation::new(ObligationKind::Precondition, "app.a", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let b = Obligation::new(ObligationKind::Precondition, "app.b", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let forward = render(&sample_input(&[a.clone(), b.clone()], &[]));
        let backward = render(&sample_input(&[b, a], &[]));
        assert_eq!(forward, backward);
    }

    #[test]
    fn envelope_has_no_terminal_newline() {
        let rendered = render(&sample_input(&[], &[]));
        assert!(!rendered.ends_with('\n'));
    }

    #[test]
    fn counts_reflect_the_derived_classification_not_a_raw_method_class() {
        // Two incomparable classes on the same obligation: classification_of
        // must break the tie, and the count must land on that winner only.
        let obligation = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::SmtProved, "t", "1"))
            .with_method(MethodRecord::new(AssuranceClass::CompilerProved, "t", "1"));
        let rendered = render(&sample_input(&[obligation], &[]));
        assert!(rendered.contains("\"compiler_proved\":1"));
        assert!(rendered.contains("\"smt_proved\":0"));
    }
}
