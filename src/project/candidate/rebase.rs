//! Stable-ID conflict selection followed by full canonical source replay.
//! Fingerprints are conservative conflict facts, never compatibility proofs.
use super::{
    intent, parse_revision, wire, ProjectCandidate, ProjectRevision, SemanticChange, MAX_CHANGES,
};
use crate::ast::{ExprKind, ParamMode, Type};
use crate::diagnostic::Diagnostic;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub const PROJECT_CANDIDATE_REBASE_SCHEMA: &str = "semaprax.project-candidate-rebase.v1";
const MAX_REPORT_BYTES: usize = 1024 * 1024;
const MAX_FINGERPRINT_BYTES: usize = 16 * 1024 * 1024;
#[path = "rebase_normalize.rs"]
mod normalize;
pub(super) fn normalize_nominal_descriptor(value: &mut Value) {
    normalize::descriptor(value);
}

fn normalized_descriptor(mut value: Option<Value>) -> Option<Value> {
    if let Some(value) = &mut value {
        normalize_nominal_descriptor(value);
    }
    value
}

/// Fully revalidated candidate with a bound ancestry report; no source authority.
pub struct ProjectCandidateRebase {
    candidate: ProjectCandidate,
    json: String,
}
impl ProjectCandidateRebase {
    pub fn candidate(&self) -> &ProjectCandidate {
        &self.candidate
    }
    pub fn into_candidate(self) -> ProjectCandidate {
        self.candidate
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
}
impl ProjectCandidate {
    /// Replay intentions on an independently admitted base. The result's diff
    /// is anchored to that new base rather than to the abandoned original base.
    pub fn rebase(
        &self,
        expected_candidate: &str,
        new_base: Arc<ProjectRevision>,
        expected_new_base: &str,
    ) -> Result<ProjectCandidateRebase, Vec<Diagnostic>> {
        expected(expected_candidate, self.candidate_digest())?;
        expected(expected_new_base, new_base.project_revision())?;
        same_manifest(&self.base, &new_base)?;
        let classifications = classify(&self.base, &new_base, &self.changes)?;
        let mut result =
            ProjectCandidate::open(Arc::clone(&new_base), new_base.project_revision())?;
        let mut original =
            ProjectCandidate::open(Arc::clone(&self.base), self.base.project_revision())?;
        for change in &self.changes {
            result = apply_rebound(result, change, original.revision())?;
            original = original.apply(original.candidate_digest(), change)?;
        }
        finish("rebase", self, None, &new_base, 0, classifications, result)
    }
    /// Merge histories sharing an original base and preserve that base for the
    /// final diff. An exact common prefix is replayed once; right suffix first.
    pub fn merge(
        &self,
        expected_candidate: &str,
        other: &Self,
        expected_other: &str,
    ) -> Result<ProjectCandidateRebase, Vec<Diagnostic>> {
        expected(expected_candidate, self.candidate_digest())?;
        expected(expected_other, other.candidate_digest())?;
        if self.base.project_revision() != other.base.project_revision() {
            return Err(conflict(
                "candidate merge requires the same original base revision",
            ));
        }
        same_manifest(&self.base, &other.base)?;
        let prefix = self
            .changes
            .iter()
            .zip(&other.changes)
            .take_while(|(left, right)| left.to_json() == right.to_json())
            .count();
        let remaining = &self.changes[prefix..];
        if other.changes.len().saturating_add(remaining.len()) > MAX_CHANGES {
            return Err(capacity("merged candidate exceeds its intention bound"));
        }
        let renamed_targets = remaining
            .iter()
            .filter(|change| change.intent["kind"] == "rename_declaration")
            .filter_map(|change| change.intent["target"].as_str())
            .collect::<BTreeSet<_>>();
        let renamed_types = if renamed_targets.is_empty() {
            BTreeMap::new()
        } else {
            type_fingerprints(self.revision(), &renamed_targets)?
        };
        // Net-zero histories must not conceal competing signature intentions.
        for left in remaining {
            for right in &other.changes[prefix..] {
                if left.intent["target"] == right.intent["target"]
                    && left.intent["kind"] == "rename_declaration"
                    && right.intent["kind"] == "rename_declaration"
                    && left.intent["target"]
                        .as_str()
                        .is_some_and(|target| renamed_types.contains_key(target))
                {
                    return Err(conflict(
                        "competing type display rename intentions target the same stable ID",
                    ));
                }
                if left.intent["target"] == right.intent["target"]
                    && left.intent["kind"] == "change_function_signature"
                    && right.intent["kind"] == "change_function_signature"
                {
                    return Err(conflict(
                        "competing signature intentions target the same stable ID",
                    ));
                }
                if left.intent["target"] == right.intent["target"]
                    && left.intent["kind"] == "move_declaration"
                    && right.intent["kind"] == "move_declaration"
                {
                    return Err(conflict(
                        "competing move intentions target the same stable ID",
                    ));
                }
            }
        }
        let mut common =
            ProjectCandidate::open(Arc::clone(&self.base), self.base.project_revision())?;
        for change in &self.changes[..prefix] {
            common = common.apply(common.candidate_digest(), change)?;
        }
        let classifications = classify(common.revision(), other.revision(), remaining)?;
        let mut result = ProjectCandidate::replay(
            Arc::clone(&other.base),
            other.base.project_revision(),
            &other.changes,
            other.to_json().as_bytes(),
        )?;
        for change in remaining {
            result = apply_rebound(result, change, common.revision())?;
            common = common.apply(common.candidate_digest(), change)?;
        }
        finish(
            "merge",
            self,
            Some(other),
            other.revision(),
            prefix,
            classifications,
            result,
        )
    }
}
fn apply_rebound(
    candidate: ProjectCandidate,
    change: &SemanticChange,
    original_revision: &ProjectRevision,
) -> Result<ProjectCandidate, Vec<Diagnostic>> {
    if change.intent["kind"] == "rename_declaration" {
        let target = change.intent["target"]
            .as_str()
            .ok_or_else(|| grammar("declaration rename lacks a target"))?;
        let targets = BTreeSet::from([target]);
        let before = type_fingerprints(original_revision, &targets)?;
        if let Some(before) = before.get(target) {
            let after = type_fingerprints(candidate.revision(), &targets)?;
            compare_type_rename(before, after.get(target))?;
        }
    }
    if change.intent["kind"] == "repair_diagnostic" {
        return Err(super::diagnostic_intent::rebase_conflict());
    }
    if change.intent["kind"] == "implement_interface" {
        compare_interface_binding(
            super::interface::rebase_fingerprint(original_revision, &change.intent)?,
            super::interface::rebase_fingerprint(candidate.revision(), &change.intent)?,
        )?;
    }
    if change.intent["kind"] == "replace_contract_expression" {
        let target = change.intent["target"]
            .as_str()
            .ok_or_else(|| grammar("contract replacement target is absent"))?;
        let expression = change.intent["expression_id"]
            .as_str()
            .ok_or_else(|| grammar("contract replacement selector is absent"))?;
        let mut dependencies =
            super::expression::contract_call_targets(original_revision, target, expression)?;
        dependencies.extend(called_intent_targets(&change.intent));
        if !dependencies.is_empty() {
            let before = fingerprints(original_revision)?;
            let after = fingerprints(candidate.revision())?;
            for dependency in dependencies {
                if !before.contains_key(&dependency)
                    && unchanged_builtin(original_revision, candidate.revision(), &dependency)?
                {
                    continue;
                }
                let left = before.get(&dependency).ok_or_else(|| {
                    conflict("contract call dependency lacks authenticated source facts")
                })?;
                let right = after
                    .get(&dependency)
                    .ok_or_else(|| conflict("contract call dependency was concurrently removed"))?;
                if left.signature != right.signature
                    || left.effects != right.effects
                    || left.contracts != right.contracts
                {
                    return Err(conflict("contract call dependency signature, effects or contracts changed concurrently"));
                }
            }
        }
    }
    // Check each original/rebased intermediate revision, not only the two
    // history roots. Earlier intentions may legitimately change a shape that
    // a later aggregate constructor references.
    for target in constructor_intent_targets(&change.intent, &["builtin_call"]) {
        if !unchanged_builtin(original_revision, candidate.revision(), &target)? {
            return Err(conflict(
                "candidate builtin target or compiler owner facts conflict with source identities",
            ));
        }
    }
    for target in constructor_intent_targets(&change.intent, &["record", "variant", "update"]) {
        let before = normalized_descriptor(intent::aggregate_dependency_fingerprint(
            original_revision,
            &target,
        )?);
        let after = normalized_descriptor(intent::aggregate_dependency_fingerprint(
            candidate.revision(),
            &target,
        )?);
        if before.is_none() || before != after {
            return Err(conflict(
                "candidate aggregate target or checked field shape changed concurrently",
            ));
        }
    }
    for target in constructor_intent_targets(&change.intent, &["project"]) {
        let before = normalized_descriptor(intent::aggregate_projection_dependency_fingerprint(
            original_revision,
            &target,
        )?);
        let after = normalized_descriptor(intent::aggregate_projection_dependency_fingerprint(
            candidate.revision(),
            &target,
        )?);
        if before.is_none() || before != after {
            return Err(conflict(
                "candidate field projection target or checked owner shape changed concurrently",
            ));
        }
    }
    for target in constructor_intent_targets(&change.intent, &["field_place"]) {
        let before = normalized_descriptor(intent::field_place_dependency_fingerprint(
            original_revision,
            &target,
        )?);
        let after = normalized_descriptor(intent::field_place_dependency_fingerprint(
            candidate.revision(),
            &target,
        )?);
        if before.is_none() || before != after {
            return Err(conflict(
                "candidate field place target or checked nominal owner shape changed concurrently",
            ));
        }
    }
    for target in constructor_intent_targets(&change.intent, &["match"]) {
        let before = normalized_descriptor(intent::aggregate_match_dependency_fingerprint(
            original_revision,
            &target,
        )?);
        let after = normalized_descriptor(intent::aggregate_match_dependency_fingerprint(
            candidate.revision(),
            &target,
        )?);
        if before.is_none() || before != after {
            return Err(conflict(
                "candidate match target or checked case inventory changed concurrently",
            ));
        }
    }
    for target in constructor_intent_targets(&change.intent, &["nominal"]) {
        let before = normalized_descriptor(intent::nominal_type_dependency_fingerprint(
            original_revision,
            &target,
        )?);
        let after = normalized_descriptor(intent::nominal_type_dependency_fingerprint(
            candidate.revision(),
            &target,
        )?);
        if before.is_none() || before != after {
            return Err(conflict(
                "candidate nominal declaration type or checked member inventory changed concurrently",
            ));
        }
    }
    let mapped;
    let intent = if change.intent["kind"] == "replace_expression" {
        mapped = super::expression::rebase_intent(
            original_revision,
            candidate.revision(),
            &change.intent,
        )?;
        &mapped
    } else if change.intent["kind"] == "replace_contract_expression" {
        mapped = super::expression::rebase_contract_intent(
            original_revision,
            candidate.revision(),
            &change.intent,
        )?;
        &mapped
    } else if change.intent["kind"] == "extract_function" {
        mapped = super::extraction::rebase_intent(
            original_revision,
            candidate.revision(),
            &change.intent,
        )?;
        &mapped
    } else {
        &change.intent
    };
    let rebound = SemanticChange::new(candidate.revision().project_revision(), intent)?;
    candidate.apply(candidate.candidate_digest(), &rebound)
}
struct Fingerprint {
    display: String,
    signature: String,
    body: String,
    contracts: String,
    effects: String,
    location: String,
}

/// Pending selectors have no replacement intention to classify. Reuse the
/// source conflict facts directly before any revision-local selector remapping.
/// The returned tuple records concurrent body and contract changes separately.
pub(super) fn pending_draft_conflicts(
    before: &ProjectRevision,
    after: &ProjectRevision,
    body_targets: &BTreeSet<&str>,
    contract_targets: &BTreeSet<&str>,
    contract_callees: &BTreeSet<String>,
) -> Result<BTreeMap<String, (bool, bool)>, Vec<Diagnostic>> {
    let old = fingerprints(before)?;
    let new = fingerprints(after)?;
    let reject = |message| vec![Diagnostic::io("SPX-G345", message)];
    let mut changes = BTreeMap::new();
    for target in body_targets.union(contract_targets) {
        let left = old
            .get(*target)
            .ok_or_else(|| reject("pending draft owner lacks explicit source facts"))?;
        let right = new
            .get(*target)
            .ok_or_else(|| reject("pending draft owner was removed or lost explicit identity"))?;
        let body_changed = left.body != right.body;
        let contracts_changed = left.contracts != right.contracts;
        if left.signature != right.signature
            || left.effects != right.effects
            || (body_targets.contains(target) && body_changed)
            || (contract_targets.contains(target) && contracts_changed)
        {
            return Err(reject("pending draft region conflicts with concurrent signature, effects or selected-region changes"));
        }
        changes.insert((*target).to_owned(), (body_changed, contracts_changed));
    }
    for target in contract_callees {
        if !old.contains_key(target) && unchanged_builtin(before, after, target)? {
            continue;
        }
        let left = old
            .get(target)
            .ok_or_else(|| reject("pending contract callee lacks authenticated source facts"))?;
        let right = new
            .get(target)
            .ok_or_else(|| reject("pending contract callee was concurrently removed"))?;
        if left.signature != right.signature
            || left.effects != right.effects
            || left.contracts != right.contracts
        {
            return Err(reject(
                "pending contract callee signature, effects or contracts changed concurrently",
            ));
        }
    }
    Ok(changes)
}

// This is not a source-function fallback: both revisions must authenticate the
// exact compiler owner and prove absence from the authored identity namespace.
// A real authored function whose ID resembles a builtin retains all ordinary
// source signature/effect/contract checks above.
fn unchanged_builtin(
    before: &ProjectRevision,
    after: &ProjectRevision,
    target: &str,
) -> Result<bool, Vec<Diagnostic>> {
    let old = intent::builtin_dependency_fingerprint(before, target)?;
    let new = intent::builtin_dependency_fingerprint(after, target)?;
    Ok(old.is_some() && old == new)
}

fn fingerprints(
    revision: &ProjectRevision,
) -> Result<BTreeMap<String, Fingerprint>, Vec<Diagnostic>> {
    let mut programs = normalize::programs(revision, parse_revision(revision)?)?;
    let mut result = BTreeMap::new();
    let mut nodes = 0usize;
    for program in &mut programs {
        let bindings = intent::call_bindings(program)?;
        // Normalize local names/import aliases through persistent declaration
        // bindings. Tokens are used only for fingerprints, never source replay.
        intent::walk_program(program, &mut nodes, &mut |expression| {
            if let ExprKind::Call { name, .. } = &mut expression.kind {
                if let Some(id) = bindings.get(name) {
                    *name = format!(
                        "spx_stable_{}",
                        &wire::digest(b"semaprax.rebase.call.v1\0", id.as_bytes())[7..]
                    );
                }
            }
            Ok(())
        })?;
        for function in &program.functions {
            if !function.explicit_id {
                continue;
            }
            let (body, overflow) = crate::bounded_output::with_limit(MAX_FINGERPRINT_BYTES, || {
                crate::format::expr(&function.body, 0)
            });
            if overflow {
                return Err(capacity(
                    "candidate rebase body fingerprint exceeds its bound",
                ));
            }
            let (requires, overflow) =
                crate::bounded_output::with_limit(MAX_FINGERPRINT_BYTES, || {
                    function
                        .requires
                        .iter()
                        .map(|expression| crate::format::expr(expression, 0))
                        .collect::<Vec<_>>()
                });
            if overflow {
                return Err(capacity(
                    "candidate rebase contract fingerprint exceeds its bound",
                ));
            }
            let (ensures, overflow) =
                crate::bounded_output::with_limit(MAX_FINGERPRINT_BYTES, || {
                    function
                        .ensures
                        .iter()
                        .map(|expression| crate::format::expr(expression, 0))
                        .collect::<Vec<_>>()
                });
            if overflow {
                return Err(capacity(
                    "candidate rebase contract fingerprint exceeds its bound",
                ));
            }
            let mut signature = json!({"parameters":function.params.iter().map(|param| json!({"name":param.name,"type":param.ty.to_string(),"mode":match param.mode {ParamMode::Value=>"value",ParamMode::Own=>"own",ParamMode::Borrow=>"borrow",ParamMode::Shared=>"shared"}})).collect::<Vec<_>>(),"return":function.return_type.to_string(), "generic_parameter_count":function.type_parameters.len()});
            if function
                .params
                .iter()
                .any(|param| matches!(param.ty, Type::Named { .. }))
                || matches!(function.return_type, Type::Named { .. })
            {
                // A display type or import alias can keep its spelling while
                // resolving to a different nominal identity on the new base.
                // Bind checked identities as well; scalar fingerprints keep
                // their historical representation.
                signature["resolved_type_identity"] =
                    nominal_signature(revision, &program.path, &function.stable_id)?;
            }
            let fact = Fingerprint {
                display: function.name.clone(),
                signature: hash_value(signature)?,
                body: wire::digest(b"semaprax.rebase.body.v1\0", body.as_bytes()),
                contracts: hash_value(json!({"requires":requires,"ensures":ensures}))?,
                effects: hash_value(json!({"effects":function.effects,"permits":program.permits}))?,
                location: hash_value(json!({"path":program.path,"module":program.module}))?,
            };
            if result.insert(function.stable_id.clone(), fact).is_some() {
                return Err(grammar(
                    "candidate rebase has ambiguous explicit identities",
                ));
            }
        }
    }
    Ok(result)
}

fn nominal_signature(
    revision: &ProjectRevision,
    path: &str,
    id: &str,
) -> Result<Value, Vec<Diagnostic>> {
    let module = revision
        .semantic
        .image_modules()
        .iter()
        .find(|module| module.path() == path)
        .ok_or_else(|| grammar("nominal rebase signature lacks its retained source module"))?;
    let (params, result) = if let Some(function) = module
        .functions()
        .iter()
        .find(|function| function.id.as_str() == id)
    {
        (&function.params, &function.return_type)
    } else if let Some(function) = module
        .function_templates()
        .iter()
        .find(|function| function.id.as_str() == id)
    {
        (&function.params, &function.return_type)
    } else {
        return Err(grammar(
            "nominal rebase signature lacks its retained checked function",
        ));
    };
    Ok(json!({
        "parameters":params.iter().map(|param| param.ty.identity_key()).collect::<Vec<_>>(),
        "return":result.identity_key(),
        "evidence_owner":"retained_checked_source_module_HIR"
    }))
}

// Record-shape conflicts are independent of function display/body changes.
// These conservative source facts select replay; they are not layout evidence.
fn record_fingerprints(
    revision: &ProjectRevision,
) -> Result<BTreeMap<String, String>, Vec<Diagnostic>> {
    let programs = parse_revision(revision)?;
    let mut result = BTreeMap::new();
    for program in &programs {
        for declaration in &program.types {
            if !declaration.explicit_id {
                continue;
            }
            if let crate::ast::TypeDeclarationKind::Record { fields } = &declaration.kind {
                let fingerprint = hash_value(json!({
                    "path":program.path,"module":program.module,"name":declaration.name,
                    "parameters":declaration.type_parameters.iter().map(|p| &p.name).collect::<Vec<_>>(),
                    "fields":fields.iter().map(|f| json!({"id":f.stable_id,"explicit":f.explicit_id,"name":f.name,"type":f.ty.to_string()})).collect::<Vec<_>>(),
                }))?;
                if result
                    .insert(declaration.stable_id.clone(), fingerprint)
                    .is_some()
                {
                    return Err(grammar("candidate rebase has ambiguous record identities"));
                }
            }
        }
    }
    Ok(result)
}
struct TypeFingerprint {
    display: String,
    kind: &'static str,
    shape: String,
    location: String,
}

// Separate from legacy callable hashes: these bounded source facts select
// replay, not Copy eligibility, checked layout, or semantic compatibility.
fn type_fingerprints(
    revision: &ProjectRevision,
    targets: &BTreeSet<&str>,
) -> Result<BTreeMap<String, TypeFingerprint>, Vec<Diagnostic>> {
    let mut result = BTreeMap::new();
    let mut items = 0usize;
    for program in parse_revision(revision)? {
        for declaration in &program.types {
            if !declaration.explicit_id {
                continue;
            }
            let mut selected = Vec::new();
            if targets.contains(declaration.stable_id.as_str()) {
                selected.push((
                    declaration.stable_id.as_str(),
                    declaration.name.as_str(),
                    None,
                ));
            }
            let (kind, members) = match &declaration.kind {
                crate::ast::TypeDeclarationKind::Record { fields } => {
                    for field in fields {
                        if field.explicit_id && targets.contains(field.stable_id.as_str()) {
                            selected.push((
                                field.stable_id.as_str(),
                                field.name.as_str(),
                                Some("record_field"),
                            ));
                        }
                    }
                    ("record", fields.len())
                }
                crate::ast::TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        if !case.explicit_id {
                            continue;
                        }
                        if targets.contains(case.stable_id.as_str()) {
                            selected.push((
                                case.stable_id.as_str(),
                                case.name.as_str(),
                                Some("variant_case"),
                            ));
                        }
                        for field in &case.fields {
                            if field.explicit_id && targets.contains(field.stable_id.as_str()) {
                                selected.push((
                                    field.stable_id.as_str(),
                                    field.name.as_str(),
                                    Some("variant_field"),
                                ));
                            }
                        }
                    }
                    (
                        "variant",
                        cases
                            .iter()
                            .try_fold(cases.len(), |count, case| {
                                count.checked_add(case.fields.len())
                            })
                            .ok_or_else(|| {
                                capacity("type rename fingerprint inventory overflow")
                            })?,
                    )
                }
                _ => continue,
            };
            if selected.is_empty() {
                continue;
            }
            items = items
                .checked_add(members)
                .and_then(|count| count.checked_add(declaration.type_parameters.len()))
                .and_then(|count| count.checked_add(program.types.len()))
                .and_then(|count| count.checked_add(program.module_uses.len()))
                .and_then(|count| count.checked_add(1))
                .ok_or_else(|| capacity("type rename fingerprint inventory overflow"))?;
            if items > 65_536 {
                return Err(capacity(
                    "type rename fingerprint inventory exceeds its bound",
                ));
            }
            let (shape, overflow) = crate::bounded_output::with_limit(
                MAX_FINGERPRINT_BYTES,
                || {
                    let fields = |fields: &[crate::ast::FieldDeclaration]| {
                        fields.iter().map(|field| json!({
                    "id":field.stable_id,"explicit":field.explicit_id,"name":field.name,"type":field.ty.to_string()
                })).collect::<Vec<_>>()
                    };
                    let members = match &declaration.kind {
                    crate::ast::TypeDeclarationKind::Record { fields: values } => json!(fields(values)),
                    crate::ast::TypeDeclarationKind::Variant { cases } => json!(cases.iter().map(|case| json!({
                        "id":case.stable_id,"explicit":case.explicit_id,"name":case.name,"fields":fields(&case.fields)
                    })).collect::<Vec<_>>()),
                    _ => unreachable!(),
                };
                    hash_value(json!({
                        "kind":kind,"parameters":declaration.type_parameters.iter().map(|parameter| &parameter.name).collect::<Vec<_>>(),
                        "members":members,
                        "local_type_bindings":program.types.iter().map(|item| json!({"name":item.name,"id":item.stable_id})).collect::<Vec<_>>(),
                        "imported_type_bindings":program.module_uses.iter().filter(|item| item.kind == crate::ast::ModuleUseKind::Type).map(|item| json!({"name":item.alias,"id":item.persistent_id,"module":item.target_module})).collect::<Vec<_>>()
                    }))
                },
            );
            if overflow {
                return Err(capacity("type rename fingerprint render exceeds its bound"));
            }
            let shape = shape?;
            let location = hash_value(json!({"path":program.path,"module":program.module}))?;
            for (id, display, member_kind) in selected {
                // Member facts bind their complete owner, including sibling
                // names/identities and payload order. This also distinguishes
                // a stable member moved to a different nominal owner.
                let selected_shape = if member_kind.is_some() {
                    hash_value(
                        json!({"owner":declaration.stable_id,"owner_name":declaration.name,"shape":shape}),
                    )?
                } else {
                    shape.clone()
                };
                let fact = TypeFingerprint {
                    display: display.to_owned(),
                    kind: member_kind.unwrap_or(kind),
                    shape: selected_shape,
                    location: location.clone(),
                };
                if result.insert(id.to_owned(), fact).is_some() {
                    return Err(grammar("type rename fingerprint identities are ambiguous"));
                }
            }
        }
    }
    Ok(result)
}

