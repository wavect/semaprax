//! Reachability detection for compiler-owned bounded Vec Wasm support.

use crate::hir::ResolvedProgram;

pub(crate) fn program_uses_vec(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            crate::cleanup::is_owned_bounded_vec_type(&function.return_type)
                || function
                    .params
                    .iter()
                    .any(|param| crate::cleanup::is_owned_bounded_vec_type(&param.ty))
                || std::iter::once(&function.body)
                    .chain(function.requires.iter())
                    .chain(function.ensures.iter())
                    .any(|expression| {
                        let mut found = false;
                        crate::hir::visit_resolved_calls(
                            expression,
                            &mut |callee, instance, type_arguments| {
                                found |= instance.is_none()
                                    && type_arguments.len() == 1
                                    && crate::vec_ops::by_id(callee.as_str()).is_some();
                            },
                        );
                        found
                    })
        })
}

pub(crate) fn program_uses_extended_vec(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            std::iter::once(&function.body)
                .chain(function.requires.iter())
                .chain(function.ensures.iter())
                .any(|expression| {
                    let mut found = false;
                    crate::hir::visit_resolved_calls(
                        expression,
                        &mut |callee, instance, type_arguments| {
                            found |= instance.is_none()
                                && type_arguments.len() == 1
                                && matches!(
                                    crate::vec_ops::by_id(callee.as_str()),
                                    Some(
                                        crate::vec_ops::VecOp::ReserveExact
                                            | crate::vec_ops::VecOp::Set
                                            | crate::vec_ops::VecOp::Clear
                                    )
                                );
                        },
                    );
                    found
                })
        })
}
