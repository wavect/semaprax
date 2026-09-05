//! AddDeclaration support for Universal Semantic Transaction v1.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::ast::{Program, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

use super::{invalid, stale, ProjectRevision};

const SOURCE_DOMAIN: &[u8] = b"semaprax.semantic-transaction.module-source.digest.v1\0";
const MAX_MODULE_IDENTITIES: usize = 65_536;

/// Append one typed declaration through the existing closed Project Candidate
/// declaration constructor after authenticating the anchor module exactly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTransactionAddDeclaration {
    pub(super) target: String,
    pub(super) expected_old_module: Value,
    pub(super) declaration: Value,
}

impl SemanticTransactionAddDeclaration {
    pub fn new(target: impl Into<String>, expected_old_module: Value, declaration: Value) -> Self {
        Self {
            target: target.into(),
            expected_old_module,
            declaration,
        }
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn expected_old_module(&self) -> &Value {
        &self.expected_old_module
    }

    pub fn declaration(&self) -> &Value {
        &self.declaration
    }

    pub(super) fn value(&self) -> Value {
        json!({
            "declaration": self.declaration,
            "expected_old_module": self.expected_old_module,
            "kind": "add_declaration",
            "target": self.target,
        })
    }

    pub(super) fn validate_shape(&self) -> Result<(), Vec<Diagnostic>> {
        require_module_snapshot_shape(&self.expected_old_module)?;
        planned_identity_inventory(&self.declaration)?;
        Ok(())
    }
}

pub(in crate::project) struct AddDeclarationEligibility {
    pub(in crate::project) expected_old_module: Option<Value>,
    pub(in crate::project) comment_free_canonical_workspace: bool,
    pub(in crate::project) explicit_identity: bool,
    pub(in crate::project) monomorphic: bool,
    unique_function: bool,
}

impl AddDeclarationEligibility {
    pub(in crate::project) fn available(&self) -> bool {
        self.unique_function
            && self.comment_free_canonical_workspace
            && self.explicit_identity
            && self.monomorphic
    }
}

pub(super) struct DeclarationAddition {
    pub(super) added_identity_inventory: Vec<String>,
    pub(super) inserted_source: String,
    pub(super) new_module: Value,
}

pub(in crate::project) fn add_declaration_eligibility(
    revision: &ProjectRevision,
    target: &str,
) -> Result<AddDeclarationEligibility, Vec<Diagnostic>> {
    let mut expected_old_module = None;
    let mut explicit_identity = false;
    let mut monomorphic = false;
    let mut matches = 0usize;
    for source in revision.sources() {
        let program =
            crate::parse(source.source(), Path::new(source.path())).map_err(|error| vec![error])?;
        for function in &program.functions {
            if function.stable_id != target {
                continue;
            }
            matches += 1;
            explicit_identity = function.explicit_id;
            monomorphic = function.type_parameters.is_empty();
            expected_old_module = Some(module_snapshot(source.path(), source.source(), &program)?);
        }
    }
    Ok(AddDeclarationEligibility {
        expected_old_module,
        comment_free_canonical_workspace: super::comment_free_canonical_workspace(revision),
        explicit_identity,
        monomorphic,
        unique_function: matches == 1,
    })
}

pub(super) fn require_add_declaration_preconditions(
    revision: &ProjectRevision,
    operation: &SemanticTransactionAddDeclaration,
) -> Result<(), Vec<Diagnostic>> {
    let eligibility = add_declaration_eligibility(revision, operation.target())?;
    if !eligibility.unique_function || !eligibility.explicit_identity || !eligibility.monomorphic {
        return Err(invalid(
            "AddDeclaration v1 requires one explicit monomorphic function anchor",
        ));
    }
    if eligibility.expected_old_module.as_ref() != Some(operation.expected_old_module()) {
        return Err(stale(
            "AddDeclaration expected old module does not match the exact base",
        ));
    }
    let planned = planned_identity_inventory(operation.declaration())?;
    let existing = project_identity_inventory(revision)?;
    if planned.iter().any(|id| existing.contains(id)) {
        return Err(invalid(
            "AddDeclaration planned stable identity already exists in the Project",
        ));
    }
    Ok(())
}

pub(super) fn require_source_preserving_declaration_addition(
    base: &ProjectRevision,
    candidate: &ProjectRevision,
    operation: &SemanticTransactionAddDeclaration,
) -> Result<DeclarationAddition, Vec<Diagnostic>> {
    let before = select_module(base, operation.target())?;
    let after = select_module(candidate, operation.target())?;
    if before.0 != after.0 || base.sources().len() != candidate.sources().len() {
        return Err(stale(
            "AddDeclaration changed the source owner or inventory",
        ));
    }
    let old_module = module_snapshot(&before.0, before.1, &before.2)?;
    if &old_module != operation.expected_old_module() {
        return Err(stale(
            "AddDeclaration old module changed before source review",
        ));
    }
    let planned = planned_identity_inventory(operation.declaration())?;
    let old_ids = declaration_identity_inventory(&before.2)?;
    let new_ids = declaration_identity_inventory(&after.2)?;
    let additions = new_ids
        .iter()
        .filter(|id| !old_ids.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    if additions != planned || !is_ordered_subsequence(&old_ids, &new_ids) {
        return Err(stale(
            "AddDeclaration did not preserve the old declaration inventory and append the planned identities",
        ));
    }
    let mut inserted_source = None;
    for old_source in base.sources() {
        let new_source = candidate
            .sources()
            .iter()
            .find(|source| source.path() == old_source.path())
            .ok_or_else(|| stale("AddDeclaration changed the source inventory"))?;
        if old_source.path() != before.0 {
            if old_source.source() != new_source.source() {
                return Err(stale("AddDeclaration changed an unrelated source"));
            }
            continue;
        }
        inserted_source = Some(single_insertion(old_source.source(), new_source.source())?);
    }
    let inserted_source = inserted_source
        .filter(|source| !source.is_empty())
        .ok_or_else(|| stale("AddDeclaration source insertion is absent"))?;
    if !planned.iter().all(|id| inserted_source.contains(id)) {
        return Err(stale(
            "AddDeclaration inserted source does not contain every planned identity",
        ));
    }
    Ok(DeclarationAddition {
        added_identity_inventory: planned,
        inserted_source: inserted_source.to_owned(),
        new_module: module_snapshot(&after.0, after.1, &after.2)?,
    })
}

fn select_module<'a>(
    revision: &'a ProjectRevision,
    target: &str,
) -> Result<(String, &'a str, Program), Vec<Diagnostic>> {
    let mut selected = None;
    for source in revision.sources() {
        let program =
            crate::parse(source.source(), Path::new(source.path())).map_err(|error| vec![error])?;
        if program
            .functions
            .iter()
            .any(|function| function.stable_id == target)
        {
            if selected.is_some() {
                return Err(invalid(
                    "AddDeclaration v1 requires one unambiguous function anchor",
                ));
            }
            selected = Some((source.path().to_owned(), source.source(), program));
        }
    }
    selected.ok_or_else(|| invalid("AddDeclaration v1 requires an explicit function anchor"))
}

fn module_snapshot(path: &str, source: &str, program: &Program) -> Result<Value, Vec<Diagnostic>> {
    Ok(json!({
        "declaration_ids": declaration_identity_inventory(program)?,
        "source_digest": source_digest(source),
        "source_path": path,
    }))
}

fn require_module_snapshot_shape(value: &Value) -> Result<(), Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("AddDeclaration expected old module is not an object"))?;
    let keys = ["declaration_ids", "source_digest", "source_path"];
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(
            "AddDeclaration expected old module field set is invalid",
        ));
    }
    let ids = object["declaration_ids"]
        .as_array()
        .ok_or_else(|| invalid("AddDeclaration declaration inventory is invalid"))?;
    if ids.len() > MAX_MODULE_IDENTITIES
        || ids.iter().any(|id| id.as_str().is_none_or(str::is_empty))
        || object["source_path"].as_str().is_none_or(str::is_empty)
        || object["source_digest"].as_str().is_none_or(|digest| {
            digest.len() != 71
                || !digest.starts_with("sha256:")
                || !digest.as_bytes()[7..]
                    .iter()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        })
    {
        return Err(invalid("AddDeclaration expected old module is invalid"));
    }
    Ok(())
}

