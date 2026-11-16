//! Source positions for function and resource renames, plus the resolved
//! call-target check for schema-less function renames.

use super::*;
use crate::ast::{Type, TypeDeclaration};
use crate::hir::items::View;
use crate::hir::{
    FunctionExecutionId, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedFunctionInstance, ResolvedFunctionTemplate, ResolvedProgram,
};
use crate::workspace_graph::WorkspaceGraphProjectionModule;

pub(super) fn function_name_positions(
    program: &crate::ast::Program,
    tokens: &[lexer::Token],
    target: &crate::ast::Function,
) -> BTreeSet<(usize, usize)> {
    let mut positions = BTreeSet::from([(target.name_span.start, target.name_span.end)]);
    let methods = program
        .types
        .iter()
        .flat_map(|declaration| match &declaration.kind {
            TypeDeclarationKind::Class { methods, .. } => methods.as_slice(),
            _ => &[],
        });
    let mut collect = |callee: &str, span: crate::ast::Span| {
        if callee != target.name {
            return;
        }
        if let Some(token) = tokens.iter().find(|token| {
            token.span.start == span.start
                && matches!(&token.kind, lexer::TokenKind::Ident(name) if name == callee)
        }) {
            positions.insert((token.span.start, token.span.end));
        }
    };
    let expressions = program
        .functions
        .iter()
        .chain(methods)
        .flat_map(|function| {
            function
                .requires
                .iter()
                .chain(&function.ensures)
                .chain(std::iter::once(&function.body))
        })
        .chain(program.types.iter().flat_map(TypeDeclaration::invariants));
    for expression in expressions {
        expression.visit_calls(&mut collect);
    }
    positions
}

/// What one call site, value invocation, or function reference resolves to.
#[derive(Clone, Debug, Eq, PartialEq)]
enum CallTarget {
    Function(String),
    LocalValue(String),
    Reference(String),
}

/// Keyed by the execution owner's identity key and the structural expression
/// id. A v1 rename changes display names only, and neither key embeds one.
type CallTargets = BTreeMap<(String, String), CallTarget>;

/// Requires the candidate to resolve every call, value invocation, and
/// function reference to the same declaration or local value as the base.
/// An unresolvable base returns the resolver's own diagnostics.
pub(super) fn validate_v1_function_rename_targets(
    before: &crate::ast::Program,
    candidate: &crate::ast::Program,
) -> Result<(), Vec<Diagnostic>> {
    let before_targets = program_call_targets(&hir::resolve(before)?);
    let candidate_targets = program_call_targets(&hir::resolve(candidate)?);
    compare_call_targets(&before_targets, &candidate_targets)
}

/// Project rename counterpart of [`validate_v1_function_rename_targets`].
/// A Project module does not resolve standalone, so this compares the retained
/// per-module HIR of every Project module in the base snapshot and the
/// candidate build.
/// The typed-fact equivalence gate omits display names and records no
/// same-module call edge, so it cannot see a call that a local value captures.
pub(crate) fn validate_project_rename_targets(
    base: &[WorkspaceGraphProjectionModule],
    candidate: &[WorkspaceGraphProjectionModule],
) -> Result<(), Vec<Diagnostic>> {
    let project_targets = |modules: &[WorkspaceGraphProjectionModule]| {
        let mut targets = CallTargets::new();
        for module in modules {
            collect_call_targets(
                module.functions(),
                module.function_templates(),
                module.function_instances(),
                &mut targets,
            );
        }
        targets
    };
    compare_call_targets(&project_targets(base), &project_targets(candidate))
}

fn compare_call_targets(
    before_targets: &CallTargets,
    candidate_targets: &CallTargets,
) -> Result<(), Vec<Diagnostic>> {
    if before_targets == candidate_targets {
        return Ok(());
    }
    let (owner, expression) = before_targets
        .keys()
        .chain(candidate_targets.keys())
        .filter(|key| before_targets.get(*key) != candidate_targets.get(*key))
        .min()
        .expect("unequal call target maps differ at some key");
    Err(vec![Diagnostic::io(
        "SPX-G111",
        format!(
            "semantic patch rename changes which declaration or local value call site \
             `{owner}`/`{expression}` resolves to"
        ),
    )
    .with_help(
        "choose a name that no local binding, parameter, or closure binder in scope at a \
         renamed call site already uses, and do not rename a function whose name a local \
         value shadows at a call site",
    )])
}

fn program_call_targets(program: &ResolvedProgram) -> CallTargets {
    let mut targets = CallTargets::new();
    collect_call_targets(
        View::slice(&program.functions),
        View::slice(&program.function_templates),
        &program.function_instances,
        &mut targets,
    );
    targets
}

