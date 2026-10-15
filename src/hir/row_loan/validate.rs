//! Independent Row Loan v1 replay over complete resolved HIR.
//!
//! Every descriptor is re-derived from its exact surroundings: the direct
//! ordinary call slot it occupies, the innermost lowered `for borrow` region
//! and that region's whole-Vec owner, element, length bound and row index,
//! and the callee's exact borrowed parameter. Each lent callee parameter is
//! then proven, from the callee's own body and contracts, to be read only
//! through Copy-scalar field places. A digest, name or cached flag is never
//! accepted as that proof.
use super::*;

mod budget;
mod comparisons;
use budget::{new_vec, push};

const MAX_ROW_LOAN_WORK: usize = crate::loan_plan::MAX_LOAN_PLAN_WORK_V1;

struct Work(usize);
impl Work {
    fn charge(&mut self) -> Result<(), Diagnostic> {
        self.units(1)
    }
    fn units(&mut self, units: usize) -> Result<(), Diagnostic> {
        let next = self.0.checked_add(units);
        let Some(next) = next.filter(|used| *used <= MAX_ROW_LOAN_WORK) else {
            self.0 = MAX_ROW_LOAN_WORK + 1;
            return Err(Diagnostic::io("SPX-H006", String::new()));
        };
        self.0 = next;
        Ok(())
    }
    fn equal(&mut self, left: &str, right: &str) -> Result<bool, Diagnostic> {
        self.units(left.len().min(right.len()).saturating_add(1))?;
        Ok(left == right)
    }
}

fn refuse(span: Span, message: &str) -> Diagnostic {
    budget::diagnostic("SPX-T284", Some(span), &[message])
}

enum Item<'a> {
    Enter {
        expression: &'a ResolvedExpr,
        slot: Option<(&'a ResolvedExpr, usize)>,
        closure: bool,
    },
    Leave,
}

struct Region<'a> {
    id: &'a ExpressionId,
    facts: crate::loan_plan::traversal::RowRegion<'a>,
}

/// Validate every row loan and prove every lent parameter. Programs without
/// row loans pay one ordinary walk and change no other validation result.
pub(crate) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    let mut work = Work(0);
    let mut proven: Vec<(&DeclarationId, usize)> = new_vec()?;
    let mut referenced: Vec<&DeclarationId> = new_vec()?;
    let executions = program
        .functions
        .iter()
        .map(|function| (function, true))
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| (&instance.function, false)),
        );
    for (function, monomorphic) in executions {
        let roots = std::iter::once(&function.body)
            .chain(&function.requires)
            .chain(&function.ensures);
        for root in roots {
            walk(
                program,
                root,
                monomorphic,
                &mut work,
                &mut proven,
                &mut referenced,
            )?;
        }
    }
    for template in &program.function_templates {
        let roots = std::iter::once(&template.body)
            .chain(&template.requires)
            .chain(&template.ensures);
        for root in roots {
            walk(
                program,
                root,
                false,
                &mut work,
                &mut proven,
                &mut referenced,
            )?;
        }
    }
    // Function values keep the ordinary aggregate ABI, so a lent helper may
    // not also be an opaque callable target.
    for (callee, _) in &proven {
        for referenced in &referenced {
            if work.equal(callee.as_str(), referenced.as_str())? {
                return Err(budget::diagnostic(
                    "SPX-T284",
                    None,
                    &[
                        "row loan helper `",
                        callee.as_str(),
                        "` cannot also be referenced as a function value",
                    ],
                ));
            }
        }
    }
    Ok(())
}

