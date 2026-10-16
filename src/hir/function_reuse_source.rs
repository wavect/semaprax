//! Exact original function-reuse eligibility over immutable ordered source slices.
use super::*;
use crate::ast::program_source::FunctionRef;
pub(super) fn environment_is_exact(
    current: &dyn ProgramSource,
    previous: &dyn ProgramSource,
) -> bool {
    // Generic declarations participate in the same complete ordered signature
    // proof, including scoped type parameters, contracts and source spans.
    // Only monomorphic HIR is cloned. Current templates and the entire instance
    // closure are rebuilt below from this exact source, never from reuse.resolved.
    current.path() == previous.path()
        && current.module() == previous.module()
        && current.module_uses() == previous.module_uses()
        && current.permits() == previous.permits()
        && current.types().iter().eq(previous.types().iter())
        && current.interfaces() == previous.interfaces()
        && current.protocols() == previous.protocols()
        && current.implementations() == previous.implementations()
        && current.function_refs().len() == previous.function_refs().len()
        && if !current.function_refs().has_stored_aliases()
            && !previous.function_refs().has_stored_aliases()
        {
            // Checked keys cannot retain transient FunctionViews. Preserve the
            // ordinary pointer-width scan without manufacturing presentation
            // values merely to compare unchanged source payload signatures.
            current
                .functions()
                .iter()
                .zip(previous.functions().iter())
                .all(|(current, previous)| FunctionRef::same_ordinary_signature(current, previous))
        } else {
            // Complete compatibility semantics for private transient inputs;
            // transient projections cannot enter checked-key reuse production.
            current
                .function_refs()
                .iter()
                .zip(previous.function_refs().iter())
                .all(|(current, previous)| current.same_signature(&previous))
        }
}
