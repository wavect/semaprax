//! `.spx` endpoint typestate checking (issue #297 follow-on, R21): the
//! smallest admissible static check that a function's own straight-line/
//! if-else sequence of calls to a declared session protocol's `via`-bound
//! functions traces one legal path through that protocol's state machine.
//!
//! # Opt-in
//!
//! A function opts in with `follows session protocol "<protocol-id>"`,
//! naming a `session protocol` declaration in the *same module* by its
//! persistent `@id` ([`crate::ast::SessionProtocolFollowsClause`]), mirroring
//! how a transition's own `via` names its realizing function by persistent id
//! rather than display name. A function that does not opt in is entirely
//! unaffected: this module changes nothing about it, byte for byte.
//!
//! # What is checked, and how
//!
//! Starting from the protocol's declared `initial` state, this walks the
//! opted-in function's body once, left to right (the same evaluation order
//! `docs/RFC-0001.md` requires everywhere else), tracking the *set* of
//! protocol states the function could be in at each program point -- a set
//! rather than one state only because an `if`/`else` genuinely can leave the
//! function in either of two different states depending on a runtime
//! condition this static check does not evaluate. A direct call to one of the
//! protocol's `via`-bound functions advances every state in the current set
//! along that function's realized transition; a call the current state does
//! not admit is refused (`SPX-K108`). Every path through the function's body
//! must end in a declared terminal state (`SPX-K108`) -- exactly "every exit
//! path reaches a terminal state or a declared escape", since `Cancel`/
//! `Timeout`/`Fail` transitions already land on an ordinary terminal state in
//! this kernel (see `docs/SESSION-PROTOCOL-TYPES-V1.md`).
//!
//! # What is refused outright, and why
//!
//! This is a small, *sound* subset, not an approximation of a larger one:
//! anything this walk cannot resolve statically and precisely is refused with
//! a stable diagnostic (`SPX-K109`) rather than silently admitted or silently
//! skipped:
//!
//! - **Loops** (`while`, `for`, `for own`) whose condition/source or body
//!   reaches a `via`-bound call: the number of iterations is not known
//!   statically, so the call could execute any number of times.
//! - **Recursion**: a followed function that calls itself anywhere in its own
//!   body, `via`-bound or not -- a straight-line/if-else walk has no model
//!   for a re-entrant call.
//! - **Closures**: a `via`-bound call written inside a closure literal may
//!   run zero, one, or many times, at a point this walk cannot order against
//!   the rest of the function.
//! - **Indirect calls**: a call whose callee name is shadowed anywhere in the
//!   function by a parameter or `let` binding is never resolved as a direct
//!   call to a `via`-bound function, even where the shadowing does not
//!   syntactically reach this exact call site -- see [`shadowed_names`].
//! - **A `via` transition with a branching (`choice`) continuation**: which
//!   branch a call actually took is a runtime fact (e.g. the remote peer's
//!   reply) this static check cannot observe, so admitting the union of every
//!   declared branch would be *unsound*, not merely imprecise (a later call
//!   that is illegal on one branch but legal on another would wrongly pass).
//! - Every other expression shape this walk does not specifically know how to
//!   step through (`match`, method calls, record construction/update,
//!   `project`, `try`, `yield`) is treated as an opaque, state-preserving
//!   expression *only when it contains no reachable `via`-bound call at all*;
//!   otherwise it is refused the same way.
//!
//! # What this never grants
//!
//! Exactly like a declaration itself (`docs/SESSION-PROTOCOL-TYPES-V1.md`,
//! "Legal order is not authority"): passing this check proves only that the
//! checked function's own call sequence traces a legal path through the
//! declared graph. It adds no effect, capability, or resource authority; it
//! has no HIR node, no native/Wasm lowering, and no runtime representation --
//! a `follows` clause is erased exactly like the declaration it names.

mod endpoint;

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::ast::{
    BinaryOp, Expr, ExprKind, Function, Program, SessionProtocolDeclaration, SessionProtocolNext,
    SessionProtocolTransition, Span, Statement,
};
use crate::diagnostic::Diagnostic;

fn k_error(program: &Program, code: &'static str, message: String, span: Span) -> Diagnostic {
    Diagnostic::error(code, message, span).at_path(&program.path)
}

