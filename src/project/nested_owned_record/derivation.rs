use std::collections::{BTreeMap, BTreeSet};

use crate::call_index::PersistentCallIndex;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, IdentityOrigin, ResolvedProgram, ResolvedType, ResolvedTypeDeclaration,
    ResolvedTypeDeclarationKind,
};

use super::super::public_api::{
    parameter_type, rust_method_name, selected_closure, valid_sha256_fact,
    validate_closure_function, validate_selected,
};
use super::*;

pub fn derive_nested_owned_record_api_descriptor(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
) -> Result<NestedOwnedRecordApiDescriptor, Diagnostic> {
    if subject.project_schema != NESTED_OWNED_RECORD_PROJECT_SCHEMA
        || !valid_sha256_fact(subject.project_revision)
        || !valid_sha256_fact(subject.workspace_revision)
        || !valid_sha256_fact(subject.project_graph_digest)
    {
        return Err(error("nested owned-record descriptor subject is invalid"));
    }
    validate_selected(selected)?;
    if !(1..=super::super::MAX_PUBLIC_API_CLOSURE_FUNCTIONS).contains(&program.functions.len()) {
        return Err(error(
            "nested owned-record linked executable inventory must contain 1..=256 functions",
        ));
    }
    let mut functions = BTreeMap::new();
    for function in &program.functions {
        if functions.insert(function.id.clone(), function).is_some() {
            return Err(error(
                "nested owned-record linked executable identities are not unique",
            ));
        }
    }
    let index = PersistentCallIndex::build(program)?;
    for id in selected_closure(program, selected, &functions, &index)? {
        validate_closure_function(
            functions
                .get(&id)
                .ok_or_else(|| error("nested owned-record closure is incomplete"))?,
        )?;
    }
    let mut types = BTreeMap::new();
    for ty in &program.types {
        if types.insert(ty.id.clone(), ty).is_some() {
            return Err(error(
                "nested owned-record type declaration identities are not unique",
            ));
        }
    }
    let mut exports = Vec::with_capacity(selected.len());
    let mut records = BTreeMap::new();
    let mut rust_methods = BTreeSet::new();
    for selected_id in selected {
        let function = functions
            .get(&DeclarationId::new(selected_id.clone()))
            .ok_or_else(|| error("nested owned-record export is absent"))?;
        let fact = program
            .declarations
            .declaration(&function.id)
            .ok_or_else(|| error("nested owned-record export lacks declaration metadata"))?;
        if fact.identity_origin != IdentityOrigin::Explicit || function.id == program.entrypoint {
            return Err(error(
                "nested owned-record export must have an explicit non-entry stable identity",
            ));
        }
        if function.params.len() > super::super::MAX_PUBLIC_API_PARAMETERS {
            return Err(error("nested owned-record export has too many parameters"));
        }
        let parameters = function
            .params
            .iter()
            .map(|parameter| {
                let ty = parameter_type(&parameter.ty, parameter.ownership)
                    .ok_or_else(|| error("nested owned-record export parameter is unsupported"))?;
                Ok((parameter.id.as_str().to_owned(), parameter.name.clone(), ty))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &function.return_type
        else {
            return Err(error("nested owned-record export result is not a record"));
        };
        if !arguments.is_empty() {
            return Err(error("nested owned-record result must be monomorphic"));
        }
        let leaves = derive_record_tree(program, &types, declaration, &mut records)?;
        let rust_method_name = rust_method_name(selected_id)?;
        if !rust_methods.insert(rust_method_name.clone()) {
            return Err(error("nested owned-record Rust method identities collide"));
        }
        exports.push(NestedOwnedRecordExport {
            stable_id: function.id.clone(),
            typescript_name: selected_id.clone(),
            rust_method_name,
            parameters,
            result_record_id: declaration.clone(),
            leaves,
        });
    }
    let descriptor = NestedOwnedRecordApiDescriptor {
        project_revision: subject.project_revision.to_owned(),
        workspace_revision: subject.workspace_revision.to_owned(),
        project_graph_digest: subject.project_graph_digest.to_owned(),
        exports,
        records: records.into_values().collect(),
    };
    validate_descriptor_size(&descriptor)?;
    Ok(descriptor)
}

fn validate_descriptor_size(descriptor: &NestedOwnedRecordApiDescriptor) -> Result<(), Diagnostic> {
    if descriptor.canonical_bytes().len() > MAX_NESTED_RECORD_DESCRIPTOR_BYTES {
        return Err(error(
            "nested owned-record descriptor exceeds its byte limit",
        ));
    }
    Ok(())
}

fn derive_record_tree(
    program: &ResolvedProgram,
    types: &BTreeMap<DeclarationId, &ResolvedTypeDeclaration>,
    root: &DeclarationId,
    records: &mut BTreeMap<DeclarationId, NestedOwnedRecordType>,
) -> Result<Vec<NestedOwnedRecordLeaf>, Diagnostic> {
    let mut work = vec![(
        ResolvedType::Nominal {
            declaration: root.clone(),
            arguments: Vec::new(),
        },
        Vec::<DeclarationId>::new(),
        1usize,
        Vec::<DeclarationId>::new(),
    )];
    let mut leaves = Vec::new();
    let mut owned_leaves = 0usize;
    let mut visited_fields = 0usize;
    while let Some((ty, path, depth, ancestors)) = work.pop() {
        match ty {
            ResolvedType::I64 | ResolvedType::Bool | ResolvedType::Usize | ResolvedType::Bytes => {
                let leaf_ty = match ty {
                    ResolvedType::I64 => NestedOwnedRecordLeafType::I64,
                    ResolvedType::Bool => NestedOwnedRecordLeafType::Bool,
                    ResolvedType::Usize => NestedOwnedRecordLeafType::Usize,
                    ResolvedType::Bytes => {
                        owned_leaves = owned_leaves.checked_add(1).ok_or_else(|| {
                            error("nested owned-record result exceeds its owned-leaf limit")
                        })?;
                        if owned_leaves > MAX_NESTED_RECORD_OWNED_LEAVES {
                            return Err(error(
                                "nested owned-record result exceeds its owned-leaf limit",
                            ));
                        }
                        NestedOwnedRecordLeafType::OwnedBytes
                    }
                    _ => unreachable!("closed scalar leaf family"),
                };
                leaves.push(NestedOwnedRecordLeaf {
                    field_path: path,
                    ordinal: leaves.len() as u32,
                    ty: leaf_ty,
                });
            }
            ResolvedType::Nominal {
                declaration: record_id,
                arguments,
            } => {
                if !arguments.is_empty()
                    || depth > MAX_NESTED_RECORD_DEPTH
                    || ancestors.contains(&record_id)
                {
                    return Err(error("nested owned-record result is recursive, generic, or exceeds its depth limit"));
                }
                let record = types
                    .get(&record_id)
                    .ok_or_else(|| error("nested owned-record declaration is absent"))?;
                if !record.type_parameters.is_empty() {
                    return Err(error("nested owned-record result must be monomorphic"));
                }
                let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
                    return Err(error(
                        "nested owned-record result contains a non-record nominal type",
                    ));
                };
                require_explicit(program, &record.id, "record")?;
                let mut descriptor_fields = Vec::with_capacity(fields.len());
                let mut field_ids = BTreeSet::new();
                for (ordinal, field) in fields.iter().enumerate() {
                    visited_fields = visited_fields.checked_add(1).ok_or_else(|| {
                        error("nested owned-record field inventory exceeds its limit")
                    })?;
                    if visited_fields > MAX_NESTED_RECORD_VISITED_FIELDS
                        || field.index as usize != ordinal
                    {
                        return Err(error("nested owned-record field inventory is invalid"));
                    }
                    require_explicit(program, &field.id, "field")?;
                    if !field_ids.insert(field.id.clone()) {
                        return Err(error("nested owned-record field identities are not unique"));
                    }
                    let field_ty = match &field.ty {
                        ResolvedType::I64 => NestedOwnedRecordFieldType::I64,
                        ResolvedType::Bool => NestedOwnedRecordFieldType::Bool,
                        ResolvedType::Usize => NestedOwnedRecordFieldType::Usize,
                        ResolvedType::Bytes => NestedOwnedRecordFieldType::OwnedBytes,
                        ResolvedType::Nominal {
                            declaration,
                            arguments,
                        } if arguments.is_empty() => {
                            NestedOwnedRecordFieldType::Record(declaration.clone())
                        }
                        _ => return Err(error("nested owned-record field type is unsupported")),
                    };
                    descriptor_fields.push(NestedOwnedRecordField {
                        stable_id: field.id.clone(),
                        source_name: field.name.clone(),
                        host_name: stable_host_name("spx_field_id_", field.id.as_str()),
                        ordinal: field.index,
                        ty: field_ty,
                    });
                }
                let candidate = NestedOwnedRecordType {
                    stable_id: record.id.clone(),
                    source_name: record.name.clone(),
                    host_name: stable_host_name("SpxRecordId", record.id.as_str()),
                    fields: descriptor_fields,
                };
                if let Some(previous) = records.insert(record.id.clone(), candidate.clone()) {
                    if previous != candidate {
                        return Err(error(
                            "nested owned-record identity has inconsistent definitions",
                        ));
                    }
                }
                let mut next_ancestors = ancestors;
                next_ancestors.push(record_id);
                for field in fields.iter().rev() {
                    let mut field_path = path.clone();
                    field_path.push(field.id.clone());
                    work.push((
                        field.ty.clone(),
                        field_path,
                        depth + usize::from(matches!(&field.ty, ResolvedType::Nominal { .. })),
                        next_ancestors.clone(),
                    ));
                }
            }
            _ => return Err(error("nested owned-record field type is unsupported")),
        }
    }
    if owned_leaves == 0 {
        return Err(error(
            "nested owned-record result requires at least one transitive Bytes field",
        ));
    }
    Ok(leaves)
}

fn require_explicit(
    program: &ResolvedProgram,
    id: &DeclarationId,
    role: &str,
) -> Result<(), Diagnostic> {
    let fact = program.declarations.declaration(id).ok_or_else(|| {
        error(format!(
            "nested owned-record {role} lacks declaration metadata"
        ))
    })?;
    if fact.identity_origin != IdentityOrigin::Explicit {
        return Err(error(format!(
            "nested owned-record {role} requires an explicit @id"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_byte_bound_is_exact() {
        let fact = format!("sha256:{}", "1".repeat(64));
        let mut descriptor = NestedOwnedRecordApiDescriptor {
            project_revision: fact.clone(),
            workspace_revision: fact.clone(),
            project_graph_digest: fact,
            exports: Vec::new(),
            records: Vec::new(),
        };
        let overhead = descriptor.canonical_bytes().len();
        descriptor
            .project_revision
            .push_str(&"x".repeat(MAX_NESTED_RECORD_DESCRIPTOR_BYTES - overhead));
        assert_eq!(
            descriptor.canonical_bytes().len(),
            MAX_NESTED_RECORD_DESCRIPTOR_BYTES
        );
        validate_descriptor_size(&descriptor).unwrap();
        descriptor.project_revision.push('x');
        assert!(validate_descriptor_size(&descriptor).is_err());
    }
}
