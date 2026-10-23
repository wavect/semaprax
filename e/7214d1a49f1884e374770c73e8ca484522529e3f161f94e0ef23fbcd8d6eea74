#[cfg(test)]
use super::{allocation, MAX_REPLAY_WORK_UNITS};
use super::{selected_profile, ReplayBudget};
use crate::cleanup::{FieldLiveness, FieldLivenessShape, LivenessFlagId};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedFunction, ResolvedType, ResolvedTypeDeclarationKind};

use super::replay_error;
use crate::cleanup_plan::{
    CLEANUP_PLAN_SCHEMA_V10, CLEANUP_PLAN_SCHEMA_V11, CLEANUP_PLAN_SCHEMA_V12,
    CLEANUP_PLAN_SCHEMA_V13, CLEANUP_PLAN_SCHEMA_V14, CLEANUP_PLAN_SCHEMA_V15,
    CLEANUP_PLAN_SCHEMA_V16, CLEANUP_PLAN_SCHEMA_V17, CLEANUP_PLAN_SCHEMA_V18,
    CLEANUP_PLAN_SCHEMA_V19, CLEANUP_PLAN_SCHEMA_V7, CLEANUP_PLAN_SCHEMA_V8,
    CLEANUP_PLAN_SCHEMA_V9,
};

fn nested_schema(schema: &str) -> bool {
    matches!(
        schema,
        CLEANUP_PLAN_SCHEMA_V7
            | CLEANUP_PLAN_SCHEMA_V8
            | CLEANUP_PLAN_SCHEMA_V9
            | CLEANUP_PLAN_SCHEMA_V10
            | CLEANUP_PLAN_SCHEMA_V11
            | CLEANUP_PLAN_SCHEMA_V12
            | CLEANUP_PLAN_SCHEMA_V13
            | CLEANUP_PLAN_SCHEMA_V15
            | CLEANUP_PLAN_SCHEMA_V16
            | CLEANUP_PLAN_SCHEMA_V17
            | CLEANUP_PLAN_SCHEMA_V18
            | CLEANUP_PLAN_SCHEMA_V19
            | CLEANUP_PLAN_SCHEMA_V14
    )
}

#[derive(Default)]
struct Budget {
    owned_leaves: usize,
    visited_fields: usize,
}

#[cfg(test)]
pub(super) fn expected_shape_for_type(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    next_flag: &mut u32,
) -> Result<FieldLivenessShape, Diagnostic> {
    derive(program, function, ty, next_flag, 0, &mut Budget::default())
}

mod paid;
pub(super) fn expected_shape_for_type_paid(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    next_flag: &mut u32,
    physical: &mut ReplayBudget,
) -> Result<FieldLivenessShape, Diagnostic> {
    paid::derive(
        program,
        function,
        ty,
        next_flag,
        0,
        &mut Budget::default(),
        physical,
    )
}

#[cfg(test)]
mod paid_before;
/// Scratch and its sole payer remain owned through the complete receiving call.
/// No address, cache key or retained observation can confer payer authority.
pub(super) struct PaidShapeReceiver<'a> {
    scratch: paid::Scratch,
    physical: &'a mut ReplayBudget,
}
impl<'a> PaidShapeReceiver<'a> {
    pub(super) fn new(physical: &'a mut ReplayBudget) -> Self {
        Self {
            scratch: paid::Scratch::default(),
            physical,
        }
    }
    pub(super) fn derive(
        &mut self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        ty: &ResolvedType,
        next_flag: &mut u32,
    ) -> Result<FieldLivenessShape, Diagnostic> {
        paid::derive_reusing(
            program,
            function,
            ty,
            next_flag,
            (0, &mut Budget::default()),
            self.physical,
            &mut self.scratch,
        )
    }
    pub(super) fn budget(&mut self) -> &mut ReplayBudget {
        self.physical
    }
    #[cfg(test)]
    pub(super) fn capacities(&self) -> (usize, usize) {
        self.scratch.capacities()
    }
    #[cfg(test)]
    pub(super) fn settled(&self) -> bool {
        self.scratch.settled()
    }
}

#[cfg(test)]
pub(super) fn expected_shape_for_type_paid_before(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    next_flag: &mut u32,
    physical: &mut ReplayBudget,
) -> Result<FieldLivenessShape, Diagnostic> {
    paid_before::derive(
        program,
        function,
        ty,
        next_flag,
        0,
        &mut Budget::default(),
        physical,
    )
}

