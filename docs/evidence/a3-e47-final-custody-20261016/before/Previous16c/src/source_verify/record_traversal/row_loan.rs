//! Row Loan v1 source admission. A row is admitted as a value only as a
//! direct argument of an ordinary monomorphic call whose parameter at that
//! position is `borrow R` for the row's exact flat owned-leaf record `R`.
//! Copy-record rows keep their by-value contract and stay refused. When the
//! callee body is visible here, its parameter must also be read only through
//! Copy scalar fields; complete HIR replay re-proves this for every callee,
//! including imported ones, from their retained bodies.
use super::super::binding::{Binding, CheckedValue};
use super::super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, Function, ParamMode, Type, TypeDeclarationKind};
use std::collections::HashMap;

pub(in crate::source_verify) const HELPER_REFUSAL: &str =
    "record traversal row can only be lent to a helper that reads Copy scalar fields of its borrowed parameter";

/// The borrowed row value admitted at `index`, or `None` for every other
/// argument shape (which then keeps the ordinary escape refusal).
pub(in crate::source_verify) fn loan_argument(
    argument: &Expr,
    target: &Function,
    index: usize,
    variables: &HashMap<String, Binding>,
    types: &TypeTable<'_>,
) -> Option<CheckedValue> {
    let ExprKind::Var(name) = &argument.kind else {
        return None;
    };
    let row = variables.get(name)?.traversal_row.as_deref()?;
    let parameter = target.params.get(index)?;
    (target.type_parameters.is_empty()
        && target.yields.is_none()
        && parameter.mode == ParamMode::Borrow
        && parameter.ty == row.element
        && matches!(&row.element, Type::Named { arguments, .. } if arguments.is_empty())
        && super::super::declared_type::owned_leaf_collection::admitted(types, &row.element))
    .then(|| CheckedValue {
        ty: row.element.clone(),
        mode: ParamMode::Borrow,
        native_unit: false,
    })
}

impl crate::source_verify::IterativeVerifier<'_, '_> {
    /// Record each admitted row argument of one ordinary call before any
    /// argument is entered; the row `Var` then reads as `borrow R`.
    pub(in crate::source_verify) fn note_row_loan_arguments(
        &mut self,
        function: &Function,
        args: &[Expr],
        scope: usize,
    ) {
        for (index, argument) in args.iter().enumerate() {
            if loan_argument(
                argument,
                function,
                index,
                &self.scopes[scope].bindings,
                self.types,
            )
            .is_none()
            {
                continue;
            }
            if !reader_proof(self.types, function, index) {
                self.diagnostics.push(super::super::diagnostics::error(
                    self.program,
                    "SPX-T284",
                    HELPER_REFUSAL,
                    argument.span,
                ));
            }
            self.row_loan_sites
                .insert((argument.span.start, argument.span.end));
        }
    }
}

/// Every occurrence of the borrowed parameter in the callee's body and
/// contracts is a `parameter.field` read of a Copy scalar field outside any
/// closure. A scalar result type alone is never treated as sufficient.
pub(in crate::source_verify) fn reader_proof(
    types: &TypeTable<'_>,
    target: &Function,
    index: usize,
) -> bool {
    // V1 is a scalar reader, not a general borrowed-record call profile.
    // Return ownership is Value for exactly these eight primitive types.
    if !target.effects.is_empty() || !crate::vec_ops::ast_element_is_admitted(&target.return_type) {
        return false;
    }
    let Some(parameter) = target.params.get(index) else {
        return false;
    };
    let Type::Named { name, .. } = &parameter.ty else {
        return false;
    };
    let Some(declaration) = types.declaration(name) else {
        return false;
    };
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return false;
    };
    let scalar = |field: &str| {
        fields
            .iter()
            .any(|item| item.name == field && crate::vec_ops::ast_element_is_admitted(&item.ty))
    };
    let mut work = super::MAX_SCAN_WORK;
    let mut pending = std::iter::once(&target.body)
        .chain(&target.requires)
        .chain(&target.ensures)
        .map(|root| (root, false))
        .collect::<Vec<_>>();
    while let Some((expression, closure)) = pending.pop() {
        let Some(remaining) = work.checked_sub(1) else {
            return false;
        };
        work = remaining;
        match &expression.kind {
            ExprKind::Var(var) if *var == parameter.name => return false,
            ExprKind::Project { base, field, .. } if matches!(&base.kind, ExprKind::Var(var) if *var == parameter.name) =>
            {
                if closure || !scalar(field) {
                    return false;
                }
                continue;
            }
            _ => {}
        }
        let nested = closure || matches!(expression.kind, ExprKind::Closure { .. });
        let mut child = 0;
        while let Some(next) = expression.child(child) {
            pending.push((next, nested));
            child += 1;
        }
    }
    true
}