fn walk<'a>(
    program: &'a ResolvedProgram,
    root: &'a ResolvedExpr,
    monomorphic: bool,
    work: &mut Work,
    proven: &mut Vec<(&'a DeclarationId, usize)>,
    referenced: &mut Vec<&'a DeclarationId>,
) -> Result<(), Diagnostic> {
    let mut regions: Vec<Region<'a>> = new_vec()?;
    let mut pending = new_vec()?;
    push(
        &mut pending,
        Item::Enter {
            expression: root,
            slot: None,
            closure: false,
        },
    )?;
    while let Some(item) = pending.pop() {
        work.charge()?;
        let Item::Enter {
            expression,
            slot,
            closure,
        } = item
        else {
            regions.pop();
            continue;
        };
        match &expression.kind {
            ResolvedExprKind::VecFieldRead {
                origin: crate::hir::VecFieldReadOrigin::Traversal,
                element,
                args,
                ..
            } => {
                check_field_origin(
                    expression,
                    element,
                    args,
                    regions.last(),
                    monomorphic,
                    closure,
                    work,
                )?;
            }
            ResolvedExprKind::RowLoan { loan, args } => {
                if closure || !monomorphic {
                    return Err(refuse(
                        expression.span,
                        "record traversal row cannot be lent from a closure or generic body",
                    ));
                }
                let index =
                    check_loan(program, expression, loan, args, slot, regions.last(), work)?;
                let mut already = false;
                for (callee, parameter) in proven.iter() {
                    if work.equal(callee.as_str(), loan.callee.as_str())? && *parameter == index {
                        already = true;
                        break;
                    }
                }
                if !already {
                    push(proven, (&loan.callee, index))?;
                    prove_reader(program, &loan.callee, index, expression.span, work)?;
                }
                continue;
            }
            ResolvedExprKind::FunctionReference { target } => {
                push(referenced, target)?;
            }
            ResolvedExprKind::Closure { body, .. } => push(
                &mut pending,
                Item::Enter {
                    expression: body,
                    slot: None,
                    closure: true,
                },
            )?,
            _ => {}
        }
        if let Some(facts) = crate::loan_plan::traversal::row_metered::row_region(
            &program.declarations,
            expression,
            &mut |comparison| work.prefix_comparison(&program.declarations, comparison),
        )? {
            push(
                &mut regions,
                Region {
                    id: &expression.id,
                    facts,
                },
            )?;
            push(&mut pending, Item::Leave)?;
        }
        if let ResolvedExprKind::Call { args, .. } = &expression.kind {
            for (index, argument) in args.iter().enumerate().rev() {
                push(
                    &mut pending,
                    Item::Enter {
                        expression: argument,
                        slot: Some((expression, index)),
                        closure,
                    },
                )?;
            }
            continue;
        }
        for child in crate::hir::lexical_exit::ChildCursor::new(expression) {
            work.charge()?;
            push(
                &mut pending,
                Item::Enter {
                    expression: child,
                    slot: None,
                    closure,
                },
            )?;
        }
    }
    Ok(())
}

