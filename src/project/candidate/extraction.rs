//! Source-authoritative extraction of one authenticated checked Copy expression.
//! Captures are resolved ValueIds, never names guessed from source text.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::ast::{Expr, ExprKind, Function, Param, ParamMode, Program, Span, Type};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, OwnershipMode, ResolvedBinding, ResolvedExprKind, ResolvedMatchPattern,
    ResolvedRecordMatchFieldPattern, ResolvedStatement, ResolvedType,
};
use crate::project::{ProjectRevision, MAX_TOTAL_SOURCE_BYTES};

use super::{declaration, expression, intent, parse_revision};

#[path = "extraction_types.rs"]
mod types;

const MAX_NODES: usize = 4096;
const MAX_DEPTH: usize = 256;
const MAX_CAPTURES: usize = 64;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

struct Capture {
    name: String,
    ty: Type,
}

pub(super) fn apply(
    revision: &ProjectRevision,
    programs: &mut [Program],
    request: &Value,
) -> Result<(intent::IntentSummary, declaration::DeclarationAddition)> {
    validate_request(request)?;
    let target = text(request, "target")?;
    let selection = expression::authored_selection(
        revision,
        programs,
        target,
        text(request, "expression_id")?,
    )?;
    let (captures, return_type) = {
        let mut types = types::Types::new(revision, &programs[selection.owner], target)?;
        let captures = capture_plan(&selection, &mut types)?;
        (captures, types.ast(&selection.expression.ty)?)
    };
    let span = Span::default();
    let call = Expr {
        kind: ExprKind::Call {
            name: text(request, "new_name")?.to_owned(),
            type_arguments: Vec::new(),
            args: captures
                .iter()
                .map(|capture| Expr {
                    kind: ExprKind::Var(capture.name.clone()),
                    span,
                })
                .collect(),
        },
        span,
    };
    let slot = expression::authored_slot(programs, &selection)?;
    let block = matches!(slot.kind, ExprKind::Block { .. });
    let replacement = if block {
        Expr {
            kind: ExprKind::Block {
                statements: Vec::new(),
                tail: Box::new(call),
            },
            span,
        }
    } else {
        call
    };
    let body = std::mem::replace(slot, replacement);
    let mut effects = selection.effects.to_vec();
    effects.sort();
    effects.dedup();
    let function = Function {
        stable_id: text(request, "new_id")?.to_owned(),
        explicit_id: true,
        name: text(request, "new_name")?.to_owned(),
        name_span: span,
        type_parameters: Vec::new(),
        params: captures
            .into_iter()
            .map(|capture| Param {
                name: capture.name,
                mode: ParamMode::Value,
                ty: capture.ty,
                span,
            })
            .collect(),
        return_type,
        effects,
        requires: Vec::new(),
        ensures: Vec::new(),
        body,
        span,
    };
    let addition = declaration::append_function(revision, programs, target, function)?;
    Ok((
        intent::IntentSummary {
            target_id: target.to_owned(),
            kind: "extract_function".to_owned(),
            migrated_calls: 0,
        },
        addition,
    ))
}

