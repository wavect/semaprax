//! Positional explicit generic call arguments, independently authenticated in HIR.
use super::{DeclarationId, ResolvedType};

pub(crate) fn arguments(owner: &DeclarationId, count: usize, arguments: &[ResolvedType]) -> bool {
    count != 0
        && arguments.iter().all(|argument| match argument {
            ResolvedType::TypeParameter {
                owner: actual_owner,
                index,
            } => actual_owner == owner && usize::try_from(*index).is_ok_and(|index| index < count),
            ResolvedType::Bytes => true,
            _ => super::type_reachability::nested_record_copy_scalar_is_admitted(argument),
        })
}