fn declaration_identity_inventory(program: &Program) -> Result<Vec<String>, Vec<Diagnostic>> {
    let mut entries = Vec::new();
    for declaration in &program.types {
        entries.push((declaration.span.start, declaration.stable_id.clone()));
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } => {
                entries.extend(
                    fields
                        .iter()
                        .map(|field| (field.span.start, field.stable_id.clone())),
                );
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    entries.push((case.span.start, case.stable_id.clone()));
                    entries.extend(
                        case.fields
                            .iter()
                            .map(|field| (field.span.start, field.stable_id.clone())),
                    );
                }
            }
            TypeDeclarationKind::Class { fields, methods } => {
                entries.extend(
                    fields
                        .iter()
                        .map(|field| (field.span.start, field.stable_id.clone())),
                );
                entries.extend(
                    methods
                        .iter()
                        .map(|method| (method.span.start, method.stable_id.clone())),
                );
            }
            TypeDeclarationKind::Resource { lifecycles } => {
                entries.extend(lifecycles.iter().filter_map(|lifecycle| {
                    lifecycle
                        .stable_id
                        .clone()
                        .map(|id| (lifecycle.span.start, id))
                }));
            }
        }
    }
    entries.extend(
        program
            .interfaces
            .iter()
            .map(|item| (item.span.start, item.stable_id.clone())),
    );
    for interface in &program.interfaces {
        entries.extend(
            interface
                .imports
                .iter()
                .map(|item| (item.span.start, item.stable_id.clone())),
        );
    }
    entries.extend(
        program
            .protocols
            .iter()
            .map(|item| (item.span.start, item.stable_id.clone())),
    );
    for protocol in &program.protocols {
        entries.extend(
            protocol
                .methods
                .iter()
                .map(|item| (item.span.start, item.stable_id.clone())),
        );
    }
    entries.extend(
        program
            .implementations
            .iter()
            .map(|item| (item.span.start, item.stable_id.clone())),
    );
    entries.extend(
        program
            .functions
            .iter()
            .map(|item| (item.span.start, item.stable_id.clone())),
    );
    entries.sort();
    if entries.len() > MAX_MODULE_IDENTITIES {
        return Err(super::capacity(
            "AddDeclaration module identity inventory exceeds its limit",
        ));
    }
    Ok(entries.into_iter().map(|(_, id)| id).collect())
}

