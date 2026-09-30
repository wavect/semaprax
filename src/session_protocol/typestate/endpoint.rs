//! Affine source endpoint values carried by the ordinary unique Bytes owner.
//! Protocol checks add no handle, capability, finalizer, or runtime table.
//! HIR ownership and canonical Bytes cleanup remain authoritative on all backends.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{
    Expr, ExprKind, Function, ParamMode, Program, SessionProtocolDeclaration, SessionProtocolNext,
    Span, Statement, Type,
};
use crate::diagnostic::Diagnostic;

fn error(program: &Program, code: &'static str, span: Span, message: &str) -> Diagnostic {
    super::k_error(program, code, message.to_owned(), span)
}

pub(super) fn check_declarations(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for protocol in &program.session_protocols {
        let Some(endpoint) = &protocol.endpoint else {
            continue;
        };
        let mut reject =
            |span, message| diagnostics.push(error(program, "SPX-K110", span, message));
        if endpoint.name != "Bytes" {
            reject(
                endpoint.span,
                "the affine endpoint profile admits only the unique Bytes carrier",
            );
        }
        for terminal in &protocol.terminals {
            if !terminal.cleanup.is_empty() {
                reject(terminal.span, "endpoint cleanup is the carrier's canonical ownership cleanup; named physical cleanup operations are not admitted");
            }
        }
        for transition in &protocol.transitions {
            let Some(via) = &transition.via else {
                reject(
                    transition.span,
                    "every endpoint transition requires a checked source via function",
                );
                continue;
            };
            let Some(function) = program.functions.iter().find(|f| f.stable_id == via.name) else {
                continue;
            };
            let SessionProtocolNext::Then(next) = &transition.next else {
                reject(
                    transition.span,
                    "endpoint transitions require an unambiguous next state",
                );
                continue;
            };
            let terminal = protocol.terminals.iter().any(|t| t.state.name == next.name);
            if !transition.consumes_resource
                || !function
                    .params
                    .first()
                    .is_some_and(|p| p.mode == ParamMode::Own && p.ty == Type::Bytes)
                || function.params.iter().skip(1).any(|p| !scalar(&p.ty))
                || if terminal {
                    !scalar(&function.return_type)
                } else {
                    function.return_type != Type::Bytes
                }
            {
                reject(transition.span, "an endpoint transition consumes resource, takes its own Bytes carrier first, and returns Bytes until a terminal scalar result");
            }
        }
    }
    diagnostics
}

#[derive(Clone, PartialEq, Eq)]
struct Flow {
    live: Option<String>,
    names: BTreeSet<String>,
}

struct Checker<'a> {
    program: &'a Program,
    via: BTreeMap<&'a str, bool>,
}

pub(super) fn check_function(
    program: &Program,
    function: &Function,
    protocol: &SessionProtocolDeclaration,
) -> Result<(), Diagnostic> {
    if protocol.endpoint.is_none() {
        return Ok(());
    }
    let carriers: Vec<_> = function
        .params
        .iter()
        .filter(|p| p.ty == Type::Bytes)
        .collect();
    if carriers.len() != 1
        || carriers[0].mode != ParamMode::Own
        || function
            .params
            .iter()
            .any(|p| p.ty != Type::Bytes && !scalar(&p.ty))
        || !scalar(&function.return_type)
    {
        return Err(error(program, "SPX-K110", function.span, "an endpoint-following function takes exactly one own Bytes parameter and returns a scalar"));
    }
    let name = carriers[0].name.clone();
    let mut flow = Flow {
        live: Some(name.clone()),
        names: BTreeSet::from([name]),
    };
    let mut via = BTreeMap::new();
    for transition in &protocol.transitions {
        if let (Some(via_id), SessionProtocolNext::Then(next)) = (&transition.via, &transition.next)
        {
            if let Some(target) = program
                .functions
                .iter()
                .find(|f| f.stable_id == via_id.name)
            {
                via.insert(
                    target.name.as_str(),
                    protocol.terminals.iter().any(|t| t.state.name == next.name),
                );
            }
        }
    }
    let checker = Checker { program, via };
    for contract in function.requires.iter().chain(&function.ensures) {
        checker.opaque(contract, &flow)?;
    }
    if checker.expression(&function.body, &mut flow)? || flow.live.is_some() {
        return Err(error(program, "SPX-K111", function.body.span, "a live endpoint must reach a consuming terminal transition before leaving its followed function"));
    }
    Ok(())
}