/// Reconstruct the exact compiler-derived helper and source splice, then check
/// the admitted original expression location still has its type/ownership.
pub(super) fn validate(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<()> {
    validate_request(request)?;
    let mut expected = parse_revision(before)?;
    let _ = apply(before, &mut expected, request)?;
    if expected.len() != after.sources().len() {
        return Err(invalid("extraction changed the declared source inventory"));
    }
    for (program, source) in expected.iter().zip(after.sources()) {
        let (canonical, overflow) =
            crate::bounded_output::with_limit(MAX_TOTAL_SOURCE_BYTES, || {
                crate::format::canonical(program)
            });
        if overflow {
            return Err(limit("extraction replay source exceeds its bound"));
        }
        if program.path != source.path() || canonical != source.source() {
            return Err(invalid(
                "extraction helper/source does not match exact compiler reconstruction",
            ));
        }
    }
    expression::validate_replacement(before, after, &expression_selector(request)?)
}

/// Called only after the rebase layer rejects competing anchor body/signature
/// edits and new-identity conflicts. This remaps a selector, not merge authority.
pub(super) fn rebase_intent(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<Value> {
    validate_request(request)?;
    let remapped = expression::rebase_intent(before, after, &expression_selector(request)?)?;
    let mut result = request.clone();
    result["expression_id"] = remapped["expression_id"].clone();
    Ok(result)
}

fn expression_selector(request: &Value) -> Result<Value> {
    Ok(
        json!({"kind":"replace_expression","target":text(request,"target")?,"expression_id":text(request,"expression_id")?,"replacement":null}),
    )
}

fn capture_plan(
    selection: &expression::AuthoredExpression<'_>,
    types: &mut types::Types<'_>,
) -> Result<Vec<Capture>> {
    let mut pending = vec![(selection.expression, 0usize)];
    let mut nodes = Vec::new();
    while let Some((node, depth)) = pending.pop() {
        if depth > MAX_DEPTH || nodes.len() >= MAX_NODES {
            return Err(limit("extraction expression traversal exceeds its bound"));
        }
        types.check(&node.ty)?;
        if node.ownership != OwnershipMode::Value {
            return Err(invalid(
                "extraction does not admit owned or borrowed expression values",
            ));
        }
        if matches!(
            node.kind,
            ResolvedExprKind::Try { .. }
                | ResolvedExprKind::TryOption { .. }
                | ResolvedExprKind::Upcast { .. }
                | ResolvedExprKind::BorrowPlace { .. }
                | ResolvedExprKind::ByteRange { .. }
        ) {
            return Err(invalid(
                "extraction cannot relocate propagation or compiler-owned ownership lowering",
            ));
        }
        nodes.push(node);
        let mut children = Vec::new();
        hir::push_resolved_expression_children_in_authored_order(node, &mut children);
        pending.extend(children.into_iter().map(|child| (child, depth + 1)));
    }
    // Definitions inside the subtree stay in the moved body, including match
    // binders. They must never be promoted into external helper parameters.
    let mut internal = BTreeSet::new();
    let mut pattern_nodes = 0;
    for node in &nodes {
        match &node.kind {
            ResolvedExprKind::Block { statements, .. } => {
                for statement in statements {
                    if matches!(statement, ResolvedStatement::Unsafe { .. }) {
                        return Err(invalid(
                            "extraction cannot relocate an unsafe audit boundary",
                        ));
                    }
                    if let ResolvedStatement::Let { binding, .. } = statement {
                        register_internal(binding, &mut internal, types)?;
                    }
                }
            }
            ResolvedExprKind::Match { arms, .. } => {
                for arm in arms {
                    pattern_definitions(&arm.pattern, &mut internal, 0, &mut pattern_nodes, types)?;
                }
            }
            _ => {}
        }
    }
    let external = selection
        .scope
        .iter()
        .map(|binding| (binding.id, binding))
        .collect::<BTreeMap<_, _>>();
    let mut used = BTreeSet::new();
    let mut captures = Vec::new();
    for node in nodes {
        if let ResolvedExprKind::Block { statements, .. } = &node.kind {
            for statement in statements {
                if let ResolvedStatement::Assign { binding, field, .. } = statement {
                    if field.is_some() || !internal.contains(binding.id.as_str()) {
                        return Err(invalid(
                            "extraction cannot copy back writes to an enclosing binding",
                        ));
                    }
                }
            }
        }
        if let ResolvedExprKind::Place(place) = &node.kind {
            let id = place.root.as_str();
            if internal.contains(id) {
                continue;
            }
            let binding = external.get(id).ok_or_else(|| {
                invalid("extraction encountered a value outside authenticated lexical scope")
            })?;
            if binding.mutable || binding.ownership != OwnershipMode::Value {
                return Err(invalid(
                    "extraction requires immutable by-value Copy captures",
                ));
            }
            // A field read captures the authenticated whole root, never a
            // field-shaped argument or a spelling-derived synthetic binding.
            types.check(binding.ty)?;
            if used.insert(id) {
                if captures.len() >= MAX_CAPTURES {
                    return Err(limit("extraction capture count exceeds its limit"));
                }
                captures.push(Capture {
                    name: binding.name.to_owned(),
                    ty: types.ast(binding.ty)?,
                });
            }
        }
    }
    Ok(captures)
}

fn register_internal(
    binding: &ResolvedBinding,
    internal: &mut BTreeSet<String>,
    types: &mut types::Types<'_>,
) -> Result<()> {
    types.check(&binding.ty)?;
    if binding.ownership != OwnershipMode::Value {
        return Err(invalid("extraction requires Copy internal bindings"));
    }
    if !internal.insert(binding.id.as_str().to_owned()) {
        return Err(invalid("extraction internal value identity is ambiguous"));
    }
    Ok(())
}

fn pattern_definitions(
    pattern: &ResolvedMatchPattern,
    internal: &mut BTreeSet<String>,
    depth: usize,
    nodes: &mut usize,
    types: &mut types::Types<'_>,
) -> Result<()> {
    pattern_budget(depth, nodes)?;
    match pattern {
        ResolvedMatchPattern::Binding(binding) => register_internal(binding, internal, types)?,
        ResolvedMatchPattern::Variant { fields, .. } => {
            for field in fields {
                register_internal(&field.binding, internal, types)?;
            }
        }
        ResolvedMatchPattern::Record { fields, .. } => {
            for field in fields {
                record_definitions(&field.pattern, internal, depth + 1, nodes, types)?;
            }
        }
        ResolvedMatchPattern::Or(alternatives) => {
            if alternatives
                .iter()
                .any(|alternative| !matches!(alternative, ResolvedMatchPattern::Literal(_)))
            {
                return Err(invalid(
                    "extraction cannot infer shared Or-pattern bindings",
                ));
            }
        }
        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => {}
    }
    Ok(())
}

fn record_definitions(
    pattern: &ResolvedRecordMatchFieldPattern,
    internal: &mut BTreeSet<String>,
    depth: usize,
    nodes: &mut usize,
    types: &mut types::Types<'_>,
) -> Result<()> {
    pattern_budget(depth, nodes)?;
    match pattern {
        ResolvedRecordMatchFieldPattern::Binding(binding) => {
            register_internal(binding, internal, types)?
        }
        ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
            for field in fields {
                record_definitions(&field.pattern, internal, depth + 1, nodes, types)?;
            }
        }
        ResolvedRecordMatchFieldPattern::Wildcard => {}
    }
    Ok(())
}
fn pattern_budget(depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    if depth > MAX_DEPTH || *nodes > MAX_NODES {
        return Err(limit(
            "extraction binding-pattern traversal exceeds its bound",
        ));
    }
    Ok(())
}
fn validate_request(value: &Value) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("extraction intention must be an object"))?;
    if object.len() != 5
        || ["kind", "target", "expression_id", "new_id", "new_name"]
            .iter()
            .any(|field| !object.contains_key(*field))
        || text(value, "kind")? != "extract_function"
    {
        return Err(invalid(
            "extraction intention has missing or unknown fields",
        ));
    }
    for field in ["target", "expression_id", "new_id", "new_name"] {
        let _ = text(value, field)?;
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("extraction intention requires text fields"))
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G225", message)]
}
fn limit(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G226", message)]
}
