//! Assurance Manifest v1 (`semaprax.assurance-manifest.v1`): a deterministic,
//! read-only, per-obligation assurance record for one verified single-file
//! SEMAPRAX module.
//!
//! See [`docs/ASSURANCE-MANIFEST-V1.md`](../docs/ASSURANCE-MANIFEST-V1.md)
//! for the full specification this module implements: obligation identity,
//! the assurance lattice, the canonical envelope, determinism, drift and
//! fail-closed replay, delta, and the redacted public view. This tranche is
//! proof data, not permission: it never executes a target, discovers or
//! runs project tests, or writes source, and it grants no execution,
//! publication, or signing authority.
//!
//! [`generate`] derives obligations from what the established
//! `verify::verify` diagnostic pass already proved for one source file (see
//! [`derive`]), optionally merges caller-supplied
//! [`AssuranceManifestOptions::external_records`], and renders the
//! canonical envelope. [`verify_envelope`] and
//! [`verify_envelope_against_source`] independently replay one envelope.
//! [`delta`] classifies what changed between two envelopes. [`public_view`]
//! strips free-text detail before sharing a classification summary.
//!
//! Diagnostics use the previously unused `SPX-Z1xx` family:
//! - `SPX-Z101`: invalid options, or a malformed/inconsistent external
//!   record (empty id, duplicate id, or a dangling assumption reference).
//! - `SPX-Z102`: obligation-count or output byte-budget exhaustion; fail
//!   closed, never truncated.
//! - `SPX-Z103`: envelope or payload structural/replay consistency failure.
//! - `SPX-Z104`: source or artifact binding drift.

mod delta;
mod derive;
mod lattice;
pub mod model_checking;
mod obligation;
pub mod proof_certificate;
mod render;
pub mod smt_discharge;
mod verify;

pub use delta::delta;
pub use lattice::{classification_of, dominates, AssuranceClass};
pub use obligation::{
    obligation_id, AssumptionRecord, ExternalRecords, MethodRecord, Obligation, ObligationKind,
};
pub use render::SCHEMA;
pub use verify::{verify_envelope, verify_envelope_against_source};

use std::collections::BTreeSet;
use std::path::Path;

use crate::bounded_output::with_limit;
use crate::diagnostic::Diagnostic;
use crate::{graph, patch};

use render::{render, RenderInput};

const DEFAULT_MAX_BYTES: usize = 262_144;
const DEFAULT_MAX_OBLIGATIONS: usize = 65_536;
const MIN_MAX_BYTES: usize = 2048;
const MAX_MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_MAX_OBLIGATIONS: usize = 65_536;

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z101", message)
}

fn budget_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z102", message)
}

/// Bounds and caller-supplied external records for [`generate`]. Construct
/// with [`Self::new`] (validated) or [`Self::default`] (the fixed defaults
/// below); attach external records with [`Self::with_external_records`].
#[derive(Clone, Debug)]
pub struct AssuranceManifestOptions {
    pub max_bytes: usize,
    pub max_obligations: usize,
    pub external_records: ExternalRecords,
}

impl AssuranceManifestOptions {
    pub fn new(max_bytes: usize, max_obligations: usize) -> Result<Self, Diagnostic> {
        if !(MIN_MAX_BYTES..=MAX_MAX_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "assurance-manifest max_bytes must be between {MIN_MAX_BYTES} and {MAX_MAX_BYTES}"
            )));
        }
        if max_obligations == 0 || max_obligations > MAX_MAX_OBLIGATIONS {
            return Err(option_error(format!(
                "assurance-manifest max_obligations must be between 1 and {MAX_MAX_OBLIGATIONS}"
            )));
        }
        Ok(Self {
            max_bytes,
            max_obligations,
            external_records: ExternalRecords::default(),
        })
    }

    #[must_use]
    pub fn with_external_records(mut self, external_records: ExternalRecords) -> Self {
        self.external_records = external_records;
        self
    }
}

