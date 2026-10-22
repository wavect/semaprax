//! Exact selected structural twin; complete original remains in the parent owner.
// Borrowed call signatures and one precharged current-HIR lookup cursor.
use super::super::call_receiving::{Kind as CallKind, Policy};
use super::*;
use crate::cleanup_plan::replay::inventory_coverage::paid::receive;
use crate::cleanup_plan::CallArgumentTransfer;

pub(super) use super::super::calls::CallCursor;

macro_rules! call_refusal {
    ($budget:ident,$function:ident,$permit:ident,$value:ty,$kind:expr) => {{
        $budget.reserve_array::<(
            &super::super::call_receiving::Permit<'_, Error>,
            Result<$value, Error>,
        )>($function, 1, "selected call closure receiving")?;
        $budget.charge($function, 2, "selected call closure receiving")?;
        || Error::static_error(&$permit, $kind)
    }};
}
macro_rules! call_index_refusal {
    ($budget:ident,$function:ident,$permit:ident,$value:ty,$kind:expr) => {{
        $budget.reserve_array::<(
            &super::super::call_receiving::Permit<'_, Error>,
            std::num::TryFromIntError,
            Result<$value, Error>,
        )>($function, 1, "selected call index closure receiving")?;
        $budget.charge($function, 3, "selected call index closure receiving")?;
        |_| Error::static_error(&$permit, $kind)
    }};
}
enum Signature<'a> {
    Byte(crate::byte_ops::ByteOp),
    String(crate::string_ops::StringOp),
    Str(crate::str_ops::StrOp),
    Function(&'a ResolvedFunction),
    Vector(
        crate::vec_ops::VecOp,
        bool,
        &'a ResolvedType,
        &'a [ResolvedExpr],
    ),
    Field(&'a [ResolvedExpr]),
}
// Inline pure signature queries avoid inventing fallible forwarding Results.
// Every exact new binding/result is reserved before the borrowed query expands.
macro_rules! signature_parameter {
    ($signature:ident,$index:ident,$budget:ident,$function:ident) => {{
        // Match on &Signature: stored references bind as double references.
        // Each Some arm has its own binding; no closure/capture is introduced.
        $budget.reserve_array::<(
            &Signature<'_>,
            usize,
            &crate::byte_ops::ByteOp,
            &crate::string_ops::StringOp,
            &crate::str_ops::StrOp,
            &&ResolvedFunction,
            &crate::vec_ops::VecOp,
            &bool,
            &&ResolvedType,
            &&[ResolvedExpr],
            &&[ResolvedExpr],
            &ResolvedType,
            &ResolvedType,
            &ResolvedType,
            &crate::hir::ResolvedParam,
            &ResolvedExpr,
            &ResolvedExpr,
            Option<(OwnershipMode, &ResolvedType)>,
        )>($function, 1, "selected signature parameter query")?;
        $budget.charge($function, 18, "selected signature parameter query")?;
        match &$signature {
            Signature::Byte(op) => match dynamic_get!(
                $budget,
                $function,
                op.param_types(),
                &[ResolvedType],
                $index,
                usize,
                Option<&ResolvedType>,
                (&ResolvedType),
                4
            ) {
                Some(ty) => Some((op.param_ownership($index), ty)),
                None => None,
            },
            Signature::String(op) => match dynamic_get!(
                $budget,
                $function,
                op.param_types(),
                &[ResolvedType],
                $index,
                usize,
                Option<&ResolvedType>,
                (&ResolvedType),
                4
            ) {
                Some(ty) => Some((op.param_ownership($index), ty)),
                None => None,
            },
            Signature::Str(op) => match dynamic_get!(
                $budget,
                $function,
                op.param_types(),
                &[ResolvedType],
                $index,
                usize,
                Option<&ResolvedType>,
                (&ResolvedType),
                4
            ) {
                Some(ty) => Some((
                    if *ty == ResolvedType::Str {
                        OwnershipMode::Borrow
                    } else {
                        OwnershipMode::Value
                    },
                    ty,
                )),
                None => None,
            },
            Signature::Function(target) => match dynamic_get!(
                $budget,
                $function,
                target.params,
                &[crate::hir::ResolvedParam],
                $index,
                usize,
                Option<&crate::hir::ResolvedParam>,
                (&crate::hir::ResolvedParam),
                4
            ) {
                Some(p) => Some((p.ownership, &p.ty)),
                None => None,
            },
            Signature::Vector(op, owned, element, args) => match dynamic_get!(
                $budget,
                $function,
                args,
                &[ResolvedExpr],
                $index,
                usize,
                Option<&ResolvedExpr>,
                (&ResolvedExpr),
                4
            ) {
                Some(arg) => Some((
                    if *owned {
                        op.param_ownership_for($index, element)
                    } else {
                        op.param_ownership($index)
                    },
                    &arg.ty,
                )),
                None => None,
            },
            Signature::Field(args) => match dynamic_get!(
                $budget,
                $function,
                args,
                &[ResolvedExpr],
                $index,
                usize,
                Option<&ResolvedExpr>,
                (&ResolvedExpr),
                4
            ) {
                Some(arg) => Some((
                    if $index == 0 {
                        OwnershipMode::Borrow
                    } else {
                        OwnershipMode::Value
                    },
                    &arg.ty,
                )),
                None => None,
            },
        }
    }};
}
macro_rules! signature_accepts_type {
    ($signature:ident,$index:ident,$ty:expr,$budget:ident,$function:ident) => {{
        $budget.reserve_array::<(
            &Signature<'_>,
            usize,
            &ResolvedType,
            &crate::byte_ops::ByteOp,
            &ResolvedType,
            bool,
        )>($function, 1, "selected signature type query")?;
        $budget.charge($function, 6, "selected signature type query")?;

        match &$signature {
            // ArrayAsSlice carries a catalog placeholder extent; authenticate
            // its actual fixed extent through the owning builtin classifier.
            Signature::Byte(op) => op.accepts_resolved($index, $ty),
            _ => match signature_parameter!($signature, $index, $budget, $function) {
                Some((_, expected)) => expected == $ty,
                None => false,
            },
        }
    }};
}
macro_rules! signature_len {
    ($signature:ident,$budget:ident,$function:ident) => {{
        $budget.reserve_array::<(
            &Signature<'_>,
            &crate::byte_ops::ByteOp,
            &crate::string_ops::StringOp,
            &crate::str_ops::StrOp,
            &&ResolvedFunction,
            &&[ResolvedExpr],
            usize,
        )>($function, 1, "selected signature arity query")?;
        $budget.charge($function, 7, "selected signature arity query")?;

        match &$signature {
            Signature::Byte(op) => op.param_types().len(),
            Signature::String(op) => op.param_types().len(),
            Signature::Str(op) => op.param_types().len(),
            Signature::Function(target) => target.params.len(),
            Signature::Vector(_, _, _, args) | Signature::Field(args) => args.len(),
        }
    }};
}

fn needs_drop(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a dyn crate::cleanup::ReplayContext,
        &'a ResolvedFunction,
        &'a ResolvedType,
        &'a mut ReplayBudget,
    );
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected needs_drop local receiving")?;
    budget.charge(function, 4, "selected needs_drop local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive_wrapper!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<bool>(function, budget)
    )?;
    if matches!(ty, ResolvedType::Nominal { .. }) {
        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &dyn crate::cleanup::ReplayContext,
                &ResolvedFunction,
                usize,
                &mut ReplayBudget,
                Result<(), Error>
            ),
            5,
            "selected control prepay child receiving",
            selected_profile::paid_children::prepay(
                program,
                function,
                crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &ResolvedType,
                        &ResolvedFunction,
                        &mut ReplayBudget,
                        Result<usize, Error>
                    ),
                    4,
                    "selected control type_key_bytes child receiving",
                    selected_profile::paid_children::type_key_bytes(ty, function, budget)
                )?,
                budget,
            )
        )?;
    }
    Ok(
        crate::cleanup_plan::replay::inventory_coverage::paid::queries::needs_drop_in!(
            permit, function, budget, program, ty
        ),
    )
}

