//! Typed selectors for existing compiler operations, not new language syntax.
//! The complete source inventory includes static facts omitted by runtime graphs.
use serde_json::{json, Value};
use std::collections::BTreeSet;

use super::{capacity, grammar, Result, MAX_ID_BYTES, MAX_WALK_NODES};
use crate::ast::Program;
use crate::byte_ops::{self, ByteOp};
use crate::hir::OwnershipMode;
use crate::project::ProjectRevision;
use crate::string_ops::{self, StringOp};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::project::candidate) enum BuiltinOp {
    Byte(ByteOp),
    String(StringOp),
}

impl BuiltinOp {
    pub(in crate::project::candidate) fn id(self) -> &'static str {
        match self {
            Self::Byte(op) => op.id(),
            Self::String(op) => op.id(),
        }
    }

    pub(in crate::project::candidate) fn name(self) -> &'static str {
        match self {
            Self::Byte(op) => op.name(),
            Self::String(op) => op.name(),
        }
    }

    pub(in crate::project::candidate) fn arity(self) -> usize {
        match self {
            Self::Byte(op) => op.arity(),
            Self::String(op) => op.arity(),
        }
    }
}

pub(in crate::project::candidate) fn by_id(target: &str) -> Option<BuiltinOp> {
    byte_ops::by_id(target)
        .map(BuiltinOp::Byte)
        .or_else(|| string_ops::by_id(target).map(BuiltinOp::String))
}

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
) -> Result<BuiltinOp> {
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

fn selected(identities: &BTreeSet<String>, target: &str) -> Result<Option<BuiltinOp>> {
    if target.is_empty() || target.len() > MAX_ID_BYTES {
        return Err(grammar("builtin target is not a bounded stable ID"));
    }
    Ok(by_id(target).filter(|_| !identities.contains(target)))
}

fn binding_available(program: &Program, op: BuiltinOp) -> bool {
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
    for op in ByteOp::ALL
        .into_iter()
        .map(BuiltinOp::Byte)
        .chain(StringOp::ALL.into_iter().map(BuiltinOp::String))
    {
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
    if by_id(target).is_none() {
        return Ok(None);
    }
    Ok(selected(&source_identities(revision)?, target)?.map(descriptor))
}

fn descriptor(op: BuiltinOp) -> Value {
    let (params, return_type, evidence_owner) = match op {
        BuiltinOp::Byte(op) => (
            byte_ops::resolved_params(op),
            op.return_type(),
            "compiler_byte_operations",
        ),
        BuiltinOp::String(op) => (
            string_ops::resolved_params(op),
            op.return_type(),
            "compiler_string_operations",
        ),
    };
    let parameters = params
        .into_iter()
        .enumerate()
        .map(|(index, param)| {
            let array_family = op == BuiltinOp::Byte(ByteOp::ArrayAsSlice) && index == 0;
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
        "parameters":parameters, "return_type_id":return_type.identity_key(),
        "effects":[], "evidence_owner":evidence_owner,
        "requires_full_candidate_validation":true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_descriptors_preserve_exact_owner_signatures_and_byte_family_shape() {
        for op in StringOp::ALL {
            let selected = BuiltinOp::String(op);
            assert_eq!(by_id(op.id()), Some(selected));
            let row = descriptor(selected);
            assert_eq!(row["evidence_owner"], "compiler_string_operations");
            assert_eq!(row["arity"], op.arity());
            assert_eq!(row["parameters"].as_array().unwrap().len(), op.arity());
            assert_eq!(row["return_type_id"], op.return_type().identity_key());
            for param in row["parameters"].as_array().unwrap() {
                assert!(param["type_family"].is_null());
            }
        }
        let concat = descriptor(BuiltinOp::String(StringOp::Concat));
        assert_eq!(
            concat["parameters"],
            json!([
                {"index":0,"name":"a","type_id":"string","type_family":null,"ownership":"own"},
                {"index":1,"name":"b","type_id":"string","type_family":null,"ownership":"own"}
            ])
        );
        let from_char = descriptor(BuiltinOp::String(StringOp::FromChar));
        assert_eq!(
            from_char["parameters"],
            json!([
                {"index":0,"name":"c","type_id":"char","type_family":null,"ownership":"value"}
            ])
        );
        let length = descriptor(BuiltinOp::String(StringOp::Len));
        assert_eq!(length["parameters"][0]["ownership"], "borrow");
        assert_eq!(length["return_type_id"], "i64");
        let array = descriptor(BuiltinOp::Byte(ByteOp::ArrayAsSlice));
        assert_eq!(array["evidence_owner"], "compiler_byte_operations");
        assert!(array["parameters"][0]["type_id"].is_null());
        assert_eq!(array["parameters"][0]["type_family"], "array_u8_any_length");
    }

    #[test]
    fn string_selectors_reject_authored_identity_and_import_spelling_collisions() {
        let program = crate::parse(
            "module builtin.fixture; @id(\"core.string.len\") fn authored()->i64 {0}",
            "fixture.spx",
        )
        .unwrap();
        let identities =
            super::super::super::interface::identities(std::slice::from_ref(&program)).unwrap();
        assert_eq!(
            plan(&identities, &program, string_ops::LEN_ID).unwrap_err()[0].code,
            "SPX-G225"
        );
        let aliased = crate::parse(
            "module builtin.fixture; use function @id(\"other.read\") from other.provider as string_len; @id(\"fixture.main\") fn main()->i64 {0}",
            "fixture.spx",
        ).unwrap();
        assert_eq!(
            plan(&BTreeSet::new(), &aliased, string_ops::LEN_ID).unwrap_err()[0].code,
            "SPX-G225"
        );
        let clean = crate::parse(
            "module builtin.fixture; @id(\"fixture.main\") fn main()->i64 {0}",
            "fixture.spx",
        )
        .unwrap();
        assert_eq!(
            plan(&BTreeSet::new(), &clean, string_ops::LEN_ID).unwrap(),
            BuiltinOp::String(StringOp::Len)
        );
        assert!(by_id("core.str.len_bytes").is_none());
    }
}
