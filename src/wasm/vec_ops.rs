//! Reachability detection and machine-carrier admission for compiler-owned
//! bounded Vec Wasm support.

use crate::hir::{ResolvedProgram, ResolvedType};

/// The compiler-owned bounded `Vec<T>` carriers the Wasm lane lowers as one
/// `i64` host handle.
///
/// `crate::cleanup::is_owned_bounded_vec_type` answers the target-neutral
/// question from the resolved type alone. The SPX-AI-019 owned-record element
/// profile additionally needs `DeclarationIndex` facts to re-derive its element
/// admission, so it stays a separate question and every Wasm site that maps a
/// carrier onto a machine representation asks this one instead. The native lane
/// keeps its own union predicate for the same reason: widening the shared one
/// would silently change both lanes at once.
pub(crate) fn is_wasm_owned_vec_type(program: &ResolvedProgram, ty: &ResolvedType) -> bool {
    crate::cleanup::is_owned_bounded_vec_type(ty)
        || crate::hir::owned_record_collection::is_owned_record_vec_type(&program.declarations, ty)
}

/// The host element tag the SPX-AI-019 owned-record element carries.
///
/// Tags 1..=8 are the Copy scalars and 9 is the owned `Bytes` payload, so the
/// record element takes the next one. The native C11 runtime chose the same
/// number for the same slot, which keeps a reader moving between the two lanes
/// from having to translate.
pub(crate) const RECORD_ELEMENT_TAG: i32 = 10;

/// The record element's capacity ceiling: the shared owned-payload budget
/// divided by this profile's single per-element charge, not a second bound
/// invented for the Wasm lane.
///
/// The host boundary enforces it, exactly as the host already enforces the
/// 8192-element `Vec<Bytes>` ceiling; this constant is what a conforming host
/// must implement. The assertion below pins the number a host is told to use to
/// the shared constants, so changing either fails this build rather than
/// letting the Wasm ceiling drift away from the reference interpreter's and the
/// native lane's.
pub(crate) const RECORD_ELEMENT_MAX_CAPACITY: u64 = crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES
    / crate::hir::owned_record_collection::OWNED_PAYLOAD_BYTES_PER_RECORD_ELEMENT;

const _: () = assert!(RECORD_ELEMENT_MAX_CAPACITY == 4_096);

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
            is_wasm_owned_vec_type(program, &function.return_type)
                || crate::iterator_ops::is_iter(&function.return_type)
                || crate::iterator_ops::is_step(&function.return_type)
                || function.params.iter().any(|param| {
                    is_wasm_owned_vec_type(program, &param.ty)
                        || crate::iterator_ops::is_iter(&param.ty)
                        || crate::iterator_ops::is_step(&param.ty)
                })
                || std::iter::once(&function.body)
                    .chain(function.requires.iter())
                    .chain(function.ensures.iter())
                    .any(|expression| {
                        if crate::iterator_ops::resolved_expression_uses_iterator(expression) {
                            return true;
                        }
                        let mut found = false;
                        crate::hir::visit_resolved_calls(
                            expression,
                            &mut |callee, instance, type_arguments| {
                                found |= instance.is_none()
                                    && type_arguments.len() == 1
                                    && (crate::vec_ops::by_id(callee.as_str()).is_some()
                                        || crate::iterator_ops::by_id(callee.as_str()).is_some());
                            },
                        );
                        found
                    })
        })
}

/// `true` when the program names the SPX-AI-019 owned-record element profile,
/// which adds exactly one function to the owned-payload host boundary.
///
/// The remaining operations reuse the owned-payload imports with the record
/// element tag, so this answers only "does the boundary carry the extra push".
pub(crate) fn program_uses_record_vec(program: &ResolvedProgram) -> bool {
    crate::hir::owned_record_collection::program_uses_profile(program)
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
