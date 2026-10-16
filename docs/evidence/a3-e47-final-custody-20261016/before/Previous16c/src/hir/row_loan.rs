//! Row Loan v1: a call-only borrow of one frozen owned-leaf `Vec` row.
//!
//! A `for borrow` row may be passed directly to an ordinary monomorphic
//! callee parameter declared `borrow R` for the exact row record `R`. The
//! argument is an explicit descriptor, never a materialized record: it names
//! the exact whole-Vec place, the iteration index, the element declaration,
//! the enclosing traversal region and its length bound, the callee, and the
//! exact borrowed parameter identity. The callee stays the single authored
//! function, compiled once; its lent parameter is independently proven to be a
//! nonescaping scalar-field reader. See `docs/SCOPED-RECORD-ROW-LOAN-V1.md`.
use super::*;

mod producer;
mod validate;
pub(crate) use validate::validate_program;

/// The additive owning version. Carriers and graph/cache encodings name it.
pub const ROW_LOAN_V1: &str = "semaprax.row-loan.v1";

/// Resolution-time placeholder callee. Validation refuses it everywhere, so a
/// row reaching any position other than a bound ordinary call fails closed.
pub(super) const UNBOUND_CALLEE: &str = "core.row_loan.unbound";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RowLoanVersion {
    V1,
}

impl RowLoanVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1 => ROW_LOAN_V1,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        (value == ROW_LOAN_V1).then_some(Self::V1)
    }
}

/// Exact call-only row loan descriptor. Its two runtime children, the whole
/// Vec place and the row index place, live beside it in
/// [`ResolvedExprKind::RowLoan`] in authored order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowLoan {
    pub version: RowLoanVersion,
    /// Exact monomorphic flat owned-leaf record of the frozen Vec.
    pub element: ResolvedType,
    /// The enclosing lowered `for borrow` region whose freeze protects the
    /// whole Vec owner and generation for the complete call.
    pub traversal: ExpressionId,
    /// The region's immutable length snapshot (the traversal bound).
    pub bound: ValueId,
    /// The ordinary monomorphic callee receiving the loan.
    pub callee: DeclarationId,
    /// The callee's exact `borrow R` parameter identity.
    pub parameter: ValueId,
}

impl RowLoan {
    pub(crate) fn is_bound(&self) -> bool {
        self.callee.as_str() != UNBOUND_CALLEE
    }
}

/// A Row Loan element is a flat owned-leaf record with explicit fields. Copy
/// records keep their by-value parameter contract and remain refused.
pub(crate) fn element_admitted(index: &DeclarationIndex, element: &ResolvedType) -> bool {
    owned_leaf_collection::layout(index, element).is_some_and(|layout| layout.fields.is_some())
}

/// The primary validator clones only the independently admitted flat type.
/// Reserve that actual string/argument backing before the clone, not afterwards.
pub(super) fn reserve_type_clone(ty: &ResolvedType) -> bool {
    producer::cloned_type_bytes(ty, 0).is_some_and(crate::bounded_output::reserve_active_required)
}

/// `true` only when `path` names an argument slot (`<call>.arg.<n>`).
fn call_argument_path(path: &str) -> bool {
    path.rsplit_once(".arg.").is_some_and(|(call, index)| {
        !call.is_empty() && !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
    })
}

