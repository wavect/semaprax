//! Exact consuming scalar iterator protocol. Runtime adapters receive no ambient authority.
use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{DeclarationId, OwnershipMode, ResolvedParam, ResolvedType, ValueId};
mod owned;
#[cfg(test)]
mod owned_tests;
pub(crate) use owned::*;
pub(crate) const ITER_ID: &str = "core.iter";
pub(crate) const STEP_ID: &str = "core.iter-step";
pub(crate) const DONE_ID: &str = "core.iter-step.done";
pub(crate) const YIELD_ID: &str = "core.iter-step.yield";
pub(crate) const ITEM_ID: &str = "core.iter-step.yield.item";
pub(crate) const REST_ID: &str = "core.iter-step.yield.rest";
pub(crate) const INTO_ITER_ID: &str = "core.vec.into-iter";
pub(crate) const NEXT_ID: &str = "core.iter.next";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IteratorOp {
    VecIntoIter,
    Next,
}
pub(crate) const ALL: [IteratorOp; 2] = [IteratorOp::VecIntoIter, IteratorOp::Next];
impl IteratorOp {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::VecIntoIter => "vec_into_iter",
            Self::Next => "iter_next",
        }
    }
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::VecIntoIter => INTO_ITER_ID,
            Self::Next => NEXT_ID,
        }
    }
    pub(crate) const fn param_ownership(self) -> OwnershipMode {
        OwnershipMode::Own
    }
    pub(crate) fn ast_param_type(self, element: &Type) -> Type {
        nominal(
            if self == Self::VecIntoIter {
                "Vec"
            } else {
                "Iter"
            },
            element.clone(),
        )
    }
    pub(crate) fn ast_return_type(self, element: &Type) -> Type {
        nominal(
            if self == Self::VecIntoIter {
                "Iter"
            } else {
                "IterStep"
            },
            element.clone(),
        )
    }
    pub(crate) fn resolved_param_type(self, element: &ResolvedType) -> ResolvedType {
        resolved(
            if self == Self::VecIntoIter {
                crate::prelude::VEC_ID
            } else {
                ITER_ID
            },
            element.clone(),
        )
    }
    pub(crate) fn resolved_return_type(self, element: &ResolvedType) -> ResolvedType {
        resolved(
            if self == Self::VecIntoIter {
                ITER_ID
            } else {
                STEP_ID
            },
            element.clone(),
        )
    }
}
pub(crate) fn by_name(name: &str) -> Option<IteratorOp> {
    ALL.into_iter().find(|op| op.name() == name)
}
pub(crate) fn by_id(id: &str) -> Option<IteratorOp> {
    ALL.into_iter().find(|op| op.id() == id)
}
pub(crate) fn ast_element_is_admitted(ty: &Type) -> bool {
    crate::vec_ops::ast_element_is_admitted(ty) || *ty == Type::Bytes
}
pub(crate) fn resolved_element_is_admitted(ty: &ResolvedType) -> bool {
    crate::vec_ops::resolved_element_is_admitted(ty) || *ty == ResolvedType::Bytes
}
fn nominal(name: &str, element: Type) -> Type {
    Type::Named {
        name: name.into(),
        arguments: vec![element],
    }
}
fn resolved(id: &str, element: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: vec![element],
    }
}
pub(crate) fn resolved_iter(element: ResolvedType) -> ResolvedType {
    resolved(ITER_ID, element)
}
pub(crate) fn resolved_iter_step(element: ResolvedType) -> ResolvedType {
    resolved(STEP_ID, element)
}
pub(crate) fn element(ty: &ResolvedType) -> Option<&ResolvedType> {
    match ty {
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if matches!(declaration.as_str(), ITER_ID | STEP_ID) => match arguments.as_slice() {
            [element] if resolved_element_is_admitted(element) => Some(element),
            _ => None,
        },
        _ => None,
    }
}
pub(crate) fn is_iter(ty: &ResolvedType) -> bool {
    matches!(ty,ResolvedType::Nominal{declaration,..} if declaration.as_str()==ITER_ID)
        && element(ty).is_some()
}
pub(crate) fn is_step(ty: &ResolvedType) -> bool {
    matches!(ty,ResolvedType::Nominal{declaration,..} if declaration.as_str()==STEP_ID)
        && element(ty).is_some()
}
pub(crate) fn ast_is_iterator(ty: &Type) -> bool {
    matches!(ty,Type::Named{name,arguments} if matches!(name.as_str(),"Iter"|"IterStep")&&matches!(arguments.as_slice(),[element] if ast_element_is_admitted(element)))
}
pub(crate) fn ast_params(op: IteratorOp, element: &Type) -> Vec<Param> {
    vec![Param {
        name: "arg0".into(),
        mode: ParamMode::Own,
        ty: op.ast_param_type(element),
        span: Span::default(),
    }]
}
pub(crate) fn resolved_params(op: IteratorOp, element: &ResolvedType) -> Vec<ResolvedParam> {
    vec![ResolvedParam {
        id: ValueId::intrinsic_parameter(op.id(), 0),
        name: "arg0".into(),
        ownership: op.param_ownership(),
        ty: op.resolved_param_type(element),
        span: Span::default(),
    }]
}
pub(crate) fn program_uses_iterator(program: &crate::ast::Program) -> bool {
    fn uses_function(function: &crate::ast::Function) -> bool {
        ast_type_uses_iterator(&function.return_type)
            || function
                .params
                .iter()
                .any(|parameter| ast_type_uses_iterator(&parameter.ty))
            || function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(ast_expression_uses_iterator)
    }
    program.functions.iter().any(uses_function)
        || program.types.iter().any(|declaration| {
            matches!(&declaration.kind, crate::ast::TypeDeclarationKind::Class { methods, .. }
                if methods.iter().any(uses_function))
        })
}