/// `SPX-K107`..`SPX-K109`: endpoint typestate checking for every function
/// that opts in with `follows session protocol "<protocol-id>"`. A no-op
/// (returns immediately) for a program with no such function, so an ordinary
/// program pays nothing for this check existing.
pub(crate) fn check(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = endpoint::check_declarations(program);
    // The clause shares the function grammar, so a class method can carry
    // it; only free functions are checked, so refuse it on a method rather
    // than admit an unchecked claim.
    for declaration in &program.types {
        let crate::ast::TypeDeclarationKind::Class { methods, .. } = &declaration.kind else {
            continue;
        };
        for method in methods {
            if let Some(follows) = &method.follows {
                diagnostics.push(k_error(
                    program,
                    "SPX-K109",
                    format!(
                        "method `{}` follows session protocol `{}`, but endpoint typestate checking admits only top-level functions",
                        method.name, follows.protocol_id
                    ),
                    follows.protocol_id_span,
                ).with_help("move the protocol calls into a top-level function that declares `follows`"));
            }
        }
    }
    if !program
        .functions
        .iter()
        .any(|function| function.follows.is_some())
    {
        return diagnostics;
    }
    for function in &program.functions {
        let Some(follows) = &function.follows else {
            continue;
        };
        let Some(protocol) = program
            .session_protocols
            .iter()
            .find(|declaration| declaration.stable_id == follows.protocol_id)
        else {
            diagnostics.push(k_error(
                program,
                "SPX-K107",
                format!(
                    "function `{}` follows session protocol `{}`, which is not declared in this module",
                    function.name, follows.protocol_id
                ),
                follows.protocol_id_span,
            ).with_help(
                "name a `session protocol` declaration of this same module by its `@id`",
            ));
            continue;
        };
        if let Err(diagnostic) = check_function(program, function, protocol) {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

/// Every function-local binding name (parameter, `let`, closure parameter, or
/// pattern binding) written anywhere in `function`'s own signature contracts
/// and body, at any nesting depth including inside closures, loops, and match
/// arms. A [`Call`](ExprKind::Call) whose callee name is in this set is never
/// resolved as a direct call to a top-level function of the same name: this
/// is deliberately whole-function and scope-insensitive (a binding's true
/// lexical scope is never computed), which can only over-refuse a legitimate
/// direct call elsewhere in the function that happens to share an unrelated
/// binding's name -- never under-refuse an actually indirect call.
fn shadowed_names(function: &Function) -> HashSet<String> {
    let mut names = HashSet::new();
    for param in &function.params {
        names.insert(param.name.clone());
    }
    for expression in function
        .requires
        .iter()
        .chain(function.ensures.iter())
        .chain(std::iter::once(&function.body))
    {
        collect_bindings(expression, &mut names);
    }
    names
}

fn collect_bindings(expr: &Expr, names: &mut HashSet<String>) {
    match &expr.kind {
        ExprKind::Closure { params, body, .. } => {
            for param in params {
                names.insert(param.name.clone());
            }
            collect_bindings(body, names);
        }
        ExprKind::Call { args, .. } | ExprKind::SuperMethod { args, .. } => {
            for arg in args {
                collect_bindings(arg, names);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_bindings(receiver, names);
            for arg in args {
                collect_bindings(arg, names);
            }
        }
        ExprKind::Unary { value, .. }
        | ExprKind::Project { base: value, .. }
        | ExprKind::Try { operand: value }
        | ExprKind::Yield { request: value } => collect_bindings(value, names),
        ExprKind::Binary { left, right, .. } => {
            collect_bindings(left, names);
            collect_bindings(right, names);
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                collect_statement_bindings(statement, names);
            }
            collect_bindings(tail, names);
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_bindings(condition, names);
            collect_bindings(then_branch, names);
            collect_bindings(else_branch, names);
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_bindings(&field.value, names);
            }
        }
        ExprKind::UpdateRecord { base, fields } => {
            collect_bindings(base, names);
            for field in fields {
                collect_bindings(&field.value, names);
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_bindings(scrutinee, names);
            for arm in arms {
                collect_pattern_bindings(&arm.pattern, names);
                if let Some(guard) = &arm.guard {
                    collect_bindings(guard, names);
                }
                collect_bindings(&arm.value, names);
            }
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_) => {}
    }
}

fn collect_statement_bindings(statement: &Statement, names: &mut HashSet<String>) {
    match statement {
        Statement::Let { name, value, .. } => {
            names.insert(name.clone());
            collect_bindings(value, names);
        }
        Statement::Assign { value, .. } => collect_bindings(value, names),
        Statement::Unsafe { body, .. } => collect_bindings(body, names),
        Statement::While {
            condition, body, ..
        } => {
            collect_bindings(condition, names);
            collect_bindings(body, names);
        }
        Statement::For {
            item, values, body, ..
        }
        | Statement::ForOwn {
            item, values, body, ..
        } => {
            names.insert(item.clone());
            collect_bindings(values, names);
            collect_bindings(body, names);
        }
    }
}

fn collect_pattern_bindings(pattern: &crate::ast::MatchPattern, names: &mut HashSet<String>) {
    use crate::ast::{MatchPattern, RecordMatchFieldPattern};
    match pattern {
        MatchPattern::Binding { name, .. } => {
            names.insert(name.clone());
        }
        MatchPattern::Variant { fields, .. } => {
            for field in fields {
                names.insert(field.binding.clone());
            }
        }
        MatchPattern::Record { fields, .. } => {
            fn walk_field(
                field: &crate::ast::RecordMatchPatternField,
                names: &mut HashSet<String>,
            ) {
                match &field.pattern {
                    RecordMatchFieldPattern::Binding { name, .. } => {
                        names.insert(name.clone());
                    }
                    RecordMatchFieldPattern::Wildcard { .. } => {}
                    RecordMatchFieldPattern::Record { fields, .. } => {
                        for nested in fields {
                            walk_field(nested, names);
                        }
                    }
                }
            }
            for field in fields {
                walk_field(field, names);
            }
        }
        MatchPattern::Or { alternatives, .. } => {
            for alternative in alternatives {
                collect_pattern_bindings(alternative, names);
            }
        }
        MatchPattern::Wildcard { .. } | MatchPattern::Literal { .. } => {}
    }
}

/// Does `expr` contain, anywhere at any nesting depth (including inside
/// closures, loops, and match arms), a direct call to one of `via_names`?
/// Used only to decide whether an otherwise-opaque construct (a loop body, a
/// closure, an unsupported expression shape) must be refused because it would
/// otherwise silently step over a protocol-relevant call.
fn contains_via_call(expr: &Expr, via_names: &BTreeSet<&str>) -> bool {
    match &expr.kind {
        ExprKind::Call { name, args, .. } => {
            via_names.contains(name.as_str())
                || args.iter().any(|arg| contains_via_call(arg, via_names))
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            contains_via_call(receiver, via_names)
                || args.iter().any(|arg| contains_via_call(arg, via_names))
        }
        ExprKind::SuperMethod { args, .. } => {
            args.iter().any(|arg| contains_via_call(arg, via_names))
        }
        ExprKind::Unary { value, .. }
        | ExprKind::Project { base: value, .. }
        | ExprKind::Try { operand: value }
        | ExprKind::Yield { request: value } => contains_via_call(value, via_names),
        ExprKind::Binary { left, right, .. } => {
            contains_via_call(left, via_names) || contains_via_call(right, via_names)
        }
        ExprKind::Block { statements, tail } => {
            statements
                .iter()
                .any(|statement| statement_contains_via_call(statement, via_names))
                || contains_via_call(tail, via_names)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            contains_via_call(condition, via_names)
                || contains_via_call(then_branch, via_names)
                || contains_via_call(else_branch, via_names)
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            fields
                .iter()
                .any(|field| contains_via_call(&field.value, via_names))
        }
        ExprKind::UpdateRecord { base, fields } => {
            contains_via_call(base, via_names)
                || fields
                    .iter()
                    .any(|field| contains_via_call(&field.value, via_names))
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            contains_via_call(scrutinee, via_names)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| contains_via_call(guard, via_names))
                        || contains_via_call(&arm.value, via_names)
                })
        }
        ExprKind::Closure { body, .. } => contains_via_call(body, via_names),
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_) => false,
    }
}

