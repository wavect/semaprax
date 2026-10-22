//! Borrowed retained-body authority for the selective byte-helper graph schema.
//! No whole-program clone and no authority from an attached schema token.
use super::*;
use hir::{OwnershipMode, ResolvedExprKind, ResolvedType};
mod proofs;
#[cfg(test)]
mod tests;
mod type_authority;
pub(in crate::workspace_graph) use proofs::Proofs;

fn target<'a>(
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    id: &hir::DeclarationId,
) -> Option<(&'a WorkspaceResolvedModule, &'a hir::ResolvedFunction)> {
    let mut found = None;
    for module in modules {
        for function in &module.functions {
            if function.id != *id {
                continue;
            }
            if found.is_some() {
                return None;
            }
            let fact = facts.get(id.as_str())?;
            if fact.kind != hir::DeclarationKind::Function
                || fact.origin != hir::IdentityOrigin::Explicit
                || fact.owner.is_some()
                || modules
                    .iter()
                    .filter(|owner| owner.path == module.path || owner.module == module.module)
                    .count()
                    != 1
                || fact.path.as_deref() != Some(module.path.as_str())
                || fact.module.as_deref() != Some(module.module.as_str())
            {
                return None;
            }
            found = Some((module, function));
        }
    }
    found
}

// A complete bounded census grants no authority and materializes no scratch
// inventory. Only a potential helper site activates the charged proof walk.
fn candidate(statement: &hir::ResolvedStatement) -> bool {
    matches!(statement, hir::ResolvedStatement::Assign { binding, field: None, value, .. }
        if binding.ty == ResolvedType::Bytes
        && matches!(&value.kind, ResolvedExprKind::Call { callee, args, .. }
            if crate::byte_ops::by_id(callee.as_str()).is_none()
                && args.iter().any(|argument| matches!(&argument.kind, ResolvedExprKind::Place(place)
                    if place.root == binding.id && place.projections.is_empty()))))
}

