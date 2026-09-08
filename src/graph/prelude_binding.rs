//! Selects the exact compiler-prelude contract represented by resolved HIR.

use crate::hir::{self, ResolvedProgram, ResolvedType};
use crate::prelude;
use sha2::{Digest, Sha256};

pub(super) fn revision_from_source(source: &str) -> String {
    let (prelude_schema, prelude_contract, _) = prelude::selected_for_source(source);
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.graph-revision.v2\0");
    hasher.update((source.len() as u64).to_le_bytes());
    hasher.update(source.as_bytes());
    hasher.update((prelude_schema.len() as u64).to_le_bytes());
    hasher.update(prelude_schema.as_bytes());
    hasher.update((prelude_contract.len() as u64).to_le_bytes());
    hasher.update(&prelude_contract);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

pub(super) fn uses_vec(program: &ResolvedProgram) -> bool {
    fn type_uses_vec(ty: &ResolvedType) -> bool {
        matches!(ty, ResolvedType::Nominal { declaration, .. } if declaration.as_str() == prelude::VEC_ID)
            || matches!(ty, ResolvedType::Nominal { arguments, .. } if arguments.iter().any(type_uses_vec))
    }

    fn function_uses_vec(function: &crate::hir::ResolvedFunction) -> bool {
        type_uses_vec(&function.return_type)
            || function
                .params
                .iter()
                .any(|parameter| type_uses_vec(&parameter.ty))
            || {
                let mut found = false;
                hir::visit_resolved_calls(&function.body, &mut |callee, _, _| {
                    found |= crate::vec_ops::by_id(callee.as_str()).is_some();
                });
                found
            }
    }

    program.types.iter().any(|declaration| {
        declaration.id.as_str() != prelude::VEC_ID
            && match &declaration.kind {
                crate::hir::ResolvedTypeDeclarationKind::Record { fields }
                | crate::hir::ResolvedTypeDeclarationKind::Class { fields, .. } => {
                    fields.iter().any(|field| type_uses_vec(&field.ty))
                }
                crate::hir::ResolvedTypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .any(|field| type_uses_vec(&field.ty)),
                crate::hir::ResolvedTypeDeclarationKind::Resource { .. } => false,
            }
    }) || program.functions.iter().any(function_uses_vec)
        || program.function_templates.iter().any(|template| {
            type_uses_vec(&template.return_type)
                || template
                    .params
                    .iter()
                    .any(|parameter| type_uses_vec(&parameter.ty))
                || {
                    let mut found = false;
                    for expression in template
                        .requires
                        .iter()
                        .chain(std::iter::once(&template.body))
                        .chain(&template.ensures)
                    {
                        hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                            found |= crate::vec_ops::by_id(callee.as_str()).is_some();
                        });
                    }
                    found
                }
        })
        || program.function_instances.iter().any(|instance| {
            instance.type_arguments.iter().any(type_uses_vec)
                || function_uses_vec(&instance.function)
        })
}

pub(super) fn uses_box(program: &ResolvedProgram) -> bool {
    fn type_uses_box(ty: &ResolvedType) -> bool {
        matches!(ty, ResolvedType::Nominal { declaration, .. } if declaration.as_str() == prelude::BOX_ID)
            || matches!(ty, ResolvedType::Nominal { arguments, .. } if arguments.iter().any(type_uses_box))
    }
    fn function_uses_box(function: &crate::hir::ResolvedFunction) -> bool {
        type_uses_box(&function.return_type)
            || function.params.iter().any(|p| type_uses_box(&p.ty))
            || {
                let mut found = false;
                for expression in function
                    .requires
                    .iter()
                    .chain(std::iter::once(&function.body))
                    .chain(&function.ensures)
                {
                    hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                        found |= crate::box_ops::by_id(callee.as_str()).is_some()
                    });
                }
                found
            }
    }
    program.functions.iter().any(function_uses_box)
        || program.function_templates.iter().any(|t| {
            type_uses_box(&t.return_type) || t.params.iter().any(|p| type_uses_box(&p.ty)) || {
                let mut found = false;
                for expression in t
                    .requires
                    .iter()
                    .chain(std::iter::once(&t.body))
                    .chain(&t.ensures)
                {
                    hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                        found |= crate::box_ops::by_id(callee.as_str()).is_some()
                    });
                }
                found
            }
        })
        || program
            .function_instances
            .iter()
            .any(|i| function_uses_box(&i.function))
}

