use super::*;

pub(super) fn owned_builtin_facts(
    declarations: &DeclarationIndex,
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
) -> Option<TypeFacts> {
    if let Some(facts) = crate::stdin_stream_ops::type_facts(declaration, arguments) {
        return Some(facts);
    }
    if let Some(facts) = crate::iterator_ops::type_facts(declarations, declaration, arguments) {
        return Some(facts);
    }
    let [element] = arguments else {
        return None;
    };
    let prefix = match declaration.as_str() {
        crate::prelude::VEC_ID
            if crate::vec_ops::resolved_vec_element_is_admitted(element)
                || crate::hir::owned_record_collection::
                    is_admitted_owned_record_collection_element(declarations, element) =>
        {
            "vec"
        }
        crate::prelude::BOX_ID if crate::box_ops::resolved_box_element_is_admitted(element) => {
            "box"
        }
        _ => return None,
    };
    Some(TypeFacts {
        copy: false,
        contains_resource: false,
        sized: true,
        needs_drop: true,
        layout_key: format!("{prefix}:{}", element.identity_key()),
    })
}

/// Structural ownership facts only. Source/callable admission separately
/// authenticates the selected native Regex constructor and borrowed method.
pub(super) fn resource_result_shape(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    let [ResolvedType::Nominal {
        declaration: resource,
        arguments: resource_arguments,
    }, ResolvedType::I64] = arguments.as_slice()
    else {
        return false;
    };
    declaration.as_str() == crate::prelude::RESULT_ID
        && resource_arguments.is_empty()
        && index
            .declaration(resource)
            .is_some_and(|d| d.kind == DeclarationKind::Resource)
}
