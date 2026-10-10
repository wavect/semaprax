//! Exact compiler-owned Vec signature rewriting, with independently checked element imports.
use super::cost::StructuralCost;
use super::{budgeted_format, graph_error, resolve_type_id};
use crate::ast::{ModuleUseKind, Program, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

pub(super) fn runtime_cost(
    ty: &Type,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(element) = vector_element(ty, target_module, caller, programs) {
        return runtime_cost(element, target_module, caller, programs, cost);
    }
    if crate::stdin_stream_ops::ast_is_reader(ty)
        || crate::map_ops::ast_collection(ty)
        || crate::vec_ops::ast_copy_vec(ty)
    {
        return Ok(());
    }
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    if !arguments.is_empty() {
        return Err(vec![graph_error(
            "SPX-G172",
            "generic cross-file types are not admitted",
        )]);
    }
    let target_id = resolve_type_id(target_module, name, programs).ok_or_else(|| {
        vec![graph_error(
            "SPX-G173",
            "cross-file type identity cost lookup disagrees",
        )]
    })?;
    let alias = caller
        .module_uses
        .iter()
        .find(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id)
        .map(|item| item.alias.as_str())
        .ok_or_else(|| {
            vec![graph_error(
                "SPX-G172",
                budgeted_format(format_args!(
                    "cross-file signature type `{target_id}` is not explicitly imported"
                )),
            )]
        })?;
    cost.string(alias)
}

pub(super) fn rewrite(
    ty: &mut Type,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    if vector_element(ty, target_module, caller, programs).is_some() {
        let Type::Named { arguments, .. } = ty else {
            unreachable!()
        };
        return rewrite(&mut arguments[0], target_module, caller, programs);
    }
    if crate::stdin_stream_ops::ast_is_reader(ty)
        || crate::map_ops::ast_collection(ty)
        || crate::vec_ops::ast_copy_vec(ty)
    {
        return Ok(());
    }
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    if !arguments.is_empty() {
        return Err(vec![graph_error(
            "SPX-G172",
            "generic cross-file types are not admitted",
        )]);
    }
    let target_id = resolve_type_id(target_module, name, programs).ok_or_else(|| {
        vec![graph_error(
            "SPX-G173",
            "cross-file type identity lookup disagrees",
        )]
    })?;
    let alias = caller
        .module_uses
        .iter()
        .find(|item| item.kind == ModuleUseKind::Type && item.persistent_id == target_id)
        .map(|item| item.alias.as_str())
        .ok_or_else(|| {
            vec![graph_error(
                "SPX-G172",
                budgeted_format(format_args!(
                    "cross-file signature type `{target_id}` is not explicitly imported"
                )),
            )]
        })?;
    *name = crate::bounded_output::budgeted_clone(alias);
    Ok(())
}

fn vector_element<'a>(
    ty: &'a Type,
    module: &str,
    caller: &Program,
    programs: &[Program],
) -> Option<&'a Type> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    let [element] = arguments.as_slice() else {
        return None;
    };
    let provider = programs.iter().find(|program| program.module == module)?;
    let identity = resolve_type_id(module, name, programs).or_else(|| {
        crate::prelude::declarations_for_program(provider)
            .iter()
            .find(|declaration| declaration.name == *name)
            .map(|declaration| declaration.stable_id.clone())
    });
    if identity.as_deref() != Some(crate::prelude::VEC_ID) {
        return None;
    }
    if crate::vec_ops::ast_vec_element_is_admitted(element) || *element == Type::String {
        return Some(element);
    }
    let Type::Named { name, arguments } = element else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let id = resolve_type_id(module, name, programs)?;
    let declaration = programs
        .iter()
        .flat_map(|program| &program.types)
        .find(|declaration| declaration.stable_id == id)?;
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    let scalar = |ty: &Type| {
        matches!(
            ty,
            Type::I64
                | Type::I32
                | Type::U8
                | Type::Usize
                | Type::Char
                | Type::F32
                | Type::F64
                | Type::Bool
        )
    };
    (declaration.explicit_id
        && declaration.type_parameters.is_empty()
        && declaration.invariants().is_empty()
        && (1..=8).contains(&fields.len())
        && fields
            .iter()
            .filter(|field| matches!(field.ty, Type::String | Type::Bytes))
            .count()
            <= 2
        && fields.iter().all(|field| {
            field.explicit_id
                && (scalar(&field.ty) || matches!(field.ty, Type::String | Type::Bytes))
        })
        && caller
            .module_uses
            .iter()
            .any(|item| item.kind == ModuleUseKind::Type && item.persistent_id == id))
    .then_some(element)
}

#[cfg(test)]
mod tests {
    use super::{rewrite, runtime_cost, StructuralCost};
    use crate::ast::{Program, Type};
    const PROVIDER: &str = r#"module vector.provider;
@id("row") record Row { @id("row.id") id:i64, @id("row.text") text:string, }
@id("inspect") fn inspect(values:borrow Vec<Row>)->usize{0usize}
"#;
    const CALLER: &str = r#"module vector.caller;
use type @id("row") from vector.provider as ImportedRow;
@id("main") fn main()->i64{0}
"#;
    fn programs(provider: &str, caller: &str) -> [Program; 2] {
        [
            crate::parse(provider, "provider.spx").unwrap(),
            crate::parse(caller, "caller.spx").unwrap(),
        ]
    }
    #[test]
    fn imported_record_vector_rewrites_only_the_authenticated_element_alias() {
        let programs = programs(PROVIDER, CALLER);
        let mut ty = programs[0].functions[0].params[0].ty.clone();
        runtime_cost(
            &ty,
            "vector.provider",
            &programs[1],
            &programs,
            &mut StructuralCost::new(),
        )
        .unwrap();
        rewrite(&mut ty, "vector.provider", &programs[1], &programs).unwrap();
        assert_eq!(
            ty,
            Type::Named {
                name: "Vec".to_owned(),
                arguments: vec![Type::Named {
                    name: "ImportedRow".to_owned(),
                    arguments: Vec::new()
                }]
            }
        );
    }
    #[test]
    fn vector_signature_rewrite_refuses_missing_nested_and_shadowed_type_authority() {
        for (provider, caller) in [
            (
                PROVIDER.to_owned(),
                CALLER.replace(
                    "use type @id(\"row\") from vector.provider as ImportedRow;",
                    "",
                ),
            ),
            (
                PROVIDER.replace("text:string", "text:Vec<string>"),
                CALLER.to_owned(),
            ),
            (
                format!(
                    "{PROVIDER}\n@id(\"fake.vec\") record Vec {{ @id(\"fake.field\") field:i64, }}"
                ),
                CALLER.to_owned(),
            ),
        ] {
            let programs = programs(&provider, &caller);
            let mut ty = programs[0].functions[0].params[0].ty.clone();
            assert!(runtime_cost(
                &ty,
                "vector.provider",
                &programs[1],
                &programs,
                &mut StructuralCost::new()
            )
            .is_err());
            assert!(rewrite(&mut ty, "vector.provider", &programs[1], &programs).is_err());
        }
    }
}
