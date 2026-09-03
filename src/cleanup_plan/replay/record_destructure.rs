//! Independent recursive record-destructure replay inventory.
//! It does not call or share construction's derivation.

use std::collections::{BTreeMap, VecDeque};

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, OwnershipMode, ResolvedMatchMode, ResolvedProgram,
    ResolvedRecordMatchFieldPattern, ResolvedRecordMatchPatternField, ResolvedType, ValueId,
};

const DEPTH_LIMIT: usize = 64;
const OWNED_LEAF_LIMIT: usize = 256;
const FIELD_WORK_LIMIT: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExpectedBinding {
    pub(super) path: Vec<DeclarationId>,
    pub(super) binding: ValueId,
}

pub(super) struct ExpectedDestructure {
    pub(super) nested: bool,
    pub(super) bindings: Vec<ExpectedBinding>,
}

pub(super) fn contains_nested(fields: &[ResolvedRecordMatchPatternField]) -> bool {
    let mut pending = fields.iter().collect::<Vec<_>>();
    while let Some(field) = pending.pop() {
        if let ResolvedRecordMatchFieldPattern::Record { fields, .. } = &field.pattern {
            pending.extend(fields);
            return true;
        }
    }
    false
}

pub(super) fn function_contains(function: &crate::hir::ResolvedFunction) -> bool {
    function
        .requires
        .iter()
        .any(super::expression_has_nested_record_destructure)
        || function
            .ensures
            .iter()
            .any(super::expression_has_nested_record_destructure)
        || super::expression_has_nested_record_destructure(&function.body)
}

struct Pending<'a> {
    record: &'a DeclarationId,
    instance: &'a ResolvedType,
    fields: &'a [ResolvedRecordMatchPatternField],
    prefix: Vec<DeclarationId>,
    ordinal_prefix: Vec<usize>,
    depth: usize,
}

pub(super) fn replay(
    program: &ResolvedProgram,
    function: &crate::hir::ResolvedFunction,
    record: &DeclarationId,
    instance: &ResolvedType,
    fields: &[ResolvedRecordMatchPatternField],
    mode: ResolvedMatchMode,
) -> Result<ExpectedDestructure, Diagnostic> {
    let mut queue = VecDeque::from([Pending {
        record,
        instance,
        fields,
        prefix: Vec::new(),
        ordinal_prefix: Vec::new(),
        depth: 1,
    }]);
    let mut nested = false;
    let mut field_work = 0usize;
    let mut bindings = BTreeMap::new();
    let mut non_byte_owned_terminal = false;

    while let Some(pending) = queue.pop_front() {
        if pending.depth > DEPTH_LIMIT {
            return Err(super::replay_error(
                function,
                "nested record destructure exceeds its replay depth bound",
            ));
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = pending.instance
        else {
            return Err(super::replay_error(
                function,
                "nested record destructure replay has a non-nominal instance",
            ));
        };
        if declaration != pending.record {
            return Err(super::replay_error(
                function,
                "nested record destructure replay changes record identity",
            ));
        }
        let declarations = program
            .declarations
            .record_fields(pending.record)
            .ok_or_else(|| {
                super::replay_error(function, "nested record destructure has no declaration")
            })?;
        if declarations.len() != pending.fields.len() {
            return Err(super::replay_error(
                function,
                "nested record destructure replay is not exact",
            ));
        }
        let by_id = pending
            .fields
            .iter()
            .map(|field| (&field.field, field))
            .collect::<BTreeMap<_, _>>();
        if by_id.len() != declarations.len() {
            return Err(super::replay_error(
                function,
                "nested record destructure replay has duplicate fields",
            ));
        }
        for (ordinal, declaration) in declarations.iter().enumerate() {
            field_work = field_work.checked_add(1).ok_or_else(|| {
                super::replay_error(function, "nested record destructure work overflow")
            })?;
            if field_work > FIELD_WORK_LIMIT {
                return Err(super::replay_error(
                    function,
                    "nested record destructure exceeds its replay field bound",
                ));
            }
            let field = by_id.get(&declaration.id).ok_or_else(|| {
                super::replay_error(function, "nested record destructure omits a field")
            })?;
            let ty = crate::hir::substitute_type(&declaration.ty, pending.record, arguments)?;
            let needs_drop = super::type_needs_drop(program, function, &ty)?;
            let mut path = pending.prefix.clone();
            path.push(declaration.id.clone());
            let mut ordinal_path = pending.ordinal_prefix.clone();
            ordinal_path.push(ordinal);
            match &field.pattern {
                ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    let ownership = if needs_drop {
                        match mode {
                            ResolvedMatchMode::Own => OwnershipMode::Own,
                            ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                            ResolvedMatchMode::Value => OwnershipMode::Value,
                        }
                    } else {
                        OwnershipMode::Value
                    };
                    if binding.ty != ty || binding.ownership != ownership {
                        return Err(super::replay_error(
                            function,
                            "nested record destructure replay binding is not canonical",
                        ));
                    }
                    non_byte_owned_terminal |= needs_drop && ty != ResolvedType::Bytes;
                    if needs_drop
                        && mode == ResolvedMatchMode::Own
                        && (bindings.len() >= OWNED_LEAF_LIMIT
                            || bindings
                                .insert(ordinal_path, (path, binding.id.clone()))
                                .is_some())
                    {
                        return Err(super::replay_error(
                            function,
                            "nested record destructure replay leaf inventory is invalid",
                        ));
                    }
                }
                ResolvedRecordMatchFieldPattern::Wildcard => {
                    if needs_drop && mode == ResolvedMatchMode::Own {
                        return Err(super::replay_error(
                            function,
                            "nested owned record destructure replay drops a live field",
                        ));
                    }
                }
                ResolvedRecordMatchFieldPattern::Record {
                    record,
                    instance,
                    fields,
                } => {
                    nested = true;
                    if instance != &ty {
                        return Err(super::replay_error(
                            function,
                            "nested record destructure replay instance is inconsistent",
                        ));
                    }
                    queue.push_back(Pending {
                        record,
                        instance,
                        fields,
                        prefix: path,
                        ordinal_prefix: ordinal_path,
                        depth: pending.depth + 1,
                    });
                }
            }
        }
    }
    if nested && non_byte_owned_terminal {
        return Err(super::replay_error(
            function,
            "exact nested record destructure replay did not reach owned Bytes leaves",
        ));
    }
    Ok(ExpectedDestructure {
        nested,
        bindings: bindings
            .into_iter()
            .map(|(_, (path, binding))| ExpectedBinding { path, binding })
            .collect(),
    })
}
