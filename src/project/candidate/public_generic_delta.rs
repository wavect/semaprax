//! Candidate-bound public generic surface delta: gate PG-4 of the
//! [Public Generic Ownership milestone](../../../docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).
//!
//! This is the candidate-bound analogue of [`super::abi_delta`], for the
//! versioned public generic type grammar rather than for the compiler's
//! internal identity keys. It selects the manifest's exports by stable
//! identity, describes those the grammar can spell on the exact immutable base
//! and final candidate revisions, and classifies the ordered pair with
//! [`crate::public_generic_surface::compare`].
//!
//! Two properties matter more than the report's shape.
//!
//! *Describing is not admitting.* A candidate surface is a description of an
//! already-checked signature. Nothing here admits a public generic signature,
//! widens a language or Project profile, or changes what the existing public
//! projections accept: they still reject generic surfaces, and the milestone's
//! separation gate keeps proving it. Public generic ownership remains
//! unsupported and unpublished.
//!
//! *An undescribable export is not a route failure.* Every export admitted by
//! a Project manifest today is scalar or borrowed-view shaped, so the grammar
//! refuses to spell at least one of its positions. That refusal is recorded as
//! an exclusion carrying the grammar's own closed reason, and an all-excluded
//! delta is a complete, valid report. Turning it into an error would make the
//! route unusable exactly where it has to be usable, and would hide the fact
//! that no public generic surface exists yet.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{wire, ProjectCandidate};
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use crate::public_generic_surface::{self as surface, CandidateSurface, Finding, Reason, Verdict};
use crate::public_generic_type::{self as grammar, TypeInventory};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// One deterministic candidate-bound comparison of two public generic surfaces.
pub const PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_SCHEMA: &str =
    "semaprax.project-candidate-public-generic-delta.v1";
/// The separate record returned by independent byte-exact replay.
pub const PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_VERIFICATION_SCHEMA: &str =
    "semaprax.project-candidate-public-generic-delta-verification.v1";
/// Canonical report bytes. The report embeds up to two 1 MiB surfaces and one
/// comparison, so it is bounded well above them and never truncated.
pub const MAX_PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_BYTES: usize = 4 * 1024 * 1024;

const MAX_FACT_BYTES: usize = 8 * 1024 * 1024;
const MAX_FACTS: usize = 4_096;
const MAX_VISITS: usize = 65_536;

const FACT_DOMAIN: &[u8] = b"semaprax.candidate-public-generic-delta.facts.v1\0";
const REPORT_DOMAIN: &[u8] = b"semaprax.candidate-public-generic-delta.report.v1\0";

/// Retained candidate facts cannot be described by this route.
pub const INCONSISTENT_FACTS: &str = "SPX-PG301";
/// A delta bound was reached. The report is never truncated or repaired.
pub const DELTA_CAPACITY: &str = "SPX-PG302";
/// Submitted bytes are not the independently recomputed report.
pub const DELTA_REPLAY_MISMATCH: &str = "SPX-PG303";

/// The selection-level exclusion reason: the identity is a generic template,
/// is absent from the retained functions, or resolves to two of them.
const NOT_A_CANDIDATE_EXPORT: &str = "not_a_candidate_export";
/// A grammar or surface bound was reached while describing one export.
const GRAMMAR_BOUND: &str = "grammar_bound";

/// Both revisions describe a surface, so the PG-3 comparison classifies them.
const BASIS_COMPARED: &str = "public_generic_compatibility_v1_over_both_described_surfaces";
/// Neither revision describes one, so there is nothing to classify.
const BASIS_NONE: &str = "no_described_export_on_either_revision";
/// Exactly one revision describes one, so only export presence is classified.
const BASIS_PRESENCE: &str = "described_export_presence_only_one_revision_describes_a_surface";

#[derive(Default)]
struct Budget {
    bytes: usize,
    facts: usize,
    visits: usize,
}

impl Budget {
    fn visit(&mut self) -> Result<()> {
        self.visits = self.visits.checked_add(1).ok_or_else(capacity)?;
        if self.visits > MAX_VISITS {
            return Err(capacity());
        }
        Ok(())
    }