fn collect_call_targets(
    functions: View<'_, ResolvedFunction>,
    templates: View<'_, ResolvedFunctionTemplate>,
    instances: &[ResolvedFunctionInstance],
    targets: &mut CallTargets,
) {
    let functions = functions
        .iter()
        .map(|function| {
            (
                FunctionExecutionId::Monomorphic(function.id.clone()),
                &function.requires,
                &function.body,
                &function.ensures,
            )
        })
        .chain(templates.iter().map(|template| {
            (
                FunctionExecutionId::Monomorphic(template.id.clone()),
                &template.requires,
                &template.body,
                &template.ensures,
            )
        }))
        .chain(instances.iter().map(|instance| {
            (
                FunctionExecutionId::Generic(instance.id.clone()),
                &instance.function.requires,
                &instance.function.body,
                &instance.function.ensures,
            )
        }));
    for (owner, requires, body, ensures) in functions {
        let owner = owner.identity_key();
        let mut pending: Vec<&ResolvedExpr> = Vec::new();
        pending.extend(ensures.iter().rev());
        pending.push(body);
        pending.extend(requires.iter().rev());
        while let Some(expression) = pending.pop() {
            let target = match &expression.kind {
                ResolvedExprKind::Call { callee, .. } => {
                    Some(CallTarget::Function(callee.to_string()))
                }
                ResolvedExprKind::Invoke { callable, .. } => match &callable.kind {
                    ResolvedExprKind::Place(place) => {
                        Some(CallTarget::LocalValue(place.root.to_string()))
                    }
                    _ => None,
                },
                ResolvedExprKind::FunctionReference { target } => {
                    Some(CallTarget::Reference(target.to_string()))
                }
                _ => None,
            };
            if let Some(target) = target {
                targets.insert((owner.clone(), expression.id.to_string()), target);
            }
            if let ResolvedExprKind::Closure { body, .. } = &expression.kind {
                pending.push(body);
            }
            hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
    }
}

pub(super) fn resource_type_positions(
    program: &crate::ast::Program,
    tokens: &[lexer::Token],
    resource: &TypeDeclaration,
) -> BTreeSet<(usize, usize)> {
    let mut positions = BTreeSet::from([(resource.name_span.start, resource.name_span.end)]);
    let resource_type = Type::Named {
        name: resource.name.clone(),
        arguments: Vec::new(),
    };

    for declaration in &program.types {
        let TypeDeclarationKind::Record { fields } = &declaration.kind else {
            continue;
        };
        for field in fields {
            if field.ty == resource_type {
                insert_named_type_token(
                    &mut positions,
                    tokens,
                    field.name_span.end,
                    field.span.end,
                    &resource.name,
                );
            }
        }
    }

    for interface in &program.interfaces {
        for import in &interface.imports {
            for param in &import.params {
                if param.ty != resource_type {
                    continue;
                }
                let end = tokens
                    .iter()
                    .find(|token| {
                        token.span.start >= param.span.end
                            && token.span.end <= import.span.end
                            && matches!(
                                token.kind,
                                lexer::TokenKind::Comma | lexer::TokenKind::RParen
                            )
                    })
                    .map_or(import.span.end, |token| token.span.start);
                insert_named_type_token(
                    &mut positions,
                    tokens,
                    param.span.end,
                    end,
                    &resource.name,
                );
            }
        }
    }

    for function in &program.functions {
        for param in &function.params {
            if param.ty != resource_type {
                continue;
            }
            let end = tokens
                .iter()
                .find(|token| {
                    token.span.start >= param.span.end
                        && matches!(
                            token.kind,
                            lexer::TokenKind::Comma | lexer::TokenKind::RParen
                        )
                })
                .map_or(function.body.span.start, |token| token.span.start);
            insert_named_type_token(&mut positions, tokens, param.span.end, end, &resource.name);
        }

        if function.return_type == resource_type {
            let mut parentheses = 0;
            let mut signature = tokens.iter().filter(|token| {
                token.span.start >= function.name_span.end
                    && token.span.end <= function.body.span.start
            });
            if let Some(arrow) = signature.find(|token| match token.kind {
                lexer::TokenKind::LParen => {
                    parentheses += 1;
                    false
                }
                lexer::TokenKind::RParen => {
                    parentheses -= 1;
                    false
                }
                lexer::TokenKind::Arrow => parentheses == 0,
                _ => false,
            }) {
                insert_named_type_token(
                    &mut positions,
                    tokens,
                    arrow.span.end,
                    function.body.span.start,
                    &resource.name,
                );
            }
        }
    }

    positions
}

fn insert_named_type_token(
    positions: &mut BTreeSet<(usize, usize)>,
    tokens: &[lexer::Token],
    start: usize,
    end: usize,
    name: &str,
) {
    if let Some(token) = tokens.iter().find(|token| {
        token.span.start >= start
            && token.span.end <= end
            && matches!(&token.kind, lexer::TokenKind::Ident(candidate) if candidate == name)
    }) {
        positions.insert((token.span.start, token.span.end));
    }
}
