//! Source-authoritative extraction with Copy captures/results and bounded
//! resource-free ownership wholly inside preserved nested lexical blocks.
//! Captures are resolved ValueIds, never names guessed from source text.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::ast::{BinaryOp, Expr, ExprKind, Function, Param, ParamMode, Program, Span, Type};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, OwnershipMode, ResolvedBinding, ResolvedExprKind, ResolvedMatchPattern,
    ResolvedRecordMatchFieldPattern, ResolvedStatement, ResolvedType,
};
use crate::project::{ProjectRevision, MAX_TOTAL_SOURCE_BYTES};

use super::{declaration, expression, intent, parse_revision};

#[path = "extraction_owned.rs"]
mod owned;
#[path = "extraction_types.rs"]
mod types;

const MAX_NODES: usize = 4096;
const MAX_DEPTH: usize = 256;
const MAX_CAPTURES: usize = 64;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

struct Capture {
    id: String,
    name: String,
    ty: Type,
    mode: ParamMode,
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
    let (captures, return_type, extended, owner_capture) = {
        let mut types = types::Types::new(revision, &programs[selection.owner], target)?;
        capture_plan(revision, target, &selection, &mut types)?
    };
    if owner_capture
        && (revision.entry_program().entrypoint.as_str() == target
            || revision.test_program().entrypoint.as_str() == target
            || revision
                .manifest()
                .web_exports()
                .iter()
                .any(|id| id == target))
    {
        return Err(owner_invalid(
            "owning capture extraction excludes entrypoints and manifest exports",
        ));
    }
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
    // A function's root locals survive through ensures. Keep the selected
    // nested scope nested, rather than promoting its owners into root locals.
    let body = if extended {
        Expr {
            kind: ExprKind::Block {
                statements: Vec::new(),
                tail: Box::new(body),
            },
            span,
        }
    } else {
        body
    };
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
                mode: capture.mode,
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
    expression::validate_replacement(before, after, &expression_selector(request)?)?;
    owned::validate(before, after, request)
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
    revision: &ProjectRevision,
    target: &str,
    selection: &expression::AuthoredExpression<'_>,
    types: &mut types::Types<'_>,
) -> Result<(Vec<Capture>, Type, bool, bool)> {
    let external = selection
        .scope
        .iter()
        .map(|binding| (binding.id, binding))
        .collect::<BTreeMap<_, _>>();
    let mut pending = vec![(selection.expression, 0usize)];
    let mut nodes = Vec::new();
    let mut extended = false;
    let mut selected_result_owned = false;
    let mut conditional = false;
    while let Some((node, depth)) = pending.pop() {
        if depth > MAX_DEPTH || nodes.len() >= MAX_NODES {
            return Err(limit("extraction expression traversal exceeds its bound"));
        }
        let external_owner_place = matches!(&node.kind, ResolvedExprKind::Place(place)
            if place.projections.is_empty()
                && external.get(place.root.as_str()).is_some_and(|binding|
                    binding.ownership == OwnershipMode::Own));
        if !external_owner_place {
            let owns = types.internal(&node.ty, node.ownership)?;
            if node.id == selection.expression.id {
                selected_result_owned = owns;
            } else {
                extended |= owns;
            }
        }
        conditional |= matches!(
            &node.kind,
            ResolvedExprKind::If { .. }
                | ResolvedExprKind::Match { .. }
                | ResolvedExprKind::Binary {
                    op: BinaryOp::And | BinaryOp::Or,
                    ..
                }
        );
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
                    if let ResolvedStatement::Let {
                        binding, mutable, ..
                    } = statement
                    {
                        let owns = types.internal(&binding.ty, binding.ownership)?;
                        if owns && *mutable {
                            return Err(invalid(
                                "extraction internal owning bindings must be immutable",
                            ));
                        }
                        extended |= owns;
                        if !internal.insert(binding.id.as_str().to_owned()) {
                            return Err(invalid("extraction internal value identity is ambiguous"));
                        }
                    }
                }
            }
            ResolvedExprKind::Match { mode, arms, .. } => {
                if *mode != hir::ResolvedMatchMode::Value {
                    return Err(invalid("extraction cannot relocate owning match patterns"));
                }
                for arm in arms {
                    pattern_definitions(&arm.pattern, &mut internal, 0, &mut pattern_nodes, types)?;
                }
            }
            _ => {}
        }
    }
    let mut used = BTreeSet::new();
    let mut captures = Vec::new();
    let mut owner_capture = None;
    for node in nodes {
        if let ResolvedExprKind::Block { statements, .. } = &node.kind {
            for statement in statements {
                if let ResolvedStatement::Assign { binding, field, .. } = statement {
                    if field.is_some()
                        || !internal.contains(binding.id.as_str())
                        || binding.ownership != OwnershipMode::Value
                    {
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
            if binding.mutable {
                return Err(invalid("extraction requires immutable captures"));
            }
            if binding.ownership == OwnershipMode::Own {
                if !place.projections.is_empty()
                    || !matches!(binding.ty, ResolvedType::Bytes | ResolvedType::String)
                    || node.ownership != OwnershipMode::Own
                {
                    return Err(owner_invalid(
                        "owning extraction capture requires one whole local Bytes or String place",
                    ));
                }
                if !used.insert(id) || owner_capture.replace(id.to_owned()).is_some() {
                    return Err(owner_invalid(
                        "owning extraction requires exactly one occurrence of one owner",
                    ));
                }
                if captures.len() >= MAX_CAPTURES {
                    return Err(limit("extraction capture count exceeds its limit"));
                }
                captures.push(Capture {
                    id: id.to_owned(),
                    name: binding.name.to_owned(),
                    ty: types.result(binding.ty, OwnershipMode::Own)?,
                    mode: if *binding.ty == ResolvedType::Bytes {
                        ParamMode::Own
                    } else {
                        ParamMode::Value
                    },
                });
                continue;
            }
            if binding.ownership != OwnershipMode::Value {
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
                    id: id.to_owned(),
                    name: binding.name.to_owned(),
                    ty: types.ast(binding.ty)?,
                    mode: ParamMode::Value,
                });
            }
        }
    }
    if let Some(owner) = owner_capture.as_deref() {
        if extended {
            return Err(owner_invalid(
                "owning extraction cannot combine an external owner with internal owning storage",
            ));
        }
        authenticate_owner_cleanup(revision, target, owner, selection.expression, conditional)?;
    } else {
        extended |= selected_result_owned;
    }
    if extended
        && (selection.path.is_empty()
            || !matches!(selection.expression.kind, ResolvedExprKind::Block { .. }))
    {
        return Err(invalid(
            "extraction internal owners require a non-root authored block",
        ));
    }
    let result = types.result(&selection.expression.ty, selection.expression.ownership)?;
    Ok((captures, result, extended, owner_capture.is_some()))
}