fn uses_vec_v3(program: &ResolvedProgram) -> bool {
    fn is_v3_id(id: &crate::hir::DeclarationId) -> bool {
        crate::vec_ops::by_id(id.as_str()).is_some_and(|op| {
            matches!(
                op,
                crate::vec_ops::VecOp::ReserveExact
                    | crate::vec_ops::VecOp::Set
                    | crate::vec_ops::VecOp::Clear
            )
        }) || crate::vec_ops::wrapper_by_id(id.as_str()).is_some_and(|op| {
            matches!(
                op,
                crate::vec_ops::VecOp::ReserveExact
                    | crate::vec_ops::VecOp::Set
                    | crate::vec_ops::VecOp::Clear
            )
        })
    }

    let mut found = false;
    for function in &program.functions {
        hir::visit_resolved_calls(&function.body, &mut |callee, _, _| {
            found |= is_v3_id(callee)
        });
    }
    for template in &program.function_templates {
        found |= is_v3_id(&template.id);
        hir::visit_resolved_calls(&template.body, &mut |callee, _, _| {
            found |= is_v3_id(callee)
        });
    }
    for instance in &program.function_instances {
        found |= is_v3_id(&instance.template);
        hir::visit_resolved_calls(&instance.function.body, &mut |callee, _, _| {
            found |= is_v3_id(callee);
        });
    }
    found
}

pub(super) fn schema(program: &ResolvedProgram) -> &'static str {
    if crate::box_ops::resolved_program_uses_owned_payload(program) {
        prelude::SCHEMA_V5
    } else if uses_box(program) {
        prelude::SCHEMA_V4
    } else if uses_vec_v3(program) {
        prelude::SCHEMA_V3
    } else if uses_vec(program) {
        prelude::SCHEMA_V2
    } else {
        prelude::SCHEMA_V1
    }
}

pub(super) fn digest(program: &ResolvedProgram) -> String {
    if crate::box_ops::resolved_program_uses_owned_payload(program) {
        prelude::digest_text_v5()
    } else if uses_box(program) {
        prelude::digest_text_v4()
    } else if uses_vec_v3(program) {
        prelude::digest_text_v3()
    } else if uses_vec(program) {
        prelude::digest_text_v2()
    } else {
        prelude::digest_text_v1()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn box_calls_in_contracts_select_v4_without_body_or_type_reachability() {
        let mut boxed = crate::hir::resolve(
            &crate::parse(
                "module test.box_contract; @id(\"app.main\") fn main()->i64{box_into_inner<i64>(box_new<i64>(1))}",
                Path::new("box-contract.spx"),
            )
            .unwrap(),
        )
        .unwrap();
        let scalar = crate::hir::resolve(
            &crate::parse(
                "module test.scalar_contract; @id(\"app.main\") fn main()->i64{0}",
                Path::new("scalar-contract.spx"),
            )
            .unwrap(),
        )
        .unwrap();
        let box_call = boxed.functions[0].body.clone();
        boxed.functions[0].body = scalar.functions[0].body.clone();
        boxed.functions[0].requires = vec![box_call.clone()];
        assert_eq!(schema(&boxed), prelude::SCHEMA_V4);

        boxed.functions[0].requires.clear();
        boxed.functions[0].ensures = vec![box_call];
        assert_eq!(schema(&boxed), prelude::SCHEMA_V4);
    }
}
