//! Private generic functions over exact compiler-owned Copy-element carriers.
use super::{DeclarationId, OwnershipMode, ResolvedFunctionTemplate, ResolvedType};
pub(crate) fn scalar(ty: &ResolvedType) -> bool {
    super::type_reachability::nested_record_copy_scalar_is_admitted(ty)
}
pub(crate) fn slot(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool {
    (1..=2).contains(&count)
        && matches!(ty, ResolvedType::Nominal { declaration, arguments }
 if matches!(declaration.as_str(), crate::prelude::BOX_ID | crate::prelude::VEC_ID | crate::iterator_ops::ITER_ID | crate::iterator_ops::STEP_ID)
 && matches!(arguments.as_slice(), [element] if parameter(element, owner, count)))
}
pub(crate) fn parameter(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool {
    (1..=2).contains(&count)
        && matches!(ty, ResolvedType::TypeParameter { owner: parameter_owner, index } if parameter_owner == owner && usize::try_from(*index).is_ok_and(|index| index < count))
}
pub(crate) fn callback(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool {
    let leaf = |ty: &ResolvedType| scalar(ty) || parameter(ty, owner, count);
    matches!(ty, ResolvedType::Function { parameters, result } if parameters.len() <= 8 && parameters.iter().all(leaf) && leaf(result))
}
pub(crate) fn profile(template: &ResolvedFunctionTemplate) -> bool {
    let slot = |ty: &ResolvedType| slot(ty, &template.id, template.type_parameters.len());
    let copy = |ty: &ResolvedType| {
        scalar(ty) || parameter(ty, &template.id, template.type_parameters.len())
    };
    let mut uses = slot(&template.return_type) || template.params.iter().any(|p| slot(&p.ty));
    super::visit_resolved_calls(&template.body, &mut |id, _, _| {
        uses |= crate::box_ops::by_id(id.as_str()).is_some()
            || crate::vec_ops::by_id(id.as_str()).is_some()
            || crate::iterator_ops::by_id(id.as_str()).is_some();
    });
    (1..=2).contains(&template.type_parameters.len())
        && uses
        && (slot(&template.return_type) || copy(&template.return_type))
        && template.params.iter().all(|p| {
            (slot(&p.ty) && p.ownership == OwnershipMode::Own)
                || ((copy(&p.ty) || callback(&p.ty, &template.id, template.type_parameters.len()))
                    && p.ownership == OwnershipMode::Value)
        })
}
pub(crate) fn arguments(arguments: &[ResolvedType]) -> bool {
    matches!(arguments, [ty] if scalar(ty))
}
pub(crate) fn concrete_signature(function: &super::ResolvedFunction) -> bool {
    let carrier = |ty: &ResolvedType| matches!(ty, ResolvedType::Nominal { declaration, arguments } if matches!(declaration.as_str(), crate::prelude::BOX_ID | crate::prelude::VEC_ID | crate::iterator_ops::ITER_ID | crate::iterator_ops::STEP_ID) && (self::arguments(arguments) || (matches!(declaration.as_str(), crate::iterator_ops::ITER_ID | crate::iterator_ops::STEP_ID) && arguments.as_slice() == [ResolvedType::Bytes])));
    (carrier(&function.return_type) || function.params.iter().any(|p| carrier(&p.ty)))
        && (carrier(&function.return_type) || scalar(&function.return_type))
        && function.params.iter().all(|p| {
            (carrier(&p.ty) && p.ownership == OwnershipMode::Own)
                || ((scalar(&p.ty) || super::function_value::is_signature(&p.ty))
                    && p.ownership == OwnershipMode::Value)
        })
}

pub(crate) fn arguments_for_count(arguments: &[ResolvedType], count: usize) -> bool {
    (1..=2).contains(&count) && arguments.len() == count && arguments.iter().all(scalar)
}
pub(crate) fn substitutions(count: usize) -> Vec<Vec<ResolvedType>> {
    if (1..=2).contains(&count) {
        super::monomorphize::resolved_owned_record_substitutions(count)
    } else {
        Vec::new()
    }
}
pub(super) fn source_parameter(
    program: &crate::ast::Program,
    execution: &super::FunctionExecutionId,
    ty: &ResolvedType,
) -> bool {
    let Some(owner) = execution.monomorphic_declaration() else {
        return false;
    };
    program
        .functions
        .iter()
        .find(|function| function.stable_id == owner.as_str())
        .is_some_and(|function| {
            crate::source_verify::generic_collection_profile(function)
                && parameter(ty, owner, function.type_parameters.len())
        })
}
pub(super) fn source_count(program: &crate::ast::Program, owner: &DeclarationId) -> usize {
    program
        .functions
        .iter()
        .find(|function| function.stable_id == owner.as_str())
        .map_or(0, |function| function.type_parameters.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_iterator_operation_scope_and_cartesian_arguments_are_exact() {
        let owner = DeclarationId::new("test.map");
        let first = ResolvedType::TypeParameter {
            owner: owner.clone(),
            index: 0,
        };
        let second = ResolvedType::TypeParameter {
            owner: owner.clone(),
            index: 1,
        };
        let foreign = ResolvedType::TypeParameter {
            owner: DeclarationId::new("test.foreign"),
            index: 1,
        };
        assert!(parameter(&first, &owner, 1));
        assert!(!parameter(&second, &owner, 1));
        assert!(parameter(&second, &owner, 2));
        assert!(!parameter(&foreign, &owner, 2));
        assert!(!parameter(&first, &owner, 3));
        let callback_type = ResolvedType::Function {
            parameters: vec![second.clone(), first.clone()],
            result: Box::new(second.clone()),
        };
        assert!(callback(&callback_type, &owner, 2));
        assert!(!callback(&callback_type, &owner, 1));
        assert!(slot(
            &crate::iterator_ops::resolved_iter_step(second.clone()),
            &owner,
            2
        ));
        assert!(!slot(
            &crate::iterator_ops::resolved_iter_step(second),
            &owner,
            1
        ));
        let pairs = substitutions(2);
        assert_eq!(pairs.len(), 64);
        assert_eq!(substitutions(1).len(), 8);
        assert!(pairs.iter().all(|pair| arguments_for_count(pair, 2)));
        assert!(pairs
            .iter()
            .any(|pair| pair == &[ResolvedType::F64, ResolvedType::Bool]));
        assert!(pairs
            .iter()
            .any(|pair| pair == &[ResolvedType::Bool, ResolvedType::F64]));
        assert!(!arguments_for_count(&[ResolvedType::I64], 2));
        assert!(!arguments_for_count(
            &[ResolvedType::I64, ResolvedType::Bytes],
            2
        ));
        assert!(!arguments(&[ResolvedType::I64, ResolvedType::Bool]));
    }
}