/// Source prelude selection scans every retained expression shape, including a
/// local `IterStep::Done` constructor that has no iterator operation call.
pub(crate) fn ast_expression_uses_iterator(expression: &crate::ast::Expr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            crate::ast::ExprKind::Closure {
                params,
                return_type,
                ..
            } => {
                if ast_type_uses_iterator(return_type)
                    || params
                        .iter()
                        .any(|parameter| ast_type_uses_iterator(&parameter.ty))
                {
                    return true;
                }
            }
            crate::ast::ExprKind::Call {
                name,
                type_arguments,
                ..
            } => {
                if by_name(name).is_some() || type_arguments.iter().any(ast_type_uses_iterator) {
                    return true;
                }
            }
            crate::ast::ExprKind::MethodCall { type_arguments, .. } => {
                if type_arguments.iter().any(ast_type_uses_iterator) {
                    return true;
                }
            }
            crate::ast::ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                ..
            }
            | crate::ast::ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                ..
            } if matches!(type_name.as_str(), "Iter" | "IterStep")
                || type_arguments.iter().any(ast_type_uses_iterator) =>
            {
                return true;
            }
            _ => {}
        }
        let mut index = 0;
        while let Some(child) = expression.child(index) {
            pending.push(child);
            index += 1;
        }
    }
    false
}

