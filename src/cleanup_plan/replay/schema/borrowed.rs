//! Guarded complete nonowning helper profile, bound to actual current members.
//! This guarded witness carries no replay verdict or production admission.
use super::*;
use crate::hir::{OwnershipMode, ResolvedStatement};
mod decision;
pub(in crate::cleanup_plan::replay) mod operands;
const DEPTH: usize = 514;
type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool)>;
#[derive(Clone, Copy)]
pub(in crate::cleanup_plan::replay) enum Call<'a> {
    Builtin(operands::Builtin),
    User(&'a ResolvedFunction),
}

pub(in crate::cleanup_plan::replay) struct Profile<'a> {
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    inventory: crate::cleanup::CleanupInventory,
    renewals: crate::cleanup_plan::ReplayRenewalBindings<'a>,
    schema_facts: decision::Facts,
    matches: Vec<&'a ResolvedExpr>,
    calls: Vec<(&'a ResolvedExpr, Call<'a>)>,
}
impl<'a> Profile<'a> {
    pub(in crate::cleanup_plan::replay) fn matches(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
    ) -> bool {
        std::ptr::eq(self.program, program) && std::ptr::eq(self.function, function)
    }
    pub(in crate::cleanup_plan::replay) fn inventory(&self) -> &crate::cleanup::CleanupInventory {
        &self.inventory
    }
    pub(in crate::cleanup_plan::replay) fn call(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<Call<'a>, Diagnostic> {
        if !self.matches(program, function) {
            return Err(fail(function));
        }
        for (candidate, call) in &self.calls {
            budget.charge(function, 1, "borrowed exact source call probe")?;
            if std::ptr::eq(*candidate, expression) {
                return Ok(*call);
            }
        }
        Err(fail(function))
    }
    pub(in crate::cleanup_plan::replay) fn call_by_id(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        id: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<(&'a ResolvedExpr, Call<'a>), Diagnostic> {
        if !self.matches(program, function) {
            return Err(fail(function));
        }
        for (expression, call) in &self.calls {
            budget.charge(function, 1, "borrowed actual call identity probe")?;
            if id_equal(function, expression.id.as_str(), id.as_str(), budget)? {
                return Ok((*expression, *call));
            }
        }
        Err(replay_error(function, "call commit expression is absent"))
    }
    pub(in crate::cleanup_plan::replay) fn no_drop(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        ty: &ResolvedType,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        if !self.matches(program, function) {
            return Err(fail(function));
        }
        budget.charge(function, 1, "borrowed view no-drop type header")?;
        if matches!(ty, ResolvedType::Str | ResolvedType::SliceU8) {
            return Ok(false);
        }
        if !copy_type(program, function, ty, budget)? {
            return Err(fail(function));
        }
        Ok(false)
    }
    pub(in crate::cleanup_plan::replay) fn match_shape(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        if !self.matches(program, function) {
            return Err(fail(function));
        }
        for candidate in &self.matches {
            budget.charge(function, 1, "borrowed exact source match probe")?;
            if std::ptr::eq(*candidate, expression) {
                return Ok(false);
            }
        }
        Err(fail(function))
    }
    pub(in crate::cleanup_plan::replay) fn validate_inventory(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        budget.charge(function, 1, "borrowed inventory actual member")?;
        if !self.matches(program, function) {
            return Err(fail(function));
        }
        // The actual constructor has proved no storage. Validate all receiving
        // headers before any traversal; hostile nonempty tables grant no proof.
        budget.charge(function, 1, "borrowed reconstructed inventory headers")?;
        if !self.inventory.slots.is_empty()
            || !self.inventory.flags.is_empty()
            || !self.inventory.entry_state.live_owned_parameters.is_empty()
            || !self
                .inventory
                .entry_state
                .conditional_owned_parameters
                .is_empty()
        {
            return Err(fail(function));
        }
        budget.charge(function, 1, "borrowed attached inventory headers")?;
        let actual = &function.cleanup;
        if !actual.slots.is_empty()
            || !actual.flags.is_empty()
            || !actual.entry_state.live_owned_parameters.is_empty()
            || !actual.entry_state.conditional_owned_parameters.is_empty()
            || !id_equal(function, self.inventory.schema, actual.schema, budget)?
        {
            return Err(fail(function));
        }
        Ok(())
    }
    pub(in crate::cleanup_plan::replay) fn renewals(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
    ) -> Option<&crate::cleanup_plan::ReplayRenewalBindings<'a>> {
        self.matches(program, function).then_some(&self.renewals)
    }
}
fn fail(function: &ResolvedFunction) -> Diagnostic {
    replay_error(function, "budgeted borrowed helper profile is not complete")
}
fn copy_type(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "borrowed helper Copy classification")?;
    if matches!(ty, ResolvedType::Nominal { .. }) {
        crate::hir::borrowed_scalar::option_u8::authenticated_metered(
            program.declarations(),
            ty,
            &mut |units| budget.charge(function, units, "borrowed helper exact Option receiver"),
        )
    } else {
        Ok(crate::hir::is_scalar_resolved_type(ty))
    }
}
pub(in crate::cleanup_plan::replay) fn same_schema(
    function: &ResolvedFunction,
    a: &str,
    b: &str,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    id_equal(function, a, b, budget)
}
fn id_equal(
    function: &ResolvedFunction,
    a: &str,
    b: &str,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "borrowed helper member identity length")?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.bytes().zip(b.bytes()) {
        budget.charge(function, 1, "borrowed helper member identity byte")?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn call<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    callee: &crate::hir::DeclarationId,
    args: &[ResolvedExpr],
    result: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<Call<'a>, Diagnostic> {
    if let Some(operation) = operands::builtin(function, callee.as_str(), budget)? {
        return Ok(Call::Builtin(operation));
    }
    let mut target = None;
    for index in 0..program.function_count() {
        budget.charge(function, 1, "borrowed helper actual callee probe")?;
        let current = program.function_at(index).ok_or_else(|| fail(function))?;
        if id_equal(function, current.id.as_str(), callee.as_str(), budget)?
            && target.replace(current).is_some()
        {
            return Err(fail(function));
        }
    }
    let target = target.ok_or_else(|| fail(function))?;
    budget.charge(function, 1, "borrowed helper current callee header")?;
    if !crate::hir::is_scalar_resolved_type(&target.return_type)
        || &target.return_type != result
        || !target.effects.is_empty()
        || target.yields.is_some()
        || args.len() != target.params.len()
    {
        return Err(fail(function));
    }
    for (argument, parameter) in args.iter().zip(&target.params) {
        budget.charge(function, 1, "borrowed helper actual callee parameter")?;
        if !crate::hir::borrowed_scalar::parameter(&parameter.ty, parameter.ownership)
            || argument.ty != parameter.ty
            || argument.ownership != parameter.ownership
        {
            return Err(fail(function));
        }
    }
    Ok(Call::User(target))
}
pub(in crate::cleanup_plan::replay) fn build<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Profile<'a>, Diagnostic> {
    budget.charge(function, 1, "borrowed helper current context header")?;
    if !budget.selected()
        || !program.interfaces().is_empty()
        || !program.function_templates().is_empty()
        || !program.function_instances().is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
        || !crate::hir::is_scalar_resolved_type(&function.return_type)
    {
        return Err(fail(function));
    }
    let mut found = false;
    for index in 0..program.function_count() {
        budget.charge(function, 1, "borrowed helper actual member probe")?;
        let member = program.function_at(index).ok_or_else(|| fail(function))?;
        if std::ptr::eq(member, function) {
            if found {
                return Err(fail(function));
            }
            found = true;
        }
    }
    if !found {
        return Err(fail(function));
    }
    let mut schema_facts = decision::Facts::default();
    let mut matches = Vec::new();
    let mut calls = Vec::new();
    schema_facts.ty(true);
    for parameter in &function.params {
        budget.charge(function, 1, "borrowed helper parameter profile")?;
        if !crate::hir::borrowed_scalar::parameter(&parameter.ty, parameter.ownership) {
            return Err(fail(function));
        }
        schema_facts.binding(parameter.ownership, true);
    }
    // Own-root absence is established by the complete typed walk below. The
    // attached empty loan proof is then a consistency guard, not its authority.
    budget.charge(function, 1, "borrowed exact loan headers")?;
    if !id_equal(
        function,
        function.loan_plan.schema,
        crate::loan_plan::LOAN_PLAN_SCHEMA_V1,
        budget,
    )? || !function.loan_plan.loans.is_empty()
        || !function.loan_plan.endpoints.is_empty()
        || !function.loan_plan.edges.is_empty()
    {
        return Err(fail(function));
    }
    budget.reserve_array::<Frame<'_>>(
        function,
        DEPTH,
        "borrowed helper complete profile cursor",
    )?;
    let mut frames = [None; DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        frames[0] = Some((crate::hir::lexical_exit::ChildCursor::new(root), false));
        let mut depth = 1;
        while depth != 0 {
            budget.charge(function, 1, "borrowed helper complete profile frame")?;
            let (mut cursor, entered) = frames[depth - 1].take().ok_or_else(|| fail(function))?;
            let expression = cursor.expression();
            if !entered {
                let admitted_type =
                    matches!(expression.ty, ResolvedType::Str | ResolvedType::SliceU8)
                        || copy_type(program, function, &expression.ty, budget)?;
                schema_facts.expression(expression, admitted_type, function, budget)?;
                if !admitted_type {
                    return Err(fail(function));
                }
                if !matches!(expression.kind, ResolvedExprKind::Match { .. })
                    && !operands::expression(function, expression, budget)?
                {
                    return Err(fail(function));
                }
                match &expression.kind {
                    ResolvedExprKind::Int(_)
                    | ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::Unary { .. }
                    | ResolvedExprKind::Binary { .. }
                    | ResolvedExprKind::If { .. }
                    | ResolvedExprKind::BorrowPlace { .. } => {}
                    ResolvedExprKind::Place(place) if place.projections.is_empty() => {}
                    ResolvedExprKind::Call {
                        callee,
                        instance: None,
                        type_arguments,
                        args,
                    } if type_arguments.is_empty() => {
                        let call = call(program, function, callee, args, &expression.ty, budget)?;
                        budget.reserve_vec(
                            function,
                            &mut calls,
                            1,
                            "borrowed exact call references",
                        )?;
                        calls.push((expression, call));
                    }
                    ResolvedExprKind::Match {
                        mode,
                        scrutinee,
                        arms,
                    } if matches!(scrutinee.ty, ResolvedType::Nominal { .. })
                        && copy_type(program, function, &scrutinee.ty, budget)? =>
                    {
                        budget.charge(function, 1, "borrowed Option match header")?;
                        if *mode != crate::hir::ResolvedMatchMode::Value
                            || expression.ownership != OwnershipMode::Value
                            || !crate::hir::is_scalar_resolved_type(&expression.ty)
                            || arms.len() != 2
                        {
                            return Err(fail(function));
                        }
                        for arm in arms {
                            budget.charge(function, 1, "borrowed Option arm header")?;
                            if arm.guard.is_some()
                                || !crate::hir::borrowed_scalar::option_u8::pattern_metered(
                                    &arm.pattern,
                                    &mut |units| {
                                        budget.charge(
                                            function,
                                            units,
                                            "borrowed exact Option pattern",
                                        )
                                    },
                                )?
                            {
                                return Err(fail(function));
                            }
                        }
                        let (
                            ResolvedMatchPattern::Variant { case: a, .. },
                            ResolvedMatchPattern::Variant { case: b, .. },
                        ) = (&arms[0].pattern, &arms[1].pattern)
                        else {
                            return Err(fail(function));
                        };
                        if id_equal(function, a.as_str(), b.as_str(), budget)? {
                            return Err(fail(function));
                        }
                        budget.reserve_vec(
                            function,
                            &mut matches,
                            1,
                            "borrowed exact match references",
                        )?;
                        matches.push(expression);
                    }
                    ResolvedExprKind::Match {
                        mode,
                        scrutinee,
                        arms,
                    } if operands::scalar_match(
                        function, expression, *mode, scrutinee, arms, budget,
                    )? =>
                    {
                        budget.reserve_vec(
                            function,
                            &mut matches,
                            1,
                            "borrowed exact scalar match references",
                        )?;
                        matches.push(expression);
                    }
                    ResolvedExprKind::Block { statements, .. } => {
                        for statement in statements {
                            budget.charge(
                                function,
                                1,
                                "borrowed helper complete statement profile",
                            )?;
                            let binding_copy = match statement {
                                ResolvedStatement::Let { binding, .. }
                                | ResolvedStatement::Assign { binding, .. } => {
                                    let admitted =
                                        copy_type(program, function, &binding.ty, budget)?;
                                    schema_facts.binding(binding.ownership, admitted);
                                    Some(admitted)
                                }
                                _ => None,
                            };
                            match statement {
                                ResolvedStatement::Let { binding, .. }
                                | ResolvedStatement::Assign {
                                    binding,
                                    field: None,
                                    ..
                                } if binding.ownership == OwnershipMode::Value
                                    && binding_copy == Some(true) => {}
                                ResolvedStatement::While { .. } => {}
                                _ => return Err(fail(function)),
                            }
                        }
                    }
                    _ => return Err(fail(function)),
                }
            }
            if let Some(child) = cursor.next() {
                if depth == DEPTH {
                    return Err(fail(function));
                }
                frames[depth - 1] = Some((cursor, true));
                frames[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    // Temporarily move the sole backing owner so constructor work can debit the
    // active replay ledger. Restore it on every Result path; no credits return.
    let mut physical = budget.physical.take().ok_or_else(|| fail(function))?;
    let result = (|| {
        let sites = crate::loop_calls::byte_writer::checked_absent_with_work_reservation(
            program,
            function,
            &mut physical,
            &mut |units| budget.charge(function, units, "borrowed helper exact writer census"),
        )?;
        if !sites.is_empty() {
            return Err(fail(function));
        }
        crate::cleanup::build_borrowed_helper_inventory_with_budget(
            program,
            function,
            &mut physical,
            &mut |units| budget.charge(function, units, "borrowed helper independent inventory"),
        )
    })();
    budget.admission = AdmissionMode::SelectedRetained;
    budget.payer = AllocationPayer::SelectedReservation;
    budget.physical = Some(physical);
    let inventory = result?;
    budget.reserve_array::<Profile<'_>>(function, 1, "borrowed helper held profile")?;
    Ok(Profile {
        program,
        function,
        inventory,
        // Every other renewal family requires an owning Bytes/String/Vec/Step
        // binding or lexical owning loop, all excluded by the complete walk.
        // Writer absence additionally retains its original census limits above.
        renewals: crate::cleanup_plan::ReplayRenewalBindings::empty(),
        schema_facts,
        matches,
        calls,
    })
}
#[cfg(test)]
mod tests;
