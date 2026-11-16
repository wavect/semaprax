//! Exact cached construction forecast: sum every module, never peak/filter.
use super::private_layout::registry::LayoutRegistry;
use super::{
    active_builder_limit, checked_usage, cost, dependency_identity_max, limit_error,
    AuthoredDeclaration, StructuralCost,
};
use crate::{ast::Program, diagnostic::Diagnostic};
use std::collections::BTreeMap;
pub(in crate::workspace_graph) const MODE: u8 = 7;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workspace_graph) enum Retention {
    Parsed,
    SharedProject,
    // The independent historical forecast tests still price the old
    // materialized checked projection.
    #[cfg(test)]
    Semantic,
}
fn overflow() -> Vec<Diagnostic> {
    vec![limit_error("builder_bytes", active_builder_limit())]
}
// Replace only the independently asserted per-expression bundle. Every other
// declaration/container, string-copy, generic/default/import and identity bound
// stays in the original conservative ceiling. ExpressionId clone shares Arc.
// Proved-absent literal/unary/binary root storage, and the auxiliary carriers
// of carrier-free roots, are excluded from each copy: the semantic cache
// and Project projection share one resolved owner and fit the original
// receiver layout.
pub(super) fn fixed_upper(
    upper: usize,
    raw: &StructuralCost,
    runtime: &StructuralCost,
    retention: Retention,
) -> Result<usize, Vec<Diagnostic>> {
    let count = raw
        .exact_expressions
        .checked_add(runtime.exact_expressions)
        .ok_or_else(overflow)?;
    let old = count
        .checked_mul(super::super::HIR_FIXED_EXPANSION_FACTOR)
        .and_then(|n| n.checked_mul(std::mem::size_of::<crate::ast::Expr>()))
        .ok_or_else(overflow)?;
    let absent = cost::exact_absent_storage(raw)?;
    let copies = match retention {
        Retention::Parsed | Retention::SharedProject => 1,
        #[cfg(test)]
        Retention::Semantic => 2,
    };
    let exact = count
        .checked_mul(super::super::HIR_EXPR_FIXED_BUNDLE)
        .and_then(|n| n.checked_sub(absent))
        .and_then(|n| n.checked_mul(copies))
        .ok_or_else(overflow)?;
    upper
        .checked_sub(old)
        .and_then(|n| n.checked_add(exact))
        .ok_or_else(overflow)
}
pub(super) fn summed(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    retention: Retention,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    summed_with_registry(programs, authored, retention, None)
}
pub(in crate::workspace_graph) fn summed_with_registry(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    retention: Retention,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    let _precharge = super::super::diagnostics::precharge_scope();
    super::super::diagnostics::begin_precharge_pass();
    let (mut resolve, mut runtime) = (0, 0);
    for program in programs {
        super::super::diagnostics::begin_precharge_module(&program.path);
        let maximum = Some(dependency_identity_max(program, authored, programs)?);
        let costs = super::synthetic_builder_bytes_scoped_with_retention_with_registry(
            program,
            authored,
            programs,
            maximum,
            true,
            (MODE, retention),
            registry,
        )?;
        let charged = costs.retained_clone_and_hir;
        super::super::diagnostics::record_precharge_module(&program.path, charged);
        resolve = checked_usage(resolve, charged, "builder_bytes", active_builder_limit())?;
        runtime = checked_usage(
            runtime,
            costs.runtime,
            "builder_bytes",
            active_builder_limit(),
        )?;
    }
    Ok((
        resolve,
        checked_usage(resolve, runtime, "builder_bytes", active_builder_limit())?,
    ))
}
pub(in crate::workspace_graph) fn initial(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    absent: bool,
    retention: Retention,
) -> Result<(usize, usize, bool, u8), Vec<Diagnostic>> {
    initial_with_registry(programs, authored, absent, retention, None)
}
pub(in crate::workspace_graph) fn initial_with_registry(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    absent: bool,
    retention: Retention,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(usize, usize, bool, u8), Vec<Diagnostic>> {
    match super::initial_core_prebound_with_registry(programs, authored, absent, registry) {
        Err(errors) if !absent && cost::is_builder_refusal(&errors) => {
            let (resolve, total) = summed_with_registry(programs, authored, retention, registry)?;
            Ok((resolve, total, false, MODE))
        }
        result => result,
    }
}
pub(in crate::workspace_graph) fn next(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    current: usize,
    mode: &mut u8,
    retention: Retention,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    next_with_registry(programs, authored, current, mode, retention, None)
}
pub(in crate::workspace_graph) fn next_with_registry(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    current: usize,
    mode: &mut u8,
    retention: Retention,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    if *mode < 4 {
        match super::next_retention_prebound_with_uncached_peak_with_registry(
            programs, authored, current, mode, false, registry,
        ) {
            Err(errors) if cost::is_builder_refusal(&errors) => {}
            result => return result,
        }
    }
    if *mode != 4 {
        return Err(overflow());
    }
    // Deliberately skip modes five and six: a present frontend retains ASTs.
    *mode = MODE;
    let result = summed_with_registry(programs, authored, retention, registry)?;
    if result.0 >= current {
        return Err(overflow());
    }
    Ok(result)
}

#[cfg(test)]
#[path = "cached_exact/tests.rs"]
mod tests;
