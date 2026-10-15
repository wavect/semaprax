//! Authored Row Loan v1 call slots bound to retained HIR. Each `helper(row)`
//! names its callee through current source: a local function, or an exact
//! `use function` import whose provider declaration is read from the current
//! provider Program. The callee's authored parameter must be `borrow R` for
//! the traversal's exact record identity; retained HIR must carry exactly one
//! matching descriptor per authored slot and no other.
use super::*;

pub(in crate::workspace_graph) struct Lend<'a> {
    function: &'a Function,
    path: String,
    root: hir::ValueId,
    index: hir::ValueId,
    region: String,
    declaration: &'a TypeDeclaration,
    callee: &'a str,
    position: usize,
}

/// The current-source callee of an authored call name and its Program.
fn callee<'a>(
    program: &'a Program,
    programs: &'a [Program],
    name: &str,
) -> Option<(&'a Program, &'a Function)> {
    if let Some(function) = program.functions.iter().find(|f| f.name == name) {
        return Some((program, function));
    }
    let import = program
        .module_uses
        .iter()
        .find(|item| item.kind == ModuleUseKind::Function && item.alias == name)?;
    let context = programs
        .iter()
        .find(|candidate| candidate.module == import.target_module)?;
    let function = context
        .functions
        .iter()
        .find(|function| function.stable_id == import.persistent_id)?;
    Some((context, function))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect<'a>(
    program: &'a Program,
    programs: &'a [Program],
    function: &'a Function,
    path: &str,
    name: &str,
    args: &'a [Expr],
    row: &Row<'a>,
    out: &mut Expected<'a>,
) -> Result<()> {
    for (position, argument) in args.iter().enumerate() {
        if !matches!(&argument.kind, ExprKind::Var(name) if name == row.name) {
            continue;
        }
        let (context, target) = callee(program, programs, name).ok_or_else(refusal)?;
        let parameter = target.params.get(position).ok_or_else(refusal)?;
        // This is an authored source error, before any retained descriptor is
        // inspected. A current provider's owning parameter cannot receive a
        // borrowed traversal row, even when the consumer was checked before
        // that provider signature changed. Retained-HIR mismatches below keep
        // their independent G173 refusal.
        if parameter.mode == crate::ast::ParamMode::Own {
            return Err(vec![Diagnostic::error(
                "SPX-T284",
                "record traversal row cannot escape or materialize aggregate storage; select a field",
                argument.span,
            )
            .at_path(&program.path)]);
        }
        let Type::Named {
            name: record,
            arguments,
        } = &parameter.ty
        else {
            return Err(refusal());
        };
        if parameter.mode != crate::ast::ParamMode::Borrow
            || !arguments.is_empty()
            || !target.type_parameters.is_empty()
            || declaration(context, programs, record)
                .is_none_or(|declared| declared.stable_id != row.declaration.stable_id)
        {
            return Err(refusal());
        }
        reserve_builder_structure(4 * std::mem::size_of::<Lend<'_>>())?;
        out.loans.push(Lend {
            function,
            path: budgeted_format(format_args!("{path}.arg.{position}")),
            root: row.root.clone(),
            index: row.index.clone(),
            region: crate::bounded_output::budgeted_clone(&row.region),
            declaration: row.declaration,
            callee: &target.stable_id,
            position,
        });
    }
    Ok(())
}

pub(super) fn count(loans: &[Lend<'_>], function: &Function) -> usize {
    loans
        .iter()
        .filter(|loan| loan.function.stable_id == function.stable_id)
        .count()
}

/// `Ok(true)` when `expression` is the exact retained descriptor of one
/// authored slot. A descriptor with no authored slot, or an authored slot
/// whose retained node disagrees, refuses.
pub(super) fn matches(
    loans: &[Lend<'_>],
    function: &Function,
    expression: &hir::ResolvedExpr,
) -> Result<bool> {
    let execution = execution(function);
    let slot = loans.iter().find(|loan| {
        loan.function.stable_id == function.stable_id
            && expression.id == hir::ExpressionId::new(&execution, &loan.path)
    });
    let hir::ResolvedExprKind::RowLoan { loan, args } = &expression.kind else {
        return if slot.is_some() {
            Err(refusal())
        } else {
            Ok(false)
        };
    };
    let expected = slot.ok_or_else(refusal)?;
    let callee = hir::DeclarationId::new(expected.callee);
    let parameter = hir::ValueId::parameter(
        &hir::FunctionExecutionId::Monomorphic(callee.clone()),
        expected.position,
    );
    if loan.version != hir::RowLoanVersion::V1
        || !element_matches(&loan.element, expected.declaration)
        || loan.traversal != hir::ExpressionId::new(&execution, &expected.region)
        || loan.bound
            != hir::ValueId::local(
                &execution,
                &budgeted_format(format_args!("{}.s0", expected.region)),
            )
        || loan.callee != callee
        || loan.parameter != parameter
        || !matches!(args.as_slice(), [vector, index]
            if exact_place(vector, &expected.root) && exact_place(index, &expected.index))
    {
        return Err(refusal());
    }
    Ok(true)
}