impl Checker<'_> {
    fn refuse(&self, span: Span, message: &str) -> Diagnostic {
        error(self.program, "SPX-K111", span, message)
    }

    fn opaque(&self, expression: &Expr, flow: &Flow) -> Result<(), Diagnostic> {
        let mut endpoint = false;
        expression.visit_all_nodes(&mut |node| {
            if let ExprKind::Var(name) = &node.kind {
                endpoint |= flow.names.contains(name);
            }
            if let ExprKind::Call { name, .. } = &node.kind {
                endpoint |= self.via.contains_key(name.as_str());
            }
        });
        if endpoint {
            Err(self.refuse(
                expression.span,
                "this expression cannot capture, inspect, duplicate, or escape an endpoint",
            ))
        } else {
            Ok(())
        }
    }

    // true means that this expression transfers the one endpoint owner.
    fn expression(&self, expression: &Expr, flow: &mut Flow) -> Result<bool, Diagnostic> {
        match &expression.kind {
            ExprKind::Var(name) if flow.names.contains(name) => {
                if flow.live.as_ref() != Some(name) {
                    return Err(self.refuse(expression.span, "endpoint used after move or close"));
                }
                flow.live = None;
                Ok(true)
            }
            ExprKind::Block { statements, tail } => {
                for statement in statements {
                    match statement {
                        Statement::Let {
                            name,
                            value,
                            mutable,
                            ..
                        } => {
                            if self.expression(value, flow)? {
                                if *mutable {
                                    return Err(self.refuse(
                                        value.span,
                                        "endpoint bindings must be immutable",
                                    ));
                                }
                                flow.names.insert(name.clone());
                                flow.live = Some(name.clone());
                            }
                        }
                        Statement::Assign { name, value, .. } => {
                            if flow.names.contains(name) {
                                return Err(self
                                    .refuse(value.span, "endpoint bindings cannot be reassigned"));
                            }
                            self.opaque(value, flow)?;
                        }
                        Statement::While {
                            condition, body, ..
                        } => {
                            self.opaque(condition, flow)?;
                            self.opaque(body, flow)?;
                        }
                        Statement::For { values, body, .. }
                        | Statement::ForOwn { values, body, .. } => {
                            self.opaque(values, flow)?;
                            self.opaque(body, flow)?;
                        }
                        Statement::Unsafe { body, .. } => self.opaque(body, flow)?,
                    }
                }
                self.expression(tail, flow)
            }
            ExprKind::Call { name, args, .. } => {
                let mut carriers = Vec::new();
                for (index, arg) in args.iter().enumerate() {
                    if self.expression(arg, flow)? {
                        carriers.push(index);
                    }
                }
                if let Some(terminal) = self.via.get(name.as_str()) {
                    if carriers != [0] {
                        return Err(self.refuse(expression.span, "transition must consume the current endpoint as its first argument; replacement carriers cannot recreate it"));
                    }
                    Ok(!terminal)
                } else if carriers.is_empty() {
                    Ok(false)
                } else {
                    Err(self.refuse(
                        expression.span,
                        "endpoint may move only to a declared via transition or a local binding",
                    ))
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.expression(condition, flow)? {
                    return Err(self.refuse(condition.span, "an endpoint is not a condition"));
                }
                let mut other = flow.clone();
                let left = self.expression(then_branch, flow)?;
                let right = self.expression(else_branch, &mut other)?;
                if left || right || flow.live != other.live {
                    return Err(self.refuse(
                        expression.span,
                        "endpoint branches must retain the same local owner or both consume it",
                    ));
                }
                flow.names.extend(other.names);
                Ok(false)
            }
            _ => {
                self.opaque(expression, flow)?;
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests;

fn scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::Char
    )
}
