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

pub(super) fn schema(program: &ResolvedProgram) -> &'static str {
    if uses_vec(program) {
        prelude::SCHEMA_V2
    } else {
        prelude::SCHEMA_V1
    }
}

pub(super) fn digest(program: &ResolvedProgram) -> String {
    if uses_vec(program) {
        prelude::digest_text_v2()
    } else {
        prelude::digest_text_v1()
    }
}
