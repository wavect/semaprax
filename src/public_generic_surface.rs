//! Candidate public generic surfaces and the semantic compatibility rules over
//! them: gate PG-3 of the
//! [Public Generic Ownership milestone](../docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).
//!
//! A *candidate surface* is a description, never an admission. Selecting an
//! export here does not make its signature public, does not widen any language
//! or Project profile, and does not change what the existing public
//! projections accept — they still reject generic signatures, and the
//! milestone's separation gate keeps proving it. This module exists so that
//! the compatibility question can be asked and answered about exact bytes
//! before any surface is ever admitted.
//!
//! The rules are deliberately stricter than source compatibility. A foreign
//! consumer reads the whole substituted field tree and the owned-leaf shape of
//! what it receives, so a field added to any reachable record is breaking even
//! where a SEMAPRAX caller would not notice. Conversely, presentation is never
//! compatibility: renaming an export, a parameter, a record, a type parameter,
//! or a field changes nothing.
//!
//! Two non-inferences are part of the contract. A classification is not a
//! semantic-version decision, and it is not a support or publication decision.
//! Nothing here maps a verdict onto a version bump or a promotion, and the
//! emitted report says so in its own fields.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};
use sha2::{Digest as _, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedProgram, ResolvedType};
use crate::public_generic_type::{self as grammar, GrammarTerm, InstanceFacts, TypeInventory};

/// One deterministic description of the selected candidate exports.
pub const CANDIDATE_SURFACE_SCHEMA: &str = "semaprax.public-generic-candidate-surface.v1";
/// One deterministic comparison of two such descriptions.
pub const COMPATIBILITY_SCHEMA: &str = "semaprax.public-generic-compatibility.v1";

const SURFACE_DOMAIN: &[u8] = b"semaprax.public-generic-candidate-surface.v1\0";
const COMPARISON_DOMAIN: &[u8] = b"semaprax.public-generic-compatibility.v1\0";

/// Selected exports per surface.
pub const MAX_SELECTED_EXPORTS: usize = 64;
/// Reachable instances per surface.
pub const MAX_REACHABLE_INSTANCES: usize = 256;
/// Canonical surface or comparison bytes.
pub const MAX_SURFACE_BYTES: usize = 1024 * 1024;

/// The selection is empty, oversized, repeated, unknown, or not a candidate.
pub const INVALID_SELECTION: &str = "SPX-PG201";
/// A surface bound was reached. Surfaces are never truncated or repaired.
pub const SURFACE_CAPACITY: &str = "SPX-PG202";
/// Submitted surface bytes do not equal the independently recomputed bytes.
pub const SURFACE_REPLAY_MISMATCH: &str = "SPX-PG203";
/// Submitted comparison bytes do not equal the independently recomputed bytes.
pub const COMPARISON_REPLAY_MISMATCH: &str = "SPX-PG204";

fn invalid(subject: &str) -> Diagnostic {
    Diagnostic::io(
        INVALID_SELECTION,
        format!("{CANDIDATE_SURFACE_SCHEMA} selection is invalid: {subject}"),
    )
}

fn capacity(subject: &str) -> Diagnostic {
    Diagnostic::io(
        SURFACE_CAPACITY,
        format!("{CANDIDATE_SURFACE_SCHEMA} exceeded its {subject}"),
    )
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn render(value: &Value) -> Result<String, Diagnostic> {
    let mut rendered =
        serde_json::to_string(value).map_err(|_| capacity("canonical byte limit"))?;
    if rendered.len() >= MAX_SURFACE_BYTES {
        return Err(capacity("canonical byte limit"));
    }
    rendered.push('\n');
    Ok(rendered)
}

const fn ownership(mode: OwnershipMode) -> &'static str {
    match mode {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}

/// One candidate signature position.
///
/// The grammar spells *data types* - what a value can own and transfer. A
/// signature also has positions that are not data types: an invocation-rooted
/// borrowed view. Those get their own closed spellings here rather than
/// widening the grammar's vocabulary, and their spellings carry a `view:`
/// prefix that no grammar term can produce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceValue {
    /// `data`, `borrowed_byte_view`, or `borrowed_text_view`.
    pub kind: &'static str,
    pub term: String,
    /// Present exactly for a data position.
    pub term_digest: Option<String>,
    /// Present exactly when the term names a record instance.
    pub instance_digest: Option<String>,
}