pub(crate) fn ast_type_uses_iterator(ty: &Type) -> bool {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        match ty {
            Type::Named { name, arguments } => {
                if matches!(name.as_str(), "Iter" | "IterStep") {
                    return true;
                }
                pending.extend(arguments);
            }
            Type::Function { parameters, result } => {
                pending.push(result);
                pending.extend(parameters);
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn resolved_type_uses_iterator(ty: &ResolvedType) -> bool {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        match ty {
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if matches!(declaration.as_str(), ITER_ID | STEP_ID) {
                    return true;
                }
                pending.extend(arguments);
            }
            ResolvedType::Function { parameters, result } => {
                pending.push(result);
                pending.extend(parameters);
            }
            _ => {}
        }
    }
    false
}

/// Retained HIR selection includes expression result and explicit call types,
/// as well as the authenticated constructor identities.
pub(crate) fn resolved_expression_uses_iterator(expression: &crate::hir::ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if resolved_type_uses_iterator(&expression.ty) {
            return true;
        }
        match &expression.kind {
            crate::hir::ResolvedExprKind::Call {
                callee,
                type_arguments,
                ..
            } => {
                if by_id(callee.as_str()).is_some()
                    || type_arguments.iter().any(resolved_type_uses_iterator)
                {
                    return true;
                }
            }
            crate::hir::ResolvedExprKind::ConstructRecord { record, .. }
                if record.as_str() == ITER_ID =>
            {
                return true;
            }
            crate::hir::ResolvedExprKind::ConstructVariant { variant, .. }
                if variant.as_str() == STEP_ID =>
            {
                return true;
            }
            _ => {}
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

pub(crate) fn type_facts(
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
) -> Option<crate::hir::TypeFacts> {
    if !matches!(declaration.as_str(), ITER_ID | STEP_ID)
        || !matches!(arguments,[element] if resolved_element_is_admitted(element))
    {
        return None;
    }
    Some(crate::hir::TypeFacts {
        copy: false,
        contains_resource: false,
        sized: true,
        needs_drop: true,
        layout_key: format!(
            "iterator-{}:{}:{}",
            if arguments[0] == ResolvedType::Bytes {
                "v2"
            } else {
                "v1"
            },
            declaration.as_str(),
            arguments[0].identity_key()
        ),
    })
}

/// Authenticate the closed Step declaration, independently of its supplied case tag.
pub(crate) fn step_shape(index: &crate::hir::DeclarationIndex, ty: &ResolvedType) -> bool {
    if !is_step(ty) {
        return false;
    }
    let owner = DeclarationId::new(STEP_ID);
    let Some(cases) = index.variant_cases(&owner) else {
        return false;
    };
    let parameter = ResolvedType::TypeParameter {
        owner: owner.clone(),
        index: 0,
    };
    matches!(cases,[done,yielded] if done.id.as_str()==DONE_ID&&done.name=="Done"&&done.index==0&&done.fields.is_empty()
 &&yielded.id.as_str()==YIELD_ID&&yielded.name=="Yield"&&yielded.index==1
 &&matches!(yielded.fields.as_slice(),[item,rest] if item.id.as_str()==ITEM_ID&&item.name=="item"&&item.index==0&&item.ty==parameter
 &&rest.id.as_str()==REST_ID&&rest.name=="rest"&&rest.index==1&&rest.ty==resolved_iter(parameter)))
}
pub(crate) fn validate_declarations(
    program: &crate::hir::ResolvedProgram,
) -> Result<(), crate::diagnostic::Diagnostic> {
    use crate::hir::DeclarationKind;
    for id in [ITER_ID, STEP_ID] {
        let id = DeclarationId::new(id);
        let Some(declaration) = program.declarations.declaration(&id) else {
            continue;
        };
        let valid = program
            .declarations
            .type_parameters(&id)
            .is_some_and(|p| p.len() == 1)
            && if id.as_str() == ITER_ID {
                declaration.kind == DeclarationKind::Record
                    && declaration.name == "Iter"
                    && program
                        .declarations
                        .record_fields(&id)
                        .is_some_and(|f| f.is_empty())
            } else {
                declaration.kind == DeclarationKind::Variant
                    && declaration.name == "IterStep"
                    && step_shape(
                        &program.declarations,
                        &resolved_iter_step(ResolvedType::I64),
                    )
            };
        if !valid {
            return Err(crate::diagnostic::Diagnostic::io(
                "SPX-H006",
                "iterator prelude declaration is not canonical",
            ));
        }
    }
    Ok(())
}

pub(crate) fn is_step_rest_field(
    owner: &DeclarationId,
    case: &DeclarationId,
    field: &crate::hir::ResolvedFieldDeclaration,
) -> bool {
    owner.as_str() == STEP_ID
        && case.as_str() == YIELD_ID
        && field.id.as_str() == REST_ID
        && field.index == 1
        && field.name == "rest"
        && field.ty
            == resolved_iter(ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: 0,
            })
}

#[cfg(test)]
mod tests {
    const SOURCE: &str = r#"module test.iterators;
@id("it.main") fn main()->i64{
 let values=vec_push<i64>(vec_with_capacity<i64>(1usize),7);
 let step=iter_next<i64>(vec_into_iter<i64>(values));
 match own step {IterStep::Done{}=>0,IterStep::Yield{item,rest}=>item,}
}"#;
    #[test]
    fn iterator_source_hir_owns_yield_remainder() {
        let program = crate::check(SOURCE, "iterator.spx").unwrap();
        let hir = crate::hir::resolve(&program).unwrap();
        crate::hir::validate(&hir).unwrap();
        assert!(super::step_shape(
            &hir.declarations,
            &super::resolved_iter_step(crate::hir::ResolvedType::I64)
        ));
        let mut forged = hir.clone();
        let step = forged
            .types
            .iter_mut()
            .find(|ty| ty.id.as_str() == super::STEP_ID)
            .unwrap();
        let crate::hir::ResolvedTypeDeclarationKind::Variant { cases } = &mut step.kind else {
            panic!("step")
        };
        cases.swap(0, 1);
        assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
    }
    #[test]
    fn iterator_source_rejects_implicit_copy_and_unsupported_owned_element() {
        let diagnostics = crate::check(
            &SOURCE.replace("match own step", "match step"),
            "iterator-copy.spx",
        )
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-O111"),
            "{diagnostics:?}"
        );
        let source="module test.invalid_iter; @id(\"it.main\") fn main()->i64{let values=vec_with_capacity<Bytes>(1usize);let iterator=vec_into_iter<String>(values);0}";
        let diagnostics = crate::check(source, "iterator-owned.spx").unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T290"),
            "{diagnostics:?}"
        );
    }
}
