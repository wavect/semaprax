//! SPX-AI-021 bounded owning-capture closure admission: `own fn() -> R { body }`.
//!
//! This is a separately checked profile from the Copy-scalar snapshot
//! closures in [`super::closure`] (Closures v1/v2): it admits exactly one
//! lexical owned `Bytes` capture, zero explicit parameters, and a body that
//! is exactly one call transferring that capture to an ordinary function.
//! See `docs/CLOSURES-OWNING-V1.md` for the full design and its bounded
//! restrictions.
//!
//! The construction and the one admitted call are both checked here, at the
//! source level, entirely through the existing move/availability lattice
//! (`Availability`, `SPX-O101`) that already governs every other owned
//! value in this language: the closure literal never becomes an owning
//! runtime carrier. Its checked type is the reserved sentinel below, which
//! cannot be spelled by any authored declaration, so a constructed value can
//! never escape through a function parameter, field, or return-type
//! annotation -- the only way to use one, other than moving it into a fresh
//! binding, is the direct zero-argument call this module recognizes.
use super::binding::{Availability, Binding, CheckedValue};
use super::diagnostics::error;
use super::loans::has_active_overlapping_loan;
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, Function, ParamMode, Program, Span, Type};
use crate::diagnostic::Diagnostic;
use std::collections::HashMap;

/// No authored identifier can begin with NUL, so this name can never collide
/// with a user-declared type and can never be spelled in source.
const SENTINEL_NAME: &str = "\u{0}owning-closure.v1";

fn sentinel_type(target: &str, result: &Type) -> Type {
    Type::Named {
        name: SENTINEL_NAME.to_owned(),
        arguments: vec![
            Type::Named {
                name: target.to_owned(),
                arguments: Vec::new(),
            },
            result.clone(),
        ],
    }
}

/// `true` for the checked type of an owning-capture closure value. Exposed
/// so `type_table::needs_drop` can recognize the shape without duplicating
/// this module's sentinel encoding.
pub(super) fn is_sentinel(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, .. } if name == SENTINEL_NAME)
}

fn sentinel_parts(ty: &Type) -> Option<(&str, &Type)> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if name != SENTINEL_NAME {
        return None;
    }
    let [Type::Named {
        name: target,
        arguments: empty,
    }, result] = arguments.as_slice()
    else {
        return None;
    };
    if !empty.is_empty() {
        return None;
    }
    Some((target.as_str(), result))
}

/// Check the construction of `own fn() -> R { body }`. `expression` is the
/// whole closure literal (for diagnostic spans); `return_type` and `body`
/// are its declared result and body. On success, marks the captured local
/// moved (exactly once, matching every other owned value in this language)
/// and returns the closure's checked value, whose `mode` is always `Own`.
#[allow(clippy::too_many_arguments)]
pub(super) fn check_construction(
    program: &Program,
    current: &Function,
    expression: &Expr,
    return_type: &Type,
    body: &Expr,
    allow_moves: bool,
    variables: &mut HashMap<String, Binding>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    if !current.type_parameters.is_empty() {
        diagnostics.push(error(
            program,
            "SPX-T291",
            "owning-capture closures are not admitted inside a generic function",
            expression.span,
        ));
        return None;
    }
    if !allow_moves {
        diagnostics.push(error(
            program,
            "SPX-O119",
            "owning-capture closures cannot be constructed in a contract expression",
            expression.span,
        ));
        return None;
    }
    // The parsed body is always a block (`{ ... }`); this bounded profile
    // admits only the trivial shape of exactly one tail call and no
    // statements, so unwrap that one layer before checking the call.
    let call_expr = match &body.kind {
        ExprKind::Block { statements, tail } if statements.is_empty() => tail.as_ref(),
        _ => body,
    };
    let ExprKind::Call {
        name: target_name,
        type_arguments,
        args,
    } = &call_expr.kind
    else {
        diagnostics.push(error(
            program,
            "SPX-T292",
            "an owning-capture closure body must be exactly one call transferring its captured `Bytes` payload",
            call_expr.span,
        ));
        return None;
    };
    if !type_arguments.is_empty() || args.len() != 1 {
        diagnostics.push(error(
            program,
            "SPX-T292",
            "an owning-capture closure body must call its target with exactly one argument: the captured `Bytes` payload",
            body.span,
        ));
        return None;
    }
    let Some(captured_name) = (match &args[0].kind {
        ExprKind::Var(name) => Some(name.as_str()),
        _ => None,
    }) else {
        diagnostics.push(error(
            program,
            "SPX-T292",
            "an owning-capture closure body's one argument must be a direct lexical capture, not a computed expression",
            args[0].span,
        ));
        return None;
    };
    let Some(target) = program.functions.iter().find(|f| f.name == *target_name) else {
        diagnostics.push(error(
            program,
            "SPX-T293",
            format!("owning-capture closure target `{target_name}` is not declared"),
            body.span,
        ));
        return None;
    };
    if !target.type_parameters.is_empty()
        || target.params.len() != 1
        || target.params[0].mode != ParamMode::Own
        || target.params[0].ty != Type::Bytes
        || target.return_type != *return_type
    {
        diagnostics.push(error(
            program,
            "SPX-T293",
            format!(
                "owning-capture closure target `{target_name}` must be an ordinary monomorphic function taking exactly one `own Bytes` parameter and returning the closure's declared result type"
            ),
            body.span,
        ));
        return None;
    }
    let Some(binding) = variables.get(captured_name) else {
        diagnostics.push(error(
            program,
            "SPX-T202",
            format!("unknown value `{captured_name}` in `{}`", current.name),
            args[0].span,
        ));
        return None;
    };
    if binding.ty != Type::Bytes || binding.mode != ParamMode::Own {
        diagnostics.push(error(
            program,
            "SPX-T294",
            "an owning-capture closure captures exactly one lexical owned `Bytes` value",
            args[0].span,
        ));
        return None;
    }
    match binding.availability {
        Availability::Moved => {
            diagnostics.push(
                error(
                    program,
                    "SPX-O101",
                    format!("use of resource `{captured_name}` after ownership was moved"),
                    args[0].span,
                )
                .with_help("borrow the resource if the callee does not need ownership"),
            );
            return None;
        }
        Availability::MaybeMoved => {
            diagnostics.push(error(
                program,
                "SPX-O107",
                format!(
                    "resource `{captured_name}` may have been moved on another control-flow path"
                ),
                args[0].span,
            ));
            return None;
        }
        Availability::Available => {}
    }
    if has_active_overlapping_loan(binding, &[]) {
        diagnostics.push(error(
            program,
            "SPX-T265",
            "move or call transfer would invalidate a lexical byte view",
            args[0].span,
        ));
        return None;
    }
    variables
        .get_mut(captured_name)
        .expect("checked above")
        .availability = Availability::Moved;
    Some(CheckedValue {
        ty: sentinel_type(target_name, return_type),
        mode: ParamMode::Own,
        native_unit: false,
    })
}

