//! Complete existing visitor with a separate imported-module local certificate.
use super::super::super::shared_local_callees::modular_session::Session;
use super::{
    call_identity, size_of, Context, Costs, Error, Expr, ExprKind, Function, Linear, Lookup, Names,
    Program, Proof, Statement, Type,
};
fn plain(ty: &Type) -> bool {
    matches!(
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
    match &expression.kind {
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
        | ExprKind::String(_)
        | ExprKind::BorrowedUtf8Literal(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. } => true,
        ExprKind::Block { statements, .. } => statements.iter().all(|s| {
            proof.charge(&s);
            match s {
                Statement::Let { declared, .. } => declared.as_ref().is_none_or(plain),
                Statement::Assign { field: None, .. } | Statement::While { .. } => true,
                _ => false,
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
        let mut complete = function.params.iter().all(|p| {
            proof.charge(&p);
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
                break;
            }
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
    if body && !supported(expression, proof) {
        *body_complete = false;
    }
    if let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &expression.kind
    {
        let callee = body && *body_complete && outside;
        proof.charge(&callee);
        if (free || callee) && outside && type_arguments.is_empty() && !names.contains(name, proof)
        {
            let loan = lookup.target(context.authored, name, args.len(), proof);
            if free && loan.auxiliary {
                proof.roots = proof.roots.saturating_add(1);
            }
            if callee {
                if let Some(target) = loan.local {
                    Session::observe(session, context, owner, target, args.len())?;
                }
            }
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