    fn fact(&mut self, value: &Value) -> Result<()> {
        let bytes = wire::render(value.clone(), MAX_FACT_BYTES.saturating_sub(self.bytes))
            .map_err(|_| capacity())?
            .len();
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(capacity)?;
        self.facts = self.facts.checked_add(1).ok_or_else(capacity)?;
        if self.bytes > MAX_FACT_BYTES || self.facts > MAX_FACTS {
            return Err(capacity());
        }
        Ok(())
    }
}

/// One revision's described and excluded exports.
struct Side {
    selected: Vec<String>,
    described: Vec<String>,
    exclusions: Vec<Value>,
    surface: Option<CandidateSurface>,
}

impl ProjectCandidate {
    /// Compare the public generic candidate surfaces of the exact immutable
    /// base and final candidate revisions.
    ///
    /// The selection basis is the manifest's complete `web_exports` set plus
    /// its command function when present, by stable identity and never by
    /// display name - the same basis as [`Self::abi_delta`]. An export whose
    /// signature the grammar cannot spell is excluded with the grammar's own
    /// closed reason rather than failing the route.
    ///
    /// The report describes candidate surfaces. It admits no public generic
    /// signature, and no version, support, or publication decision follows
    /// from it.
    pub fn public_generic_delta(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let mut budget = Budget::default();
        let before = side(&self.base, &mut budget)?;
        let after = side(&self.revision, &mut budget)?;

        let (basis, verdict, findings, comparison) = classify(&before, &after, &mut budget)?;
        let facts = json!({
            "selection": {
                "basis": SELECTION_BASIS,
                "base": before.selected,
                "candidate": after.selected,
            },
            "base": side_json(&before, &mut budget)?,
            "candidate": side_json(&after, &mut budget)?,
            "comparison": {
                "basis": basis,
                "verdict": verdict,
                "findings": findings,
                "base_surface_digest": digest_of(&before),
                "candidate_surface_digest": digest_of(&after),
                "public_generic_compatibility_v1": comparison,
            },
        });
        budget.fact(&facts)?;
        let facts_bytes = wire::render(
            facts.clone(),
            MAX_PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_BYTES,
        )
        .map_err(|_| capacity())?;
        let value = json!({
            "schema": PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_SCHEMA,
            "grammar_schema": grammar::PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA,
            "surface_schema": surface::CANDIDATE_SURFACE_SCHEMA,
            "compatibility_schema": surface::COMPATIBILITY_SCHEMA,
            "milestone": "semaprax.public-generic-ownership.v1",
            "candidate_digest": expected_candidate,
            "base_project_revision": self.base.project_revision(),
            "project_revision": self.revision.project_revision(),
            "base_workspace_revision": self.base.workspace_revision(),
            "workspace_revision": self.revision.workspace_revision(),
            "base_project_graph_digest": self.base.semantic_graph_digest(),
            "project_graph_digest": self.revision.semantic_graph_digest(),
            "facts_digest": wire::digest(FACT_DOMAIN, facts_bytes.as_bytes()),
            "facts": facts,
            "inventory": {
                "base_selected": before.selected.len(),
                "candidate_selected": after.selected.len(),
                "base_described": before.described.len(),
                "candidate_described": after.described.len(),
                "base_excluded": before.exclusions.len(),
                "candidate_excluded": after.exclusions.len(),
                "base_described_instances": instances_of(&before),
                "candidate_described_instances": instances_of(&after),
            },
            "selection_basis": SELECTION_BASIS,
            "exclusion_basis": EXCLUSION_BASIS,
            "compatibility_authority": COMPATIBILITY_AUTHORITY,
            "admission": ADMISSION,
            "support": "not_assessed",
            "publication": "not_assessed",
            "runtime": "not_observed",
            "semantic_version_decision": "not_inferred",
            "source_authority": false,
            "filesystem_authority": false,
            "execution_authority": false,
            "publication_authority": false,
            "deployment_authority": false,
            "limits": {
                "max_report_bytes": MAX_PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_BYTES,
                "max_fact_work_bytes": MAX_FACT_BYTES,
                "max_facts": MAX_FACTS,
                "max_visits": MAX_VISITS,
                "max_selected_exports": surface::MAX_SELECTED_EXPORTS,
                "max_reachable_instances": surface::MAX_REACHABLE_INSTANCES,
                "max_surface_bytes": surface::MAX_SURFACE_BYTES,
                "max_term_bytes": grammar::MAX_TERM_BYTES,
            },
            "nonclaims": NONCLAIMS,
        });
        wire::render(value, MAX_PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_BYTES)
            .map_err(|_| capacity())
    }

