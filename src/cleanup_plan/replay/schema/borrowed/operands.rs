//! Receiving probes over the exact typed nodes admitted by the parent profile.
//! Every evaluated identity comparison debits actual visited bytes; unknown
//! calls still require the parent's independent current-member authentication.
use super::*;
#[derive(Clone, Copy)]
pub(in crate::cleanup_plan::replay) enum Builtin {
    FromU8,
    Get,
    Len,
    Str(crate::str_ops::StrOp),
}
pub(super) fn builtin(
    function: &ResolvedFunction,
    id: &str,
    budget: &mut ReplayBudget,
) -> Result<Option<Builtin>, Diagnostic> {
    for (expected, operation) in [
        (crate::string_ops::I64_FROM_U8_ID, Builtin::FromU8),
        (crate::byte_ops::GET_ID, Builtin::Get),
        (crate::byte_ops::LEN_ID, Builtin::Len),
        (
            crate::str_ops::LEN_BYTES_ID,
            Builtin::Str(crate::str_ops::StrOp::LenBytes),
        ),
        (
            crate::str_ops::IS_EMPTY_ID,
            Builtin::Str(crate::str_ops::StrOp::IsEmpty),
        ),
        (
            crate::str_ops::STARTS_WITH_ID,
            Builtin::Str(crate::str_ops::StrOp::StartsWith),
        ),
        (
            crate::str_ops::CONTAINS_ID,
            Builtin::Str(crate::str_ops::StrOp::Contains),
        ),
    ] {
        if id_equal(function, id, expected, budget)? {
            return Ok(Some(operation));
        }
    }
    Ok(None)
}
fn root(
    function: &ResolvedFunction,
    place: &crate::hir::Place,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "borrowed rooted projection header")?;
    if !place.projections.is_empty() {
        return Ok(false);
    }
    let mut found = false;
    for parameter in &function.params {
        budget.charge(function, 1, "borrowed actual root parameter")?;
        if id_equal(function, parameter.id.as_str(), place.root.as_str(), budget)? {
            budget.charge(function, 1, "borrowed exact root mode and type")?;
            if found
                || !parameter.id.same_cached_fingerprint(&place.root)
                || parameter.ownership != OwnershipMode::Borrow
                || &parameter.ty != ty
            {
                return Ok(false);
            }
            found = true;
        }
    }
    Ok(found)
}
pub(super) fn expression(
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    // The parent has independently authenticated the complete type immediately
    // before this receiver, including canonical Option metadata for nominals.
    budget.charge(function, 1, "borrowed operand type and ownership")?;
    let scalar = crate::hir::is_scalar_resolved_type(&expression.ty)
        && expression.ownership == OwnershipMode::Value;
    let copy = (scalar || matches!(expression.ty, ResolvedType::Nominal { .. }))
        && expression.ownership == OwnershipMode::Value;
    match &expression.kind {
        ResolvedExprKind::Place(place)
            if matches!(expression.ty, ResolvedType::Str | ResolvedType::SliceU8) =>
        {
            Ok(expression.ownership == OwnershipMode::Borrow
                && root(function, place, &expression.ty, budget)?)
        }
        ResolvedExprKind::BorrowPlace { operation, place } => Ok(expression.ownership
            == OwnershipMode::Borrow
            && expression.ty == ResolvedType::SliceU8
            && id_equal(
                function,
                operation.as_str(),
                crate::byte_ops::STR_AS_BYTES_ID,
                budget,
            )?
            && root(function, place, &ResolvedType::Str, budget)?),
        ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            args,
        } => {
            let Some(operation) = builtin(function, callee.as_str(), budget)? else {
                return Ok(scalar);
            };
            budget.charge(function, 1, "borrowed builtin call header")?;
            if !copy || instance.is_some() || !type_arguments.is_empty() {
                return Ok(false);
            }
            for _ in args {
                budget.charge(function, 1, "borrowed builtin actual argument")?;
            }
            Ok(match operation {
                Builtin::FromU8 => {
                    expression.ty == ResolvedType::I64
                        && args.len() == 1
                        && args[0].ty == ResolvedType::U8
                        && args[0].ownership == OwnershipMode::Value
                }
                Builtin::Get => {
                    matches!(expression.ty, ResolvedType::Nominal { .. })
                        && args.len() == 2
                        && args[0].ty == ResolvedType::SliceU8
                        && args[0].ownership == OwnershipMode::Borrow
                        && args[1].ty == ResolvedType::Usize
                        && args[1].ownership == OwnershipMode::Value
                }
                Builtin::Len => {
                    expression.ty == ResolvedType::Usize
                        && args.len() == 1
                        && args[0].ty == ResolvedType::SliceU8
                        && args[0].ownership == OwnershipMode::Borrow
                }
                Builtin::Str(operation) => {
                    expression.ty == operation.return_type()
                        && args.len() == operation.arity()
                        && args.iter().all(|arg| {
                            arg.ty == ResolvedType::Str && arg.ownership == OwnershipMode::Borrow
                        })
                }
            })
        }
        ResolvedExprKind::Place(place) if matches!(expression.ty, ResolvedType::Nominal { .. }) => {
            Ok(copy && place.projections.is_empty())
        }
        _ => Ok(scalar),
    }
}