/// One parameter position. `name` is presentation: it appears in the rendered
/// surface and is excluded from every identity-bearing fact, because an
/// expression-scoped spelling must not move a compatibility verdict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceParameter {
    pub index: u32,
    pub name: String,
    pub ownership: &'static str,
    pub value: SurfaceValue,
}

/// One selected candidate export, keyed by its persistent declaration identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceEntry {
    pub export: String,
    pub name: String,
    pub effects: Vec<String>,
    pub parameters: Vec<SurfaceParameter>,
    pub result: SurfaceValue,
}

/// The complete candidate description: the selected entries plus every record
/// instance reachable from their signatures, each described by the grammar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateSurface {
    entries: BTreeMap<String, SurfaceEntry>,
    instances: BTreeMap<String, InstanceFacts>,
    digest: String,
}

impl CandidateSurface {
    /// Describe the selected exports of one checked program.
    ///
    /// Selection is by persistent declaration identity, never by display name.
    /// A generic template is not a candidate export: a public surface names no
    /// type parameters, so a template selection fails closed rather than
    /// silently describing one instantiation of it.
    pub fn derive(program: &ResolvedProgram, exports: &[String]) -> Result<Self, Diagnostic> {
        Self::derive_from(
            &TypeInventory::of(program),
            &program.functions,
            &program
                .function_templates
                .iter()
                .map(|template| template.id.as_str())
                .collect::<Vec<_>>(),
            exports,
        )
    }

    /// Describe the selected exports of retained checked facts: the type
    /// inventory, the monomorphic functions, and the generic template
    /// identities that must be refused as selections.
    pub fn derive_from(
        inventory: &TypeInventory<'_>,
        functions: &[crate::hir::ResolvedFunction],
        templates: &[&str],
        exports: &[String],
    ) -> Result<Self, Diagnostic> {
        if exports.is_empty() {
            return Err(invalid("no export was selected"));
        }
        if exports.len() > MAX_SELECTED_EXPORTS {
            return Err(capacity("selected export limit"));
        }
        let unique = exports.iter().collect::<BTreeSet<_>>();
        if unique.len() != exports.len() {
            return Err(invalid("an export was selected twice"));
        }

        let mut entries = BTreeMap::new();
        let mut roots = Vec::new();
        for export in exports {
            if templates.iter().any(|template| template == export) {
                return Err(invalid(&format!(
                    "`{export}` is a generic template, not a candidate export"
                )));
            }
            let mut found = None;
            for function in functions {
                if function.id.as_str() == export && found.replace(function).is_some() {
                    return Err(invalid(&format!("`{export}` resolves to two declarations")));
                }
            }
            let Some(function) = found else {
                return Err(invalid(&format!("`{export}` is not a checked declaration")));
            };

            let mut parameters = Vec::with_capacity(function.params.len());
            for (index, parameter) in function.params.iter().enumerate() {
                parameters.push(SurfaceParameter {
                    index: index as u32,
                    name: parameter.name.clone(),
                    ownership: ownership(parameter.ownership),
                    value: value_of(inventory, &parameter.ty)?,
                });
                roots.push(parameter.ty.clone());
            }
            roots.push(function.return_type.clone());
            let entry = SurfaceEntry {
                export: export.clone(),
                name: function.name.clone(),
                effects: function.effects.clone(),
                parameters,
                result: value_of(inventory, &function.return_type)?,
            };
            entries.insert(export.clone(), entry);
        }

        let mut instances = BTreeMap::new();
        for root in roots {
            reach(inventory, &root, &mut instances)?;
        }

        let mut surface = Self {
            entries,
            instances,
            digest: String::new(),
        };
        surface.digest = digest(
            SURFACE_DOMAIN,
            surface.identity_json().to_string().as_bytes(),
        );
        Ok(surface)
    }

    /// The selected entries, keyed by declaration identity in byte order.
    pub fn entries(&self) -> &BTreeMap<String, SurfaceEntry> {
        &self.entries
    }

    /// Every reachable record instance, keyed by canonical term.
    pub fn instances(&self) -> &BTreeMap<String, InstanceFacts> {
        &self.instances
    }

    /// The domain-separated digest of the identity-bearing facts only.
    /// Presentation names are excluded, so a rename leaves it unchanged.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Canonical compact JSON plus one trailing newline. This carries the
    /// presentation names as well, so it is not the digest preimage.
    pub fn canonical_json(&self) -> Result<String, Diagnostic> {
        render(&self.to_json())
    }