fn statement_contains_via_call(statement: &Statement, via_names: &BTreeSet<&str>) -> bool {
    match statement {
        Statement::Let { value, .. } | Statement::Assign { value, .. } => {
            contains_via_call(value, via_names)
        }
        Statement::Unsafe { body, .. } => contains_via_call(body, via_names),
        Statement::While {
            condition, body, ..
        } => contains_via_call(condition, via_names) || contains_via_call(body, via_names),
        Statement::For { values, body, .. } | Statement::ForOwn { values, body, .. } => {
            contains_via_call(values, via_names) || contains_via_call(body, via_names)
        }
    }
}

/// One followed function's checking context: the protocol it follows, which
/// of its `via`-bound realizing functions map to which transitions (keyed by
/// the realizing function's *display* name, since a source [`Call`] names its
/// callee that way, never by persistent id), and the whole-function shadow
/// set (see [`shadowed_names`]).
struct Context<'p> {
    program: &'p Program,
    function: &'p Function,
    protocol: &'p SessionProtocolDeclaration,
    via_by_name: BTreeMap<&'p str, Vec<&'p SessionProtocolTransition>>,
    shadowed: HashSet<String>,
}

fn check_function(
    program: &Program,
    function: &Function,
    protocol: &SessionProtocolDeclaration,
) -> Result<(), Diagnostic> {
    let by_stable_id: BTreeMap<&str, &Function> = program
        .functions
        .iter()
        .map(|candidate| (candidate.stable_id.as_str(), candidate))
        .collect();
    let mut via_by_name: BTreeMap<&str, Vec<&SessionProtocolTransition>> = BTreeMap::new();
    for transition in &protocol.transitions {
        if let Some(via) = &transition.via {
            // A `via` naming no function of this module is already refused by
            // `SPX-K104`; skip it here rather than cascade a second
            // diagnostic against the same defect.
            if let Some(target) = by_stable_id.get(via.name.as_str()) {
                via_by_name
                    .entry(target.name.as_str())
                    .or_default()
                    .push(transition);
            }
        }
    }
    let context = Context {
        program,
        function,
        protocol,
        via_by_name,
        shadowed: shadowed_names(function),
    };
    let via_names: BTreeSet<&str> = context.via_by_name.keys().copied().collect();
    for contract in function.requires.iter().chain(function.ensures.iter()) {
        if contains_via_call(contract, &via_names) {
            return Err(k_error(
                program,
                "SPX-K109",
                format!(
                    "function `{}` follows session protocol `{}`, but calls one of its `via`-bound functions from a `requires`/`ensures` contract, which typestate checking does not admit",
                    function.name, protocol.name
                ),
                contract.span,
            ));
        }
    }
    let mut states: BTreeSet<String> = BTreeSet::new();
    states.insert(protocol.initial.name.clone());
    let ends = walk_expr(&context, &function.body, states)?;
    let terminals: BTreeSet<&str> = protocol
        .terminals
        .iter()
        .map(|terminal| terminal.state.name.as_str())
        .collect();
    for end in &ends {
        if !terminals.contains(end.as_str()) {
            return Err(k_error(
                program,
                "SPX-K108",
                format!(
                    "function `{}` follows session protocol `{}`, but a path through its body ends in non-terminal state `{end}` instead of a declared terminal state",
                    function.name, protocol.name
                ),
                function.span,
            ));
        }
    }
    endpoint::check_function(program, function, protocol)?;
    Ok(())
}

