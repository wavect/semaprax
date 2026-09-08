//! Source-side bounded collection signatures; HIR authenticates nominal identity.
use super::*;
pub(in crate::source_verify) fn slot(function: &Function, ty: &Type) -> bool {
    let [parameter] = function.type_parameters.as_slice() else {
        return false;
    };
    matches!(ty, Type::Named { name, arguments } if matches!(name.as_str(), "Box" | "Vec") && matches!(arguments.as_slice(), [Type::Named { name, arguments }] if name == &parameter.name && arguments.is_empty()))
}
pub(crate) fn profile(function: &Function) -> bool {
    let [parameter] = function.type_parameters.as_slice() else {
        return false;
    };
    let copy = |ty: &Type| {
        crate::vec_ops::ast_element_is_admitted(ty)
            || matches!(ty, Type::Named { name, arguments } if name == &parameter.name && arguments.is_empty())
    };
    let mut uses = slot(function, &function.return_type)
        || function.params.iter().any(|p| slot(function, &p.ty));
    function.body.visit_calls(&mut |name, _| {
        uses |= crate::box_ops::by_name(name).is_some() || crate::vec_ops::by_name(name).is_some();
    });
    uses && (slot(function, &function.return_type) || copy(&function.return_type))
        && function.params.iter().all(|p| {
            (slot(function, &p.ty) && p.mode == ParamMode::Own)
                || (copy(&p.ty) && p.mode == ParamMode::Value)
        })
}