    /// Independently recompute the canonical bytes and require byte equality.
    /// Submitted bytes are never treated as source, HIR, identity, or
    /// authority.
    pub fn verify(&self, submitted: &str) -> Result<(), Diagnostic> {
        if self.canonical_json()? == submitted {
            return Ok(());
        }
        Err(Diagnostic::io(
            SURFACE_REPLAY_MISMATCH,
            format!(
                "{CANDIDATE_SURFACE_SCHEMA} replay mismatch: submitted bytes are not the \
                 independently recomputed surface"
            ),
        ))
    }

    fn to_json(&self) -> Value {
        let mut entries = Map::new();
        for (export, entry) in &self.entries {
            entries.insert(
                export.clone(),
                json!({
                    "export": entry.export,
                    "name": entry.name,
                    "effects": entry.effects,
                    "parameters": entry
                        .parameters
                        .iter()
                        .map(|parameter| json!({
                            "index": parameter.index,
                            "name": parameter.name,
                            "ownership": parameter.ownership,
                            "type": value_json(&parameter.value),
                        }))
                        .collect::<Vec<_>>(),
                    "result": value_json(&entry.result),
                }),
            );
        }
        let mut instances = Map::new();
        for (term, facts) in &self.instances {
            instances.insert(term.clone(), instance_json(facts));
        }
        json!({
            "schema": CANDIDATE_SURFACE_SCHEMA,
            "grammar": grammar::PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA,
            "entries": Value::Object(entries),
            "instances": Value::Object(instances),
            "digest": self.digest,
            "selection_basis": "exact_persistent_declaration_identities_never_display_names",
            "admission": "candidate_description_only_no_public_signature_is_admitted",
            "support": "not_assessed",
            "publication": "not_assessed",
        })
    }

    /// The identity-bearing projection: every fact a compatibility verdict may
    /// depend on, and nothing else.
    fn identity_json(&self) -> Value {
        let mut entries = Map::new();
        for (export, entry) in &self.entries {
            entries.insert(
                export.clone(),
                json!({
                    "effects": entry.effects,
                    "parameters": entry
                        .parameters
                        .iter()
                        .map(|parameter| json!({
                            "index": parameter.index,
                            "ownership": parameter.ownership,
                            "kind": parameter.value.kind,
                            "term": parameter.value.term,
                            "instance": parameter.value.instance_digest,
                        }))
                        .collect::<Vec<_>>(),
                    "result": {
                        "kind": entry.result.kind,
                        "term": entry.result.term,
                        "instance": entry.result.instance_digest,
                    },
                }),
            );
        }
        let mut instances = Map::new();
        for (term, facts) in &self.instances {
            instances.insert(term.clone(), json!(facts.instance_digest));
        }
        json!({
            "schema": CANDIDATE_SURFACE_SCHEMA,
            "entries": Value::Object(entries),
            "instances": Value::Object(instances),
        })
    }
}

fn value_json(value: &SurfaceValue) -> Value {
    json!({
        "kind": value.kind,
        "term": value.term,
        "term_digest": value.term_digest,
        "instance_digest": value.instance_digest,
    })
}

fn instance_json(facts: &InstanceFacts) -> Value {
    json!({
        "term": facts.term,
        "term_digest": facts.term_digest,
        "instance_digest": facts.instance_digest,
        "template": {
            "declaration": facts.template.declaration,
            "name": facts.template.name,
            "arity": facts.template.arity,
            "digest": facts.template.digest,
            "parameters": facts
                .template
                .parameters
                .iter()
                .map(|parameter| json!({
                    "owner": parameter.owner,
                    "index": parameter.index,
                    "name": parameter.name,
                }))
                .collect::<Vec<_>>(),
        },
        "arguments": facts
            .arguments
            .iter()
            .map(|argument| json!({
                "index": argument.index,
                "parameter_owner": argument.parameter_owner,
                "parameter_index": argument.parameter_index,
                "term": argument.term,
                "digest": argument.digest,
            }))
            .collect::<Vec<_>>(),
        "fields": facts
            .fields
            .iter()
            .map(|field| json!({
                "index": field.index,
                "id": field.id,
                "name": field.name,
                "term": field.term,
                "digest": field.digest,
            }))
            .collect::<Vec<_>>(),
        "owned_leaves": facts.owned_leaves,
    })
}