/// Union `f` over every state in `states`, deduplicated. `states` is a
/// [`BTreeSet`] throughout this module rather than a list: the number of
/// distinct protocol states is already bounded (`SPX-K106`, at most 64), so
/// this can never grow without bound across arbitrarily nested `if`/`else`,
/// and a set is deterministic by construction (`AGENTS.md`'s
/// determinism invariant) with no separate sort/dedup step needed.
fn advance_all(
    states: &BTreeSet<String>,
    mut f: impl FnMut(&str) -> Result<BTreeSet<String>, Diagnostic>,
) -> Result<BTreeSet<String>, Diagnostic> {
    let mut out = BTreeSet::new();
    for state in states {
        out.extend(f(state)?);
    }
    Ok(out)
}

fn walk_expr(
    context: &Context<'_>,
    expr: &Expr,
    states: BTreeSet<String>,
) -> Result<BTreeSet<String>, Diagnostic> {
    match &expr.kind {
        ExprKind::Call { name, args, .. } => {
            let mut states = states;
            for arg in args {
                states = walk_expr(context, arg, states)?;
            }
            if name == &context.function.name {
                return Err(k_error(
                    context.program,
                    "SPX-K109",
                    format!(
                        "function `{}` follows session protocol `{}` and calls itself; typestate checking does not admit recursion",
                        context.function.name, context.protocol.name
                    ),
                    expr.span,
                ));
            }
            let Some(transitions) = context.via_by_name.get(name.as_str()) else {
                return Ok(states);
            };
            if context.shadowed.contains(name) {
                return Err(k_error(
                    context.program,
                    "SPX-K109",
                    format!(
                        "call to `{name}` is not a direct call typestate checking can resolve: `{name}` is shadowed by a local binding elsewhere in function `{}`",
                        context.function.name
                    ),
                    expr.span,
                ).with_help("typestate checking admits only a direct call to a `via`-bound function by its own name"));
            }
            advance_all(&states, |state| {
                let matches: Vec<&&SessionProtocolTransition> = transitions
                    .iter()
                    .filter(|transition| transition.from.name == state)
                    .collect();
                match matches.as_slice() {
                    [] => {
                        let admitted = transitions
                            .iter()
                            .map(|transition| transition.from.name.as_str())
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect::<Vec<_>>()
                            .join("`, `");
                        Err(k_error(
                            context.program,
                            "SPX-K108",
                            format!(
                                "call to `{name}` is not legal from state `{state}`; session protocol `{}` admits it only from state `{admitted}`",
                                context.protocol.name
                            ),
                            expr.span,
                        ))
                    }
                    [one] => match &one.next {
                        SessionProtocolNext::Then(target) => {
                            let mut next = BTreeSet::new();
                            next.insert(target.name.clone());
                            Ok(next)
                        }
                        SessionProtocolNext::Choice(_) => Err(k_error(
                            context.program,
                            "SPX-K109",
                            format!(
                                "call to `{name}` realizes session protocol `{}` transition `{}.{}`, which has a branching `choice` continuation; typestate checking does not admit resolving which branch a call took",
                                context.protocol.name, one.from.name, one.label.name
                            ),
                            expr.span,
                        )),
                    },
                    _ => Err(k_error(
                        context.program,
                        "SPX-K109",
                        format!(
                            "call to `{name}` is ambiguous: it realizes more than one session protocol `{}` transition from state `{state}`",
                            context.protocol.name
                        ),
                        expr.span,
                    )),
                }
            })
        }
        ExprKind::Unary { value, .. } => walk_expr(context, value, states),
        ExprKind::Binary { left, right, op } => {
            let states = walk_expr(context, left, states)?;
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                let via_names: BTreeSet<&str> = context.via_by_name.keys().copied().collect();
                if contains_via_call(right, &via_names) {
                    return Err(k_error(
                        context.program,
                        "SPX-K109",
                        format!(
                            "function `{}` follows session protocol `{}`, but calls one of its `via`-bound functions from the conditionally evaluated right-hand side of `{}`; typestate checking does not admit a call that may be skipped",
                            context.function.name,
                            context.protocol.name,
                            op.text(),
                        ),
                        right.span,
                    ));
                }
            }
            walk_expr(context, right, states)
        }
        ExprKind::Block { statements, tail } => {
            let mut states = states;
            for statement in statements {
                states = walk_statement(context, statement, states)?;
            }
            walk_expr(context, tail, states)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let states = walk_expr(context, condition, states)?;
            advance_all(&states, |state| {
                let mut single = BTreeSet::new();
                single.insert(state.to_owned());
                let then_ends = walk_expr(context, then_branch, single.clone())?;
                let else_ends = walk_expr(context, else_branch, single)?;
                Ok(then_ends.into_iter().chain(else_ends).collect())
            })
        }
        _ => {
            let via_names: BTreeSet<&str> = context.via_by_name.keys().copied().collect();
            if contains_via_call(expr, &via_names) {
                Err(k_error(
                    context.program,
                    "SPX-K109",
                    format!(
                        "function `{}` follows session protocol `{}`, but one of its `via`-bound functions is called from a construct (`match`, a method call, record construction/update, `project`, `try`, `yield`, or a closure) typestate checking does not admit",
                        context.function.name, context.protocol.name
                    ),
                    expr.span,
                ))
            } else {
                Ok(states)
            }
        }
    }
}

