//! Exact original function-reuse eligibility over immutable ordered source slices.
use super::*;
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
        && current.functions().len() == previous.functions().len()
        && current
            .functions()
            .iter()
            .zip(previous.functions().iter())
            .all(|(current, previous)| function_signature_is_exact(current, previous))
}