/// The bounded scalar decision subset used by the actual UTF-8 readers. The
/// final arm is irrefutable; earlier arms are exact same-typed literal domains.
pub(super) fn scalar_match(
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    mode: crate::hir::ResolvedMatchMode,
    scrutinee: &ResolvedExpr,
    arms: &[ResolvedMatchArm],
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "borrowed scalar match header")?;
    if mode != crate::hir::ResolvedMatchMode::Value
        || scrutinee.ownership != OwnershipMode::Value
        || expression.ownership != OwnershipMode::Value
        || !crate::hir::is_refutable_match_scalar(&scrutinee.ty)
        || !crate::hir::is_scalar_resolved_type(&expression.ty)
        || arms.is_empty()
    {
        return Ok(false);
    }
    for (index, arm) in arms.iter().enumerate() {
        budget.charge(function, 1, "borrowed scalar actual arm")?;
        if arm.guard.is_some()
            || arm.value.ty != expression.ty
            || arm.value.ownership != OwnershipMode::Value
        {
            return Ok(false);
        }
        let last = index + 1 == arms.len();
        match &arm.pattern {
            ResolvedMatchPattern::Wildcard if last => {}
            ResolvedMatchPattern::Binding(binding) if last => {
                budget.charge(function, 1, "borrowed scalar catch-all binding")?;
                if binding.ty != scrutinee.ty || binding.ownership != OwnershipMode::Value {
                    return Ok(false);
                }
            }
            ResolvedMatchPattern::Literal(value) if !last => {
                budget.charge(function, 1, "borrowed scalar literal type")?;
                if value.ty() != scrutinee.ty {
                    return Ok(false);
                }
            }
            ResolvedMatchPattern::Or(alternatives) if !last && !alternatives.is_empty() => {
                for alternative in alternatives {
                    budget.charge(function, 1, "borrowed scalar actual alternative")?;
                    let ResolvedMatchPattern::Literal(value) = alternative else {
                        return Ok(false);
                    };
                    if value.ty() != scrutinee.ty {
                        return Ok(false);
                    }
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_builtin_probes_match_original_whitelist_and_independent_byte_census() {
        let program = crate::hir::resolve(
            &crate::check(
                "module probe; @id(\"main\") fn main()->i64 {0}",
                "builtin-probes.spx",
            )
            .unwrap(),
        )
        .unwrap();
        let function = &program.functions[0];
        let candidates = [
            crate::string_ops::I64_FROM_U8_ID,
            crate::byte_ops::GET_ID,
            crate::byte_ops::LEN_ID,
            crate::str_ops::LEN_BYTES_ID,
            crate::str_ops::IS_EMPTY_ID,
            crate::str_ops::STARTS_WITH_ID,
            crate::str_ops::CONTAINS_ID,
        ];
        for id in candidates.into_iter().chain([
            "",
            "core.bytes.get.forged",
            crate::str_ops::BYTE_AT_ID,
            "core.str.containsX",
            "unrelated.helper",
        ]) {
            // This census is independent of the receiving callback: each
            // length probe and each byte through the first difference counts.
            let mut expected = 0;
            for target in candidates {
                expected += 1;
                if target.len() == id.len() {
                    for (a, b) in target.bytes().zip(id.bytes()) {
                        expected += 1;
                        if a != b {
                            break;
                        }
                    }
                }
                if target == id {
                    break;
                }
            }
            let mut budget = ReplayBudget::new();
            budget.remaining = expected;
            assert_eq!(
                builtin(function, id, &mut budget).unwrap().is_some(),
                crate::hir::borrowed_scalar::builtin(id),
                "{id}"
            );
            assert_eq!(budget.remaining, 0, "{id}");
            let mut short = ReplayBudget::new();
            short.remaining = expected - 1;
            assert_eq!(
                builtin(function, id, &mut short).err().unwrap().code,
                "SPX-H006"
            );
        }
    }
}
