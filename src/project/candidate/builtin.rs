//! Typed selectors for existing compiler byte operations, not new language syntax.
//! The complete source inventory includes static facts omitted by runtime graphs.
use serde_json::{json, Value};
use std::collections::BTreeSet;

use super::{capacity, grammar, Result, MAX_ID_BYTES, MAX_WALK_NODES};
use crate::ast::Program;
use crate::byte_ops::{self, ByteOp};
use crate::hir::OwnershipMode;
use crate::project::ProjectRevision;

/// Check the edited, disposable ASTs too: an intention can introduce an ID
/// after its body has been constructed, or after an earlier builtin intention.
/// This conservative history guard never infers that an old call was removed.
pub(in crate::project::candidate) fn validate_builtin_namespace<'a>(
    programs: &[Program],
    intentions: impl Iterator<Item = &'a Value>,
) -> Result<()> {
    let mut targets = std::collections::BTreeSet::new();
    let mut visited = 0usize;
    for intention in intentions {
        let mut stack = vec![intention];
        while let Some(value) = stack.pop() {
            visited += 1;
            if visited > MAX_WALK_NODES {
                return Err(capacity(
                    "builtin history namespace scan exceeds its work bound",
                ));
            }
            match value {
                Value::Object(object) => {
                    if object.get("kind").and_then(Value::as_str) == Some("builtin_call") {
                        if let Some(target) = object.get("target").and_then(Value::as_str) {
                            targets.insert(target);
                        }
                    }
                    stack.extend(object.values());
                }
                Value::Array(values) => stack.extend(values),
                _ => {}
            }
        }
    }
    if targets.is_empty() {
        return Ok(());
    }
    let identities = super::super::interface::identities(programs)?;
    if targets.iter().any(|target| identities.contains(*target))
        || programs
            .iter()
            .flat_map(|program| &program.module_uses)
            .any(|binding| targets.contains(binding.persistent_id.as_str()))
    {
        return Err(grammar(
            "authored declaration or import collides with a retained builtin selector",
        ));
    }
    Ok(())
}

pub(super) fn plan(
    identities: &BTreeSet<String>,
    program: &Program,
    target: &str,
) -> Result<ByteOp> {
    let op = selected(identities, target)?.ok_or_else(|| {
        grammar("builtin call target is unknown or collides with an authored identity")
    })?;
    if !binding_available(program, op) {
        return Err(grammar(
            "builtin call spelling or identity collides with a source binding",
        ));
    }
    Ok(op)
}

pub(super) fn source_identities(revision: &ProjectRevision) -> Result<BTreeSet<String>> {
    let programs = super::super::parse_revision(revision)?;
    super::super::interface::identities(&programs)
}

fn selected(identities: &BTreeSet<String>, target: &str) -> Result<Option<ByteOp>> {
    if target.is_empty() || target.len() > MAX_ID_BYTES {
        return Err(grammar("builtin target is not a bounded stable ID"));
    }
    Ok(byte_ops::by_id(target).filter(|_| !identities.contains(target)))
}

fn binding_available(program: &Program, op: ByteOp) -> bool {
    !program
        .functions
        .iter()
        .any(|item| item.name == op.name() || item.stable_id == op.id())
        && !program
            .module_uses
            .iter()
            .any(|item| item.alias == op.name() || item.persistent_id == op.id())
        && !program.types.iter().any(|item| item.name == op.name())
        && !program.interfaces.iter().any(|item| {
            item.name == op.name()
                || item
                    .imports
                    .iter()
                    .any(|import| import.name == op.name() || import.stable_id == op.id())
        })
        && !program.protocols.iter().any(|item| item.name == op.name())
}

/// Fixed compiler-owned inventory, filtered only by source identity/binding
/// ambiguity. Argument ownership, lexical scope and contracts still require
/// the ordinary full candidate admission; this catalogue grants no permission.
pub(in crate::project::candidate) fn builtin_constructors(
    revision: &ProjectRevision,
    program: &Program,
) -> Result<Vec<Value>> {
    let identities = source_identities(revision)?;
    let mut rows = Vec::new();
    for op in ByteOp::ALL {
        if selected(&identities, op.id())?.is_some() && binding_available(program, op) {
            rows.push(descriptor(op));
        }
    }
    Ok(rows)
}

/// Rebase uses compiler owner facts and the current source identity namespace.
/// Local spelling/scope is checked again by `plan` at each ordinary replay.
pub(in crate::project::candidate) fn builtin_dependency_fingerprint(
    revision: &ProjectRevision,
    target: &str,
) -> Result<Option<Value>> {
    if byte_ops::by_id(target).is_none() {
        return Ok(None);
    }
    Ok(selected(&source_identities(revision)?, target)?.map(descriptor))
}

fn descriptor(op: ByteOp) -> Value {
    let parameters = byte_ops::resolved_params(op)
        .into_iter()
        .enumerate()
        .map(|(index, param)| {
            let array_family = op == ByteOp::ArrayAsSlice && index == 0;
            json!({
                "index":index,
                "name":param.name,
                "type_id":if array_family { None } else { Some(param.ty.identity_key()) },
                "type_family":if array_family { Some("array_u8_any_length") } else { None },
                "ownership":match param.ownership {
                    OwnershipMode::Value => "value",
                    OwnershipMode::Own => "own",
                    OwnershipMode::Borrow => "borrow",
                    OwnershipMode::Shared => "shared",
                },
            })
        })
        .collect::<Vec<_>>();
    json!({
        "kind":"builtin_call", "target":op.id(), "name":op.name(), "arity":op.arity(),
        "parameters":parameters, "return_type_id":op.return_type().identity_key(),
        "effects":[], "evidence_owner":"compiler_byte_operations",
        "requires_full_candidate_validation":true,
    })
}
