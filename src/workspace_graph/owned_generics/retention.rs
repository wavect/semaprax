//! One exact selector governs both generic census preflight and retention.
use super::*;

pub(super) fn selected(
    program: &Program,
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    item: &hir::ResolvedFunctionInstance,
) -> bool {
    authored
        .get(item.template.as_str())
        .is_some_and(|owner| owner.module == program.module)
        || program.module_uses.iter().any(|module_use| {
            module_use.persistent_id == item.template.as_str()
                && (imported_vec_wrapper(programs, module_use).is_some()
                    || imported_box_wrapper(programs, module_use).is_some())
        })
}

#[cfg(test)]
mod tests;