fn value_of(inventory: &TypeInventory<'_>, ty: &ResolvedType) -> Result<SurfaceValue, Diagnostic> {
    if let Some((kind, spelling)) = borrowed_view(ty) {
        return Ok(SurfaceValue {
            kind,
            term: spelling.to_owned(),
            term_digest: None,
            instance_digest: None,
        });
    }
    let term = grammar::term(inventory, ty)?;
    let instance_digest = match ty {
        ResolvedType::Nominal { .. } => Some(grammar::describe(inventory, ty)?.instance_digest),
        _ => None,
    };
    Ok(SurfaceValue {
        kind: "data",
        term_digest: Some(grammar::term_digest(&term)),
        term,
        instance_digest,
    })
}

/// The closed non-data position vocabulary.
const fn borrowed_view(ty: &ResolvedType) -> Option<(&'static str, &'static str)> {
    match ty {
        ResolvedType::SliceU8 => Some(("borrowed_byte_view", "view:slice-u8")),
        ResolvedType::Str => Some(("borrowed_text_view", "view:str")),
        _ => None,
    }
}

/// Collect the reachable record instances of one signature position: the
/// instance itself, its ordered arguments, and its substituted fields.
fn reach(
    inventory: &TypeInventory<'_>,
    ty: &ResolvedType,
    output: &mut BTreeMap<String, InstanceFacts>,
) -> Result<(), Diagnostic> {
    let ResolvedType::Nominal { arguments, .. } = ty else {
        return Ok(());
    };
    let facts = grammar::describe(inventory, ty)?;
    if output.contains_key(&facts.term) {
        return Ok(());
    }
    if output.len() >= MAX_REACHABLE_INSTANCES {
        return Err(capacity("reachable instance limit"));
    }
    output.insert(facts.term.clone(), facts);
    for argument in arguments {
        reach(inventory, argument, output)?;
    }
    for field in grammar::concrete_fields(inventory, ty)? {
        reach(inventory, &field, output)?;
    }
    Ok(())
}

/// The closed compatibility verdict over two candidate surfaces.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Verdict {
    Unchanged,
    Compatible,
    Breaking,
}

impl Verdict {
    /// The closed wire spelling.
    pub const fn text(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Compatible => "compatible",
            Self::Breaking => "breaking",
        }
    }
}

/// Why a surface pair is classified as it is. Closed: a new reason is a new
/// compatibility version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reason {
    /// A candidate export the earlier surface did not select.
    ExportAdded,
    /// A candidate export the later surface no longer carries.
    ExportRemoved,
    /// The declared effect vector changed.
    EffectsChanged,
    /// The parameter count changed.
    ParameterCountChanged,
    /// A parameter's ownership mode changed at the same position.
    ParameterOwnershipChanged,
    /// A parameter's type changed at the same position, permutation included.
    ParameterTypeChanged,
    /// The result type changed.
    ResultTypeChanged,
    /// At some signature position, the same declaration slot names a
    /// different template identity or a different declared arity. Bound to
    /// the exact position: an entry parameter or result, or a nested type
    /// argument slot inside one, never the whole instance closure.
    InstanceTemplateChanged,
    /// At some signature position, one ordered type argument was permuted,
    /// substituted, or otherwise replaced while the enclosing template
    /// identity stayed the same. The subject names the exact argument slot
    /// (`<position>/arg<index>`, nested when the argument is itself an
    /// instance), so a permutation of two arguments yields two findings and a
    /// single substitution yields exactly one.
    InstanceArgumentsChanged,
    /// A reachable instance's substituted field inventory changed.
    InstanceFieldsChanged,
    /// A reachable instance's transitive owned-leaf shape changed.
    InstanceOwnedLeavesChanged,
    /// A reachable instance appeared. Its cause is classified at the entry it
    /// became reachable from.
    ReachableInstanceAdded,
    /// A reachable instance disappeared, likewise.
    ReachableInstanceRemoved,
}

