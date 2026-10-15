//! Borrowed call signatures and one precharged current-HIR lookup cursor.
use super::call_receiving::{Kind, Policy};
use super::*;
use crate::cleanup_plan::CallArgumentTransfer;
use crate::hir::lexical_exit::ChildCursor;

pub(super) struct CallCursor<'a> {
    frames: Vec<Option<ChildCursor<'a>>>,
}
impl<'a> CallCursor<'a> {
    pub(super) fn new<E: Policy>(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, E> {
        let permit = E::enter::<Self>(function, budget)?;
        let capacity = 514;
        budget.reserve_array::<Option<ChildCursor<'_>>>(
            function,
            capacity,
            "structural call cursor",
        )?;
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(capacity)
            .map_err(|_| E::static_error(&permit, Kind::StructuralCallCursorAllocationRefused))?;
        if frames.capacity() != capacity {
            return Err(E::static_error(
                &permit,
                Kind::StructuralCallCursorCapacityDiffersFromReservation,
            ));
        }
        frames.resize(capacity, None);
        Ok(Self { frames })
    }
    fn find<E: Policy>(
        &mut self,
        function: &'a ResolvedFunction,
        id: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<&'a ResolvedExpr, E> {
        let permit = E::enter::<&ResolvedExpr>(function, budget)?;
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            self.frames[0] = Some(ChildCursor::new(root));
            let mut depth = 1;
            while depth != 0 {
                budget.charge(function, 1, "structural call lookup")?;
                let frame = self.frames[depth - 1]
                    .as_mut()
                    .expect("occupied structural call cursor");
                let node = frame.expression();
                if !frame.is_started() && node.id == *id {
                    return Ok(node);
                }
                if let Some(child) = frame.next() {
                    if depth == self.frames.len() {
                        return Err(E::static_error(
                            &permit,
                            Kind::StructuralCallLookupDepthExceedsBound,
                        ));
                    }
                    self.frames[depth] = Some(ChildCursor::new(child));
                    depth += 1;
                } else {
                    self.frames[depth - 1] = None;
                    depth -= 1;
                }
            }
        }
        Err(E::static_error(&permit, Kind::CallCommitExpressionIsAbsent))
    }
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
impl<'a> Signature<'a> {
    fn parameter(&self, index: usize) -> Option<(OwnershipMode, &ResolvedType)> {
        match self {
            Self::Byte(op) => op
                .param_types()
                .get(index)
                .map(|ty| (op.param_ownership(index), ty)),
            Self::String(op) => op
                .param_types()
                .get(index)
                .map(|ty| (op.param_ownership(index), ty)),
            Self::Str(op) => op.param_types().get(index).map(|ty| {
                (
                    if *ty == ResolvedType::Str {
                        OwnershipMode::Borrow
                    } else {
                        OwnershipMode::Value
                    },
                    ty,
                )
            }),
            Self::Function(target) => target.params.get(index).map(|p| (p.ownership, &p.ty)),
            Self::Vector(op, owned, element, args) => args.get(index).map(|arg| {
                (
                    if *owned {
                        op.param_ownership_for(index, element)
                    } else {
                        op.param_ownership(index)
                    },
                    &arg.ty,
                )
            }),
            Self::Field(args) => args.get(index).map(|arg| {
                (
                    if index == 0 {
                        OwnershipMode::Borrow
                    } else {
                        OwnershipMode::Value
                    },
                    &arg.ty,
                )
            }),
        }
    }
    fn accepts_type(&self, index: usize, ty: &ResolvedType) -> bool {
        match self {
            // ArrayAsSlice carries a catalog placeholder extent; authenticate
            // its actual fixed extent through the owning builtin classifier.
            Self::Byte(op) => op.accepts_resolved(index, ty),
            _ => self
                .parameter(index)
                .is_some_and(|(_, expected)| expected == ty),
        }
    }
    fn len(&self) -> usize {
        match self {
            Self::Byte(op) => op.param_types().len(),
            Self::String(op) => op.param_types().len(),
            Self::Str(op) => op.param_types().len(),
            Self::Function(target) => target.params.len(),
            Self::Vector(_, _, _, args) | Self::Field(args) => args.len(),
        }
    }
}
fn needs_drop(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if matches!(ty, ResolvedType::Nominal { .. }) {
        super::super::selected_profile::collections::prepay(
            program,
            function,
            super::super::selected_profile::collections::type_key_bytes(ty)?,
            budget,
        )?;
    }
    crate::cleanup::selected_inventory_needs_drop_in(program, ty)
}

