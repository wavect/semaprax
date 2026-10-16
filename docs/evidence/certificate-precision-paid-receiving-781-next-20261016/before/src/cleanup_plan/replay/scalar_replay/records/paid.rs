//! Complete scalar-record shape receiver with upfront closed error data.
use crate::cleanup::ReplayContext;
use crate::cleanup_plan::renewal_profile::{child_refusal::Refusal, scalar_refusal::ScalarRefusal};
use crate::cleanup_plan::replay::selected_context::child::Permit;
use crate::cleanup_plan::replay::{selected_profile::paid_children, ReplayBudget};
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType};

// Lexical copies of the original pure type-key arithmetic. The original
// allocating-error helpers remain untouched. No new helper Result is returned.
macro_rules! type_key {
    ($permit:ident,$ty:expr)=>{{
        match $ty { ResolvedType::Nominal {declaration,arguments}=>{
        let nested=match arguments.as_slice() {
            []=>0,
            [element]=>match element {
                ResolvedType::Nominal {declaration,arguments} if arguments.is_empty()=>declaration.as_str().len(),
                ResolvedType::Nominal {..}|ResolvedType::Function {..}|ResolvedType::TypeParameter {..}=>return Err($permit.refuse(Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedCollectionElementIsNotABoundedLeaf))),
                _=>1,
            },
            _=>return Err($permit.refuse(Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedTypeHasUnsupportedArgumentCount))),
        };
        match declaration.as_str().len().checked_add(nested) {
            Some(value)=>value,
            None=>return Err($permit.refuse(Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::SelectedTypeKeyLengthOverflow))),
        }
        },_=>1}
    }};
}
macro_rules! required {
    ($permit:ident,$value:expr,$reason:ident) => {
        match $value {
            Some(value) => value,
            None => return Err($permit.refuse(Refusal::Scalar(ScalarRefusal::$reason))),
        }
    };
}

type Error = crate::cleanup_plan::replay::expression_facts::errors::EngineError;
pub(in crate::cleanup_plan::replay) fn signature(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    let _permit = Permit::profile::<bool>(function, budget)?;
    if !crate::hir::is_scalar_resolved_type(&function.return_type) {
        return Ok(false);
    }
    for parameter in &function.params {
        if crate::hir::borrowed_scalar::parameter(&parameter.ty, parameter.ownership) {
            continue;
        }
        if parameter.ownership != OwnershipMode::Borrow
            || !matches!(&parameter.ty, ResolvedType::Nominal { arguments, .. } if arguments.is_empty())
        {
            return Ok(false);
        }
        fields(program, function, &parameter.ty, budget)?;
    }
    Ok(true)
}

fn fields<'a>(
    program: &'a dyn ReplayContext,
    function: &ResolvedFunction,
    ty: &'a ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<&'a [crate::hir::ResolvedFieldDeclaration], Error> {
    let permit = Permit::profile::<&'a [crate::hir::ResolvedFieldDeclaration]>(function, budget)?;
    paid_children::prepay(program, function, type_key!(permit, ty), budget)?;
    match crate::cleanup::selected_inline::selected_flat_record_fields_inline(program, ty) {
        Ok(Some((_, fields))) => Ok(fields),
        Ok(None) => Err(permit.refuse(Refusal::Scalar(ScalarRefusal::Layout))),
        Err(reason) => Err(permit.refuse(reason)),
    }
}

fn root(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    let permit = Permit::profile::<bool>(function, budget)?;
    let ResolvedExprKind::Place(place) = &expression.kind else {
        return Ok(false);
    };
    if expression.ownership != OwnershipMode::Borrow
        || !place.projections.is_empty()
        || !matches!(&expression.ty, ResolvedType::Nominal { arguments, .. } if arguments.is_empty())
    {
        return Ok(false);
    }
    fields(program, function, &expression.ty, budget)?;
    let mut found = false;
    for parameter in &function.params {
        budget.charge(
            function,
            required!(
                permit,
                parameter
                    .id
                    .as_str()
                    .len()
                    .checked_add(place.root.as_str().len())
                    .and_then(|n| n.checked_add(1)),
                RootWork
            ),
            "scalar borrowed record parameter identity",
        )?;
        if parameter.id == place.root {
            budget.charge(
                function,
                required!(
                    permit,
                    type_key!(permit, &parameter.ty)
                        .checked_add(type_key!(permit, &expression.ty))
                        .and_then(|n| n.checked_add(1)),
                    TypeWork
                ),
                "scalar borrowed record parameter type identity",
            )?;
            if found
                || parameter.ownership != OwnershipMode::Borrow
                || parameter.ty != expression.ty
            {
                return Ok(false);
            }
            found = true;
        }
    }
    Ok(found)
}