impl Reason {
    /// The closed wire spelling.
    pub const fn text(self) -> &'static str {
        match self {
            Self::ExportAdded => "export_added",
            Self::ExportRemoved => "export_removed",
            Self::EffectsChanged => "effects_changed",
            Self::ParameterCountChanged => "parameter_count_changed",
            Self::ParameterOwnershipChanged => "parameter_ownership_changed",
            Self::ParameterTypeChanged => "parameter_type_changed",
            Self::ResultTypeChanged => "result_type_changed",
            Self::InstanceTemplateChanged => "instance_template_changed",
            Self::InstanceArgumentsChanged => "instance_arguments_changed",
            Self::InstanceFieldsChanged => "instance_fields_changed",
            Self::InstanceOwnedLeavesChanged => "instance_owned_leaves_changed",
            Self::ReachableInstanceAdded => "reachable_instance_added",
            Self::ReachableInstanceRemoved => "reachable_instance_removed",
        }
    }

    /// The verdict weight this reason contributes.
    pub const fn verdict(self) -> Verdict {
        match self {
            Self::ExportAdded | Self::ReachableInstanceAdded | Self::ReachableInstanceRemoved => {
                Verdict::Compatible
            }
            Self::ExportRemoved
            | Self::EffectsChanged
            | Self::ParameterCountChanged
            | Self::ParameterOwnershipChanged
            | Self::ParameterTypeChanged
            | Self::ResultTypeChanged
            | Self::InstanceTemplateChanged
            | Self::InstanceArgumentsChanged
            | Self::InstanceFieldsChanged
            | Self::InstanceOwnedLeavesChanged => Verdict::Breaking,
        }
    }
}

/// One classified difference, bound to the subject it was found on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    pub subject: String,
    pub reason: Reason,
    pub detail: Option<String>,
}

/// The complete comparison of two candidate surfaces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibilityReport {
    verdict: Verdict,
    findings: Vec<Finding>,
    before_digest: String,
    after_digest: String,
    digest: String,
}

impl CompatibilityReport {
    /// The overall verdict: the heaviest finding, `Unchanged` when there is
    /// none.
    pub fn verdict(&self) -> Verdict {
        self.verdict
    }

    /// Every classified difference, in a deterministic order.
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// The domain-separated digest of the comparison's own facts.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Canonical compact JSON plus one trailing newline.
    pub fn canonical_json(&self) -> Result<String, Diagnostic> {
        render(&self.to_json())
    }

    fn to_json(&self) -> Value {
        json!({
            "schema": COMPATIBILITY_SCHEMA,
            "grammar": grammar::PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA,
            "before_surface_digest": self.before_digest,
            "after_surface_digest": self.after_digest,
            "verdict": self.verdict.text(),
            "findings": self
                .findings
                .iter()
                .map(|finding| json!({
                    "subject": finding.subject,
                    "reason": finding.reason.text(),
                    "verdict": finding.reason.verdict().text(),
                    "detail": finding.detail,
                }))
                .collect::<Vec<_>>(),
            "digest": self.digest,
            "comparison_basis": "identity_bearing_facts_only_presentation_names_excluded",
            "semantic_version_decision": "not_inferred",
            "support": "not_assessed",
            "publication": "not_assessed",
            "runtime": "not_observed",
        })
    }
}

/// Classify one surface pair. Total: every pair yields exactly one verdict, and
/// every difference yields at least one closed reason.
pub fn compare(before: &CandidateSurface, after: &CandidateSurface) -> CompatibilityReport {
    let mut findings = Vec::new();
    let exports = before
        .entries
        .keys()
        .chain(after.entries.keys())
        .collect::<BTreeSet<_>>();
    for export in exports {
        match (before.entries.get(export), after.entries.get(export)) {
            (None, Some(_)) => findings.push(Finding {
                subject: export.clone(),
                reason: Reason::ExportAdded,
                detail: None,
            }),
            (Some(_), None) => findings.push(Finding {
                subject: export.clone(),
                reason: Reason::ExportRemoved,
                detail: None,
            }),
            (Some(left), Some(right)) => compare_entry(export, left, right, &mut findings),
            (None, None) => unreachable!("the key came from one of the two maps"),
        }
    }

    let instances = before
        .instances
        .keys()
        .chain(after.instances.keys())
        .collect::<BTreeSet<_>>();
    for term in instances {
        match (before.instances.get(term), after.instances.get(term)) {
            (None, Some(_)) => findings.push(Finding {
                subject: term.clone(),
                reason: Reason::ReachableInstanceAdded,
                detail: None,
            }),
            (Some(_), None) => findings.push(Finding {
                subject: term.clone(),
                reason: Reason::ReachableInstanceRemoved,
                detail: None,
            }),
            (Some(left), Some(right)) => compare_instance(term, left, right, &mut findings),
            (None, None) => unreachable!("the key came from one of the two maps"),
        }
    }

    let verdict = findings
        .iter()
        .map(|finding| finding.reason.verdict())
        .max()
        .unwrap_or(Verdict::Unchanged);
    let mut report = CompatibilityReport {
        verdict,
        findings,
        before_digest: before.digest.clone(),
        after_digest: after.digest.clone(),
        digest: String::new(),
    };
    let preimage = json!({
        "before": report.before_digest,
        "after": report.after_digest,
        "verdict": report.verdict.text(),
        "findings": report
            .findings
            .iter()
            .map(|finding| json!([finding.subject, finding.reason.text(), finding.detail]))
            .collect::<Vec<_>>(),
    });
    report.digest = digest(COMPARISON_DOMAIN, preimage.to_string().as_bytes());
    report
}