fn signature<'a, E: Policy>(
    program: &'a dyn ReplayContext,
    function: &ResolvedFunction,
    fact: &CallFact<'_>,
    expression: &'a ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Signature<'a>, E> {
    let permit = E::enter::<Signature<'a>>(function, budget)?;
    if matches!(expression.kind, ResolvedExprKind::VecFieldRead { .. }) {
        E::field(program, function, expression, budget)?;
        let ResolvedExprKind::VecFieldRead { args, .. } = &expression.kind else {
            unreachable!()
        };
        return Ok(Signature::Field(args));
    }
    if let Some(owned) = E::call_facts(program, function, expression, budget)? {
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
        return Err(E::static_error(
            &permit,
            Kind::SelectedCallCommitDoesNotSupportGenericCalls,
        ));
    }
    if let Some(op) = crate::byte_ops::by_id(fact.callee.as_str()) {
        return Ok(Signature::Byte(op));
    }
    if let Some(op) = crate::string_ops::by_id(fact.callee.as_str()) {
        if op == crate::string_ops::StringOp::Take {
            E::string_operand(function, expression, budget)?.ok_or_else(|| {
                E::static_error(&permit, Kind::SelectedStringTransferCallIdentityDiffers)
            })?;
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
    let Some(target) = program.resolve_call_target(&fact.callee, None) else {
        return Err(E::missing_callee(&permit, &fact.callee));
    };
    if !target.effects.is_empty() || target.yields.is_some() {
        return Err(E::static_error(
            &permit,
            Kind::SelectedCallCommitDoesNotSupportEffectfulCallees,
        ));
    }
    Ok(Signature::Function(target))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn validate<'a, E: Policy>(
    program: &dyn ReplayContext,
    function: &'a ResolvedFunction,
    call: &ExpressionId,
    fact: &CallFact<'_>,
    arguments: &[CallArgumentTransfer],
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    cursor: Option<&mut CallCursor<'a>>,
    budget: &mut ReplayBudget,
) -> Result<(), E> {
    let permit = E::enter::<()>(function, budget)?;
    #[cfg(test)]
    E::borrowed_controls(function, budget)?;
    #[cfg(test)]
    let borrowed = program.borrowed_helper_lease(function);
    #[cfg(test)]
    let held = match &borrowed {
        Some(lease) => Some(lease.call_by_id(program, function, call, budget)?),
        None => None,
    };
    #[cfg(test)]
    let expression = match &held {
        Some((expression, _)) => *expression,
        None => cursor
            .ok_or_else(|| E::static_error(&permit, Kind::SelectedCallLacksItsLookupCursor))?
            .find::<E>(function, call, budget)?,
    };
    #[cfg(not(test))]
    let expression = cursor
        .ok_or_else(|| E::static_error(&permit, Kind::SelectedCallLacksItsLookupCursor))?
        .find::<E>(function, call, budget)?;
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
            return Err(E::static_error(
                &permit,
                Kind::CallCommitDoesNotNameASelectedOrdinaryCall,
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
            for (arg, id) in args.iter().zip(&fact.arguments) {
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
            && args
                .iter()
                .zip(&fact.arguments)
                .all(|(arg, id)| &arg.id == id)
    };
    #[cfg(not(test))]
    let agrees = callee == fact.callee.as_str()
        && type_arguments == fact.type_arguments.as_slice()
        && instance == fact.instance
        && args.len() == fact.arguments.len()
        && args
            .iter()
            .zip(&fact.arguments)
            .all(|(arg, id)| &arg.id == id);
    if !agrees {
        return Err(E::static_error(
            &permit,
            Kind::CallCommitFactsDisagreeWithCurrentTypedCall,
        ));
    }
    #[cfg(test)]
    let signature = match &held {
        Some((_, call)) => {
            use schema::borrowed::{Call, operands::Builtin};
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
        None => signature::<E>(program, function, fact, expression, budget)?,
    };
    #[cfg(not(test))]
    let signature = signature::<E>(program, function, fact, expression, budget)?;
    if signature.len() != fact.arguments.len() {
        return Err(E::static_error(
            &permit,
            Kind::CallCommitCalleeSignatureHasInconsistentArity,
        ));
    }
    let mut actual = arguments.iter();
    for (index, arg) in args.iter().enumerate().take(signature.len()) {
        budget.charge(function, 1, "structural call parameters")?;
        let (mode, ty) = signature.parameter(index).expect("exact signature bound");
        #[cfg(test)]
        let drop = if held.is_some() {
            nested_shape::type_needs_drop_with_budget(program, function, ty, budget)?
        } else {
            needs_drop(program, function, ty, budget)?
        };
        #[cfg(not(test))]
        let drop = needs_drop(program, function, ty, budget)?;
        if !signature.accepts_type(index, &arg.ty)
            || !match mode {
                OwnershipMode::Own => arg.ownership == OwnershipMode::Own,
                OwnershipMode::Borrow => {
                    matches!(arg.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
                }
                OwnershipMode::Value => arg.ownership == OwnershipMode::Value,
                OwnershipMode::Shared => false,
            }
        {
            return Err(E::static_error(
                &permit,
                Kind::CallCommitArgumentDisagreesWithExactParameterTypeOrOwnership,
            ));
        }
        if mode == OwnershipMode::Own && drop {
            let expected = u32::try_from(index)
                .map_err(|_| E::static_error(&permit, Kind::TooManyCallParameters))?;
            if actual
                .next()
                .is_none_or(|arg| arg.parameter_index != expected)
            {
                return Err(E::static_error(
                    &permit,
                    Kind::CallCommitDoesNotContainEveryAndOnlyOwnedDroppableParameter,
                ));
            }
        }
    }
    if actual.next().is_some() {
        return Err(E::static_error(
            &permit,
            Kind::CallCommitDoesNotContainEveryAndOnlyOwnedDroppableParameter,
        ));
    }
    let mut previous = None;
    let mut consumed = BTreeSet::new();
    for argument in arguments {
        budget.charge(function, 1, "structural call epochs")?;
        if previous.is_some_and(|value| value >= argument.parameter_index) {
            return Err(E::static_error(
                &permit,
                Kind::CallCommitParametersAreNotInStrictSignatureOrder,
            ));
        }
        previous = Some(argument.parameter_index);
        let index = usize::try_from(argument.parameter_index)
            .map_err(|_| E::static_error(&permit, Kind::CallCommitParameterIndexDoesNotFitUsize))?;
        let expected = fact.arguments.get(index).ok_or_else(|| {
            E::static_error(&permit, Kind::CallCommitParameterIndexExceedsCallArity)
        })?;
        if !matches!(&argument.source.storage,StorageId::CallArgument{call:storage_call,parameter_index,value_expression} if storage_call==call && parameter_index==&argument.parameter_index && value_expression==expected && argument.source.projections.is_empty())
        {
            return Err(E::static_error(
                &permit,
                Kind::CallCommitDoesNotConsumeItsMatchingWholeArgumentEpoch,
            ));
        }
        for flag in
            path_join::storage::validate_place(function, &argument.source, storage, leaves, budget)?
        {
            if consumed.contains(&flag) {
                return Err(E::static_error(
                    &permit,
                    Kind::AtomicCallCommitConsumesOverlappingArgumentEpochs,
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

#[cfg(test)]
mod custody_tests;