/// Depth-first shape derivation over an explicit work stack.
///
/// A record chain is as deep as the program admits, and the legacy flat
/// schemas carry no depth bound, so recursing per level overflows the default
/// stack. The build side already lowers this shape iteratively; mirror it here
/// so replay agrees. Side effects keep their recursive order exactly: a
/// parent charges its fields before any child is entered, children are entered
/// left to right, and liveness flags are therefore still allocated in
/// depth-first order.
#[cfg(test)]
fn derive(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    root: &ResolvedType,
    next_flag: &mut u32,
    depth: usize,
    budget: &mut Budget,
) -> Result<FieldLivenessShape, Diagnostic> {
    struct CaseMeta {
        case: DeclarationId,
        case_index: u32,
        fields: Vec<(DeclarationId, u32)>,
    }
    enum Frame {
        Enter(ResolvedType, usize),
        FinishRecord(DeclarationId, Vec<(DeclarationId, u32)>),
        FinishVariant(DeclarationId, Vec<CaseMeta>),
    }

    let mut frames = vec![Frame::Enter(root.clone(), depth)];
    let mut shapes = Vec::<FieldLivenessShape>::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(ty, depth) => {
                if !type_needs_drop(program, function, &ty)? {
                    shapes.push(FieldLivenessShape::NoDrop);
                    continue;
                }
                if let Some(id) = crate::cleanup::primitive_leaf_lifecycle(&ty) {
                    if depth > 1 && !nested_schema(function.cleanup_plan.schema) {
                        return Err(replay_error(
                            function,
                            "nested compiler-owned Bytes cleanup leaf is outside flat record v1",
                        ));
                    }
                    charge_leaf(function, budget)?;
                    let flag = next_flag_id(function, next_flag)?;
                    shapes.push(FieldLivenessShape::Leaf {
                        flag,
                        lifecycle: DeclarationId::new(id),
                    });
                    continue;
                }
                if crate::cleanup::is_owned_bounded_vec_type(&ty)
                    || crate::hir::owned_record_collection::is_owned_record_vec_type(
                        program.declarations(),
                        &ty,
                    )
                    || crate::hir::owned_leaf_collection::is_copy_or_leaf_vec(
                        program.declarations(),
                        &ty,
                    )
                {
                    charge_leaf(function, budget)?;
                    shapes.push(FieldLivenessShape::Leaf {
                        flag: next_flag_id(function, next_flag)?,
                        lifecycle: DeclarationId::new(crate::cleanup::VEC_DROP_LIFECYCLE_ID),
                    });
                    continue;
                }
                if crate::iterator_ops::is_iter(&ty) {
                    if !matches!(
                        function.cleanup_plan.schema,
                        CLEANUP_PLAN_SCHEMA_V10
                            | CLEANUP_PLAN_SCHEMA_V11
                            | CLEANUP_PLAN_SCHEMA_V12
                            | CLEANUP_PLAN_SCHEMA_V13
                            | CLEANUP_PLAN_SCHEMA_V15
                            | CLEANUP_PLAN_SCHEMA_V16
                            | CLEANUP_PLAN_SCHEMA_V17
                            | CLEANUP_PLAN_SCHEMA_V18
                            | CLEANUP_PLAN_SCHEMA_V19
                            | CLEANUP_PLAN_SCHEMA_V14
                    ) {
                        return Err(replay_error(
                            function,
                            "iterator cleanup requires schema v10",
                        ));
                    }
                    charge_leaf(function, budget)?;
                    shapes.push(FieldLivenessShape::Leaf {
                        flag: next_flag_id(function, next_flag)?,
                        lifecycle: DeclarationId::new(crate::cleanup::ITER_DROP_LIFECYCLE_ID),
                    });
                    continue;
                }
                if crate::cleanup::is_owned_bounded_box_type(&ty) {
                    charge_leaf(function, budget)?;
                    shapes.push(FieldLivenessShape::Leaf {
                        flag: next_flag_id(function, next_flag)?,
                        lifecycle: DeclarationId::new(crate::cleanup::BOX_DROP_LIFECYCLE_ID),
                    });
                    continue;
                }
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &ty
                else {
                    return Err(replay_error(
                        function,
                        "droppable supplemental slot is not nominal",
                    ));
                };
                let item = program
                    .types()
                    .iter()
                    .find(|item| item.id == *declaration)
                    .ok_or_else(|| {
                        replay_error(
                            function,
                            format_args!("unknown cleanup type `{declaration}`"),
                        )
                    })?;
                match &item.kind {
                    ResolvedTypeDeclarationKind::Resource { drop } => {
                        if !arguments.is_empty() {
                            return Err(replay_error(
                                function,
                                "resource cleanup slot has generic arguments",
                            ));
                        }
                        charge_leaf(function, budget)?;
                        shapes.push(FieldLivenessShape::Leaf {
                            flag: next_flag_id(function, next_flag)?,
                            lifecycle: drop.id.clone(),
                        });
                    }
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                        if !arguments.is_empty()
                            && !matches!(&item.kind, ResolvedTypeDeclarationKind::Record { .. })
                        {
                            return Err(replay_error(
                                function,
                                "record cleanup slot has generic arguments",
                            ));
                        }
                        let child_depth = depth.checked_add(1).ok_or_else(|| {
                            replay_error(function, "cleanup projection depth overflowed")
                        })?;
                        if nested_schema(function.cleanup_plan.schema)
                            && child_depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                        {
                            return Err(replay_error(
                                function,
                                format_args!(
                                    "cleanup shape exceeds the {} record-depth limit",
                                    crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                                ),
                            ));
                        }
                        charge_fields(function, budget, fields.len())?;
                        let meta = fields
                            .iter()
                            .map(|field| (field.id.clone(), field.index))
                            .collect::<Vec<_>>();
                        frames
                            .try_reserve(fields.len().saturating_add(1))
                            .map_err(|_| {
                                replay_error(
                                    function,
                                    "cleanup shape capacity exceeds address space",
                                )
                            })?;
                        frames.push(Frame::FinishRecord(declaration.clone(), meta));
                        for field in fields.iter().rev() {
                            frames.push(Frame::Enter(
                                crate::hir::substitute_type(&field.ty, declaration, arguments)?,
                                child_depth,
                            ));
                        }
                    }
                    ResolvedTypeDeclarationKind::Variant { cases } => {
                        let count = cases
                            .iter()
                            .try_fold(0usize, |count, case| count.checked_add(case.fields.len()))
                            .ok_or_else(|| {
                                replay_error(function, "cleanup visited-field count overflowed")
                            })?;
                        charge_fields(function, budget, count)?;
                        let container = ResolvedType::Nominal {
                            declaration: declaration.clone(),
                            arguments: arguments.clone(),
                        };
                        let mut meta = Vec::with_capacity(cases.len());
                        let mut entries = Vec::with_capacity(count);
                        for case in cases {
                            let mut case_fields = Vec::with_capacity(case.fields.len());
                            for field in &case.fields {
                                let ty =
                                    crate::hir::substitute_type(&field.ty, declaration, arguments)?;
                                let nested_record = crate::cleanup::variant_record_field(
                                    program, &container, &case.id, &field.id, &ty,
                                );
                                if type_needs_drop(program, function, &ty)?
                                    && !nested_record
                                    && crate::cleanup::variant_leaf_lifecycle(
                                        program, &container, &case.id, &field.id, &ty,
                                    )
                                    .is_none()
                                {
                                    return Err(replay_error(
                                        function,
                                        "variant owner is outside its admitted cleanup profile",
                                    ));
                                }
                                case_fields.push((field.id.clone(), field.index));
                                entries.push(ty);
                            }
                            meta.push(CaseMeta {
                                case: case.id.clone(),
                                case_index: case.index,
                                fields: case_fields,
                            });
                        }
                        frames
                            .try_reserve(entries.len().saturating_add(1))
                            .map_err(|_| {
                                replay_error(
                                    function,
                                    "cleanup shape capacity exceeds address space",
                                )
                            })?;
                        frames.push(Frame::FinishVariant(declaration.clone(), meta));
                        for ty in entries.into_iter().rev() {
                            frames.push(Frame::Enter(ty, depth));
                        }
                    }
                }
            }
            Frame::FinishRecord(declaration, meta) => {
                let start = shapes.len().checked_sub(meta.len()).ok_or_else(|| {
                    replay_error(function, "cleanup record field shape is absent")
                })?;
                let children = shapes.split_off(start);
                let expected = meta
                    .into_iter()
                    .zip(children)
                    .map(|((field, field_index), shape)| FieldLiveness {
                        field,
                        field_index,
                        shape,
                    })
                    .collect::<Vec<_>>();
                shapes.push(FieldLivenessShape::Record {
                    declaration,
                    fields: expected,
                });
            }
            Frame::FinishVariant(declaration, meta) => {
                let total = meta.iter().map(|case| case.fields.len()).sum::<usize>();
                let start = shapes.len().checked_sub(total).ok_or_else(|| {
                    replay_error(function, "cleanup variant field shape is absent")
                })?;
                let mut children = shapes.split_off(start).into_iter();
                let mut expected_cases = Vec::with_capacity(meta.len());
                for case in meta {
                    let mut expected_fields = Vec::with_capacity(case.fields.len());
                    for (field, field_index) in case.fields {
                        let shape = children.next().ok_or_else(|| {
                            replay_error(function, "cleanup variant field shape is absent")
                        })?;
                        expected_fields.push(FieldLiveness {
                            field,
                            field_index,
                            shape,
                        });
                    }
                    expected_cases.push(crate::cleanup::VariantCaseLiveness {
                        case: case.case,
                        case_index: case.case_index,
                        fields: expected_fields,
                    });
                }
                shapes.push(FieldLivenessShape::Variant {
                    declaration,
                    cases: expected_cases,
                });
            }
        }
    }
    let shape = shapes
        .pop()
        .ok_or_else(|| replay_error(function, "cleanup shape is absent"))?;
    if !shapes.is_empty() {
        return Err(replay_error(function, "cleanup shape did not settle"));
    }
    Ok(shape)
}