/// Entry-level reasons are about the signature *shape*: the position kind and
/// the canonical term. What a term denotes is compared once, on the instance
/// itself, so a field added to a reachable record is reported where it
/// happened instead of again on every position that mentions it.
fn compare_entry(
    export: &str,
    before: &SurfaceEntry,
    after: &SurfaceEntry,
    findings: &mut Vec<Finding>,
) {
    if before.effects != after.effects {
        findings.push(Finding {
            subject: export.to_owned(),
            reason: Reason::EffectsChanged,
            detail: Some(format!("{:?} -> {:?}", before.effects, after.effects)),
        });
    }
    if before.parameters.len() != after.parameters.len() {
        findings.push(Finding {
            subject: export.to_owned(),
            reason: Reason::ParameterCountChanged,
            detail: Some(format!(
                "{} -> {}",
                before.parameters.len(),
                after.parameters.len()
            )),
        });
    }
    for (left, right) in before.parameters.iter().zip(&after.parameters) {
        let subject = format!("{export}#{}", left.index);
        if left.ownership != right.ownership {
            findings.push(Finding {
                subject: subject.clone(),
                reason: Reason::ParameterOwnershipChanged,
                detail: Some(format!("{} -> {}", left.ownership, right.ownership)),
            });
        }
        if left.value.kind != right.value.kind || left.value.term != right.value.term {
            findings.push(Finding {
                subject: subject.clone(),
                reason: Reason::ParameterTypeChanged,
                detail: Some(format!("{} -> {}", left.value.term, right.value.term)),
            });
        }
        if left.value.kind == "data" && right.value.kind == "data" {
            compare_data_positions(&subject, &left.value.term, &right.value.term, findings);
        }
    }
    if before.result.kind != after.result.kind || before.result.term != after.result.term {
        findings.push(Finding {
            subject: export.to_owned(),
            reason: Reason::ResultTypeChanged,
            detail: Some(format!("{} -> {}", before.result.term, after.result.term)),
        });
    }
    if before.result.kind == "data" && after.result.kind == "data" {
        let subject = format!("{export}#result");
        compare_data_positions(&subject, &before.result.term, &after.result.term, findings);
    }
}

/// Walk two canonical data terms at the same signature position and report
/// the exact nested type-argument slot each structural difference lives at,
/// rather than only the coarse fact that the position's term changed (already
/// reported by [`Reason::ParameterTypeChanged`] / [`Reason::ResultTypeChanged`]
/// at the caller). Terms that parse identically produce no finding here.
///
/// Only proceeds when both terms are generic instances: a plain scalar or
/// `Bytes` position that changed has no argument shape to descend into, and
/// the caller's own coarse finding already names the exact before/after term.
///
/// Both terms are this module's own canonical grammar output, so parsing
/// either back is expected to succeed; a parse failure fails closed by simply
/// adding no finer detail, and the caller's own coarse finding still stands.
fn compare_data_positions(
    path: &str,
    before_term: &str,
    after_term: &str,
    findings: &mut Vec<Finding>,
) {
    if before_term == after_term {
        return;
    }
    let (Ok(before), Ok(after)) = (
        grammar::parse_term(before_term),
        grammar::parse_term(after_term),
    ) else {
        return;
    };
    if !matches!(before, GrammarTerm::Instance { .. })
        || !matches!(after, GrammarTerm::Instance { .. })
    {
        return;
    }
    walk_terms(path, &before, &after, findings);
}