fn selected_under_conditional(
    body: &hir::ResolvedExpr,
    selected: &hir::ResolvedExpr,
) -> Result<bool> {
    let mut pending = vec![(body, false)];
    let mut visited = 0usize;
    while let Some((node, conditional)) = pending.pop() {
        if visited >= MAX_NODES {
            return Err(limit(
                "owning extraction control-flow authentication exceeds its bound",
            ));
        }
        visited += 1;
        if node.id == selected.id {
            return Ok(conditional);
        }
        let descendants_conditional = conditional
            || matches!(
                &node.kind,
                ResolvedExprKind::If { .. }
                    | ResolvedExprKind::Match { .. }
                    | ResolvedExprKind::Binary {
                        op: BinaryOp::And | BinaryOp::Or,
                        ..
                    }
            );
        let mut children = Vec::new();
        hir::push_resolved_expression_children_in_authored_order(node, &mut children);
        pending.extend(
            children
                .into_iter()
                .map(|child| (child, descendants_conditional)),
        );
    }
    Err(owner_auth(
        "owning extraction selection is absent from the authenticated provider HIR",
    ))
}

fn authenticate_owner_cleanup(
    revision: &ProjectRevision,
    target: &str,
    owner: &str,
    selected: &hir::ResolvedExpr,
    selection_contains_conditional: bool,
) -> Result<()> {
    let mut function = None;
    for module in revision.semantic.image_modules() {
        for candidate in module
            .functions()
            .iter()
            .filter(|function| function.id.as_str() == target)
        {
            if function.replace(candidate).is_some() {
                return Err(owner_auth(
                    "owning extraction provider identity is ambiguous",
                ));
            }
        }
    }
    let function =
        function.ok_or_else(|| owner_auth("owning extraction provider HIR is absent"))?;
    if selection_contains_conditional || selected_under_conditional(&function.body, selected)? {
        return Err(owner_invalid(
            "owning extraction cannot move an owner through conditional control flow",
        ));
    }
    if function
        .params
        .iter()
        .any(|parameter| parameter.id.as_str() == owner)
    {
        return Err(owner_invalid(
            "owning extraction requires a local owner, not a parameter",
        ));
    }
    let count = |root: &hir::ResolvedExpr| -> Result<usize> {
        let mut pending = vec![root];
        let mut total = 0usize;
        let mut visited = 0usize;
        while let Some(node) = pending.pop() {
            if visited >= MAX_NODES {
                return Err(limit("owning extraction authentication exceeds its bound"));
            }
            visited += 1;
            if matches!(&node.kind, ResolvedExprKind::Place(place)
                if place.root.as_str() == owner)
            {
                total += 1;
            }
            hir::push_resolved_expression_children_in_authored_order(node, &mut pending);
        }
        Ok(total)
    };
    let mut contract_count = 0usize;
    for condition in function.requires.iter().chain(&function.ensures) {
        contract_count = contract_count
            .checked_add(count(condition)?)
            .ok_or_else(|| limit("owning extraction occurrence count exceeds its bound"))?;
    }
    if contract_count != 0 || count(&function.body)? != 1 || count(selected)? != 1 {
        return Err(owner_auth(
            "owning extraction requires one body-local consuming occurrence and no contract occurrence",
        ));
    }
    let inventory = function
        .cleanup
        .slots
        .iter()
        .filter(|slot| {
            matches!(&slot.origin, crate::cleanup::CleanupStorageOrigin::Binding { value }
            if value.as_str() == owner)
        })
        .collect::<Vec<_>>();
    let plans = function
        .cleanup_plan
        .slots
        .iter()
        .filter(|slot| {
            matches!(&slot.storage, crate::cleanup_plan::StorageId::Value(value)
            if value.as_str() == owner)
        })
        .collect::<Vec<_>>();
    if inventory.len() != 1
        || plans.len() != 1
        || inventory[0].ty != plans[0].ty
        || !matches!(inventory[0].ty, ResolvedType::Bytes | ResolvedType::String)
    {
        return Err(owner_auth(
            "owning extraction local lacks one exact authenticated cleanup slot",
        ));
    }
    Ok(())
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
fn owner_invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G506", message)]
}
fn owner_auth(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G507", message)]
}
fn owner_replay(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G508", message)]
}