fn project_identity_inventory(
    revision: &ProjectRevision,
) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
    let mut identities = BTreeSet::new();
    for source in revision.sources() {
        let program =
            crate::parse(source.source(), Path::new(source.path())).map_err(|error| vec![error])?;
        identities.extend(declaration_identity_inventory(&program)?);
        identities.extend(
            program
                .module_uses
                .iter()
                .map(|binding| binding.persistent_id.clone()),
        );
    }
    Ok(identities)
}

fn planned_identity_inventory(declaration: &Value) -> Result<Vec<String>, Vec<Diagnostic>> {
    let object = declaration
        .as_object()
        .ok_or_else(|| invalid("AddDeclaration declaration is not an object"))?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| invalid("AddDeclaration declaration identity is invalid"))?;
    let mut identities = vec![id.to_owned()];
    match object.get("kind").and_then(Value::as_str) {
        None => {}
        Some("record") => append_field_ids(object.get("fields"), &mut identities)?,
        Some("variant") => {
            let cases = object
                .get("cases")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("AddDeclaration variant cases are invalid"))?;
            for case in cases {
                identities.push(required_id(
                    case,
                    "AddDeclaration variant case identity is invalid",
                )?);
                append_field_ids(case.get("fields"), &mut identities)?;
            }
        }
        Some(_) => return Err(invalid("AddDeclaration declaration kind is unsupported")),
    }
    let unique = identities.iter().collect::<BTreeSet<_>>();
    if unique.len() != identities.len() || identities.len() > MAX_MODULE_IDENTITIES {
        return Err(invalid(
            "AddDeclaration planned identity inventory is invalid",
        ));
    }
    Ok(identities)
}

fn append_field_ids(
    value: Option<&Value>,
    identities: &mut Vec<String>,
) -> Result<(), Vec<Diagnostic>> {
    let fields = value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("AddDeclaration field inventory is invalid"))?;
    for field in fields {
        identities.push(required_id(
            field,
            "AddDeclaration field identity is invalid",
        )?);
    }
    Ok(())
}

fn required_id(value: &Value, message: &'static str) -> Result<String, Vec<Diagnostic>> {
    value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid(message))
}

fn single_insertion<'a>(before: &str, after: &'a str) -> Result<&'a str, Vec<Diagnostic>> {
    if after.len() <= before.len() {
        return Err(stale("AddDeclaration did not add source"));
    }
    let mut prefix = 0usize;
    for (left, right) in before.bytes().zip(after.bytes()) {
        if left != right {
            break;
        }
        prefix += 1;
    }
    while !before.is_char_boundary(prefix) || !after.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let remaining_before = &before[prefix..];
    let remaining_after = &after[prefix..];
    if !remaining_after.ends_with(remaining_before) {
        return Err(stale(
            "AddDeclaration changed source outside one insertion span",
        ));
    }
    Ok(&remaining_after[..remaining_after.len() - remaining_before.len()])
}

fn is_ordered_subsequence(old: &[String], new: &[String]) -> bool {
    let mut cursor = 0usize;
    for id in new {
        if old.get(cursor) == Some(id) {
            cursor += 1;
        }
    }
    cursor == old.len()
}

fn source_digest(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_DOMAIN);
    hasher.update((source.len() as u64).to_le_bytes());
    hasher.update(source.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}
