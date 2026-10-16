//! Selected coverage rederives complete primitive inventory and supplemental
//! owned call epochs from borrowed typed HIR; attached inventories grant nothing.
use super::*;

pub(super) fn validate_inventory_coverage(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        return lease.inventory(program, function, budget);
    }
    let inventory = {
        let ReplayBudget {
            remaining,
            physical,
            ..
        } = budget;
        let physical = physical.as_deref_mut().ok_or_else(|| {
            replay_error(
                function,
                "selected inventory coverage requires a physical ledger",
            )
        })?;
        crate::cleanup::build_flat_record_inventory_with_budget(
            program,
            function,
            physical,
            &mut |units| {
                if let Some(next) = remaining.checked_sub(units) {
                    *remaining = next;
                    Ok(())
                } else {
                    *remaining = 0;
                    Err(replay_error(function, "selected inventory work exhausted"))
                }
            },
        )?
    };
    if inventory != function.cleanup {
        return Err(replay_error(
            function,
            "cleanup inventory disagrees with independently rebuilt typed HIR",
        ));
    }
    let plan = &function.cleanup_plan;
    let base_len = inventory.slots.len();
    if plan.slots.len() < base_len {
        return Err(replay_error(
            function,
            "cleanup plan omits storage required by the cleanup inventory",
        ));
    }
    for (index, slot) in inventory.slots.iter().enumerate() {
        let actual = &plan.slots[index];
        let storage = inventory_storage_id(&slot.origin);
        if actual.id.0 != slot.id.0
            || actual.storage_index != slot.discovery_index
            || actual.storage != storage
            || actual.ty != slot.ty
            || actual.field_liveness_shape != slot.shape
        {
            return Err(replay_error(
                function,
                format_args!(
                    "cleanup plan base slot {index} disagrees with the independent inventory"
                ),
            ));
        }
    }
    if !inventory
        .entry_state
        .conditional_owned_parameters
        .is_empty()
        || !plan.entry_state.conditional_owned_parameters.is_empty()
    {
        return Err(replay_error(
            function,
            "selected inventory coverage does not support conditional entry owners",
        ));
    }
    if inventory.entry_state.live_owned_parameters.len()
        != plan.entry_state.live_owned_parameters.len()
    {
        return Err(replay_error(
            function,
            "cleanup plan entry ownership disagrees with the independent inventory",
        ));
    }
    for (id, place) in inventory
        .entry_state
        .live_owned_parameters
        .iter()
        .zip(&plan.entry_state.live_owned_parameters)
    {
        let slot = inventory
            .slots
            .iter()
            .find(|slot| &slot.id == id)
            .ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup inventory entry references unknown storage",
                )
            })?;
        if !place.projections.is_empty() || place.storage != inventory_storage_id(&slot.origin) {
            return Err(replay_error(
                function,
                "cleanup plan entry ownership disagrees with the independent inventory",
            ));
        }
    }
    let mut next_flag = u32::try_from(inventory.flags.len())
        .map_err(|_| replay_error(function, "too many inventory liveness flags"))?;
    let mut supplemental = Vec::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_supplemental(
            program,
            function,
            expression,
            &mut next_flag,
            &mut supplemental,
            budget,
        )?;
    }
    if plan.slots.len() - base_len != supplemental.len() {
        return Err(replay_error(
            function,
            "cleanup plan has missing or extra call-argument storage",
        ));
    }
    for (offset, (actual, expected)) in plan.slots[base_len..].iter().zip(&supplemental).enumerate()
    {
        let index = base_len
            .checked_add(offset)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| replay_error(function, "too many cleanup slots"))?;
        if actual.id.0 != index
            || actual.storage_index != index
            || actual.storage != expected.storage
            || actual.ty != expected.ty
            || actual.field_liveness_shape != expected.shape
        {
            return Err(replay_error(
                function,
                format_args!("supplemental call-argument slot {offset} disagrees with typed HIR"),
            ));
        }
    }
    Ok(())
}

