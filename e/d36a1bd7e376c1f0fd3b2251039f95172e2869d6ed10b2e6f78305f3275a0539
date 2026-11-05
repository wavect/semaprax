//! Complete old builtin fact selection with paid retained layout/key storage.
use super::key;
use super::storage::{Payer, Storage};
use crate::hir::{DeclarationId, DeclarationIndex, ResolvedType, TypeFacts};

fn owned<P: Payer>(layout_key: String, _storage: &mut Storage<P>) -> Option<TypeFacts> {
    Some(TypeFacts {
        copy: false,
        contains_resource: false,
        sized: true,
        needs_drop: true,
        layout_key,
    })
}

pub(super) fn facts<P: Payer>(
    index: &DeclarationIndex,
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
    storage: &mut Storage<P>,
) -> Option<TypeFacts> {
    if declaration.as_str() == crate::stdin_stream_ops::READER_ID && arguments.is_empty() {
        let text = storage.literal("stdin-stream-reader.v1")?;
        return owned(text, storage);
    }
    // Retain iterator selection and profile precedence before the later Vec/
    // Box branches. Original admission predicates borrow current declarations.
    if matches!(
        declaration.as_str(),
        crate::iterator_ops::ITER_ID | crate::iterator_ops::STEP_ID
    ) && matches!(arguments, [element] if crate::iterator_ops::resolved_element_is_admitted_in(index, element))
    {
        let version =
            if crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(
                index,
                &arguments[0],
            ) {
                "record-v3"
            } else if crate::hir::owned_leaf_collection::layout(index, &arguments[0]).is_some() {
                "owned-leaf-v4"
            } else if arguments[0] == ResolvedType::Bytes {
                "v2"
            } else {
                "v1"
            };
        let key = key::identity(&arguments[0], storage)?;
        let text = storage.text(format_args!(
            "iterator-{}:{}:{}",
            version,
            declaration.as_str(),
            key
        ))?;
        return owned(text, storage);
    }
    let [element] = arguments else {
        return None;
    };
    let prefix = match declaration.as_str() {
        crate::prelude::VEC_ID if crate::vec_ops::resolved_vec_element_is_admitted(element)
            || crate::hir::copy_record_collection::admitted(index, element)
            || crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(index, element) => "vec",
        crate::prelude::VEC_ID if crate::hir::declared_collection::text_element(index, element) => "declared-vector-only",
        crate::prelude::VEC_ID if crate::hir::owned_leaf_collection::layout(index, element).is_some() => "owned-leaf-vector-v1",
        crate::prelude::BOX_ID if crate::box_ops::resolved_box_element_is_admitted(element) => "box",
        _ => return None,
    };
    let key = key::identity(element, storage)?;
    let text = storage.text(format_args!("{prefix}:{}", key))?;
    owned(text, storage)
}