fn projected_place(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    place: &crate::hir::Place,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    let permit = Permit::profile::<bool>(function, budget)?;
    budget.charge(function, 1, "scalar record projection shape")?;
    let [crate::hir::PlaceProjection::Field(field)] = place.projections.as_slice() else {
        return Ok(false);
    };
    if expression.ownership != OwnershipMode::Value
        || !crate::hir::is_scalar_resolved_type(&expression.ty)
    {
        return Ok(false);
    }
    let mut root = None;
    for parameter in &function.params {
        budget.charge(
            function,
            required!(
                permit,
                parameter
                    .id
                    .as_str()
                    .len()
                    .checked_add(place.root.as_str().len())
                    .and_then(|n| n.checked_add(1)),
                RootWork
            ),
            "scalar projected record parameter identity",
        )?;
        if parameter.id == place.root && root.replace(parameter).is_some() {
            return Ok(false);
        }
    }
    let Some(root) = root else {
        return Ok(false);
    };
    budget.charge(function, 1, "scalar projected record parameter mode")?;
    if root.ownership != OwnershipMode::Borrow
        || !matches!(&root.ty, ResolvedType::Nominal { arguments, .. } if arguments.is_empty())
    {
        return Ok(false);
    }
    let declared = fields(program, function, &root.ty, budget)?;
    let mut found = None;
    for current in declared {
        budget.charge(
            function,
            required!(
                permit,
                current
                    .id
                    .as_str()
                    .len()
                    .checked_add(field.as_str().len())
                    .and_then(|n| n.checked_add(1)),
                FieldWork
            ),
            "scalar projected record field identity",
        )?;
        if current.id == *field && found.replace(current).is_some() {
            return Ok(false);
        }
    }
    budget.charge(function, 1, "scalar projected record field type")?;
    Ok(found.is_some_and(|f| crate::hir::is_scalar_resolved_type(&f.ty) && f.ty == expression.ty))
}

pub(in crate::cleanup_plan::replay) fn expression(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Option<bool>, Error> {
    let permit = Permit::profile::<Option<bool>>(function, budget)?;
    match &expression.kind {
        ResolvedExprKind::Place(place) if !place.projections.is_empty() => Ok(Some(
            projected_place(program, function, expression, place, budget)?,
        )),
        ResolvedExprKind::Place(_) if matches!(expression.ty, ResolvedType::Nominal { .. }) => {
            Ok(Some(root(program, function, expression, budget)?))
        }
        ResolvedExprKind::Project { base, field }
            if matches!(base.ty, ResolvedType::Nominal { .. }) =>
        {
            if !root(program, function, base, budget)?
                || expression.ownership != OwnershipMode::Value
                || !crate::hir::is_scalar_resolved_type(&expression.ty)
            {
                return Ok(Some(false));
            }
            let declared = fields(program, function, &base.ty, budget)?;
            budget.charge(
                function,
                required!(
                    permit,
                    field
                        .as_str()
                        .len()
                        .checked_add(1)
                        .and_then(|n| n.checked_mul(declared.len())),
                    FieldWork
                ),
                "scalar record field identity",
            )?;
            let mut found = None;
            for current in declared {
                if current.id == *field && found.replace(current).is_some() {
                    return Ok(Some(false));
                }
            }
            Ok(Some(found.is_some_and(|current| {
                crate::hir::is_scalar_resolved_type(&current.ty) && current.ty == expression.ty
            })))
        }
        _ => Ok(None),
    }
}