fn parameter<'a>(
    program: &'a dyn ReplayContext,
    function: &ResolvedFunction,
    call: &'a ResolvedExpr,
    index: usize,
) -> Result<Option<(OwnershipMode, &'a ResolvedType)>, Diagnostic> {
    let ResolvedExprKind::Call {
        callee,
        args,
        type_arguments,
        ..
    } = &call.kind
    else {
        return Err(replay_error(
            function,
            "selected parameter receiver has no call",
        ));
    };
    if let Some(op) = crate::vec_ops::by_id(callee.as_str()) {
        let [element] = type_arguments.as_slice() else {
            return Err(replay_error(
                function,
                "selected Vec parameter lacks exact element",
            ));
        };
        let owned = *element == ResolvedType::Bytes
            || *element == ResolvedType::String
            || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element);
        return Ok(args.get(index).map(|arg| {
            (
                if owned {
                    op.param_ownership_for(index, element)
                } else {
                    op.param_ownership(index)
                },
                &arg.ty,
            )
        }));
    }
    if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
        return Ok(op
            .param_types()
            .get(index)
            .map(|ty| (op.param_ownership(index), ty)));
    }
    if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
        return Ok(op
            .param_types()
            .get(index)
            .map(|ty| (op.param_ownership(index), ty)));
    }
    if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
        return Ok(op.param_types().get(index).map(|ty| {
            (
                if *ty == ResolvedType::Str {
                    OwnershipMode::Borrow
                } else {
                    OwnershipMode::Value
                },
                ty,
            )
        }));
    }
    let target = program.resolve_call_target(callee, None).ok_or_else(|| {
        replay_error(
            function,
            "selected supplemental call has no authenticated body",
        )
    })?;
    Ok(target.params.get(index).map(|p| (p.ownership, &p.ty)))
}

