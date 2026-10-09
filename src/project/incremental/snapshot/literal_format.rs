//! A cached format template and every child are authenticated against a new
//! resolution of the cached synthetic AST. The ordinary Project replay then
//! proves that AST was reconstructed from the canonical source inventory.
use super::*;

pub(super) fn replay_source(
    source: &str,
    synthetic: &Program,
    retained: &crate::hir::ResolvedProgram,
) -> Result<()> {
    let retained_uses = retained.functions.iter().any(|function| {
        std::iter::once(&function.body)
            .chain(&function.requires)
            .chain(&function.ensures)
            .any(crate::literal_format::expression_uses)
    });
    if !retained_uses && !source.contains(crate::literal_format::NAME) {
        return Ok(());
    }
    let fresh = crate::hir::resolve(synthetic).map_err(|_| {
        invalid("semantic snapshot literal-format source cannot be independently resolved")
    })?;
    for function in &fresh.functions {
        let expected = retained
            .functions
            .iter()
            .find(|item| item.id == function.id)
            .ok_or_else(|| invalid("semantic snapshot literal-format function is absent"))?;
        let uses = std::iter::once(&function.body)
            .chain(&function.requires)
            .chain(&function.ensures)
            .any(crate::literal_format::expression_uses)
            || std::iter::once(&expected.body)
                .chain(&expected.requires)
                .chain(&expected.ensures)
                .any(crate::literal_format::expression_uses);
        if uses
            && (function.body != expected.body
                || function.requires != expected.requires
                || function.ensures != expected.ensures)
        {
            return Err(invalid(
                "semantic snapshot literal-format HIR disagrees with canonical source",
            ));
        }
    }
    Ok(())
}