    /// Replay the complete candidate from its retained base and typed history,
    /// recompute the report, and accept only byte-exact equality.
    ///
    /// Submitted JSON is never treated as source, HIR, target evidence, a
    /// surface, a verdict, or authority: it is compared, never read.
    pub fn verify_public_generic_delta(
        &self,
        expected_candidate: &str,
        bytes: &[u8],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bytes.len() > MAX_PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_BYTES {
            return Err(capacity());
        }
        let replay = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        if replay.public_generic_delta(expected_candidate)?.as_bytes() != bytes {
            return Err(verification());
        }
        wire::render(
            json!({
                "schema": PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_VERIFICATION_SCHEMA,
                "result": "exact_recomputation",
                "candidate_digest": expected_candidate,
                "base_project_revision": self.base.project_revision(),
                "project_revision": self.revision.project_revision(),
                "delta_digest": wire::digest(REPORT_DOMAIN, bytes),
                "replay_basis": "complete_candidate_replayed_from_retained_base_and_typed_history",
                "submitted_bytes_authority": false,
                "compatibility_authority": COMPATIBILITY_AUTHORITY,
                "admission": ADMISSION,
                "support": "not_assessed",
                "publication": "not_assessed",
                "runtime": "not_observed",
                "source_authority": false,
                "execution_authority": false,
            }),
            MAX_PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_BYTES,
        )
        .map_err(|_| capacity())
    }
}

const SELECTION_BASIS: &str =
    "exact_manifest_web_exports_and_command_by_stable_identity_never_display_name";
const EXCLUSION_BASIS: &str =
    "first_undescribable_position_left_to_right_parameters_then_result_with_the_grammars_closed_reason";
const COMPATIBILITY_AUTHORITY: &str =
    "description_of_candidate_surfaces_only_compatibility_support_and_publication_remain_with_the_public_generic_ownership_milestone";
const ADMISSION: &str =
    "candidate_description_only_no_public_generic_signature_is_admitted_and_no_public_projection_is_widened";
const NONCLAIMS: [&str; 6] = [
    "not_a_public_generic_signature_admission",
    "not_a_semantic_version_support_or_publication_decision",
    "not_runtime_allocation_settlement_or_external_consumer_evidence",
    "not_a_descriptor_carrier_package_or_calling_convention",
    "not_hosted_evidence_for_any_milestone_gate",
    "no_source_filesystem_execution_publication_or_deployment_authority",
];

fn digest_of(side: &Side) -> Value {
    side.surface
        .as_ref()
        .map_or(Value::Null, |surface| json!(surface.digest()))
}

fn instances_of(side: &Side) -> usize {
    side.surface
        .as_ref()
        .map_or(0, |surface| surface.instances().len())
}

/// Describe one revision's selected exports.
///
/// The type inventory, the monomorphic functions, and the generic template
/// identities all come from the revision's retained projection modules. The
/// inventory records a repeated declaration identity on insert, so an
/// ambiguous nominal is an `ambiguous_declaration` exclusion rather than a
/// silent first match.
fn side(revision: &ProjectRevision, budget: &mut Budget) -> Result<Side> {
    let modules = revision.semantic.image_modules();
    let mut inventory = TypeInventory::new();
    for module in modules {
        budget.visit()?;
        inventory.extend(module.types());
    }
    let functions = modules
        .iter()
        .flat_map(|module| module.functions().iter().cloned())
        .collect::<Vec<_>>();
    let templates = modules
        .iter()
        .flat_map(|module| {
            module
                .function_templates()
                .iter()
                .map(|template| template.id.as_str())
        })
        .collect::<Vec<_>>();

    let mut selected = revision
        .manifest()
        .web_exports()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(command) = revision.manifest().command() {
        selected.insert(command.to_owned());
    }
    let selected = selected.into_iter().collect::<Vec<_>>();
    if selected.len() > surface::MAX_SELECTED_EXPORTS {
        return Err(capacity());
    }

    let mut described = Vec::new();
    let mut exclusions = Vec::new();
    for export in &selected {
        budget.visit()?;
        match position_exclusion(&inventory, &functions, &templates, export, budget)? {
            Some((position, reason)) => {
                let row = json!({"export": export, "position": position, "reason": reason});
                budget.fact(&row)?;
                exclusions.push(row);
            }
            None => described.push(export.clone()),
        }
    }

    let surface = if described.is_empty() {
        None
    } else {
        Some(
            CandidateSurface::derive_from(&inventory, &functions, &templates, &described)
                .map_err(surface_failure)?,
        )
    };
    Ok(Side {
        selected,
        described,
        exclusions,
        surface,
    })
}

