//! Borrowed call signatures and one precharged current-HIR lookup cursor.
use super::*;
use crate::cleanup_plan::CallArgumentTransfer;
use crate::hir::lexical_exit::ChildCursor;

pub(super) struct CallCursor<'a> {
    frames: Vec<Option<ChildCursor<'a>>>,
}
impl<'a> CallCursor<'a> {
    pub(super) fn new(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        let capacity = 514;
        budget.reserve_array::<Option<ChildCursor<'_>>>(
            function,
            capacity,
            "structural call cursor",
        )?;
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(capacity)
            .map_err(|_| replay_error(function, "structural call cursor allocation refused"))?;
        if frames.capacity() != capacity {
            return Err(replay_error(
                function,
                "structural call cursor capacity differs from reservation",
            ));
        }
        frames.resize(capacity, None);
        Ok(Self { frames })
    }
    fn find(
        &mut self,
        function: &'a ResolvedFunction,
        id: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<&'a ResolvedExpr, Diagnostic> {
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
                        return Err(replay_error(
                            function,
                            "structural call lookup depth exceeds bound",
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
        Err(replay_error(function, "call commit expression is absent"))
    }
}

pub(super) enum Signature<'a> {
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
    pub(super) fn parameter(&self, index: usize) -> Option<(OwnershipMode, &ResolvedType)> {
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
    pub(super) fn accepts_type(&self, index: usize, ty: &ResolvedType) -> bool {
        match self {
            // ArrayAsSlice carries a catalog placeholder extent; authenticate
            // its actual fixed extent through the owning builtin classifier.
            Self::Byte(op) => op.accepts_resolved(index, ty),
            _ => self
                .parameter(index)
                .is_some_and(|(_, expected)| expected == ty),
        }
    }
    pub(super) fn len(&self) -> usize {
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

pub(super) fn signature<'a>(
    program: &'a dyn ReplayContext,
    function: &ResolvedFunction,
    fact: &CallFact<'_>,
    expression: &'a ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Signature<'a>, Diagnostic> {
    if matches!(expression.kind, ResolvedExprKind::VecFieldRead { .. }) {
        super::super::selected_profile::collections::field(program, function, expression, budget)?;
        let ResolvedExprKind::VecFieldRead { args, .. } = &expression.kind else {
            unreachable!()
        };
        return Ok(Signature::Field(args));
    }
    if let Some(owned) = super::super::selected_profile::collections::call_facts(
        program, function, expression, budget,
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
        return Err(replay_error(
            function,
            "selected call commit does not support generic calls",
        ));
    }
    if let Some(op) = crate::byte_ops::by_id(fact.callee.as_str()) {
        return Ok(Signature::Byte(op));
    }
    if let Some(op) = crate::string_ops::by_id(fact.callee.as_str()) {
        if op == crate::string_ops::StringOp::Take {
            super::super::selected_profile::string_transfer::operand(function, expression, budget)?
                .ok_or_else(|| {
                    replay_error(function, "selected String transfer call identity differs")
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
    let target = program
        .resolve_call_target(&fact.callee, None)
        .ok_or_else(|| {
            replay_error(
                function,
                format_args!("call commit has unknown callee `{}`", fact.callee),
            )
        })?;
    if !target.effects.is_empty() || target.yields.is_some() {
        return Err(replay_error(
            function,
            "selected call commit does not support effectful callees",
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
) -> Result<(), Diagnostic> {
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
            .ok_or_else(|| replay_error(function, "selected call lacks its lookup cursor"))?
            .find(function, call, budget)?,
    };
    #[cfg(not(test))]
    let expression = cursor
        .ok_or_else(|| replay_error(function, "selected call lacks its lookup cursor"))?
        .find(function, call, budget)?;
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
            return Err(replay_error(
                function,
                "call commit does not name a selected ordinary call",
            ))
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
        return Err(replay_error(
            function,
            "call commit facts disagree with current typed call",
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
        None => signature(program, function, fact, expression, budget)?,
    };
    #[cfg(not(test))]
    let signature = signature(program, function, fact, expression, budget)?;
    if signature.len() != fact.arguments.len() {
        return Err(replay_error(
            function,
            "call commit callee signature has inconsistent arity",
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
            return Err(replay_error(
                function,
                "call commit argument disagrees with exact parameter type or ownership",
            ));
        }
        if mode == OwnershipMode::Own && drop {
            let expected = u32::try_from(index)
                .map_err(|_| replay_error(function, "too many call parameters"))?;
            if actual
                .next()
                .is_none_or(|arg| arg.parameter_index != expected)
            {
                return Err(replay_error(
                    function,
                    "call commit does not contain every and only owned droppable parameter",
                ));
            }
        }
    }
    if actual.next().is_some() {
        return Err(replay_error(
            function,
            "call commit does not contain every and only owned droppable parameter",
        ));
    }
    let mut previous = None;
    let mut consumed = BTreeSet::new();
    for argument in arguments {
        budget.charge(function, 1, "structural call epochs")?;
        if previous.is_some_and(|value| value >= argument.parameter_index) {
            return Err(replay_error(
                function,
                "call-commit parameters are not in strict signature order",
            ));
        }
        previous = Some(argument.parameter_index);
        let index = usize::try_from(argument.parameter_index).map_err(|_| {
            replay_error(function, "call-commit parameter index does not fit usize")
        })?;
        let expected = fact.arguments.get(index).ok_or_else(|| {
            replay_error(function, "call-commit parameter index exceeds call arity")
        })?;
        if !matches!(&argument.source.storage,StorageId::CallArgument{call:storage_call,parameter_index,value_expression} if storage_call==call && parameter_index==&argument.parameter_index && value_expression==expected && argument.source.projections.is_empty())
        {
            return Err(replay_error(
                function,
                "call commit does not consume its matching whole argument epoch",
            ));
        }
        for flag in
            path_join::storage::validate_place(function, &argument.source, storage, leaves, budget)?
        {
            if consumed.contains(&flag) {
                return Err(replay_error(
                    function,
                    "atomic call commit consumes overlapping argument epochs",
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
