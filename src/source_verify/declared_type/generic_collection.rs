//! Source-side bounded collection signatures; HIR authenticates nominal identity.
use super::*;

fn parameter(function: &Function, index: usize) -> Option<&str> {
    function
        .type_parameters
        .get(index)
        .map(|parameter| parameter.name.as_str())
}

fn direct_parameter(ty: &Type, parameter: &str) -> bool {
    matches!(ty, Type::Named { name, arguments } if name == parameter && arguments.is_empty())
}

fn carrier(ty: &Type, carrier: &str, parameter: &str) -> bool {
    matches!(ty, Type::Named { name, arguments }
        if name == carrier && matches!(arguments.as_slice(), [element] if direct_parameter(element, parameter)))
}

pub(in crate::source_verify) fn slot(function: &Function, ty: &Type) -> bool {
    if !(1..=2).contains(&function.type_parameters.len()) {
        return false;
    }
    ["Box", "Vec", "Iter", "IterStep"]
        .into_iter()
        .any(|carrier_name| {
            (0..function.type_parameters.len()).any(|index| {
                parameter(function, index)
                    .is_some_and(|parameter| carrier(ty, carrier_name, parameter))
            })
        })
}

pub(in crate::source_verify) fn callback(function: &Function, ty: &Type) -> bool {
    if !(1..=2).contains(&function.type_parameters.len()) {
        return false;
    }
    let leaf = |ty: &Type| {
        function_value_scalar_type(ty)
            || (0..function.type_parameters.len()).any(|index| {
                parameter(function, index).is_some_and(|parameter| direct_parameter(ty, parameter))
            })
    };
    matches!(ty, Type::Function { parameters, result }
        if parameters.len() <= 8 && parameters.iter().all(leaf) && leaf(result))
}

fn bounded_profile(function: &Function) -> bool {
    if !(1..=2).contains(&function.type_parameters.len()) {
        return false;
    }
    let copy = |ty: &Type| {
        crate::vec_ops::ast_element_is_admitted(ty)
            || (0..function.type_parameters.len()).any(|index| {
                parameter(function, index).is_some_and(|parameter| direct_parameter(ty, parameter))
            })
    };
    let mut uses = slot(function, &function.return_type)
        || function
            .params
            .iter()
            .any(|parameter| slot(function, &parameter.ty));
    function.body.visit_calls(&mut |name, _| {
        uses |= crate::box_ops::by_name(name).is_some()
            || crate::vec_ops::by_name(name).is_some()
            || crate::iterator_ops::by_name(name).is_some();
    });
    uses && (slot(function, &function.return_type) || copy(&function.return_type))
        && function.params.iter().all(|parameter| {
            (slot(function, &parameter.ty) && parameter.mode == ParamMode::Own)
                || ((copy(&parameter.ty) || callback(function, &parameter.ty))
                    && parameter.mode == ParamMode::Value)
        })
}

pub(crate) fn profile(function: &Function) -> bool {
    bounded_profile(function)
}

pub(in crate::source_verify) fn arguments(function: &Function, arguments: &[Type]) -> bool {
    profile(function)
        && arguments.len() == function.type_parameters.len()
        && arguments
            .iter()
            .all(crate::vec_ops::ast_element_is_admitted)
}