/// The first position of one selected export that the grammar cannot spell, in
/// evaluation order: parameters left to right, then the result. `None` means
/// every position is describable.
fn position_exclusion(
    inventory: &TypeInventory<'_>,
    functions: &[crate::hir::ResolvedFunction],
    templates: &[&str],
    export: &str,
    budget: &mut Budget,
) -> Result<Option<(String, &'static str)>> {
    if templates.contains(&export) {
        return Ok(Some(("selection".to_owned(), NOT_A_CANDIDATE_EXPORT)));
    }
    let mut found = None;
    let mut repeated = false;
    for function in functions {
        if function.id.as_str() == export && found.replace(function).is_some() {
            repeated = true;
        }
    }
    let Some(function) = found.filter(|_| !repeated) else {
        return Ok(Some(("selection".to_owned(), NOT_A_CANDIDATE_EXPORT)));
    };
    for (index, parameter) in function.params.iter().enumerate() {
        budget.visit()?;
        if let Err(diagnostic) = grammar::term(inventory, &parameter.ty) {
            return Ok(Some((
                format!("parameter#{index}"),
                exclusion_reason(&diagnostic)?,
            )));
        }
    }
    budget.visit()?;
    if let Err(diagnostic) = grammar::term(inventory, &function.return_type) {
        return Ok(Some(("result".to_owned(), exclusion_reason(&diagnostic)?)));
    }
    Ok(None)
}

/// Map one grammar or surface diagnostic onto this route's closed exclusion
/// vocabulary. An unrecognized reason fails closed instead of being emitted.
fn exclusion_reason(diagnostic: &Diagnostic) -> Result<&'static str> {
    match diagnostic.code {
        grammar::REJECTED_TYPE => grammar::Rejection::of(diagnostic)
            .map(grammar::Rejection::reason)
            .ok_or_else(invalid),
        grammar::GRAMMAR_CAPACITY | surface::SURFACE_CAPACITY => Ok(GRAMMAR_BOUND),
        surface::INVALID_SELECTION => Ok(NOT_A_CANDIDATE_EXPORT),
        _ => Err(invalid()),
    }
}

/// A surface refusal after every selected position was already admitted is a
/// bound or an inconsistency, never a silent partial description.
fn surface_failure(diagnostic: Diagnostic) -> Vec<Diagnostic> {
    match diagnostic.code {
        surface::SURFACE_CAPACITY | grammar::GRAMMAR_CAPACITY => capacity(),
        _ => invalid(),
    }
}

/// Classify the ordered surface pair.
///
/// With both surfaces present this is exactly the PG-3 comparison, carried
/// whole. With neither present there is nothing described and therefore
/// nothing changed. With one present the described export set itself moved,
/// which the PG-3 reason vocabulary already spells; no other difference can be
/// classified, because there is no second surface to compare against.
fn classify(
    before: &Side,
    after: &Side,
    budget: &mut Budget,
) -> Result<(&'static str, &'static str, Vec<Value>, Value)> {
    match (&before.surface, &after.surface) {
        (Some(left), Some(right)) => {
            let report = surface::compare(left, right);
            let bytes = report.canonical_json().map_err(surface_failure)?;
            let value: Value = serde_json::from_str(&bytes).map_err(|_| invalid())?;
            let findings = findings_json(report.findings(), budget)?;
            Ok((BASIS_COMPARED, report.verdict().text(), findings, value))
        }
        (None, None) => Ok((
            BASIS_NONE,
            Verdict::Unchanged.text(),
            Vec::new(),
            Value::Null,
        )),
        (present, _) => {
            let (side, reason) = match present {
                Some(_) => (before, Reason::ExportRemoved),
                None => (after, Reason::ExportAdded),
            };
            let findings = side
                .described
                .iter()
                .map(|export| Finding {
                    subject: export.clone(),
                    reason,
                    detail: None,
                })
                .collect::<Vec<_>>();
            let verdict = findings
                .iter()
                .map(|finding| finding.reason.verdict())
                .max()
                .unwrap_or(Verdict::Unchanged);
            Ok((
                BASIS_PRESENCE,
                verdict.text(),
                findings_json(&findings, budget)?,
                Value::Null,
            ))
        }
    }
}