fn next_flag_id(function: &ResolvedFunction, next: &mut u32) -> Result<LivenessFlagId, Diagnostic> {
    let flag = LivenessFlagId(*next);
    *next = next
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "too many cleanup flags"))?;
    Ok(flag)
}

fn charge_leaf(function: &ResolvedFunction, budget: &mut Budget) -> Result<(), Diagnostic> {
    budget.owned_leaves = budget
        .owned_leaves
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "cleanup owned-leaf count overflowed"))?;
    if nested_schema(function.cleanup_plan.schema)
        && budget.owned_leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES
    {
        return Err(replay_error(
            function,
            format_args!(
                "cleanup shape exceeds the {} owned-leaf limit",
                crate::cleanup::MAX_CLEANUP_OWNED_LEAVES
            ),
        ));
    }
    Ok(())
}

fn charge_fields(
    function: &ResolvedFunction,
    budget: &mut Budget,
    count: usize,
) -> Result<(), Diagnostic> {
    budget.visited_fields = budget
        .visited_fields
        .checked_add(count)
        .ok_or_else(|| replay_error(function, "cleanup visited-field count overflowed"))?;
    if nested_schema(function.cleanup_plan.schema)
        && budget.visited_fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS
    {
        return Err(replay_error(
            function,
            format_args!(
                "cleanup shape exceeds the {} visited-field limit",
                crate::cleanup::MAX_CLEANUP_VISITED_FIELDS
            ),
        ));
    }
    Ok(())
}

