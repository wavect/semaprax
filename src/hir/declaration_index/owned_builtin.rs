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
    let (prefix, element) = owned_builtin_prefix(declarations, declaration, arguments)?;
    Some(TypeFacts {
        copy: false,
        contains_resource: false,
        sized: true,
        needs_drop: true,
        layout_key: format!("{prefix}:{}", element.identity_key()),
    })
}

/// Flag-only sibling of [`owned_builtin_facts`] with the same admission order
/// and the same four facts. A flag query discards `layout_key`, so this never
/// builds it: the element identity key and its formatted text were unpaid
/// allocations on the funded validator chain (9c6005b20 shortcut).
pub(super) fn owned_builtin_flags(
    declarations: &DeclarationIndex,
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
) -> Option<super::TypeFlags> {
    if let Some(facts) = crate::stdin_stream_ops::type_facts(declaration, arguments) {
        return Some(super::TypeFlags::from(&facts));
    }
    if let Some(facts) = crate::iterator_ops::type_facts(declarations, declaration, arguments) {
        return Some(super::TypeFlags::from(&facts));
    }
    owned_builtin_prefix(declarations, declaration, arguments)?;
    Some(super::TypeFlags {
        copy: false,
        contains_resource: false,
        sized: true,
        needs_drop: true,
    })
}

fn owned_builtin_prefix<'t>(
    declarations: &DeclarationIndex,
    declaration: &DeclarationId,
    arguments: &'t [ResolvedType],
) -> Option<(&'static str, &'t ResolvedType)> {
    let [element] = arguments else {
        return None;
    };
    let prefix = match declaration.as_str() {
        crate::prelude::VEC_ID
            if crate::vec_ops::resolved_vec_element_is_admitted(element)
                || crate::hir::copy_record_collection::admitted(declarations, element)
                || crate::hir::owned_record_collection::
                    is_admitted_owned_record_collection_element(declarations, element) =>
        {
            "vec"
        }
        crate::prelude::VEC_ID if super::super::declared_collection::text_element(declarations,element) => "declared-vector-only",
        crate::prelude::VEC_ID if crate::hir::owned_leaf_collection::layout(declarations,element).is_some() => "owned-leaf-vector-v1",
        crate::prelude::BOX_ID if crate::box_ops::resolved_box_element_is_admitted(element) => {
            "box"
        }
        _ => return None,
    };
    Some((prefix, element))
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

#[cfg(test)]
impl DeclarationIndex {
    pub(crate) fn remove_type_parameters_for_test(&mut self, owner: &DeclarationId) -> bool {
        self.type_parameters.remove(owner).is_some()
    }
}