/// Replays one descriptor and returns its exact callee parameter index.
fn check_loan(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    loan: &RowLoan,
    args: &[ResolvedExpr],
    slot: Option<(&ResolvedExpr, usize)>,
    region: Option<&Region<'_>>,
    work: &mut Work,
) -> Result<usize, Diagnostic> {
    let span = expression.span;
    let invalid = |message: &str| budget::diagnostic("SPX-H006", None, &["row loan ", message]);
    let Some((call, index)) = slot else {
        return Err(refuse(
            span,
            "record traversal row can only be lent as a direct ordinary call argument",
        ));
    };
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args: call_args,
    } = &call.kind
    else {
        return Err(refuse(
            span,
            "record traversal row can only be lent to an ordinary monomorphic call",
        ));
    };
    work.identities(loan.callee.as_str(), callee.as_str())?;
    work.identities(loan.callee.as_str(), UNBOUND_CALLEE)?;
    if !type_arguments.is_empty()
        || !std::ptr::eq(&call_args[index], expression)
        || loan.version != RowLoanVersion::V1
        || !loan.is_bound()
        || loan.callee != *callee
    {
        return Err(refuse(
            span,
            "record traversal row loan is not bound to its exact ordinary call",
        ));
    }
    work.types(&expression.ty, &loan.element)?;
    work.layout(&program.declarations, &loan.element, false)?;
    if expression.ty != loan.element
        || expression.ownership != OwnershipMode::Borrow
        || !element_admitted(&program.declarations, &loan.element)
    {
        return Err(invalid(
            "element or borrowed result disagrees with its record",
        ));
    }
    let [source, row_index] = args else {
        return Err(invalid(
            "children are not exactly a Vec place and row index",
        ));
    };
    let (ResolvedExprKind::Place(owner), ResolvedExprKind::Place(index_place)) =
        (&source.kind, &row_index.kind)
    else {
        return Err(invalid("children are not exact places"));
    };
    if !index_place.projections.is_empty()
        || row_index.ty != ResolvedType::Usize
        || row_index.ownership != OwnershipMode::Value
    {
        return Err(invalid("row index is not the immutable traversal index"));
    }
    if let Some(region) = region {
        work.identities(region.id.as_str(), loan.traversal.as_str())?;
    }
    let Some(region) = region.filter(|region| *region.id == loan.traversal) else {
        return Err(invalid(
            "is not inside its exact innermost record traversal",
        ));
    };
    let facts = &region.facts;
    work.places(owner, facts.owner)?;
    work.types(&source.ty, &facts.source.ty)?;
    work.types(facts.element, &loan.element)?;
    work.identities(facts.length.as_str(), loan.bound.as_str())?;
    work.identities(index_place.root.as_str(), facts.row_index.as_str())?;
    if owner != facts.owner
        || source.ty != facts.source.ty
        || source.ownership != facts.source.ownership
        || *facts.element != loan.element
        || *facts.length != loan.bound
        || index_place.root != *facts.row_index
    {
        return Err(invalid(
            "Vec place, generation freeze, bound or index disagrees with its traversal",
        ));
    }
    let target = find_function(program, &loan.callee, work)?.ok_or_else(|| {
        refuse(
            span,
            "record traversal row can only be lent to an indexed source function",
        )
    })?;
    let parameter = target
        .params
        .get(index)
        .ok_or_else(|| invalid("parameter is absent"))?;
    budget::reserve_bytes(loan.callee.as_str().len())?;
    let execution = FunctionExecutionId::Monomorphic(loan.callee.clone());
    work.identities(parameter.id.as_str(), loan.parameter.as_str())?;
    // matches_parameter hashes the full ID and streams the canonical spelling.
    let digits = |n: usize| if n == 0 { 1 } else { n.ilog10() as usize + 1 };
    let parameter_digits = digits(index);
    let expected_bytes = "declaration:".len()
        + digits(loan.callee.as_str().len())
        + 1
        + loan.callee.as_str().len()
        + ":value:param:".len()
        + digits(parameter_digits)
        + 1
        + parameter_digits;
    work.units(loan.parameter.as_str().len().saturating_add(expected_bytes))?;
    work.types(&parameter.ty, &loan.element)?;
    if parameter.id != loan.parameter
        || !loan.parameter.matches_parameter(&execution, index)
        || parameter.ownership != OwnershipMode::Borrow
        || parameter.ty != loan.element
    {
        return Err(refuse(
            span,
            "record traversal row can only be lent to the exact `borrow` parameter of its record",
        ));
    }
    work.identities(target.id.as_str(), program.entrypoint.as_str())?;
    if target.id == program.entrypoint || target.yields.is_some() {
        return Err(refuse(
            span,
            "record traversal row cannot be lent to an entry point or suspending function",
        ));
    }
    Ok(index)
}