fn walk_statement(
    context: &Context<'_>,
    statement: &Statement,
    states: BTreeSet<String>,
) -> Result<BTreeSet<String>, Diagnostic> {
    match statement {
        Statement::Let { value, .. } | Statement::Assign { value, .. } => {
            walk_expr(context, value, states)
        }
        Statement::Unsafe { body, .. } => walk_expr(context, body, states),
        Statement::While {
            condition,
            body,
            span,
            ..
        } => {
            let via_names: BTreeSet<&str> = context.via_by_name.keys().copied().collect();
            if contains_via_call(condition, &via_names) || contains_via_call(body, &via_names) {
                Err(loop_refusal(context, *span))
            } else {
                Ok(states)
            }
        }
        Statement::For {
            values, body, span, ..
        }
        | Statement::ForOwn {
            values, body, span, ..
        } => {
            let via_names: BTreeSet<&str> = context.via_by_name.keys().copied().collect();
            if contains_via_call(values, &via_names) || contains_via_call(body, &via_names) {
                Err(loop_refusal(context, *span))
            } else {
                Ok(states)
            }
        }
    }
}

fn loop_refusal(context: &Context<'_>, span: Span) -> Diagnostic {
    k_error(
        context.program,
        "SPX-K109",
        format!(
            "function `{}` follows session protocol `{}`, but a loop reaches one of its `via`-bound functions; typestate checking does not admit a call whose number of executions is not statically known",
            context.function.name, context.protocol.name
        ),
        span,
    )
}

#[cfg(test)]
#[path = "typestate/tests.rs"]
mod tests;
