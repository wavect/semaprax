//! Immutable literal roots are authenticated from typed module expressions.
//! The bounded walk owns no heap inventory and never trusts attached plans.
use super::{
    hir_error, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedType,
};
use crate::diagnostic::Diagnostic;

mod backing;
pub use backing::StaticUtf8;
mod origin;
pub(crate) use origin::str_origin;

pub(crate) const MAX_POOL_BYTES: usize = 65_536;
const DEPTH: usize = 514;
const WORK: usize = 1_048_576;

pub(crate) fn validate_literal(value: &str) -> Result<(), Diagnostic> {
    if value.len() > MAX_POOL_BYTES {
        return Err(hir_error(
            "static UTF-8 literal exceeds 65536 decoded bytes",
        ));
    }
    Ok(())
}

pub(crate) fn validate_literal_inline(
    value: &str,
) -> Result<(), crate::cleanup_plan::renewal_profile::child_refusal::Refusal> {
    if value.len() > MAX_POOL_BYTES {
        return Err(crate::cleanup_plan::renewal_profile::child_refusal::Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::StaticUtf8LiteralExceeds65536DecodedBytes));
    }
    Ok(())
}

/// Authored-order traversal, including all contracts and concrete instances.
/// The callback receives the exact producer, decoded bytes and static/owned kind.
/// Format templates count toward the conservative selected decoded-data bound.
pub(crate) fn visit_literals(
    program: &ResolvedProgram,
    visit: impl FnMut(&ResolvedExpr, &str, bool) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    visit_roots(
        program
            .functions
            .iter()
            .flat_map(function_roots)
            .chain(program.function_templates.iter().flat_map(|template| {
                template
                    .requires
                    .iter()
                    .chain(std::iter::once(&template.body))
                    .chain(&template.ensures)
            }))
            .chain(
                program
                    .function_instances
                    .iter()
                    .flat_map(|instance| function_roots(&instance.function)),
            ),
        visit,
    )
}

fn function_roots(function: &super::ResolvedFunction) -> impl Iterator<Item = &ResolvedExpr> {
    function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
}

pub(crate) fn visit_roots<'a>(
    roots: impl IntoIterator<Item = &'a ResolvedExpr>,
    visit: impl FnMut(&ResolvedExpr, &str, bool) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let mut work = WORK;
    visit_roots_with_work(roots, &mut work, visit)
}

pub(crate) fn visit_roots_with_work<'a>(
    roots: impl IntoIterator<Item = &'a ResolvedExpr>,
    work: &mut usize,
    mut visit: impl FnMut(&ResolvedExpr, &str, bool) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    type Frame<'a> = Option<(&'a ResolvedExpr, usize)>;
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<[Frame<'_>; DEPTH]>()) {
        return Err(hir_error(
            "static UTF-8 literal census exceeds private scratch budget",
        ));
    }
    let mut frames: [Frame<'_>; DEPTH] = [None; DEPTH];
    for root in roots {
        frames[0] = Some((root, usize::MAX));
        let mut depth = 1;
        while depth != 0 {
            *work = work
                .checked_sub(1)
                .ok_or_else(|| hir_error("static UTF-8 literal census exceeds work limit"))?;
            let (expression, next) = frames[depth - 1].expect("live bounded literal cursor");
            if next == usize::MAX {
                match &expression.kind {
                    ResolvedExprKind::BorrowedUtf8Literal(value) => {
                        if expression.ty != ResolvedType::Str
                            || expression.ownership != OwnershipMode::Borrow
                        {
                            return Err(hir_error(
                                "static UTF-8 producer has invalid type or ownership",
                            ));
                        }
                        if !value.is_backed() {
                            return Err(hir_error("static UTF-8 literal backing was refused"));
                        }
                        validate_literal(value)?;
                        visit(expression, value, true)?;
                    }
                    ResolvedExprKind::String(value)
                    | ResolvedExprKind::LiteralFormat {
                        template: value, ..
                    } => visit(expression, value, false)?,
                    _ => {}
                }
                frames[depth - 1] = Some((expression, 0));
                continue;
            }
            // The open child accessor scans statement/arm prefixes. Charge
            // their full width on every lookup before performing that work.
            let lookup = match &expression.kind {
                ResolvedExprKind::Block { statements, .. }
                | ResolvedExprKind::VecConstructor { statements, .. } => statements.len(),
                ResolvedExprKind::Match { arms, .. } => arms.len(),
                _ => 0,
            };
            *work = work
                .checked_sub(lookup)
                .ok_or_else(|| hir_error("static UTF-8 literal census exceeds work limit"))?;
            if let Some(child) = super::lexical_exit::child(expression, next) {
                if depth == DEPTH {
                    return Err(hir_error(
                        "static UTF-8 literal census exceeds ordinary HIR depth",
                    ));
                }
                frames[depth - 1] = Some((expression, next + 1));
                frames[depth] = Some((child, usize::MAX));
                depth += 1;
            } else {
                frames[depth - 1] = None;
                depth -= 1;
            }
        }
    }
    Ok(())
}

/// Selective pool admission. The sum counts every source occurrence, including
/// concrete instances: no guessed deduplication discount or runtime input debit.
pub(crate) fn validate_pool(program: &ResolvedProgram) -> Result<bool, Diagnostic> {
    let mut selected = false;
    let mut bytes = 0usize;
    visit_literals(program, |_, value, is_static| {
        selected |= is_static;
        bytes = bytes
            .checked_add(value.len())
            .ok_or_else(|| hir_error("static UTF-8 decoded pool length overflows"))?;
        Ok(())
    })?;
    if selected && bytes > MAX_POOL_BYTES {
        return Err(hir_error(
            "static UTF-8 selected decoded pool exceeds 65536 bytes",
        ));
    }
    Ok(selected)
}

#[cfg(test)]
mod tests;