/// Proves that one callee parameter is a nonescaping scalar-field reader in
/// the callee's body, requires and ensures. The scalar result type alone is
/// never sufficient: every occurrence of the parameter is checked.
fn prove_reader(
    program: &ResolvedProgram,
    callee: &DeclarationId,
    index: usize,
    span: Span,
    work: &mut Work,
) -> Result<(), Diagnostic> {
    let function = find_function(program, callee, work)?
        .ok_or_else(|| refuse(span, "row loan helper is not an indexed source function"))?;
    work.charge()?;
    if !function.effects.is_empty()
        || !crate::vec_ops::resolved_element_is_admitted(&function.return_type)
        || function.body.ownership != OwnershipMode::Value
    {
        return Err(refuse(
            span,
            "row loan helper must be effect-free and return one Value Copy scalar",
        ));
    }
    let parameter = &function.params[index];
    let element = &parameter.ty;
    let escape = |name: &str| {
        budget::diagnostic(
            "SPX-T284",
            Some(span),
            &[
                "row loan helper `",
                &function.name,
                "` may only read Copy scalar fields of `",
                name,
                "`; it cannot store, return, capture, forward, view or rebind the row",
            ],
        )
    };
    let mut pending = new_vec()?;
    for root in std::iter::once(&function.body)
        .chain(&function.requires)
        .chain(&function.ensures)
    {
        work.charge()?;
        push(&mut pending, root)?;
    }
    while let Some(expression) = pending.pop() {
        work.charge()?;
        match &expression.kind {
            ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } => {
                work.identities(place.root.as_str(), parameter.id.as_str())?;
            }
            ResolvedExprKind::Closure { captures, .. } => {
                for capture in captures {
                    work.charge()?;
                    if let ResolvedExprKind::Place(place) = &capture.value.kind {
                        work.identities(place.root.as_str(), parameter.id.as_str())?;
                    }
                }
            }
            ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } => {
                for statement in statements {
                    work.charge()?;
                    if let ResolvedStatement::Assign { binding, .. } = statement {
                        work.identities(binding.id.as_str(), parameter.id.as_str())?;
                    }
                }
            }
            _ => {}
        }
        match &expression.kind {
            ResolvedExprKind::Place(place) if place.root == parameter.id => {
                work.layout(&program.declarations, element, true)?;
                work.places(place, place)?;
                let field = lent_field_read(&program.declarations, element, place)
                    .ok_or_else(|| escape(&parameter.name))?;
                work.types(&expression.ty, &field.declaration.ty)?;
                if expression.ty != field.declaration.ty
                    || expression.ownership != OwnershipMode::Value
                {
                    return Err(escape(&parameter.name));
                }
            }
            ResolvedExprKind::BorrowPlace { place, .. } if place.root == parameter.id => {
                return Err(escape(&parameter.name));
            }
            ResolvedExprKind::Closure { captures, body, .. } => {
                if captures.iter().any(|capture| {
                    matches!(&capture.value.kind, ResolvedExprKind::Place(place) if place.root == parameter.id)
                }) {
                    return Err(escape(&parameter.name));
                }
                push(&mut pending, body)?;
            }
            ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. }
                if statements.iter().any(|statement| {
                    matches!(statement, ResolvedStatement::Assign { binding, .. } if binding.id == parameter.id)
                }) =>
            {
                return Err(escape(&parameter.name));
            }
            _ => {}
        }
        for child in crate::hir::lexical_exit::ChildCursor::new(expression) {
            work.charge()?;
            push(&mut pending, child)?;
        }
    }
    Ok(())
}

fn find_function<'a>(
    program: &'a ResolvedProgram,
    id: &DeclarationId,
    work: &mut Work,
) -> Result<Option<&'a ResolvedFunction>, Diagnostic> {
    for function in &program.functions {
        if work.equal(function.id.as_str(), id.as_str())? {
            return Ok(Some(function));
        }
    }
    Ok(None)
}

// Traversal form is reconstructed from its enclosing region, never accepted
// because a cached origin tag says that a row or freeze exists.
fn check_field_origin(
    expression: &ResolvedExpr,
    element: &ResolvedType,
    args: &[ResolvedExpr],
    region: Option<&Region<'_>>,
    monomorphic: bool,
    closure: bool,
    work: &mut Work,
) -> Result<(), Diagnostic> {
    let fail = || {
        refuse(
            expression.span,
            "row-derived field read lacks its exact traversal, carrier or row index",
        )
    };
    if !monomorphic || closure {
        return Err(fail());
    }
    let region = region.ok_or_else(fail)?;
    let [vector, index] = args else {
        return Err(fail());
    };
    let (ResolvedExprKind::Place(owner), ResolvedExprKind::Place(row_index)) =
        (&vector.kind, &index.kind)
    else {
        return Err(fail());
    };
    work.types(element, region.facts.element)?;
    work.types(&vector.ty, &region.facts.source.ty)?;
    work.places(owner, region.facts.owner)?;
    work.identities(row_index.root.as_str(), region.facts.row_index.as_str())?;
    if vector.ty != region.facts.source.ty
        || vector.ownership != region.facts.source.ownership
        || element != region.facts.element
        || owner != region.facts.owner
        || row_index.root != *region.facts.row_index
        || !row_index.projections.is_empty()
        || index.ty != ResolvedType::Usize
        || index.ownership != OwnershipMode::Value
    {
        return Err(fail());
    }
    Ok(())
}