impl Default for AssuranceManifestOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            max_obligations: DEFAULT_MAX_OBLIGATIONS,
            external_records: ExternalRecords::default(),
        }
    }
}

/// Generate the canonical `semaprax.assurance-manifest.v1` envelope JSON for
/// one verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and
/// the final check or generation fails closed, exactly like
/// `capability_manifest::generate` and `region_report::generate`.
pub fn generate(
    source_path: &Path,
    options: &AssuranceManifestOptions,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = crate::parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = crate::verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    let mut obligations = derive::derive_obligations(&program);
    obligations.extend(options.external_records.obligations.iter().cloned());
    let assumptions = options.external_records.assumptions.clone();
    validate_obligations_and_assumptions(&obligations, &assumptions)?;

    if obligations.len() > options.max_obligations {
        return Err(vec![budget_error(format!(
            "assurance-manifest derived {} obligations, exceeding max_obligations {}",
            obligations.len(),
            options.max_obligations
        ))]);
    }

    let source_sha256 = render::source_digest(snapshot.source());
    let path_text = source_path.display().to_string();
    let input = RenderInput {
        source_path_text: &path_text,
        revision: &revision,
        source_sha256: &source_sha256,
        obligations: &obligations,
        assumptions: &assumptions,
        max_bytes: options.max_bytes,
        max_obligations: options.max_obligations,
    };
    let (envelope, overflowed) = with_limit(options.max_bytes, || render(&input));
    if overflowed {
        return Err(vec![budget_error(
            "assurance-manifest output exceeds the max-bytes budget; refusing to truncate"
                .to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

/// Reject a malformed or inconsistent obligation/assumption set before any
/// output budget is spent: an empty or duplicate id, or a method record
/// referencing an assumption that is not present anywhere in this exact
/// call's combined (automatic + external) record set.
fn validate_obligations_and_assumptions(
    obligations: &[Obligation],
    assumptions: &[AssumptionRecord],
) -> Result<(), Vec<Diagnostic>> {
    let mut assumption_ids = BTreeSet::new();
    for assumption in assumptions {
        if assumption.id.is_empty() {
            return Err(vec![option_error(
                "external assumption id must not be empty".to_owned(),
            )]);
        }
        if !assumption_ids.insert(assumption.id.as_str()) {
            return Err(vec![option_error(format!(
                "duplicate external assumption id `{}`",
                assumption.id
            ))]);
        }
    }

    let mut obligation_ids = BTreeSet::new();
    for obligation in obligations {
        if obligation.id.is_empty() {
            return Err(vec![option_error(
                "obligation id must not be empty".to_owned(),
            )]);
        }
        if !obligation_ids.insert(obligation.id.as_str()) {
            return Err(vec![option_error(format!(
                "duplicate obligation id `{}`; an external record collided with an automatically \
                 derived obligation, or with another external record",
                obligation.id
            ))]);
        }
        for method in &obligation.methods {
            for referenced in &method.assumption_ids {
                if !assumption_ids.contains(referenced.as_str()) {
                    return Err(vec![option_error(format!(
                        "obligation `{}` references assumption `{referenced}`, which is not \
                         supplied in external_records.assumptions",
                        obligation.id
                    ))]);
                }
            }
            // "A failed solver run remains evidence about an attempt, not a
            // successful classification": reject this incoherence at
            // generation time too, not only on later replay (`verify::
            // check_method` enforces the identical rule independently).
            if matches!(
                method.class,
                AssuranceClass::Open
                    | AssuranceClass::Assumed
                    | AssuranceClass::AttemptInconclusive
            ) && method.proof_ref.is_some()
            {
                return Err(vec![option_error(format!(
                    "obligation `{}` carries a `{}` method with a `proof_ref`; \
                     an attempt that did not reach a positive verdict has no proof",
                    obligation.id,
                    method.class.token()
                ))]);
            }
        }
    }
    Ok(())
}

/// Independently verify `envelope`, then re-render it with every
/// `method.detail`, `method.inputs`, `method.proof_ref`,
/// `method.counterexample_ref`, and `assumption.rationale` field replaced
/// by JSON `null`, and `source.path`/`source.sha256` dropped. See
/// "Redacted public view" in the owning specification for why this output's
/// key order is alphabetical rather than the canonical envelope's declared
/// order, and why it carries no outer `digest`/`bytes` wrapper.
pub fn public_view(envelope: &str) -> Result<String, Diagnostic> {
    verify_envelope(envelope)?;
    let mut value: serde_json::Value =
        serde_json::from_str(envelope).expect("verify_envelope already accepted this JSON");
    let mut payload = value["payload"].take();

    if let Some(source) = payload
        .get_mut("source")
        .and_then(serde_json::Value::as_object_mut)
    {
        source.remove("path");
        source.remove("sha256");
    }
    if let Some(obligations) = payload
        .get_mut("obligations")
        .and_then(serde_json::Value::as_array_mut)
    {
        for obligation in obligations {
            if let Some(methods) = obligation
                .get_mut("methods")
                .and_then(serde_json::Value::as_array_mut)
            {
                for method in methods {
                    if let Some(object) = method.as_object_mut() {
                        object.insert("detail".to_owned(), serde_json::Value::Null);
                        object.insert("inputs".to_owned(), serde_json::Value::Array(Vec::new()));
                        object.insert("proof_ref".to_owned(), serde_json::Value::Null);
                        object.insert("counterexample_ref".to_owned(), serde_json::Value::Null);
                    }
                }
            }
        }
    }
    if let Some(assumptions) = payload
        .get_mut("assumptions")
        .and_then(serde_json::Value::as_array_mut)
    {
        for assumption in assumptions {
            if let Some(object) = assumption.as_object_mut() {
                object.insert("rationale".to_owned(), serde_json::Value::Null);
            }
        }
    }
    serde_json::to_string(&payload).map_err(|error| {
        Diagnostic::io(
            "SPX-Z103",
            format!("public view failed to serialize: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Every executable module needs `fn main() -> i64` or `verify::verify`
    /// rejects it before `generate` ever reaches obligation derivation.
    fn write_temp(source: &str, label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "semaprax-assurance-manifest-{label}-{}-{}.spx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            format!("{source}\n@id(\"app.generate_unit.main\")\nfn main() -> i64 {{ 0 }}\n"),
        )
        .unwrap();
        path
    }

    #[test]
    fn options_reject_out_of_bounds_values() {
        assert!(AssuranceManifestOptions::new(512, 16).is_err());
        assert!(AssuranceManifestOptions::new(DEFAULT_MAX_BYTES, 0).is_err());
        assert!(AssuranceManifestOptions::new(DEFAULT_MAX_BYTES, MAX_MAX_OBLIGATIONS + 1).is_err());
        assert!(AssuranceManifestOptions::new(DEFAULT_MAX_BYTES, 16).is_ok());
    }

    #[test]
    fn generate_over_valid_source_derives_and_verifies() {
        let path = write_temp(
            "module app.generate_unit;\n\n@id(\"app.generate_unit.check\")\nfn check(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
            "unit",
        );
        let envelope = generate(&path, &AssuranceManifestOptions::default());
        std::fs::remove_file(&path).ok();
        let envelope = envelope.expect("generate should succeed");
        assert!(envelope.contains("\"schema\":\"semaprax.assurance-manifest.v1\""));
        verify_envelope(&envelope).expect("generated envelope must replay");
    }

    #[test]
    fn duplicate_obligation_ids_are_rejected_before_rendering() {
        let external = ExternalRecords {
            obligations: vec![Obligation::new(
                ObligationKind::Precondition,
                "app.generate_unit.check",
                "require:0",
            )
            .with_method(MethodRecord::new(AssuranceClass::TestEvidenced, "t", "1"))],
            assumptions: Vec::new(),
        };
        let path = write_temp(
            "module app.generate_unit;\n\n@id(\"app.generate_unit.check\")\nfn check(a: i64) -> i64\n    requires a >= 0\n{ a }\n",
            "dup",
        );
        let options = AssuranceManifestOptions::default().with_external_records(external);
        let result = generate(&path, &options);
        std::fs::remove_file(&path).ok();
        let diagnostics = result.expect_err("colliding ids must fail closed");
        assert_eq!(diagnostics[0].code, "SPX-Z101");
    }

    /// Issue #230: a library/provider module (no `main`) can never receive an
    /// assurance envelope, because `generate` reuses the exact single-file
    /// `verify::verify` pass `capability_manifest`/`region_report` also use,
    /// and that pass requires the parsed file to be a standalone runnable
    /// program. This is deliberate (see "Why this is a separate module from
    /// Assurance Manifest v1" in `docs/ASSURANCE-POLICY-V1.md` and the
    /// "Known limitations" entry this issue adds to
    /// `docs/ASSURANCE-MANIFEST-V1.md`), not an oversight: pin the exact
    /// rejection so a future change cannot silently start accepting (or
    /// silently keep rejecting for a different, unintended reason) a library
    /// source. Contrast with `generate_over_valid_source_derives_and_verifies`
    /// above, the accepted case with `main` present.
    #[test]
    fn generate_rejects_a_library_module_with_no_main() {
        let path = std::env::temp_dir().join(format!(
            "semaprax-assurance-manifest-no-main-{}-{}.spx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            "module app.lib_probe;\n\n@id(\"app.lib_probe.helper\")\nfn helper(a: i64) -> i64 { a }\n",
        )
        .unwrap();
        let result = generate(&path, &AssuranceManifestOptions::default());
        std::fs::remove_file(&path).ok();
        let diagnostics = result.expect_err("a library module with no `main` must be refused");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.code == "SPX-T105"),
            "{diagnostics:?}"
        );
    }

    /// Issue #230, the other half of the same boundary: a library module
    /// that imports another module (exactly what makes it a *library*
    /// module worth documenting assurance for) is refused even earlier, by
    /// the single-file pass's blanket `SPX-G172` for any `module_uses`
    /// (single-file `generate` never resolves cross-module imports; see
    /// `src/source_verify/declaration.rs`). Adding `main` would not clear
    /// this rejection, confirming the gap is structural rather than merely
    /// the missing-`main` check.
    #[test]
    fn generate_rejects_a_module_with_cross_module_imports_even_with_main() {
        let path = std::env::temp_dir().join(format!(
            "semaprax-assurance-manifest-imports-{}-{}.spx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            "module app.lib_probe2;\n\nuse function @id(\"other.helper\") from other.module as helper;\n\n@id(\"app.lib_probe2.main\")\nfn main() -> i64 { 0 }\n",
        )
        .unwrap();
        let result = generate(&path, &AssuranceManifestOptions::default());
        std::fs::remove_file(&path).ok();
        let diagnostics =
            result.expect_err("a module with cross-module imports must be refused standalone");
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "SPX-G172");
    }

    #[test]
    fn public_view_drops_free_text_and_path_but_keeps_classification() {
        let path = write_temp(
            "module app.generate_unit;\n\n@id(\"app.generate_unit.check\")\nfn check(a: i64) -> i64\n    ensures result == a\n{ a }\n",
            "public",
        );
        let envelope = generate(&path, &AssuranceManifestOptions::default()).unwrap();
        let path_text = path.display().to_string();
        std::fs::remove_file(&path).ok();
        let view = public_view(&envelope).expect("public view");
        assert!(view.contains("\"classification\":\"runtime_guarded\""));
        assert!(!view.contains(path_text.as_str()));
        assert!(view.contains("\"detail\":null"));
    }
}
