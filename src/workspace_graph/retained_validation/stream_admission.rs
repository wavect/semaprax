//! Signature admission confined to the explicit streaming command profile.

use crate::hir::{self, OwnershipMode, ResolvedParam, ResolvedType};

pub(in crate::workspace_graph) fn stream_parameter_admitted(parameter: &ResolvedParam) -> bool {
    (crate::stdin_stream_ops::is_reader(&parameter.ty)
        && matches!(
            parameter.ownership,
            OwnershipMode::Own | OwnershipMode::Borrow
        ))
        || hir::useful_data_workspace_parameter_admitted(&parameter.ty, parameter.ownership)
}

pub(in crate::workspace_graph) fn stream_return_admitted(ty: &ResolvedType) -> bool {
    crate::stdin_stream_ops::is_reader(ty) || hir::useful_data_workspace_return_admitted(ty)
}