impl Resolver<'_> {
    /// `None` when `expr` is not a source row. A row reaching an argument slot
    /// becomes an unbound descriptor that only `bind_row_loans` on an ordinary
    /// call can complete; every other row use is refused.
    pub(super) fn row_argument(
        &self,
        function: &FunctionExecutionId,
        expr: &crate::ast::Expr,
        bindings: &BTreeMap<String, Binding>,
        path: resolve_expr::path_arena::ExprPath<'_>,
    ) -> Option<Result<ResolvedExpr, Diagnostic>> {
        let crate::ast::ExprKind::Var(name) = &expr.kind else {
            return None;
        };
        let row = bindings.get(name)?.traversal_row.as_deref()?;
        path.with_text(|path| Some(
            self.provisional_row_loan(function, expr.span, path, row)
                .ok_or_else(|| {
                    self.error(
                        "SPX-H006",
                        "record traversal row cannot escape or materialize aggregate storage; select a field",
                        expr.span,
                    )
                }),
        ))
    }

    fn provisional_row_loan(
        &self,
        function: &FunctionExecutionId,
        span: Span,
        path: &str,
        row: &resolve_for::record::Row,
    ) -> Option<ResolvedExpr> {
        if !call_argument_path(path)
            || function.instance().is_some()
            || !element_admitted(&self.declarations, &row.element)
        {
            return None;
        }
        producer::materialize(function, span, path, row)
    }

    /// Bind every provisional row argument of one ordinary call to the exact
    /// callee parameter. Generic, method, intrinsic and callable targets never
    /// reach this seam, so their row arguments stay unbound and are refused.
    pub(super) fn bind_row_loans(
        &self,
        callee: &DeclarationId,
        instance: &Option<FunctionInstanceId>,
        type_arguments: &[ResolvedType],
        mut args: Vec<ResolvedExpr>,
    ) -> Result<Vec<ResolvedExpr>, Diagnostic> {
        for (index, argument) in args.iter_mut().enumerate() {
            let span = argument.span;
            let ResolvedExprKind::RowLoan { loan, .. } = &mut argument.kind else {
                continue;
            };
            let refuse = || {
                self.error(
                    "SPX-T284",
                    "record traversal row can only be lent to an ordinary monomorphic `borrow` parameter of its exact record",
                    span,
                )
            };
            if instance.is_some() || !type_arguments.is_empty() || loan.is_bound() {
                return Err(refuse());
            }
            let target = self
                .program
                .functions()
                .iter()
                .find(|function| function.stable_id == callee.as_str())
                .ok_or_else(refuse)?;
            let parameter = target.params.get(index).ok_or_else(refuse)?;
            if !target.type_parameters.is_empty()
                || parameter.mode != crate::ast::ParamMode::Borrow
                || self.resolve_type(&parameter.ty, span)? != loan.element
            {
                return Err(refuse());
            }
            let Some((bound_callee, bound_parameter)) = producer::bind(callee, index) else {
                return Err(Diagnostic::error("SPX-H006", String::new(), span));
            };
            loan.callee = bound_callee;
            loan.parameter = bound_parameter;
        }
        Ok(args)
    }
}

/// Every `(callee, parameter index)` receiving at least one row loan. Backends
/// compile exactly these parameters over the versioned borrowed-record
/// carrier; every other parameter keeps its ordinary ABI. Only call this on a
/// program that passed [`validate_program`].
pub(crate) fn lent_parameters(
    program: &ResolvedProgram,
) -> BTreeMap<DeclarationId, BTreeSet<usize>> {
    let mut lent: BTreeMap<DeclarationId, BTreeSet<usize>> = BTreeMap::new();
    for function in &program.functions {
        let mut pending = std::iter::once(&function.body)
            .chain(&function.requires)
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            if let ResolvedExprKind::Call {
                callee,
                instance: None,
                args,
                ..
            } = &expression.kind
            {
                for (index, argument) in args.iter().enumerate() {
                    if matches!(argument.kind, ResolvedExprKind::RowLoan { .. }) {
                        lent.entry(callee.clone()).or_default().insert(index);
                    }
                }
            }
            push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
    }
    lent
}

/// Whether any validated program function lends a row.
pub(crate) fn program_uses(program: &ResolvedProgram) -> bool {
    !lent_parameters(program).is_empty()
}

/// Exact lent parameter lookup for one function, in parameter order.
pub(crate) fn lent_parameter_ids(
    lent: &BTreeMap<DeclarationId, BTreeSet<usize>>,
    function: &ResolvedFunction,
) -> BTreeSet<ValueId> {
    lent.get(&function.id)
        .map_or_else(BTreeSet::new, |indices| {
            indices
                .iter()
                .filter_map(|index| function.params.get(*index))
                .map(|parameter| parameter.id.clone())
                .collect()
        })
}

/// The scalar field read of a lent parameter: `parameter.field` as one exact
/// one-projection Place. Returns the selected field declaration position.
pub(crate) fn lent_field_read<'a>(
    index: &'a DeclarationIndex,
    element: &ResolvedType,
    place: &Place,
) -> Option<vec_field::Field<'a>> {
    let [PlaceProjection::Field(field)] = place.projections.as_slice() else {
        return None;
    };
    vec_field::field(index, element, field)
        .filter(|selected| crate::vec_ops::resolved_element_is_admitted(&selected.declaration.ty))
}

#[cfg(test)]
mod tests;
