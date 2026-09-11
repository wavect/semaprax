use super::*;

pub(super) fn owned_builtin_facts(
    declarations: &DeclarationIndex,
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
) -> Option<TypeFacts> {
    if let Some(facts) = crate::iterator_ops::type_facts(declaration, arguments) {
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
