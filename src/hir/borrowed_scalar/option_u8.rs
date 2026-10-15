//! Exact compiler-owned Copy `Option<u8>`; no authored or general nominal layout.
use super::super::*;
pub(crate) fn is_type(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == crate::prelude::OPTION_ID
            && arguments.as_slice() == [ResolvedType::U8])
}
pub(crate) fn pattern(pattern: &ResolvedMatchPattern) -> bool {
    let ResolvedMatchPattern::Variant {
        variant,
        case,
        fields,
    } = pattern
    else {
        return false;
    };
    if variant.as_str() != crate::prelude::OPTION_ID {
        return false;
    }
    match case.as_str() {
        crate::prelude::OPTION_NONE_ID => fields.is_empty(),
        crate::prelude::OPTION_SOME_ID => matches!(fields.as_slice(), [field]
            if field.field.as_str() == crate::prelude::OPTION_SOME_VALUE_ID
            && field.binding.ty == ResolvedType::U8
            && field.binding.ownership == OwnershipMode::Value),
        _ => false,
    }
}
pub(crate) fn authenticated(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    match authenticated_metered(index, ty, &mut |_| Ok::<_, std::convert::Infallible>(())) {
        Ok(found) => found,
        Err(never) => match never {},
    }
}
pub(crate) use metered::authenticated as authenticated_metered;
#[cfg(test)]
pub(crate) use metered::pattern as pattern_metered;
pub(crate) use metered::type_shape as is_type_metered;
mod metered;

// Receiving schema/inventory lookup work is not yet closed over the selected
// session. Keep production admission closed until that owner is implemented;
// the exact predicates and reconstruction can be exercised independently.
pub(crate) fn selected_profile_ready() -> bool {
    false
}
#[cfg(test)]
pub(crate) fn forge_parameter_index_for_test(index: &mut DeclarationIndex, value: u32) {
    index
        .type_parameters
        .get_mut(&DeclarationId::new(crate::prelude::OPTION_ID))
        .expect("canonical Option parameter fixture")[0]
        .index = value;
}
#[cfg(test)]
mod tests;
