//! Independent closed fresh contract-local owner replay; no attached fact.
use super::super::{Availability, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};
#[cfg(test)]
use super::super::{ValidationBinding, ValueId};
#[cfg(test)]
use std::collections::BTreeMap;

pub(super) fn local(expr: &ResolvedExpr, scope: &dyn super::ScopeRead) -> bool {
    expr.ownership == OwnershipMode::Own
        && matches!(expr.ty, ResolvedType::Bytes | ResolvedType::String)
        && matches!(&expr.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()
            && scope.get(&place.root).is_some_and(|binding| binding.contract_local && binding.ownership == OwnershipMode::Own
                && binding.ty == expr.ty && binding.availability == Availability::Available))
}
pub(super) fn fresh(expr: &ResolvedExpr, scope: &dyn super::ScopeRead) -> bool {
    if expr.ownership != OwnershipMode::Own {
        return false;
    }
    if local(expr, scope) {
        return true;
    }
    match (&expr.kind, &expr.ty) {
        (ResolvedExprKind::String(_), ResolvedType::String) => true,
        (
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            },
            ResolvedType::Bytes,
        ) if crate::byte_ops::by_id(callee.as_str()) == Some(crate::byte_ops::ByteOp::Zeroed)
            && type_arguments.is_empty()
            && instance.is_none() =>
        {
            matches!(args.as_slice(), [ResolvedExpr {kind: ResolvedExprKind::Usize(n), ..}]
                    if *n <= crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES)
        }
        _ => false,
    }
}

impl super::HirValidator<'_> {
    #[cfg(test)]
    pub(super) fn validate_contracts(
        &mut self,
        execution: &super::FunctionExecutionId,
        contracts: &[ResolvedExpr],
        outer: &BTreeMap<ValueId, ValidationBinding>,
        phase: &str,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        self.validate_contracts_baseline(execution, contracts, outer, phase)
    }
    #[cfg(test)]
    pub(super) fn validate_contracts_persistent(
        &mut self,
        execution: &super::FunctionExecutionId,
        contracts: &[ResolvedExpr],
        outer: &super::scope_checkpoint::Scope,
        phase: &str,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        if contracts.is_empty() {
            return Ok(());
        }
        let baseline = super::scope_checkpoint::Scope::fresh_baseline_persistent(outer)?;
        for (index, contract) in contracts.iter().enumerate() {
            let mut scope = baseline.clone();
            self.validate_expr_iterative_effects(
                execution,
                contract,
                &mut scope,
                super::path_label::Label::child(phase, ".{}", [index, 0])?,
                false,
                None,
            )?;
            self.require_type(
                &contract.ty,
                &ResolvedType::Bool,
                if phase == "requires" {
                    "precondition"
                } else {
                    "postcondition"
                },
            )?;
        }
        Ok(())
    }
    #[cfg(test)]
    fn validate_contracts_baseline(
        &mut self,
        execution: &super::FunctionExecutionId,
        contracts: &[ResolvedExpr],
        outer: &dyn super::ScopeRead,
        phase: &str,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        if contracts.is_empty() {
            return Ok(());
        }
        let baseline = super::scope_checkpoint::Scope::fresh_baseline(outer)?;
        for (index, contract) in contracts.iter().enumerate() {
            let mut scope = baseline.clone();
            self.validate_expr_iterative_effects(
                execution,
                contract,
                &mut scope,
                super::path_label::Label::child(phase, ".{}", [index, 0])?,
                false,
                None,
            )?;
            self.require_type(
                &contract.ty,
                &ResolvedType::Bool,
                if phase == "requires" {
                    "precondition"
                } else {
                    "postcondition"
                },
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