#[cfg(test)]
fn census<'a>(roots: impl IntoIterator<Item = &'a hir::ResolvedExpr>) -> Option<(bool, bool)> {
    let mut work = crate::cache_codec::MAX_NODES;
    census_with_work(roots, &mut work, |_| {})
}
#[cfg(test)]
fn census_with_work<'a>(
    roots: impl IntoIterator<Item = &'a hir::ResolvedExpr>,
    work: &mut usize,
    mut visit: impl FnMut(&'a hir::ResolvedExpr),
) -> Option<(bool, bool)> {
    reserve_builder_structure(std::mem::size_of::<proofs::Walk<'_>>()).ok()?;
    let mut stack = [None; crate::cache_codec::MAX_DEPTH];
    census_in(roots, work, &mut visit, &mut stack)
}
fn census_in<'a>(
    roots: impl IntoIterator<Item = &'a hir::ResolvedExpr>,
    work: &mut usize,
    visit: &mut impl FnMut(&'a hir::ResolvedExpr),
    stack: &mut proofs::Walk<'a>,
) -> Option<(bool, bool)> {
    stack.fill(None);
    let mut found = false;
    let mut static_text = false;
    for root in roots {
        stack[0] = Some(hir::lexical_exit::ChildCursor::new(root));
        let mut depth = 1usize;
        while depth != 0 {
            *work = work.checked_sub(1)?;
            let index = depth - 1;
            let mut cursor = stack[index]?;
            let expression = cursor.expression();
            if !cursor.is_started() {
                visit(expression);
                static_text |= matches!(expression.kind, ResolvedExprKind::BorrowedUtf8Literal(_));
                if let ResolvedExprKind::Block { statements, .. }
                | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
                {
                    *work = work.checked_sub(statements.len())?;
                    found |= statements.iter().any(candidate);
                }
            }
            if let Some(child) = cursor.next() {
                if depth == stack.len() {
                    return None;
                }
                stack[index] = Some(cursor);
                stack[depth] = Some(hir::lexical_exit::ChildCursor::new(child));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Some((found, static_text))
}

#[cfg(test)]
pub(super) fn schema(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<Option<&'static str>, Vec<Diagnostic>> {
    schema_with_proofs(module, modules, facts, &mut Proofs::new(modules, facts))
}

// The existing first Walk admission owns only the three new primitive
// payment frames and the single terminal report. Each actual member acquires
// its fresh callback/borrow payload BEFORE constructing it; no future member
// payload is prepaid, retained by assumption, or credited from an old fee.
const STATIC_LEXICAL_PAYMENT: usize =
    2 * std::mem::size_of::<usize>() + 2 * std::mem::size_of::<Result<(), Vec<Diagnostic>>>();
const STATIC_LEXICAL_MEMBER: usize = 2 * std::mem::size_of::<(&mut bool, &hir::ResolvedExpr)>()
    + 2 * std::mem::size_of::<&mut bool>()
    + 8 * std::mem::size_of::<bool>()
    + 2 * std::mem::size_of::<&Option<Box<crate::workspace_graph::static_text::Inventory>>>()
    + 2 * std::mem::size_of::<Option<&crate::workspace_graph::static_text::Inventory>>()
    + std::mem::size_of::<&crate::workspace_graph::static_text::Inventory>()
    + 2 * std::mem::size_of::<
        &Option<Box<crate::workspace_graph::static_text::primitive::Inventory>>,
    >();
const STATIC_LEXICAL_TEMPLATE_GATE: usize = 2 * std::mem::size_of::<
    &Option<Box<crate::workspace_graph::static_text::Inventory>>,
>() + 4 * std::mem::size_of::<bool>();
const STATIC_LEXICAL_TERMINAL_REPORT: usize = 2 * std::mem::size_of::<
    crate::cleanup_plan::SelectedReplayReportForm,
>() + 2 * std::mem::size_of::<Vec<Diagnostic>>()
    + 2 * std::mem::size_of::<(&str, &str)>()
    + 2 * std::mem::size_of::<Diagnostic>()
    + 2 * std::mem::size_of::<String>()
    + 2 * std::mem::size_of::<std::borrow::Cow<'static, str>>()
    + std::mem::size_of::<Diagnostic>()
    + b"retained static UTF-8 source-site authority disagrees".len();
const STATIC_LEXICAL_CENSUS_CONTROLS: usize = std::mem::size_of::<proofs::Walk<'static>>()
    + 3 * STATIC_LEXICAL_PAYMENT
    + STATIC_LEXICAL_TERMINAL_REPORT;

pub(super) fn schema_with_proofs<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    proofs: &mut Proofs<'a>,
) -> Result<Option<&'static str>, Vec<Diagnostic>> {
    let fail = || {
        vec![graph_error(
            "SPX-G410",
            "retained byte helper renewal authority disagrees",
        )]
    };
    let lookup_work = modules
        .iter()
        .try_fold(0usize, |sum, module| {
            sum.checked_add(module.functions.len())
        })
        .ok_or_else(fail)?;
    reserve_builder_structure(STATIC_LEXICAL_CENSUS_CONTROLS)?;
    let mut census_stack = [None; crate::cache_codec::MAX_DEPTH];
    let mut any = false;
    for function in module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
    {
        let mut census_work = crate::cache_codec::MAX_NODES;
        reserve_builder_structure(STATIC_LEXICAL_MEMBER)?;
        let mut lexical = false;
        let (potential, static_text) = census_in(
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures),
            &mut census_work,
            &mut |expression| {
                lexical |= matches!(expression.kind, ResolvedExprKind::LexicalExit { .. })
            },
            &mut census_stack,
        )
        .ok_or_else(fail)?;
        if lexical {
            if let Some(inventory) = module.static_text.as_deref() {
                if inventory.lexical.is_none() {
                    return Err(super::static_text::REPORT.legacy());
                }
            }
        }
        if static_text && module.static_text.is_none() {
            return Err(fail());
        }
        if !potential {
            if function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19 {
                return Err(fail());
            }
            continue;
        }
        // The caller and every callee must have one source declaration owner.
        let (_, retained) = target(modules, facts, &function.id).ok_or_else(fail)?;
        if !std::ptr::eq(retained, function) {
            return Err(fail());
        }
        let required = function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19;
        let mut found = false;
        let mut stack = proofs.take_walk()?;
        stack[0] = Some(hir::lexical_exit::ChildCursor::new(&function.body));
        let mut depth = 1usize;
        let mut work = crate::cache_codec::MAX_NODES;
        while depth != 0 {
            work = work.checked_sub(1).ok_or_else(fail)?;
            let index = depth - 1;
            let mut cursor = stack[index].ok_or_else(fail)?;
            let expression = cursor.expression();
            if !cursor.is_started() {
                if let ResolvedExprKind::Block { statements, .. }
                | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
                {
                    work = work.checked_sub(statements.len()).ok_or_else(fail)?;
                    for statement in statements {
                        if !candidate(statement) {
                            continue;
                        }
                        let hir::ResolvedStatement::Assign {
                            binding,
                            field: None,
                            value,
                            ..
                        } = statement
                        else {
                            continue;
                        };
                        if binding.ty != ResolvedType::Bytes {
                            continue;
                        }
                        let ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            instance,
                            args,
                        } = &value.kind
                        else {
                            continue;
                        };
                        if crate::byte_ops::by_id(callee.as_str()).is_some() {
                            continue;
                        }
                        let Some((_, callee)) = target(modules, facts, callee) else {
                            if required {
                                return Err(fail());
                            }
                            continue;
                        };
                        if instance.is_some() || !type_arguments.is_empty() || value.ty != binding.ty
                            || value.ownership != OwnershipMode::Own || binding.ownership != OwnershipMode::Own
                            || callee.return_type != ResolvedType::Bytes || args.len() != callee.params.len()
                            || callee.params.iter().filter(|p| p.ownership == OwnershipMode::Own).count() != 1
                            || !callee.params.iter().zip(args).all(|(p, a)| p.ty == a.ty && match p.ownership {
                                OwnershipMode::Own => p.ty == ResolvedType::Bytes && a.ownership == OwnershipMode::Own
                                    && matches!(&a.kind, ResolvedExprKind::Place(place) if place.root == binding.id && place.projections.is_empty()),
                                OwnershipMode::Value => a.ownership == OwnershipMode::Value,
                                OwnershipMode::Borrow => matches!(a.ownership, OwnershipMode::Borrow | OwnershipMode::Own),
                                OwnershipMode::Shared => false,
                            }) {
                            if required { return Err(fail()); }
                            continue;
                        }
                        if !proofs.prove(modules, facts, callee, lookup_work)? {
                            // Ordinary allocating helpers retain the old profile.
                            // A v19 claim must replay every candidate exactly.
                            if required {
                                return Err(fail());
                            }
                            continue;
                        }
                        found = true;
                    }
                }
            }
            if let Some(child) = cursor.next() {
                if depth == stack.len() {
                    return Err(fail());
                }
                stack[index] = Some(cursor);
                stack[depth] = Some(hir::lexical_exit::ChildCursor::new(child));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
        proofs.restore_walk(stack);
        if found != (function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19) {
            return Err(fail());
        }
        any |= found;
    }
    reserve_builder_structure(STATIC_LEXICAL_TEMPLATE_GATE)?;
    if any || module.static_text.is_some() {
        for template in &module.function_templates {
            let mut census_work = crate::cache_codec::MAX_NODES;
            reserve_builder_structure(STATIC_LEXICAL_MEMBER)?;
            let mut lexical = false;
            let (_, static_text) = census_in(
                template
                    .requires
                    .iter()
                    .chain(std::iter::once(&template.body))
                    .chain(&template.ensures),
                &mut census_work,
                &mut |expression| {
                    lexical |= matches!(expression.kind, ResolvedExprKind::LexicalExit { .. })
                },
                &mut census_stack,
            )
            .ok_or_else(fail)?;
            if lexical {
                if let Some(inventory) = module.static_text.as_deref() {
                    if inventory.lexical.is_none() {
                        return Err(super::static_text::REPORT.legacy());
                    }
                }
            }
            if static_text && module.static_text.is_none() {
                return Err(fail());
            }
        }
    }
    Ok(any.then_some("semaprax.graph.v76"))
}

// Complete independently compiled old producer, preserving original payment.
#[cfg(test)]
pub(super) fn before_static_guard_schema_with_proofs<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    proofs: &mut Proofs<'a>,
) -> Result<Option<&'static str>, Vec<Diagnostic>> {
    let fail = || {
        vec![graph_error(
            "SPX-G410",
            "retained byte helper renewal authority disagrees",
        )]
    };
    let lookup_work = modules
        .iter()
        .try_fold(0usize, |sum, module| {
            sum.checked_add(module.functions.len())
        })
        .ok_or_else(fail)?;
    reserve_builder_structure(std::mem::size_of::<proofs::Walk<'a>>())?;
    let mut census_stack = [None; crate::cache_codec::MAX_DEPTH];
    let mut any = false;
    for function in module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
    {
        let mut census_work = crate::cache_codec::MAX_NODES;
        let (potential, static_text) = census_in(
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures),
            &mut census_work,
            &mut |_| {},
            &mut census_stack,
        )
        .ok_or_else(fail)?;
        if static_text && module.static_text.is_none() {
            return Err(fail());
        }
        if !potential {
            if function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19 {
                return Err(fail());
            }
            continue;
        }
        // The caller and every callee must have one source declaration owner.
        let (_, retained) = target(modules, facts, &function.id).ok_or_else(fail)?;
        if !std::ptr::eq(retained, function) {
            return Err(fail());
        }
        let required = function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19;
        let mut found = false;
        let mut stack = proofs.take_walk()?;
        stack[0] = Some(hir::lexical_exit::ChildCursor::new(&function.body));
        let mut depth = 1usize;
        let mut work = crate::cache_codec::MAX_NODES;
        while depth != 0 {
            work = work.checked_sub(1).ok_or_else(fail)?;
            let index = depth - 1;
            let mut cursor = stack[index].ok_or_else(fail)?;
            let expression = cursor.expression();
            if !cursor.is_started() {
                if let ResolvedExprKind::Block { statements, .. }
                | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
                {
                    work = work.checked_sub(statements.len()).ok_or_else(fail)?;
                    for statement in statements {
                        if !candidate(statement) {
                            continue;
                        }
                        let hir::ResolvedStatement::Assign {
                            binding,
                            field: None,
                            value,
                            ..
                        } = statement
                        else {
                            continue;
                        };
                        if binding.ty != ResolvedType::Bytes {
                            continue;
                        }
                        let ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            instance,
                            args,
                        } = &value.kind
                        else {
                            continue;
                        };
                        if crate::byte_ops::by_id(callee.as_str()).is_some() {
                            continue;
                        }
                        let Some((_, callee)) = target(modules, facts, callee) else {
                            if required {
                                return Err(fail());
                            }
                            continue;
                        };
                        if instance.is_some() || !type_arguments.is_empty() || value.ty != binding.ty
                            || value.ownership != OwnershipMode::Own || binding.ownership != OwnershipMode::Own
                            || callee.return_type != ResolvedType::Bytes || args.len() != callee.params.len()
                            || callee.params.iter().filter(|p| p.ownership == OwnershipMode::Own).count() != 1
                            || !callee.params.iter().zip(args).all(|(p, a)| p.ty == a.ty && match p.ownership {
                                OwnershipMode::Own => p.ty == ResolvedType::Bytes && a.ownership == OwnershipMode::Own
                                    && matches!(&a.kind, ResolvedExprKind::Place(place) if place.root == binding.id && place.projections.is_empty()),
                                OwnershipMode::Value => a.ownership == OwnershipMode::Value,
                                OwnershipMode::Borrow => matches!(a.ownership, OwnershipMode::Borrow | OwnershipMode::Own),
                                OwnershipMode::Shared => false,
                            }) {
                            if required { return Err(fail()); }
                            continue;
                        }
                        if !proofs.prove(modules, facts, callee, lookup_work)? {
                            // Ordinary allocating helpers retain the old profile.
                            // A v19 claim must replay every candidate exactly.
                            if required {
                                return Err(fail());
                            }
                            continue;
                        }
                        found = true;
                    }
                }
            }
            if let Some(child) = cursor.next() {
                if depth == stack.len() {
                    return Err(fail());
                }
                stack[index] = Some(cursor);
                stack[depth] = Some(hir::lexical_exit::ChildCursor::new(child));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
        proofs.restore_walk(stack);
        if found != (function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19) {
            return Err(fail());
        }
        any |= found;
    }
    if any && module.static_text.is_none() {
        for template in &module.function_templates {
            let mut census_work = crate::cache_codec::MAX_NODES;
            let (_, static_text) = census_in(
                template
                    .requires
                    .iter()
                    .chain(std::iter::once(&template.body))
                    .chain(&template.ensures),
                &mut census_work,
                &mut |_| {},
                &mut census_stack,
            )
            .ok_or_else(fail)?;
            if static_text && module.static_text.is_none() {
                return Err(fail());
            }
        }
    }
    Ok(any.then_some("semaprax.graph.v76"))
}

#[test]
fn static_lexical_guard_retains_the_complete_old_helper_and_funded_callback_capture() {
    let before = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/static-lexical-authority-guard-20261018/before/src/workspace_graph/source_schema/byte_helper.rs"));
    let start = before
        .find("pub(super) fn schema_with_proofs<'a>(")
        .unwrap();
    let expected = before[start..].trim_end().replacen(
        "pub(super) fn schema_with_proofs<",
        "pub(super) fn before_static_guard_schema_with_proofs<",
        1,
    );
    let current = include_str!("byte_helper.rs");
    let start = current
        .find("pub(super) fn before_static_guard_schema_with_proofs<'a>(")
        .unwrap();
    let end = current[start..].find("\n#[test]").unwrap() + start;
    assert_eq!(
        current[start..end].trim_end(),
        expected,
        "complete genuine old producer and original reservation, no new bank in baseline"
    );
    let mut lexical = false;
    let capture = |expression: &hir::ResolvedExpr| {
        lexical |= matches!(expression.kind, ResolvedExprKind::LexicalExit { .. })
    };
    assert_eq!(
        std::mem::size_of_val(&capture),
        std::mem::size_of::<&mut bool>(),
        "exact borrowed callback capture ABI on the pinned compiler"
    );
    assert_eq!(
        super::static_text::REPORT.message(),
        "retained static UTF-8 source-site authority disagrees"
    );
}
