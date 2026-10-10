//! Direct bounded collection outcomes; no recursive or synthesized authority.
use super::*;

pub(crate) mod nested;
mod owned;
pub(crate) use owned::admitted as owned_admitted;

pub(crate) fn runtime_admitted(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    admitted(index, ty) || owned_admitted(index, ty) || nested::admitted(index, ty)
}

/// Independently replay the finite collection match join on both HIR paths.
pub(crate) fn match_result(
    index: &DeclarationIndex,
    expression: &ResolvedExpr,
    arm: &ResolvedExpr,
) -> bool {
    let ResolvedExprKind::Match {
        mode, scrutinee, ..
    } = &expression.kind
    else {
        return false;
    };
    *mode != ResolvedMatchMode::Value
        && runtime_admitted(index, &scrutinee.ty)
        && ((arm.ownership == OwnershipMode::Value
            && (crate::vec_ops::resolved_element_is_admitted(&arm.ty)
                || copy_record_collection::admitted(index, &arm.ty)))
            || (*mode == ResolvedMatchMode::Own
                && arm.ownership == OwnershipMode::Own
                && runtime_admitted(index, &arm.ty)))
}

pub(crate) fn match_arm_execution(
    program: &ResolvedProgram,
    execution: &FunctionExecutionId,
    expression: &ResolvedExpr,
    arm: &ResolvedExpr,
) -> bool {
    let ResolvedExprKind::Match { mode, .. } = &expression.kind else {
        return false;
    };
    *mode == ResolvedMatchMode::Value
        || match_result(&program.declarations, expression, arm)
        || super::generic_variant::match_result_execution(
            program,
            execution,
            *mode,
            &arm.ty,
            arm.ownership,
        )
        || (matches!(arm.ty, ResolvedType::I64 | ResolvedType::Bool)
            && arm.ownership == OwnershipMode::Value)
}

pub(crate) fn match_join(index: &DeclarationIndex, expression: &ResolvedExpr) -> bool {
    let ResolvedExprKind::Match { arms, .. } = &expression.kind else {
        return false;
    };
    !arms.is_empty()
        && arms.iter().all(|arm| {
            arm.value.ty == expression.ty
                && arm.value.ownership == expression.ownership
                && match_result(index, expression, &arm.value)
        })
}

pub(crate) fn field_admitted(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    copy_record_collection::is_vec(index, ty) || owned_leaf_collection::is_vec(index, ty)
}

pub(crate) fn admitted(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if !arguments.is_empty()
        || index.declaration(declaration).is_none_or(|d| {
            d.kind != DeclarationKind::Variant || d.identity_origin != IdentityOrigin::Explicit
        })
        || index
            .type_parameters(declaration)
            .is_none_or(|p| !p.is_empty())
    {
        return false;
    }
    let Some(cases) = index.variant_cases(declaration) else {
        return false;
    };
    if cases.len() != 2 {
        return false;
    }
    let mut owners = 0;
    for case in cases {
        if !(1..=8).contains(&case.fields.len()) {
            return false;
        }
        for field in &case.fields {
            if copy_record_collection::is_vec(index, &field.ty) {
                owners += 1;
            } else if !crate::vec_ops::resolved_element_is_admitted(&field.ty) {
                return false;
            }
        }
    }
    (1..=2).contains(&owners)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = r#"module collection.profile;
@id("r") record R { @id("r.x") x:i64, }
@id("o") variant O {
 @id("o.ok") Decoded { @id("o.a") a:Vec<R>, @id("o.b") b:Vec<R>, },
 @id("o.error") Error { @id("o.code") code:i64, @id("o.offset") offset:usize, @id("o.field") field:i64, },
}
@id("forward") fn forward(value:own O)->O {value}
@id("app.main") fn main()->i64 {
 let outcome=forward(O::Error{code:4,offset:2usize,field:3});
 match own outcome { O::Decoded{a,b}=>0, O::Error{code,offset,field}=>code, }
}
"#;
    fn program() -> ResolvedProgram {
        resolve(&crate::check(SOURCE, "collection-profile.spx").unwrap()).unwrap()
    }
    fn outcome() -> ResolvedType {
        ResolvedType::Nominal {
            declaration: DeclarationId::new("o"),
            arguments: Vec::new(),
        }
    }
    #[test]
    fn collection_outcome_source_hir_and_v29_profile_replay_independent_facts() {
        let checked = program();
        assert!(admitted(&checked.declarations, &outcome()));
        validate(&checked).unwrap();
        // Even an error-only executable closure must emit its owned field types.
        crate::codegen::emit_hir_c(&checked).unwrap();
        let bytes = crate::wasm::emit_resolved_module(&checked).unwrap();
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        super::super::validate_stream_record_program(&checked, None).unwrap();
        assert!(super::super::validate_stream_data_program(&checked, None).is_err());
        for origin in [IdentityOrigin::Automatic, IdentityOrigin::CompilerOwned] {
            let mut forged = checked.clone();
            forged
                .declarations
                .declarations
                .get_mut(&DeclarationId::new("o"))
                .unwrap()
                .identity_origin = origin;
            assert!(!admitted(&forged.declarations, &outcome()));
            assert!(super::super::validate_stream_record_program(&forged, None).is_err());
        }
        let mut stale = checked.clone();
        stale
            .declarations
            .record_fields
            .get_mut(&DeclarationId::new("r"))
            .unwrap()[0]
            .ty = ResolvedType::String;
        assert!(!admitted(&stale.declarations, &outcome()));
        assert!(validate(&stale).is_err());
        let mut mismatched = checked.clone();
        let ResolvedTypeDeclarationKind::Variant { cases } = &mut mismatched
            .types
            .iter_mut()
            .find(|d| d.id.as_str() == "o")
            .unwrap()
            .kind
        else {
            panic!("variant")
        };
        cases[0].fields[0].ty = ResolvedType::Bytes;
        assert!(validate(&mismatched).is_err());
        let mut forged = checked.clone();
        forged
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "forward")
            .unwrap()
            .params[0]
            .ownership = OwnershipMode::Value;
        assert!(super::super::validate_stream_record_program(&forged, None).is_err());
        assert!(crate::codegen::emit_hir_c(&forged).is_err());
        assert!(crate::wasm::emit_resolved_module(&forged).is_err());
    }
    #[test]
    fn collection_outcome_declarations_refuse_unbounded_or_noncopy_owners() {
        for source in [
            SOURCE.replace("a:Vec<R>", "a:Vec<i64>"),
            SOURCE.replace("a:Vec<R>", "a:Vec<Bytes>"),
            SOURCE.replace("a:Vec<R>", "a:O"),
            SOURCE.replace("b:Vec<R>,", "b:Vec<R>, @id(\"o.c\") c:Vec<R>,"),
        ] {
            let errors = crate::check(&source, "refused-collection.spx").unwrap_err();
            assert!(errors.iter().any(|d| d.code == "SPX-T215"), "{errors:?}");
        }
    }
}