fn findings_json(findings: &[Finding], budget: &mut Budget) -> Result<Vec<Value>> {
    let mut rows = Vec::with_capacity(findings.len());
    for finding in findings {
        let row = json!({
            "subject": finding.subject,
            "reason": finding.reason.text(),
            "verdict": finding.reason.verdict().text(),
            "detail": finding.detail,
        });
        budget.fact(&row)?;
        rows.push(row);
    }
    Ok(rows)
}

fn side_json(side: &Side, budget: &mut Budget) -> Result<Value> {
    let surface = match &side.surface {
        None => Value::Null,
        Some(surface) => {
            let bytes = surface.canonical_json().map_err(surface_failure)?;
            serde_json::from_str(&bytes).map_err(|_| invalid())?
        }
    };
    let row = json!({
        "selected": side.selected.len(),
        "described": side.described,
        "excluded": side.exclusions,
        "surface_digest": digest_of(side),
        "described_instances": instances_of(side),
        "surface": surface,
    });
    budget.fact(&row)?;
    Ok(row)
}

fn invalid() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        INCONSISTENT_FACTS,
        format!(
            "{PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_SCHEMA} cannot describe the retained \
             candidate facts"
        ),
    )]
}

fn capacity() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        DELTA_CAPACITY,
        format!(
            "{PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_SCHEMA} exceeds its bounded work or output"
        ),
    )]
}

fn verification() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        DELTA_REPLAY_MISMATCH,
        format!("{PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_SCHEMA} failed exact independent replay"),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every reason this route can emit is either the grammar's own closed
    /// spelling or one of the two selection-level reasons, and the grammar's
    /// twelve are enumerated exactly once each.
    #[test]
    fn the_exclusion_reason_vocabulary_is_closed_and_matches_the_grammar() {
        let reasons = grammar::Rejection::ALL
            .iter()
            .map(|rejection| rejection.reason())
            .collect::<BTreeSet<_>>();
        assert_eq!(reasons.len(), grammar::Rejection::ALL.len());
        assert!(reasons.contains("borrowed_byte_view"));
        assert!(reasons.contains("type_parameter"));
        assert!(!reasons.contains(NOT_A_CANDIDATE_EXPORT));
        assert!(!reasons.contains(GRAMMAR_BOUND));
    }

    /// A reason is extracted from the owning artifact's diagnostic, so a
    /// message this route does not recognize fails closed rather than emitting
    /// an invented reason.
    #[test]
    fn an_unrecognized_grammar_message_fails_closed() {
        let forged = Diagnostic::io(grammar::REJECTED_TYPE, "hand written rejection");
        assert_eq!(
            exclusion_reason(&forged).err().unwrap()[0].code,
            INCONSISTENT_FACTS
        );
        let unknown = Diagnostic::io("SPX-G522", "some other subsystem");
        assert_eq!(
            exclusion_reason(&unknown).err().unwrap()[0].code,
            INCONSISTENT_FACTS
        );
        assert_eq!(
            exclusion_reason(&Diagnostic::io(surface::INVALID_SELECTION, "x")).unwrap(),
            NOT_A_CANDIDATE_EXPORT
        );
        assert_eq!(
            exclusion_reason(&Diagnostic::io(grammar::GRAMMAR_CAPACITY, "x")).unwrap(),
            GRAMMAR_BOUND
        );
    }
}