/// If `name` is bound to a checked owning-capture closure value, check its
/// zero-argument invocation and mark it consumed. Returns `None` when `name`
/// is not such a binding, so ordinary Call dispatch proceeds unchanged;
/// returns `Some(result)` -- possibly `None` after pushing a diagnostic --
/// when it is, so the caller never falls through to treat the sentinel type
/// as an unrelated bound value or unknown function.
#[allow(clippy::too_many_arguments)]
pub(super) fn check_call(
    program: &Program,
    name: &str,
    type_arguments: &[Type],
    args: &[Expr],
    span: Span,
    variables: &mut HashMap<String, Binding>,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Option<CheckedValue>> {
    let binding_ty = variables.get(name).map(|binding| binding.ty.clone())?;
    let (_target, result) = sentinel_parts(&binding_ty)?;
    let result = result.clone();
    if !type_arguments.is_empty() || !args.is_empty() {
        diagnostics.push(error(
            program,
            "SPX-T295",
            format!("owning-capture closure `{name}` takes no explicit arguments in this bounded profile"),
            span,
        ));
        return Some(None);
    }
    let binding = variables.get_mut(name).expect("checked above");
    match binding.availability {
        Availability::Moved => {
            diagnostics.push(
                error(
                    program,
                    "SPX-O101",
                    format!("use of resource `{name}` after ownership was moved"),
                    span,
                )
                .with_help("an owning-capture closure may be invoked at most once"),
            );
            return Some(None);
        }
        Availability::MaybeMoved => {
            diagnostics.push(error(
                program,
                "SPX-O107",
                format!("resource `{name}` may have been moved on another control-flow path"),
                span,
            ));
            return Some(None);
        }
        Availability::Available => {}
    }
    binding.availability = Availability::Moved;
    Some(Some(CheckedValue::returned(
        result.clone(),
        types.needs_drop(&result),
    )))
}

/// `true` when reading `name` as a plain value (anywhere other than the
/// direct call this module checks) must be rejected: passing it as an
/// argument, returning it, storing it in a field, aliasing it into another
/// `let`, comparing it, and so on all reduce to exactly this check, since
/// the callee of a call expression is never itself a `Var` node.
pub(super) fn reject_escaping_read(
    program: &Program,
    name: &str,
    ty: &Type,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !is_sentinel(ty) {
        return false;
    }
    diagnostics.push(
        error(
            program,
            "SPX-T296",
            format!(
                "owning-capture closure `{name}` may only be invoked directly as `{name}()`; it cannot be passed, returned, stored, or otherwise read as a value"
            ),
            span,
        )
        .with_help(format!(
            "call it directly, e.g. `let result = {name}();`, or drop it uncalled"
        )),
    );
    true
}
