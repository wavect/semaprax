//! Complete existing visitor with a separate imported-module local certificate.
use super::super::super::shared_local_callees::modular_session::Session;
use super::{
    call_identity, size_of, Context, Costs, Error, Expr, ExprKind, Function, Linear, Lookup, Names,
    Proof, Statement, Type,
};
fn plain(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Named { arguments, .. } if arguments.is_empty()
    ) || matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::String
            | Type::Bytes
            | Type::Str
            | Type::SliceU8
    )
}
fn ordinary_data(function: &Function, proof: &mut Proof) -> bool {
    proof.charge(&(function, &function.params, &function.return_type));
    function.explicit_id
        && function.type_parameters.is_empty()
        && function.yields.is_none()
        && function.follows.is_none()
        && plain(&function.return_type)
        && function.params.iter().all(|p| {
            proof.charge(&p);
            plain(&p.ty)
        })
}
fn supported(expression: &Expr, proof: &mut Proof) -> bool {
    proof.charge(&(expression, &expression.kind));
    // Reuse the existing constructor/statement match. No second child walk or
    // statement iterator is formed for retention. The extra predicate value
    // is accounted independently of the old classifier's returned boolean.
    proof.bytes = proof.bytes.saturating_add(2 * size_of::<bool>());
    if !matches!(
        expression.kind,
        ExprKind::Call { .. }
            | ExprKind::If { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::Var(_)
            | ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::Char(_)
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::Block { .. }
    ) {
        proof.retention_closed = false;
    }
    match &expression.kind {
        ExprKind::Call { .. }
        | ExprKind::If { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Project { .. }
        | ExprKind::Var(_)
        | ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Char(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::BorrowedUtf8Literal(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. } => true,
        ExprKind::Block { statements, .. } => statements.iter().all(|s| {
            proof.charge(&s);
            match s {
                Statement::Let { declared, .. } => {
                    // The original Option callback now carries this one lender;
                    // its original type loan and plain-result control survive.
                    proof.bytes = proof
                        .bytes
                        .saturating_add(size_of::<&mut Proof>() + 2 * size_of::<bool>());
                    declared.as_ref().is_none_or(|ty| {
                        proof.retention_closed &= matches!(
                            ty,
                            Type::I64
                                | Type::I32
                                | Type::U8
                                | Type::Usize
                                | Type::Char
                                | Type::F32
                                | Type::F64
                                | Type::Bool
                        );
                        plain(ty)
                    })
                }
                Statement::Assign { field: None, .. } | Statement::While { .. } => true,
                _ => {
                    proof.retention_closed = false;
                    false
                }
            }
        }),
        _ => false,
    }
}

pub(in crate::workspace_graph::expected_projection) fn count(
    context: Context<'_, '_>,
    imports: bool,
) -> Result<(Proof, Costs), Vec<crate::diagnostic::Diagnostic>> {
    // The directory owns its original selector and selected capacity. Its
    // coupled entry will invoke the same visitor below on that already-built DATA.
    super::super::directory::count_modular(context, imports).map_err(|_| {
        vec![super::super::super::super::limit_error(
            "builder_bytes",
            super::super::super::super::active_builder_limit(),
        )]
    })
}

pub(in crate::workspace_graph::expected_projection) fn linear(
    context: Context<'_, '_>,
    imports: bool,
    selector: usize,
) -> Result<(Proof, Costs), Error> {
    let mut lookup = Linear {
        program: context.program,
        imports,
    };
    let proof = Proof {
        roots: 0,
        bytes: selector,
        ..Default::default()
    };
    count_with_lookup(context, imports, proof, &mut lookup)
}

pub(in crate::workspace_graph::expected_projection) fn count_with_lookup<'p, 'a, L: Lookup<'p>>(
    context: Context<'p, 'a>,
    imports: bool,
    mut proof: Proof,
    lookup: &mut L,
) -> Result<(Proof, Costs), Error> {
    let program = context.program;
    proof.charge(&(program, context.authored, imports));
    proof.bytes = proof.bytes.saturating_add(size_of::<Proof>());
    proof.charge(&(context, &mut *lookup, Option::<Session<'_, '_>>::None));
    let mut session = None;
    for function in &program.functions {
        proof.charge(&function);
        if !function.type_parameters.is_empty() {
            continue;
        }
        let mut names = Names {
            entries: [None; 128],
            len: 0,
        };
        proof.charge(&names);
        proof.current_retained = 0;
        proof.retention_closed = matches!(function.yields, None)
            && matches!(function.follows, None)
            && matches!(
                function.return_type,
                Type::I64
                    | Type::I32
                    | Type::U8
                    | Type::Usize
                    | Type::Char
                    | Type::F32
                    | Type::F64
                    | Type::Bool
            );
        proof.charge(&(proof.current_retained, proof.retention_closed));
        let mut complete = function.params.iter().all(|p| {
            proof.charge(&p);
            let scalar = matches!(p.mode, crate::ast::ParamMode::Value)
                && matches!(
                    p.ty,
                    Type::I64
                        | Type::I32
                        | Type::U8
                        | Type::Usize
                        | Type::Char
                        | Type::F32
                        | Type::F64
                        | Type::Bool
                );
            proof.charge(&scalar);
            proof.retention_closed &= scalar;
            names.insert(&p.name, &mut proof)
        });
        for root in call_identity::roots(function) {
            proof.bytes = proof
                .bytes
                .saturating_add(size_of::<[Option<(&Expr, usize)>; 129]>());
            complete &= call_identity::walk(root, &mut |expression| {
                proof.charge(&expression);
                match &expression.kind {
                    ExprKind::Block { statements, .. } => statements.iter().all(|s| match s {
                        Statement::Let { name, .. } | Statement::Assign { name, .. } => {
                            names.insert(name, &mut proof)
                        }
                        Statement::For { item, .. } | Statement::ForOwn { item, .. } => {
                            names.insert(item, &mut proof)
                        }
                        Statement::ForBorrow { item, index, .. } => {
                            names.insert(item, &mut proof)
                                && index
                                    .as_ref()
                                    .is_none_or(|i| names.insert(&i.name, &mut proof))
                        }
                        _ => true,
                    }),
                    ExprKind::Closure { params, .. } => params.iter().all(|p| {
                        proof.charge(&p);
                        names.insert(&p.name, &mut proof)
                    }),
                    ExprKind::Match { arms, .. } => arms
                        .iter()
                        .all(|a| names.pattern(&a.pattern, 0, &mut proof)),
                    _ => true,
                }
            });
        }
        proof.charge(&complete);
        if !complete {
            continue;
        }
        let before = proof.roots;
        let callee_before = Session::checkpoint(&session);
        let body_allowed = Session::eligible(program) && ordinary_data(function, &mut proof);
        let mut body_complete = true;
        proof.charge(&(before, callee_before, body_allowed, body_complete));
        for root in call_identity::roots(function) {
            if !visit(
                root,
                false,
                true,
                0,
                &names,
                lookup,
                context,
                function,
                body_allowed && std::ptr::eq(root, &function.body),
                &mut body_complete,
                &mut session,
                &mut proof,
            )? {
                proof.roots = before;
                body_complete = false;
                proof.retention_closed = false;
                break;
            }
        }
        // No partial function can authorize an exclusion. Generic functions
        // were rejected before this existing traversal; runtime trees never enter.
        if proof.retention_closed {
            proof.charge(&(proof.current_retained, proof.retained));
            // Two checked aggregate additions (actual/formal operands and
            // returned/receiving Options), plus the MAX operand/result bank.
            proof.bytes =
                proof
                    .bytes
                    .saturating_add(size_of::<([usize; 12], [Option<usize>; 4], [bool; 4])>());
            proof.retained.expressions = proof
                .retained
                .expressions
                .checked_add(proof.current_retained)
                .ok_or(Error::Arithmetic)?;
            proof.retained.maximum_function =
                proof.retained.maximum_function.max(proof.current_retained);
            proof.retained.functions = proof
                .retained
                .functions
                .checked_add(1)
                .ok_or(Error::Arithmetic)?;
        }
        if !body_complete {
            Session::rollback(&mut session, callee_before)?;
        }
    }
    let costs = Session::finish(session, context)?;
    Ok((proof, costs))
}

#[allow(clippy::too_many_arguments)]
fn visit<'p, 'a, L: Lookup<'p>>(
    expression: &Expr,
    free: bool,
    outside: bool,
    depth: usize,
    names: &Names<'_>,
    lookup: &mut L,
    context: Context<'p, 'a>,
    owner: &'p Function,
    body: bool,
    body_complete: &mut bool,
    session: &mut Option<Session<'p, 'a>>,
    proof: &mut Proof,
) -> Result<bool, Error> {
    proof.charge(&(
        expression,
        free,
        outside,
        depth,
        names,
        &mut *lookup,
        context,
        owner,
        body,
        &mut *body_complete,
        &mut *session,
    ));
    proof.bytes = proof.bytes.saturating_add(size_of::<Result<bool, Error>>());
    if depth == 128 {
        return Ok(false);
    }
    if body || proof.retention_closed {
        let supported = supported(expression, proof);
        if body && !supported {
            *body_complete = false;
        }
    }
    // Only one bounded scalar field is added to the already executing visit.
    // Five arithmetic operand/result words and two predicate values cover the
    // checked-before-increment counter. There is no checked_add Option ABI.
    proof.bytes = proof.bytes.saturating_add(size_of::<bool>());
    if proof.retention_closed {
        proof.bytes =
            proof
                .bytes
                .saturating_add(size_of::<(usize, usize, usize, usize, usize, bool, bool)>());
        if depth < 32 && proof.current_retained < crate::loan_plan::MAX_LOAN_ENDPOINTS_V1 / 2 {
            // The explicit upper bound is at most 2,048, so this cannot overflow.
            proof.current_retained += 1;
        } else {
            proof.retention_closed = false;
        }
    }
    if let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &expression.kind
    {
        let callee = body && *body_complete && outside;
        proof.charge(&callee);
        let retained_call = proof.retention_closed;
        proof.charge(&retained_call);
        if (free || callee || retained_call)
            && outside
            && type_arguments.is_empty()
            && !names.contains(name, proof)
        {
            let loan = lookup.target(context.authored, name, args.len(), proof);
            if retained_call {
                // The directory's local loan authenticates exact membership,
                // arity and absence of competing/import/interface names.
                let mut scalar = false;
                proof.charge(&scalar);
                if let Some(target) = loan.local {
                    proof.charge(&target);
                    scalar = matches!(
                        target.return_type,
                        Type::I64
                            | Type::I32
                            | Type::U8
                            | Type::Usize
                            | Type::Char
                            | Type::F32
                            | Type::F64
                            | Type::Bool
                    );
                    // A distinct iterator is formed here; a borrowed Function
                    // or Param does not pay its IntoIterator/next receivers.
                    proof.bytes = proof.bytes.saturating_add(size_of::<(
                        &Vec<crate::ast::Param>,
                        &Vec<crate::ast::Param>,
                        std::slice::Iter<'_, crate::ast::Param>,
                        std::slice::Iter<'_, crate::ast::Param>,
                    )>());
                    for parameter in &target.params {
                        proof.bytes = proof.bytes.saturating_add(size_of::<(
                            [&mut std::slice::Iter<'_, crate::ast::Param>; 4],
                            [Option<&crate::ast::Param>; 2],
                            [&crate::ast::Param; 2],
                            [bool; 4],
                        )>());
                        proof.charge(&parameter);
                        scalar &= matches!(parameter.mode, crate::ast::ParamMode::Value)
                            && matches!(
                                parameter.ty,
                                Type::I64
                                    | Type::I32
                                    | Type::U8
                                    | Type::Usize
                                    | Type::Char
                                    | Type::F32
                                    | Type::F64
                                    | Type::Bool
                            );
                    }
                    // The terminal next/None publication is reached even by
                    // a zero-parameter target and cannot use a prior item bank.
                    proof.bytes = proof.bytes.saturating_add(size_of::<(
                        [&mut std::slice::Iter<'_, crate::ast::Param>; 4],
                        [Option<&crate::ast::Param>; 2],
                        [bool; 4],
                    )>());
                }
                proof.retention_closed &= scalar;
            }
            if free && loan.auxiliary {
                proof.roots = proof.roots.saturating_add(1);
            }
            if callee {
                if let Some(target) = loan.local {
                    Session::observe(session, context, owner, target, args.len())?;
                }
            }
        } else if retained_call {
            proof.retention_closed = false;
        }
    }
    let outside = outside && !matches!(expression.kind, ExprKind::Closure { .. });
    let mut index = 0;
    while let Some(child) = expression.child(index) {
        let free = match &expression.kind {
            ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::If { .. }
            | ExprKind::Call { .. } => true,
            ExprKind::Block { tail, .. } => std::ptr::eq(child, tail.as_ref()),
            _ => false,
        };
        proof.charge(&(index, child, free, outside));
        if !visit(
            child,
            free,
            outside,
            depth + 1,
            names,
            lookup,
            context,
            owner,
            body,
            body_complete,
            session,
            proof,
        )? {
            return Ok(false);
        }
        index += 1;
    }
    let complete = true;
    proof.charge(&complete);
    Ok(complete)
}

#[cfg(test)]
pub(in crate::workspace_graph::expected_projection) fn linear_loan(
    context: Context<'_, '_>,
    imports: bool,
    name: &str,
    arity: usize,
) -> (bool, bool) {
    let mut lookup = Linear {
        program: context.program,
        imports,
    };
    let loan = lookup.target(context.authored, name, arity, &mut Proof::default());
    (loan.auxiliary, loan.local.is_some())
}
