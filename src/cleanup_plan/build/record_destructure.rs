//! Canonical recursive record-destructure inventory for cleanup construction.
//! This derivation is intentionally separate from replay's implementation.

use std::collections::{BTreeMap, BTreeSet};

use crate::cleanup_plan::{BlockId, CleanupPlace, CleanupRegionId};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedMatchArm,
    ResolvedMatchMode, ResolvedMatchPattern, ResolvedProgram, ResolvedRecordMatchFieldPattern,
    ResolvedRecordMatchPatternField, ResolvedType,
};

pub(super) mod update;

const MAX_DEPTH: usize = 64;
const MAX_OWNED_LEAVES: usize = 256;
const MAX_VISITED_FIELDS: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OwnedBinding {
    pub(super) source_path: Vec<DeclarationId>,
    pub(super) destination: ResolvedBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Destructure {
    pub(super) nested: bool,
    pub(super) owned_bindings: Vec<OwnedBinding>,
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

pub(super) fn transfer_owned(
    builder: &mut super::PlanBuilder<'_>,
    expression: &ResolvedExpr,
    block: BlockId,
    region: CleanupRegionId,
    source: &CleanupPlace,
    state: &mut super::FlowState,
    arm: &ResolvedMatchArm,
) -> Result<(), Diagnostic> {
    let ResolvedMatchPattern::Record {
        record,
        instance,
        fields,
    } = &arm.pattern
    else {
        return Err(super::plan_error(
            "owned record match has no exact record pattern",
        ));
    };
    let destructure = derive(
        builder.program,
        record,
        instance,
        fields,
        ResolvedMatchMode::Own,
    )?;
    // One effect-free block owns the complete declaration-order transfer
    // sequence. No branch, status selection, or finalizer can observe a
    // partially committed destructure.
    for binding in destructure.owned_bindings {
        let mut field_source = source.clone();
        for field in binding.source_path {
            field_source = field_source.projected(field);
        }
        let destination = builder
            .binding_slot(&binding.destination, region)?
            .ok_or_else(|| super::plan_error("nested record binding has no cleanup slot"))?;
        builder.transfer(
            block,
            expression.id.clone(),
            field_source,
            destination,
            state,
            false,
        )?;
    }
    if builder
        .flags_under(source)
        .iter()
        .any(|flag| state.is_live(*flag))
    {
        return Err(super::plan_error(
            "nested record destructure left an owned source leaf live",
        ));
    }
    Ok(())
}

pub(super) fn observe_borrow(
    builder: &super::PlanBuilder<'_>,
    arm: &ResolvedMatchArm,
) -> Result<(), Diagnostic> {
    let ResolvedMatchPattern::Record {
        record,
        instance,
        fields,
    } = &arm.pattern
    else {
        return Ok(());
    };
    if contains_nested(fields) {
        let result = derive(
            builder.program,
            record,
            instance,
            fields,
            ResolvedMatchMode::Borrow,
        )?;
        if !result.nested {
            return Err(super::plan_error(
                "borrowed record destructure lost its recursive pattern",
            ));
        }
    }
    Ok(())
}

struct Frame<'a> {
    record: &'a DeclarationId,
    instance: &'a ResolvedType,
    fields: &'a [ResolvedRecordMatchPatternField],
    path: Vec<DeclarationId>,
    ordinal_path: Vec<usize>,
    depth: usize,
}

pub(super) fn derive(
    program: &ResolvedProgram,
    record: &DeclarationId,
    instance: &ResolvedType,
    fields: &[ResolvedRecordMatchPatternField],
    mode: ResolvedMatchMode,
) -> Result<Destructure, Diagnostic> {
    let mut stack = vec![Frame {
        record,
        instance,
        fields,
        path: Vec::new(),
        ordinal_path: Vec::new(),
        depth: 1,
    }];
    let mut nested = false;
    let mut visited_fields = 0usize;
    let mut owned_bindings = BTreeMap::new();
    let mut non_byte_owned_terminal = false;

    while let Some(frame) = stack.pop() {
        if frame.depth > MAX_DEPTH {
            return Err(super::plan_error(
                "nested record destructure exceeds its depth bound",
            ));
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = frame.instance
        else {
            return Err(super::plan_error(
                "nested record destructure has a non-nominal instance",
            ));
        };
        if declaration != frame.record {
            return Err(super::plan_error(
                "nested record destructure changes record identity",
            ));
        }
        let declared = program
            .declarations
            .record_fields(frame.record)
            .ok_or_else(|| super::plan_error("nested record destructure has no field inventory"))?;
        if frame.fields.len() != declared.len() {
            return Err(super::plan_error(
                "nested record destructure is not an exact field inventory",
            ));
        }
        let mut seen = BTreeSet::new();
        for field in frame.fields {
            if !seen.insert(&field.field) {
                return Err(super::plan_error(
                    "nested record destructure repeats a field identity",
                ));
            }
        }

        // Stack reversal preserves declaration order when bindings are emitted.
        for (ordinal, declaration) in declared.iter().enumerate().rev() {
            visited_fields = visited_fields
                .checked_add(1)
                .ok_or_else(|| super::plan_error("nested record destructure work overflow"))?;
            if visited_fields > MAX_VISITED_FIELDS {
                return Err(super::plan_error(
                    "nested record destructure exceeds its field-work bound",
                ));
            }
            let field = frame
                .fields
                .iter()
                .find(|candidate| candidate.field == declaration.id)
                .ok_or_else(|| {
                    super::plan_error("nested record destructure omits a declared field")
                })?;
            let field_ty = crate::hir::substitute_type(&declaration.ty, frame.record, arguments)?;
            let needs_drop = program
                .declarations
                .type_facts(&field_ty)
                .ok_or_else(|| super::plan_error("nested record field has no type facts"))?
                .needs_drop;
            let mut path = frame.path.clone();
            path.push(declaration.id.clone());
            let mut ordinal_path = frame.ordinal_path.clone();
            ordinal_path.push(ordinal);
            match &field.pattern {
                ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    let expected = if needs_drop {
                        match mode {
                            ResolvedMatchMode::Own => OwnershipMode::Own,
                            ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                            ResolvedMatchMode::Value => OwnershipMode::Value,
                        }
                    } else {
                        OwnershipMode::Value
                    };
                    if binding.ty != field_ty || binding.ownership != expected {
                        return Err(super::plan_error(
                            "nested record binding type or ownership is not canonical",
                        ));
                    }
                    non_byte_owned_terminal |= needs_drop && field_ty != ResolvedType::Bytes;
                    if needs_drop && mode == ResolvedMatchMode::Own {
                        if owned_bindings.len() >= MAX_OWNED_LEAVES {
                            return Err(super::plan_error(
                                "nested record destructure exceeds its owned-leaf bound",
                            ));
                        }
                        // Reverse traversal is corrected once after all frames are consumed.
                        if owned_bindings
                            .insert(
                                ordinal_path,
                                OwnedBinding {
                                    source_path: path,
                                    destination: binding.clone(),
                                },
                            )
                            .is_some()
                        {
                            return Err(super::plan_error(
                                "nested record destructure repeats a declaration-order leaf",
                            ));
                        }
                    }
                }
                ResolvedRecordMatchFieldPattern::Wildcard => {
                    if needs_drop && mode == ResolvedMatchMode::Own {
                        return Err(super::plan_error(
                            "owned nested record destructure leaves a live field unbound",
                        ));
                    }
                }
                ResolvedRecordMatchFieldPattern::Record {
                    record,
                    instance,
                    fields,
                } => {
                    nested = true;
                    if &field_ty != instance {
                        return Err(super::plan_error(
                            "nested record destructure instance disagrees with its field",
                        ));
                    }
                    stack.push(Frame {
                        record,
                        instance,
                        fields,
                        path,
                        ordinal_path,
                        depth: frame.depth + 1,
                    });
                }
            }
        }
    }
    if nested && non_byte_owned_terminal {
        return Err(super::plan_error(
            "exact nested record destructure must recurse to owned Bytes leaves",
        ));
    }
    Ok(Destructure {
        nested,
        owned_bindings: owned_bindings.into_values().collect(),
    })
}
