//! One source traversal, separate auxiliary and local-callee results.
use super::super::shared_local_callees::{Context, Costs, Failure, Session};
use super::{
    call_identity, compiler_dispatch, AuthoredDeclaration, BTreeMap, Expr, ExprKind, ModuleUseKind,
    Names, Program, Proof, Statement,
};
use crate::ast::{Function, Type};
use std::mem::size_of;
type Error = Failure;
type Index<'a> = BTreeMap<&'a str, AuthoredDeclaration<'a>>;

pub(super) struct Loan<'a> {
    pub(super) auxiliary: bool,
    pub(super) local: Option<&'a Function>,
}
impl Loan<'_> {
    pub(super) const NONE: Self = Self {
        auxiliary: false,
        local: None,
    };
}
pub(super) trait Lookup<'p> {
    fn target(
        &mut self,
        authored: &Index<'_>,
        name: &str,
        arity: usize,
        proof: &mut Proof,
    ) -> Loan<'p>;
}
struct Linear<'p> {
    program: &'p Program,
    imports: bool,
}
impl<'p> Lookup<'p> for Linear<'p> {
    fn target(
        &mut self,
        authored: &Index<'_>,
        name: &str,
        arity: usize,
        proof: &mut Proof,
    ) -> Loan<'p> {
        let program = self.program;
        let imports = self.imports;
        proof.charge(&(program, authored, imports, name, arity));
        proof.bytes = proof
            .bytes
            .saturating_add(2 * size_of::<Loan<'_>>() + 2 * size_of::<&mut Self>());
        if compiler_dispatch(name, proof) {
            return Loan::NONE;
        }
        for interface in &program.interfaces {
            proof.charge(&interface);
            for import in &interface.imports {
                proof.charge(&import);
                if import.name == name {
                    return Loan::NONE;
                }
            }
        }
        let mut found = None;
        let mut ambiguous = false;
        let mut local = None;
        for function in &program.functions {
            proof.charge(&function);
            if function.name == name {
                ambiguous |= found.is_some();
                found = Some(function);
                local = Some(function);
            }
        }
        for item in program.module_uses.iter().filter(|_| imports) {
            proof.charge(&item);
            if item.alias != name {
                continue;
            }
            // Any competing nonfunction alias also closes the proof.
            if item.kind != ModuleUseKind::Function {
                return Loan::NONE;
            }
            let Some(header) = authored.get(item.persistent_id.as_str()) else {
                return Loan::NONE;
            };
            proof.charge(&header);
            if header.module != item.target_module {
                return Loan::NONE;
            }
            let Some(function) = header.function else {
                return Loan::NONE;
            };
            proof.charge(&function);
            if function.stable_id != item.persistent_id {
                return Loan::NONE;
            }
            ambiguous |= found.is_some();
            found = Some(function);
            local = None;
        }
        proof.charge(&(found, ambiguous));
        let admitted = !ambiguous
            && found.is_some_and(|f| f.type_parameters.is_empty() && f.params.len() == arity);
        proof.charge(&admitted);
        Loan {
            auxiliary: admitted,
            local: if admitted { local } else { None },
        }
    }
}

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
    super::directory::count_joint(context, imports).map_err(|_| {
        vec![super::super::super::limit_error(
            "builder_bytes",
            super::super::super::active_builder_limit(),
        )]
    })
}

pub(super) fn linear(
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

pub(super) fn count_with_lookup<'p, 'a, L: Lookup<'p>>(
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
#[path = "shared_local_callees/reuse_tests.rs"]
mod tests;

#[path = "shared_local_callees/modular_census.rs"]
pub(in crate::workspace_graph::expected_projection) mod modular_census;