pub(super) fn type_needs_drop(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    match allocation::authority(program, function) {
        Ok(Some(lease)) => {
            if program.selected_flat_records() {
                lease.fail_physical("selected inventory is outside the ordinary oracle receiver");
                return Err(Diagnostic::io_static(
                    "SPX-H006",
                    "ordinary oracle receiver refused selected inventory admission",
                ));
            }
            // A new helper budget owns no allowance: every byte is debited from
            // the same cumulative account, with the existing Ordinary ceilings.
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
            return super::paid_type_facts::needs_drop(program, function, ty, &mut budget);
        }
        Ok(None) => {}
        Err(_) => {
            return Err(Diagnostic::io_static(
                "SPX-H006",
                "oracle invocation authority refused a type probe",
            ))
        }
    }
    if program.selected_flat_records() {
        return crate::cleanup::selected_inventory_needs_drop_in(program, ty);
    }
    crate::cleanup::type_needs_resource_cleanup(program, ty)
        .map_err(|message| replay_error(function, message))
}

/// The guarded helper receiver authenticates every nominal probe on the same
/// ledger; ordinary replay retains its original classification and diagnostics.
pub(super) fn type_needs_drop_with_budget(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        return lease.no_drop(program, function, ty, budget);
    }
    if program.selected_flat_records() && matches!(ty, ResolvedType::Nominal { .. }) {
        selected_profile::collections::prepay(
            program,
            function,
            selected_profile::collections::type_key_bytes(ty)?,
            budget,
        )?;
    }
    if program.selected_flat_records() {
        return type_needs_drop(program, function, ty);
    }
    super::paid_type_facts::needs_drop(program, function, ty, budget)
}

/// Authenticate the unbudgeted caller before any substitution acquisition. A
/// helper budget owns no allowance: its payer is the same cumulative account.
pub(super) fn substitute_type_current(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
) -> Result<ResolvedType, Diagnostic> {
    #[cfg(test)]
    match allocation::authority(program, function) {
        Ok(Some(lease)) => {
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
            return super::ordinary_inventory::substitute::substitute_type(
                template,
                owner,
                arguments,
                function,
                &mut budget,
            );
        }
        Ok(None) => {}
        Err(_) => {
            return Err(Diagnostic::io_static(
                "SPX-H006",
                "oracle invocation authority refused field substitution",
            ))
        }
    }
    let _ = (program, function);
    crate::hir::substitute_type(template, owner, arguments)
}