fn compare_type_rename(
    before: &TypeFingerprint,
    after: Option<&TypeFingerprint>,
) -> Result<(), Vec<Diagnostic>> {
    let after = after.ok_or_else(|| {
        conflict("type rename target was deleted or lost its source record or variant identity")
    })?;
    if before.display != after.display {
        return Err(conflict(
            "concurrent display renames target the same stable type ID",
        ));
    }
    if before.kind != after.kind || before.shape != after.shape || before.location != after.location
    {
        return Err(conflict(
            "type rename conflicts with concurrent type shape, binding or origin changes",
        ));
    }
    Ok(())
}

fn compare_interface_binding(
    before: Option<Value>,
    after: Option<Value>,
) -> Result<(), Vec<Diagnostic>> {
    let before = before.ok_or_else(|| {
        conflict("interface implementation dependencies are absent from the selected history base")
    })?;
    if after.as_ref() != Some(&before) {
        return Err(conflict(
            "interface receiver, protocol or member binding changed concurrently",
        ));
    }
    Ok(())
}

fn classify(
    old: &ProjectRevision,
    new: &ProjectRevision,
    changes: &[SemanticChange],
) -> Result<Vec<Value>, Vec<Diagnostic>> {
    let old_facts = fingerprints(old)?;
    let new_facts = fingerprints(new)?;
    let renamed_targets = changes
        .iter()
        .filter(|change| change.intent["kind"] == "rename_declaration")
        .filter_map(|change| change.intent["target"].as_str())
        .collect::<BTreeSet<_>>();
    let (old_types, new_types) = if renamed_targets.is_empty() {
        (BTreeMap::new(), BTreeMap::new())
    } else {
        (
            type_fingerprints(old, &renamed_targets)?,
            type_fingerprints(new, &renamed_targets)?,
        )
    };
    let has_record_changes = changes
        .iter()
        .any(|change| change.intent["kind"] == "add_record_field");
    let (old_records, new_records) = if has_record_changes {
        (record_fingerprints(old)?, record_fingerprints(new)?)
    } else {
        (BTreeMap::new(), BTreeMap::new())
    };
    let new_graph: Value = serde_json::from_str(new.semantic_graph())
        .map_err(|_| grammar("candidate rebase graph is invalid"))?;
    let mut new_ids = new_graph["declarations"]
        .as_array()
        .ok_or_else(|| grammar("candidate rebase graph lacks declarations"))?
        .iter()
        .filter_map(|declaration| declaration["id"].as_str())
        .collect::<BTreeSet<_>>();
    let new_programs = parse_revision(new)?;
    let source_ids = super::interface::identities(&new_programs)?;
    new_ids.extend(source_ids.iter().map(String::as_str));
    for program in &new_programs {
        new_ids.extend(
            program
                .module_uses
                .iter()
                .map(|binding| binding.persistent_id.as_str()),
        );
    }
    let mut introduced = BTreeSet::new();
    let mut report = Vec::new();
    for change in changes {
        let target = change.intent["target"]
            .as_str()
            .ok_or_else(|| grammar("candidate rebase intent lacks a target"))?;
        let kind = change.intent["kind"]
            .as_str()
            .ok_or_else(|| grammar("candidate rebase intent lacks a kind"))?;
        let additions = added_intent_ids(&change.intent)?;
        if additions
            .iter()
            .any(|id| new_ids.contains(id) || introduced.contains(id))
        {
            return Err(conflict(
                "candidate addition identity exists in the destination or another intention",
            ));
        }
        let (signature_changed, body_changed, contracts_changed, display_changed, effects_changed) =
            if kind == "implement_interface" {
                compare_interface_binding(
                    super::interface::rebase_fingerprint(old, &change.intent)?,
                    super::interface::rebase_fingerprint(new, &change.intent)?,
                )?;
                (false, false, false, false, false)
            } else if kind == "add_record_field" && !introduced.contains(target) {
                let before = old_records.get(target).ok_or_else(|| {
                    conflict("record addition target is absent from its original base")
                })?;
                let after = new_records.get(target).ok_or_else(|| {
                    conflict("record addition target was deleted or changed declaration kind")
                })?;
                if before != after {
                    return Err(conflict(
                        "record field addition conflicts with concurrent record shape changes",
                    ));
                }
                (false, false, false, false, false)
            } else if kind == "rename_declaration" && old_types.contains_key(target) {
                compare_type_rename(&old_types[target], new_types.get(target))?;
                (false, false, false, false, false)
            } else if let Some(before) = old_facts.get(target) {
                let after = new_facts.get(target).ok_or_else(|| {
                    conflict("candidate rebase target was deleted or lost explicit identity")
                })?;
                (
                    before.signature != after.signature,
                    before.body != after.body,
                    before.contracts != after.contracts,
                    before.display != after.display,
                    before.effects != after.effects,
                )
            } else if introduced.contains(target) {
                // Earlier replay in this exact history creates the target.
                // Destination-ID collisions were rejected before that replay.
                (false, false, false, false, false)
            } else {
                return Err(conflict(
                    "candidate rebase target is absent from its original base",
                ));
            };
        match kind {
            "repair_diagnostic" => return Err(super::diagnostic_intent::rebase_conflict()),
            "rename_declaration" if display_changed => return Err(conflict("concurrent display renames target the same stable ID")),
            "replace_function_body" | "replace_expression" | "extract_function" if signature_changed || body_changed || effects_changed => return Err(conflict("body replacement conflicts with concurrent target body, signature or effects")),
            "change_function_signature" if signature_changed || body_changed || effects_changed => return Err(conflict("signature evolution conflicts with concurrent target signature, body or effects")),
            "add_contract" if signature_changed || effects_changed => return Err(conflict("contract addition conflicts with concurrent target signature or effects")),
            "replace_contract_expression" if signature_changed || contracts_changed || effects_changed => return Err(conflict("contract replacement conflicts with concurrent target signature, contracts or effects")),
            "add_declaration" if signature_changed || effects_changed => return Err(conflict("declaration addition conflicts with concurrent target signature or effects")),
            "move_declaration" if signature_changed || effects_changed => return Err(conflict("declaration move conflicts with concurrent target signature or effects")),
            "rename_declaration" | "replace_function_body" | "replace_expression" | "replace_contract_expression" | "change_function_signature" | "add_contract" | "add_declaration" | "extract_function" | "add_record_field" | "move_declaration" | "implement_interface" => {},
            _ => return Err(grammar("candidate rebase does not admit this intention kind")),
        }
        if kind == "move_declaration" {
            let destination = change.intent["destination"]
                .as_str()
                .ok_or_else(|| grammar("declaration move lacks a destination anchor"))?;
            for id in [target, destination] {
                if let Some(before) = old_facts.get(id) {
                    let after = new_facts.get(id).ok_or_else(|| {
                        conflict("declaration move target or destination was deleted")
                    })?;
                    if before.location != after.location {
                        return Err(conflict(
                            "declaration move target or destination was concurrently relocated",
                        ));
                    }
                } else if !introduced.contains(id) {
                    return Err(conflict(
                        "declaration move destination is absent from its original history",
                    ));
                }
            }
        }
        for dependency in called_intent_targets(&change.intent) {
            if let Some(before) = old_facts.get(&dependency) {
                let after = new_facts
                    .get(&dependency)
                    .ok_or_else(|| conflict("candidate call target was concurrently deleted"))?;
                if before.signature != after.signature
                    || before.effects != after.effects
                    || before.contracts != after.contracts
                {
                    return Err(conflict(
                        "candidate call target signature, effects or contracts changed concurrently",
                    ));
                }
            }
        }
        let mut classification = json!({"target":target,"intent":kind,"concurrent_display_change":display_changed,"concurrent_signature_change":signature_changed,"concurrent_body_change":body_changed,"concurrent_contract_change":contracts_changed,"concurrent_effect_change":effects_changed,"decision":"replay_required"});
        if kind == "implement_interface" {
            classification["interface_binding_change"] = json!(false);
        }
        report.push(classification);
        introduced.extend(additions);
    }
    Ok(report)
}
/// Complete planned identity inventory, including nested type members. These
/// are already admitted history intentions; this pass adds conflict selection,
/// while each intermediate candidate still replays the full constructor.
fn added_intent_ids<'a>(request: &'a Value) -> Result<BTreeSet<&'a str>, Vec<Diagnostic>> {
    let mut ids = BTreeSet::new();
    let mut add = |value: &'a Value| -> Result<(), Vec<Diagnostic>> {
        let id = value
            .as_str()
            .ok_or_else(|| grammar("candidate addition lacks a planned identity"))?;
        if !ids.insert(id) {
            return Err(grammar("candidate addition repeats a planned identity"));
        }
        Ok(())
    };
    match request["kind"].as_str() {
        Some("add_declaration") => {
            let declaration = &request["declaration"];
            add(&declaration["id"])?;
            match declaration["kind"].as_str() {
                None => {}
                Some("record") => {
                    for field in declaration["fields"]
                        .as_array()
                        .ok_or_else(|| grammar("record addition lacks fields"))?
                    {
                        add(&field["id"])?;
                    }
                }
                Some("variant") => {
                    for case in declaration["cases"]
                        .as_array()
                        .ok_or_else(|| grammar("variant addition lacks cases"))?
                    {
                        add(&case["id"])?;
                        for field in case["fields"]
                            .as_array()
                            .ok_or_else(|| grammar("variant case addition lacks fields"))?
                        {
                            add(&field["id"])?;
                        }
                    }
                }
                _ => return Err(grammar("unsupported declaration addition kind")),
            }
        }
        Some("extract_function") => add(&request["new_id"])?,
        Some("add_record_field") => add(&request["field"]["id"])?,
        Some("implement_interface") => add(&request["id"])?,
        _ => {}
    }
    Ok(ids)
}
fn called_intent_targets(value: &Value) -> BTreeSet<String> {
    let mut stack = vec![value];
    let mut targets = BTreeSet::new();
    while let Some(value) = stack.pop() {
        match value {
            Value::Object(object) => {
                if object.get("kind").and_then(Value::as_str) == Some("call") {
                    if let Some(target) = object.get("target").and_then(Value::as_str) {
                        targets.insert(target.to_owned());
                    }
                }
                stack.extend(object.values());
            }
            Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    targets
}
fn constructor_intent_targets(value: &Value, kinds: &[&str]) -> BTreeSet<String> {
    // SemanticChange already bounds JSON depth, node count and total bytes.
    let mut stack = vec![value];
    let mut targets = BTreeSet::new();
    while let Some(value) = stack.pop() {
        match value {
            Value::Object(object) => {
                if object
                    .get("kind")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kinds.contains(&kind))
                {
                    if let Some(target) = object.get("target").and_then(Value::as_str) {
                        targets.insert(target.to_owned());
                    }
                }
                stack.extend(object.values());
            }
            Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    targets
}
fn hash_value(value: Value) -> Result<String, Vec<Diagnostic>> {
    let text = wire::render(value, MAX_FINGERPRINT_BYTES)
        .map_err(|_| capacity("candidate rebase fingerprint exceeds its bound"))?;
    Ok(wire::digest(
        b"semaprax.rebase.semantic-fact.v1\0",
        text.as_bytes(),
    ))
}
fn same_manifest(old: &ProjectRevision, new: &ProjectRevision) -> Result<(), Vec<Diagnostic>> {
    if old.manifest().to_canonical_toml() != new.manifest().to_canonical_toml() {
        return Err(grammar(
            "candidate rebase requires the same canonical Project manifest",
        ));
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn finish(
    kind: &str,
    left: &ProjectCandidate,
    right: Option<&ProjectCandidate>,
    new_base: &ProjectRevision,
    prefix: usize,
    classifications: Vec<Value>,
    candidate: ProjectCandidate,
) -> Result<ProjectCandidateRebase, Vec<Diagnostic>> {
    let json=wire::render(json!({"schema":PROJECT_CANDIDATE_REBASE_SCHEMA,"operation":kind,"left_parent_candidate":left.candidate_digest(),"right_parent_candidate":right.map(ProjectCandidate::candidate_digest),"original_base_revision":left.base.project_revision(),"onto_revision":new_base.project_revision(),"result_base_revision":candidate.base.project_revision(),"result_revision":candidate.revision.project_revision(),"result_candidate_digest":candidate.candidate_digest(),"shared_history_prefix":prefix,"classifications":classifications,"validation":"complete_candidate_source_replay","source_authority":false,"nonclaims":["not_behavioral_equivalence","not_external_consumer_compatibility","no_runtime_or_project_test_execution","no_source_commit_authority","conservative_conflicts_not_general_semantic_merge"]}),MAX_REPORT_BYTES).map_err(|_|capacity("candidate rebase report exceeds its bound"))?;
    Ok(ProjectCandidateRebase { candidate, json })
}
fn expected(supplied: &str, actual: &str) -> Result<(), Vec<Diagnostic>> {
    if supplied.len() > 71 {
        return Err(capacity("candidate rebase selector exceeds its byte bound"));
    }
    if supplied != actual {
        return Err(conflict("candidate rebase selector is stale or invalid"));
    }
    Ok(())
}
fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G233", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G234", message)]
}
fn conflict(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G235", message)]
}