fn signature<'a>(
    program: &'a dyn ReplayContext,
    function: &ResolvedFunction,
    fact: &CallFact<'_>,
    expression: &'a ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Signature<'a>, Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a dyn crate::cleanup::ReplayContext,
        &'a ResolvedFunction,
        &'a CallFact<'a>,
        &'a ResolvedExpr,
        &'a mut ReplayBudget,
        &'a DeclarationId,
        &'a [ResolvedExpr],
        &'a [ResolvedType],
        bool,
        &'a ResolvedFunction,
        crate::vec_ops::VecOp,
    );
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected signature local receiving")?;
    budget.charge(function, 11, "selected signature local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &mut ReplayBudget,
            Result<super::super::call_receiving::Permit<'_, Error>, Error>
        ),
        3,
        "selected control signature permit receiving",
        Error::enter::<Signature<'a>>(function, budget)
    )?;
    if matches!(expression.kind, ResolvedExprKind::VecFieldRead { .. }) {
        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &dyn crate::cleanup::ReplayContext,
                &ResolvedFunction,
                &ResolvedExpr,
                &mut ReplayBudget,
                Result<(), Error>
            ),
            5,
            "selected control field child receiving",
            Error::field(program, function, expression, budget)
        )?;
        let ResolvedExprKind::VecFieldRead { args, .. } = &expression.kind else {
            unreachable!()
        };
        return Ok(Signature::Field(args));
    }
    if let Some(owned) = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction,
            &ResolvedExpr,
            &mut ReplayBudget,
            Result<Option<bool>, Error>
        ),
        5,
        "selected control call_facts child receiving",
        Error::call_facts(program, function, expression, budget)
    )? {
        let ResolvedExprKind::Call {
            callee,
            type_arguments,
            args,
            ..
        } = &expression.kind
        else {
            unreachable!()
        };
        return Ok(Signature::Vector(
            crate::vec_ops::by_id(callee.as_str()).expect("authenticated Vec operation"),
            owned,
            &type_arguments[0],
            args,
        ));
    }
    if fact.instance.is_some() || !fact.type_arguments.is_empty() {
        return Err(Error::static_error(
            &permit,
            CallKind::SelectedCallCommitDoesNotSupportGenericCalls,
        ));
    }
    if let Some(op) = crate::byte_ops::by_id(fact.callee.as_str()) {
        return Ok(Signature::Byte(op));
    }
    if let Some(op) = crate::string_ops::by_id(fact.callee.as_str()) {
        if op == crate::string_ops::StringOp::Take {
            crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                budget,
                function,
                (
                    &ResolvedFunction,
                    &ResolvedExpr,
                    &mut ReplayBudget,
                    Result<Option<&ResolvedExpr>, Error>
                ),
                4,
                "selected control string_operand child receiving",
                Error::string_operand(function, expression, budget)
            )?
            .ok_or_else(call_refusal!(
                budget,
                function,
                permit,
                &ResolvedExpr,
                CallKind::SelectedStringTransferCallIdentityDiffers
            ))?;
        }
        return Ok(Signature::String(op));
    }
    if let Some(op) = crate::str_ops::by_id(fact.callee.as_str()) {
        return Ok(Signature::Str(op));
    }
    budget.charge(
        function,
        program.functions().len(),
        "structural current callee lookup",
    )?;
    let Some(target) = receive!(
        budget,
        function,
        (
            &dyn ReplayContext,
            &DeclarationId,
            Option<&FunctionInstanceId>,
            Option<&ResolvedFunction>,
            &ResolvedFunction
        ),
        5,
        "selected current call target receiving",
        program.resolve_call_target(&fact.callee, None)
    ) else {
        return Err(Error::missing_callee(&permit, &fact.callee));
    };
    if !target.effects.is_empty() || target.yields.is_some() {
        return Err(Error::static_error(
            &permit,
            CallKind::SelectedCallCommitDoesNotSupportEffectfulCallees,
        ));
    }
    Ok(Signature::Function(target))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn validate<'a>(
    program: &dyn ReplayContext,
    function: &'a ResolvedFunction,
    call: &ExpressionId,
    fact: &CallFact<'_>,
    arguments: &[CallArgumentTransfer],
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    cursor: Option<&mut CallCursor<'a>>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a dyn crate::cleanup::ReplayContext,
        &'a ResolvedFunction,
        &'a ExpressionId,
        &'a CallFact<'a>,
        &'a [crate::cleanup_plan::CallArgumentTransfer],
        &'a BTreeSet<StorageId>,
        &'a Leaves,
        Option<&'a mut super::super::calls::CallCursor<'a>>,
        &'a mut ReplayBudget,
        &'a ResolvedExpr,
        &'a DeclarationId,
        &'a [ResolvedType],
        &'a [ResolvedExpr],
        Signature<'a>,
        std::slice::Iter<'a, crate::cleanup_plan::CallArgumentTransfer>,
        usize,
        &'a ResolvedExpr,
        OwnershipMode,
        &'a ResolvedType,
        bool,
        &'a crate::cleanup_plan::CallArgumentTransfer,
        usize,
        &'a crate::cleanup_plan::CallArgumentTransfer,
        std::slice::Iter<'a, ResolvedExpr>,
        std::iter::Enumerate<std::slice::Iter<'a, ResolvedExpr>>,
        std::iter::Take<std::iter::Enumerate<std::slice::Iter<'a, ResolvedExpr>>>,
        Option<u32>,
        BTreeSet<LivenessFlagId>,
        LivenessFlagId,
        std::vec::IntoIter<LivenessFlagId>,
    );
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected validate local receiving")?;
    budget.charge(function, 30, "selected validate local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &mut ReplayBudget,
            Result<super::super::call_receiving::Permit<'_, Error>, Error>
        ),
        3,
        "selected control existing call permit receiving",
        Error::enter::<()>(function, budget)
    )?;
    #[cfg(test)]
    Error::borrowed_controls(function, budget)?;
    #[cfg(test)]
    let borrowed = program.borrowed_helper_lease(function);
    #[cfg(test)]
    let held = match &borrowed {
        Some(lease) => Some(lease.call_by_id(program, function, call, budget)?),
        None => None,
    };
    #[cfg(test)]
    if held.is_none() {
        budget.reserve_array::<(
            Option<&mut super::super::calls::CallCursor<'_>>,
            &mut super::super::calls::CallCursor<'_>,
            &ResolvedFunction,
            &ExpressionId,
            &mut ReplayBudget,
            Result<&mut super::super::calls::CallCursor<'_>, Error>,
            Result<&ResolvedExpr, Error>,
            &ResolvedExpr,
        )>(function, 1, "selected existing call lookup receiving")?;
        budget.charge(function, 8, "selected existing call lookup receiving")?;
    }
    #[cfg(not(test))]
    {
        budget.reserve_array::<(
            Option<&mut super::super::calls::CallCursor<'_>>,
            &mut super::super::calls::CallCursor<'_>,
            &ResolvedFunction,
            &ExpressionId,
            &mut ReplayBudget,
            Result<&mut super::super::calls::CallCursor<'_>, Error>,
            Result<&ResolvedExpr, Error>,
            &ResolvedExpr,
        )>(function, 1, "selected existing call lookup receiving")?;
        budget.charge(function, 8, "selected existing call lookup receiving")?;
    }
    #[cfg(test)]
    let expression = match &held {
        Some((expression, _)) => *expression,
        None => cursor
            .ok_or_else(call_refusal!(
                budget,
                function,
                permit,
                &mut CallCursor<'_>,
                CallKind::SelectedCallLacksItsLookupCursor
            ))?
            .find::<Error>(function, call, budget)?,
    };
    #[cfg(not(test))]
    let expression = cursor
        .ok_or_else(call_refusal!(
            budget,
            function,
            permit,
            &mut CallCursor<'_>,
            CallKind::SelectedCallLacksItsLookupCursor
        ))?
        .find::<Error>(function, call, budget)?;
    let (callee, type_arguments, instance, args) = match &expression.kind {
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => (
            callee.as_str(),
            type_arguments.as_slice(),
            instance.as_ref(),
            args.as_slice(),
        ),
        ResolvedExprKind::VecFieldRead { args, .. } => (
            crate::vec_field::operation_id().as_str(),
            &[][..],
            None,
            args.as_slice(),
        ),
        _ => {
            return Err(Error::static_error(
                &permit,
                CallKind::CallCommitDoesNotNameASelectedOrdinaryCall,
            ));
        }
    };
    #[cfg(test)]
    let agrees = if held.is_some() {
        budget.charge(function, 1, "borrowed commit actual fact headers")?;
        let mut agrees = instance.is_none()
            && fact.instance.is_none()
            && type_arguments.is_empty()
            && fact.type_arguments.is_empty()
            && args.len() == fact.arguments.len()
            && schema::borrowed::same_schema(function, callee, fact.callee.as_str(), budget)?;
        if agrees {
            receive!(
                budget,
                function,
                (
                    std::slice::Iter<'_, ResolvedExpr>,
                    crate::cleanup_plan::replay::call_payload::ArgumentIds<'_>,
                    std::iter::Zip<
                        std::slice::Iter<'_, ResolvedExpr>,
                        crate::cleanup_plan::replay::call_payload::ArgumentIds<'_>,
                    >
                ),
                3,
                "selected metered argument iterator headers",
                ()
            );
            let mut argument_entries = args.iter().zip(&fact.arguments);
            while let Some((arg, id)) = dynamic_next!(
                budget,
                function,
                argument_entries,
                std::iter::Zip<
                    std::slice::Iter<'_, ResolvedExpr>,
                    crate::cleanup_plan::replay::call_payload::ArgumentIds<'_>,
                >,
                (&ResolvedExpr, &ExpressionId),
                (&ResolvedExpr, &ExpressionId),
                4
            ) {
                budget.charge(function, 1, "borrowed commit actual argument probe")?;
                if !schema::borrowed::same_schema(function, arg.id.as_str(), id.as_str(), budget)? {
                    agrees = false;
                    break;
                }
            }
        }
        agrees
    } else {
        callee == fact.callee.as_str()
            && type_arguments == fact.type_arguments.as_slice()
            && instance == fact.instance
            && args.len() == fact.arguments.len()
            && receive!(
                budget,
                function,
                (
                    std::slice::Iter<'_, ResolvedExpr>,
                    crate::cleanup_plan::replay::call_payload::ArgumentIds<'_>,
                    std::iter::Zip<
                        std::slice::Iter<'_, ResolvedExpr>,
                        crate::cleanup_plan::replay::call_payload::ArgumentIds<'_>,
                    >,
                    Option<(&ResolvedExpr, &ExpressionId)>,
                    &ResolvedExpr,
                    &ExpressionId,
                    bool,
                    bool
                ),
                8,
                "selected call argument equality scan receiving",
                {
                    let mut same = true;
                    let mut argument_entries = args.iter().zip(&fact.arguments);
                    while let Some((arg, id)) = dynamic_next!(
                        budget,
                        function,
                        argument_entries,
                        std::iter::Zip<
                            std::slice::Iter<'_, ResolvedExpr>,
                            crate::cleanup_plan::replay::call_payload::ArgumentIds<'_>,
                        >,
                        (&ResolvedExpr, &ExpressionId),
                        (&ResolvedExpr, &ExpressionId),
                        4
                    ) {
                        if &arg.id != id {
                            same = false;
                            break;
                        }
                    }
                    same
                }
            )
    };
    #[cfg(not(test))]
    let agrees = callee == fact.callee.as_str()
        && type_arguments == fact.type_arguments.as_slice()
        && instance == fact.instance
        && args.len() == fact.arguments.len()
        && receive!(
            budget,
            function,
            (
                std::slice::Iter<'_, ResolvedExpr>,
                crate::cleanup_plan::replay::call_payload::ArgumentIds<'_>,
                std::iter::Zip<
                    std::slice::Iter<'_, ResolvedExpr>,
                    crate::cleanup_plan::replay::call_payload::ArgumentIds<'_>,
                >,
                Option<(&ResolvedExpr, &ExpressionId)>,
                &ResolvedExpr,
                &ExpressionId,
                bool,
                bool
            ),
            8,
            "selected call argument equality scan receiving",
            {
                let mut same = true;
                let mut argument_entries = args.iter().zip(&fact.arguments);
                while let Some((arg, id)) = dynamic_next!(
                    budget,
                    function,
                    argument_entries,
                    std::iter::Zip<
                        std::slice::Iter<'_, ResolvedExpr>,
                        crate::cleanup_plan::replay::call_payload::ArgumentIds<'_>,
                    >,
                    (&ResolvedExpr, &ExpressionId),
                    (&ResolvedExpr, &ExpressionId),
                    4
                ) {
                    if &arg.id != id {
                        same = false;
                        break;
                    }
                }
                same
            }
        );
    if !agrees {
        return Err(Error::static_error(
            &permit,
            CallKind::CallCommitFactsDisagreeWithCurrentTypedCall,
        ));
    }
    #[cfg(test)]
    let signature = match &held {
        Some((_, call)) => {
            use schema::borrowed::{operands::Builtin, Call};
            match call {
                Call::User(target) => Signature::Function(target),
                Call::Builtin(Builtin::FromU8) => {
                    Signature::String(crate::string_ops::StringOp::I64FromU8)
                }
                Call::Builtin(Builtin::Get) => Signature::Byte(crate::byte_ops::ByteOp::Get),
                Call::Builtin(Builtin::Len) => Signature::Byte(crate::byte_ops::ByteOp::Len),
                Call::Builtin(Builtin::Str(op)) => Signature::Str(*op),
            }
        }
        None => crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &dyn crate::cleanup::ReplayContext,
                &ResolvedFunction,
                &CallFact<'_>,
                &ResolvedExpr,
                &mut ReplayBudget,
                Result<Signature<'_>, Error>
            ),
            6,
            "selected control signature call receiving",
            signature(program, function, fact, expression, budget)
        )?,
    };
    #[cfg(not(test))]
    let signature = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        (
            &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction,
            &CallFact<'_>,
            &ResolvedExpr,
            &mut ReplayBudget,
            Result<Signature<'_>, Error>
        ),
        6,
        "selected control signature call receiving",
        signature(program, function, fact, expression, budget)
    )?;
    if signature_len!(signature, budget, function) != fact.arguments.len() {
        return Err(Error::static_error(
            &permit,
            CallKind::CallCommitCalleeSignatureHasInconsistentArity,
        ));
    }
    let mut actual = arguments.iter();
    let mut parameter_entries = args
        .iter()
        .enumerate()
        .take(signature_len!(signature, budget, function));
    while let Some((index, arg)) = dynamic_next!(
        budget,
        function,
        parameter_entries,
        std::iter::Take<std::iter::Enumerate<std::slice::Iter<'_, ResolvedExpr>>>,
        (usize, &ResolvedExpr),
        (usize, &ResolvedExpr),
        4
    ) {
        budget.charge(function, 1, "structural call parameters")?;
        let (mode, ty) = signature_parameter!(signature, index, budget, function)
            .expect("exact signature bound");
        #[cfg(test)]
        let drop = if held.is_some() {
            nested_shape::type_needs_drop_with_budget(program, function, ty, budget)?
        } else {
            crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                budget,
                function,
                (
                    &dyn crate::cleanup::ReplayContext,
                    &ResolvedFunction,
                    &ResolvedType,
                    &mut ReplayBudget,
                    Result<bool, Error>
                ),
                5,
                "selected control needs_drop call receiving",
                needs_drop(program, function, ty, budget)
            )?
        };
        #[cfg(not(test))]
        let drop = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &dyn crate::cleanup::ReplayContext,
                &ResolvedFunction,
                &ResolvedType,
                &mut ReplayBudget,
                Result<bool, Error>
            ),
            5,
            "selected control needs_drop call receiving",
            needs_drop(program, function, ty, budget)
        )?;
        if !signature_accepts_type!(signature, index, &arg.ty, budget, function)
            || !match mode {
                OwnershipMode::Own => arg.ownership == OwnershipMode::Own,
                OwnershipMode::Borrow => {
                    matches!(arg.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
                }
                OwnershipMode::Value => arg.ownership == OwnershipMode::Value,
                OwnershipMode::Shared => false,
            }
        {
            return Err(Error::static_error(
                &permit,
                CallKind::CallCommitArgumentDisagreesWithExactParameterTypeOrOwnership,
            ));
        }
        if mode == OwnershipMode::Own && drop {
            let expected = u32::try_from(index).map_err(call_index_refusal!(
                budget,
                function,
                permit,
                u32,
                CallKind::TooManyCallParameters
            ))?;
            if receive!(
                budget,
                function,
                (Option<&CallArgumentTransfer>, &CallArgumentTransfer, bool),
                3,
                "selected owned argument next query receiving",
                {
                    match dynamic_next!(
                        budget,
                        function,
                        actual,
                        std::slice::Iter<'_, CallArgumentTransfer>,
                        &CallArgumentTransfer,
                        (&CallArgumentTransfer),
                        3
                    ) {
                        Some(arg) => arg.parameter_index != expected,
                        None => true,
                    }
                }
            ) {
                return Err(Error::static_error(
                    &permit,
                    CallKind::CallCommitDoesNotContainEveryAndOnlyOwnedDroppableParameter,
                ));
            }
        }
    }
    if dynamic_next!(
        budget,
        function,
        actual,
        std::slice::Iter<'_, CallArgumentTransfer>,
        &CallArgumentTransfer,
        (),
        2
    )
    .is_some()
    {
        return Err(Error::static_error(
            &permit,
            CallKind::CallCommitDoesNotContainEveryAndOnlyOwnedDroppableParameter,
        ));
    }
    let mut previous = None;
    let mut consumed = BTreeSet::new();
    let mut transfer_entries = arguments.iter();
    while let Some(argument) = dynamic_next!(
        budget,
        function,
        transfer_entries,
        std::slice::Iter<'_, CallArgumentTransfer>,
        &CallArgumentTransfer,
        (&CallArgumentTransfer),
        3
    ) {
        budget.charge(function, 1, "structural call epochs")?;
        if receive!(
            budget,
            function,
            (u32, bool),
            2,
            "selected argument prior index query receiving",
            {
                match previous {
                    Some(value) => value >= argument.parameter_index,
                    None => false,
                }
            }
        ) {
            return Err(Error::static_error(
                &permit,
                CallKind::CallCommitParametersAreNotInStrictSignatureOrder,
            ));
        }
        previous = Some(argument.parameter_index);
        let index = usize::try_from(argument.parameter_index).map_err(call_index_refusal!(
            budget,
            function,
            permit,
            usize,
            CallKind::CallCommitParameterIndexDoesNotFitUsize
        ))?;
        let expected = dynamic_get!(
            budget,
            function,
            fact.arguments,
            &crate::cleanup_plan::replay::call_payload::Arguments<'_>,
            index,
            usize,
            Option<&ExpressionId>,
            (&ExpressionId),
            4
        )
        .ok_or_else(call_refusal!(
            budget,
            function,
            permit,
            &ExpressionId,
            CallKind::CallCommitParameterIndexExceedsCallArity
        ))?;
        if !matches!(&argument.source.storage,StorageId::CallArgument{call:storage_call,parameter_index,value_expression} if storage_call==call && parameter_index==&argument.parameter_index && value_expression==expected && argument.source.projections.is_empty())
        {
            return Err(Error::static_error(
                &permit,
                CallKind::CallCommitDoesNotConsumeItsMatchingWholeArgumentEpoch,
            ));
        }
        let mut flag_entries = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            function,
            (
                &ResolvedFunction,
                &CleanupPlace,
                &BTreeSet<StorageId>,
                &Leaves,
                &mut ReplayBudget,
                Result<Vec<LivenessFlagId>, Error>
            ),
            6,
            "selected control validate_place call receiving",
            helpers::validate_place(function, &argument.source, storage, leaves, budget)
        )?
        .into_iter();
        while let Some(flag) = dynamic_next!(
            budget,
            function,
            flag_entries,
            std::vec::IntoIter<LivenessFlagId>,
            LivenessFlagId,
            (LivenessFlagId),
            3
        ) {
            if consumed.contains(&flag) {
                return Err(Error::static_error(
                    &permit,
                    CallKind::AtomicCallCommitConsumesOverlappingArgumentEpochs,
                ));
            }
            budget.reserve_btree_node::<LivenessFlagId, ()>(
                function,
                "structural committed epoch flags",
            )?;
            consumed.insert(flag);
        }
    }
    Ok(())
}