/// Recursive structural diff of two parsed grammar terms at one path.
///
/// Two instances of the same declaration are walked argument by argument, so
/// a permutation of two arguments yields one finding per swapped slot, a
/// single substitution yields exactly one finding, and a nested instance
/// argument recurses instead of being reported as one opaque blob. Two
/// instances of different declarations, or the same declaration at a
/// different declared arity, are reported once at this path rather than
/// walked further: there is no shared argument shape to align.
fn walk_terms(path: &str, before: &GrammarTerm, after: &GrammarTerm, findings: &mut Vec<Finding>) {
    if before == after {
        return;
    }
    if let (
        GrammarTerm::Instance {
            declaration: before_declaration,
            arguments: before_arguments,
        },
        GrammarTerm::Instance {
            declaration: after_declaration,
            arguments: after_arguments,
        },
    ) = (before, after)
    {
        if before_declaration == after_declaration
            && before_arguments.len() == after_arguments.len()
        {
            for (index, (left, right)) in before_arguments.iter().zip(after_arguments).enumerate() {
                if left == right {
                    continue;
                }
                let child = format!("{path}/arg{index}");
                let same_declaration = matches!(
                    (left, right),
                    (
                        GrammarTerm::Instance { declaration: l, .. },
                        GrammarTerm::Instance { declaration: r, .. },
                    ) if l == r
                );
                if same_declaration {
                    walk_terms(&child, left, right, findings);
                } else {
                    findings.push(Finding {
                        subject: child,
                        reason: Reason::InstanceArgumentsChanged,
                        detail: Some(format!(
                            "parameter {before_declaration}#{index}: {} -> {}",
                            left.render(),
                            right.render()
                        )),
                    });
                }
            }
            return;
        }
        findings.push(Finding {
            subject: path.to_owned(),
            reason: Reason::InstanceTemplateChanged,
            detail: Some(format!(
                "{before_declaration}<arity {}> -> {after_declaration}<arity {}>",
                before_arguments.len(),
                after_arguments.len()
            )),
        });
        return;
    }
    findings.push(Finding {
        subject: path.to_owned(),
        reason: Reason::InstanceArgumentsChanged,
        detail: Some(format!("{} -> {}", before.render(), after.render())),
    });
}

fn compare_instance(
    term: &str,
    before: &InstanceFacts,
    after: &InstanceFacts,
    findings: &mut Vec<Finding>,
) {
    // No template-identity or ordered-argument check runs here: `term` is the
    // map key both `before` and `after` were looked up by, and a canonical
    // term is exactly `declaration<argument, ...>` rendered recursively (see
    // `public_generic_type::GrammarTerm::write`). Two facts sharing one term
    // key are therefore already proven to share one declaration identity, one
    // declared arity, and byte-identical ordered argument terms - nothing
    // about the template or arguments *can* differ here. That comparison
    // instead happens where a term mismatch is genuinely possible: at the
    // entry parameter/result position and its nested argument slots, in
    // `compare_data_positions` / `walk_terms` below, which is where
    // `Reason::InstanceTemplateChanged` and `Reason::InstanceArgumentsChanged`
    // are actually produced.
    let fields = |facts: &InstanceFacts| {
        facts
            .fields
            .iter()
            .map(|field| (field.index, field.id.clone(), field.digest.clone()))
            .collect::<Vec<_>>()
    };
    if fields(before) != fields(after) {
        findings.push(Finding {
            subject: term.to_owned(),
            reason: Reason::InstanceFieldsChanged,
            detail: None,
        });
    }
    if before.owned_leaves != after.owned_leaves {
        findings.push(Finding {
            subject: term.to_owned(),
            reason: Reason::InstanceOwnedLeavesChanged,
            detail: Some(format!(
                "{} -> {} owned leaves",
                before.owned_leaves.len(),
                after.owned_leaves.len()
            )),
        });
    }
}

/// Independently recompute the comparison of two surfaces and require the
/// submitted bytes to equal it exactly.
pub fn verify_comparison(
    before: &CandidateSurface,
    after: &CandidateSurface,
    submitted: &str,
) -> Result<(), Diagnostic> {
    if compare(before, after).canonical_json()? == submitted {
        return Ok(());
    }
    Err(Diagnostic::io(
        COMPARISON_REPLAY_MISMATCH,
        format!(
            "{COMPATIBILITY_SCHEMA} replay mismatch: submitted bytes are not the independently \
             recomputed comparison"
        ),
    ))
}

#[cfg(test)]
mod tests;