fn collect_supplemental(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    next_flag: &mut u32,
    slots: &mut Vec<ExpectedSupplementalSlot>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    enum Frame<'a> {
        Expr(expression_children::ReplayChildCursor<'a>, usize),
        Argument(&'a ResolvedExpr, usize),
    }
    budget.reserve_array::<Frame<'_>>(function, 1028, "supplemental expression cursor")?;
    let mut frames = Vec::with_capacity(1028);
    if frames.capacity() != 1028 {
        return Err(replay_error(
            function,
            "supplemental expression cursor capacity disagrees with reservation",
        ));
    }
    frames.push(Frame::Expr(
        expression_children::ReplayChildCursor::new(expression),
        0,
    ));
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Expr(mut cursor, next) => {
                let expression = cursor.expression();
                if let ResolvedExprKind::Call {
                    callee,
                    instance,
                    type_arguments,
                    args,
                } = &expression.kind
                {
                    if instance.is_some()
                        || (!type_arguments.is_empty()
                            && crate::vec_ops::by_id(callee.as_str()).is_none())
                    {
                        return Err(replay_error(
                            function,
                            "selected supplemental call does not support generic instances",
                        ));
                    }
                    if parameter(program, function, expression, args.len())?.is_some()
                        || (!args.is_empty()
                            && parameter(program, function, expression, args.len() - 1)?.is_none())
                    {
                        return Err(replay_error(
                            function,
                            "selected supplemental call has inconsistent arity",
                        ));
                    }
                    if let Some(argument) = args.get(next) {
                        if frames.len() + 3 > frames.capacity() {
                            return Err(replay_error(
                                function,
                                "supplemental-slot traversal exceeds the admitted depth",
                            ));
                        }
                        frames.push(Frame::Expr(cursor, next + 1));
                        frames.push(Frame::Argument(expression, next));
                        frames.push(Frame::Expr(
                            expression_children::ReplayChildCursor::new(argument),
                            0,
                        ));
                    }
                } else {
                    if matches!(
                        expression.kind,
                        ResolvedExprKind::Invoke { .. }
                            | ResolvedExprKind::Closure { .. }
                            | ResolvedExprKind::NativeRustImportCall(_)
                            | ResolvedExprKind::HostCommandCall(_)
                            | ResolvedExprKind::LiteralFormat { .. }
                    ) {
                        return Err(replay_error(
                            function,
                            "selected supplemental inventory does not support delegated calls",
                        ));
                    }
                    if let Some(child) = cursor.next() {
                        if frames.len() + 2 > frames.capacity() {
                            return Err(replay_error(
                                function,
                                "supplemental-slot traversal exceeds the admitted depth",
                            ));
                        }
                        frames.push(Frame::Expr(cursor, next + 1));
                        frames.push(Frame::Expr(
                            expression_children::ReplayChildCursor::new(child),
                            0,
                        ));
                    }
                }
            }
            Frame::Argument(expression, index) => {
                let ResolvedExprKind::Call { args, .. } = &expression.kind else {
                    return Err(replay_error(
                        function,
                        "supplemental continuation lacks a call",
                    ));
                };
                let (ownership, ty) =
                    parameter(program, function, expression, index)?.ok_or_else(|| {
                        replay_error(function, "supplemental call parameter is missing")
                    })?;
                let argument = &args[index];
                if ownership == OwnershipMode::Own && matches!(ty, ResolvedType::Nominal { .. }) {
                    selected_profile::collections::prepay(
                        program,
                        function,
                        selected_profile::collections::type_key_bytes(ty)?,
                        budget,
                    )?;
                }
                if ownership == OwnershipMode::Own
                    && crate::cleanup::selected_inventory_needs_drop_in(program, ty)?
                {
                    if ty != &argument.ty {
                        return Err(replay_error(
                            function,
                            "supplemental owned argument type disagrees with its parameter",
                        ));
                    }
                    let shape = flat_records::shape(program, function, ty, next_flag, budget)?;
                    budget.reserve_clone(function, ty, "supplemental type backing")?;
                    budget.reserve_vec(function, slots, 1, "supplemental slot vector")?;
                    slots.push(ExpectedSupplementalSlot {
                        storage: StorageId::CallArgument {
                            call: expression.id.clone(),
                            parameter_index: u32::try_from(index)
                                .map_err(|_| replay_error(function, "too many call parameters"))?,
                            value_expression: argument.id.clone(),
                        },
                        ty: ty.clone(),
                        shape,
                    });
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn program() -> ResolvedProgram {
        let source = crate::check(
            r#"module replay.inventory_coverage;
@id("coverage.fill") fn fill(data:own Bytes)->Bytes
requires 0usize<byte_len(bytes_as_slice(data)) {
 let mut next=data; next=bytes_set(next,0usize,65u8); next
}
@id("coverage.main") fn main()->i64 {
 let mut data=bytes_zeroed(1usize); data=fill(data); let mut at=0;
 while at<2 {data=fill(data);at=at+1;if at==1 {continue;}else{0} 0}
 let view=bytes_as_slice(data); if byte_len(view)==1usize {42}else{0}
}"#,
            "replay-inventory-coverage.spx",
        )
        .unwrap();
        crate::hir::resolve(&source).unwrap()
    }
    fn run(
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        limit: usize,
    ) -> Result<(), Diagnostic> {
        validate_inventory_coverage(
            program,
            function,
            &mut ReplayBudget::with_physical(function, limit)?,
        )
    }
    #[test]
    fn independent_primitive_and_call_epoch_coverage_matches_legacy_exact_one_short() {
        let program = program();
        for function in program
            .functions
            .iter()
            .filter(|f| f.id.as_str().starts_with("coverage."))
        {
            super::super::ordinary_inventory::owned::validate_inventory_coverage(
                &program, function,
            )
            .unwrap();
            let (_, refused, bytes) = crate::bounded_output::with_limit_usage(usize::MAX, || {
                run(&program, function, usize::MAX).unwrap()
            });
            assert!(!refused);
            let (exact, refused, used) =
                crate::bounded_output::with_limit_usage(bytes, || run(&program, function, bytes));
            exact.unwrap();
            assert!(!refused);
            assert_eq!(used, bytes);
            let (short, refused, used) = crate::bounded_output::with_limit_usage(bytes - 1, || {
                run(&program, function, usize::MAX)
            });
            assert_eq!(short.unwrap_err().code, "SPX-H006");
            assert!(refused);
            assert!(used < bytes);
        }
    }
    #[test]
    fn attached_inventory_and_supplemental_slots_never_grant_authority() {
        let program = program();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "coverage.main")
            .unwrap();
        let mut forged = function.clone();
        forged.cleanup.slots[0].ty = ResolvedType::String;
        assert_eq!(
            run(&program, &forged, usize::MAX).unwrap_err().code,
            "SPX-H006"
        );
        let mut forged = function.clone();
        assert!(
            forged.cleanup_plan.slots.len() > forged.cleanup.slots.len(),
            "fixture must contain actual supplemental owned argument epochs"
        );
        forged.cleanup_plan.slots.pop();
        assert_eq!(
            run(&program, &forged, usize::MAX).unwrap_err().code,
            "SPX-H006"
        );
    }
}

pub(in crate::cleanup_plan::replay) mod paid;

#[cfg(test)]
mod typed_tests;
