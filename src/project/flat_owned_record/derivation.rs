use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::call_index::PersistentCallIndex;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, IdentityOrigin, ResolvedFunction, ResolvedProgram, ResolvedType,
    ResolvedTypeDeclarationKind,
};

use super::super::public_api::{
    parameter_type, rust_method_name, selected_closure, valid_sha256_fact,
    validate_closure_function, validate_selected,
};
use super::{
    error, FlatOwnedRecordApiDescriptor, FlatOwnedRecordExport, FlatOwnedRecordField,
    FlatOwnedRecordFieldType, PublicApiSubject, FLAT_OWNED_RECORD_PROJECT_SCHEMA,
    MAX_FLAT_RECORD_DESCRIPTOR_BYTES, MAX_FLAT_RECORD_FIELDS,
};

pub fn derive_flat_owned_record_api_descriptor(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
) -> Result<FlatOwnedRecordApiDescriptor, Diagnostic> {
    if subject.project_schema != FLAT_OWNED_RECORD_PROJECT_SCHEMA
        || !valid_sha256_fact(subject.project_revision)
        || !valid_sha256_fact(subject.workspace_revision)
        || !valid_sha256_fact(subject.project_graph_digest)
    {
        return Err(error("flat owned-record descriptor subject is invalid"));
    }
    validate_selected(selected)?;
    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function))
        .collect::<BTreeMap<_, _>>();
    let index = PersistentCallIndex::build(program)?;
    let closure = selected_closure(program, selected, &functions, &index)?;
    for id in closure {
        validate_closure_function(
            functions
                .get(&id)
                .ok_or_else(|| error("flat owned-record closure is incomplete"))?,
        )?;
    }

    let mut exports = Vec::with_capacity(selected.len());
    let mut rust_methods = BTreeSet::new();
    for stable_id in selected {
        let function = functions
            .get(&DeclarationId::new(stable_id.clone()))
            .ok_or_else(|| error("flat owned-record export is absent"))?;
        exports.push(derive_export(
            program,
            function,
            stable_id,
            &mut rust_methods,
        )?);
    }
    let mut record_names = BTreeMap::<String, DeclarationId>::new();
    for export in &exports {
        if let Some(previous) =
            record_names.insert(export.record_host_name.clone(), export.record_id.clone())
        {
            if previous != export.record_id {
                return Err(error("flat owned-record host type identities collide"));
            }
        }
        if export
            .fields
            .iter()
            .map(|field| field.host_name.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != export.fields.len()
        {
            return Err(error("flat owned-record host field identities collide"));
        }
    }
    let descriptor = FlatOwnedRecordApiDescriptor {
        project_revision: subject.project_revision.to_owned(),
        workspace_revision: subject.workspace_revision.to_owned(),
        project_graph_digest: subject.project_graph_digest.to_owned(),
        exports,
    };
    if descriptor.canonical_bytes().len() > MAX_FLAT_RECORD_DESCRIPTOR_BYTES {
        return Err(error("flat owned-record descriptor exceeds its byte limit"));
    }
    Ok(descriptor)
}

fn derive_export(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    stable_id: &str,
    rust_methods: &mut BTreeSet<String>,
) -> Result<FlatOwnedRecordExport, Diagnostic> {
    let function_fact = program
        .declarations
        .declaration(&function.id)
        .ok_or_else(|| error("flat owned-record export lacks declaration metadata"))?;
    if function_fact.identity_origin != IdentityOrigin::Explicit
        || function.id == program.entrypoint
    {
        return Err(error(
            "flat owned-record export must have an explicit non-entry stable identity",
        ));
    }
    let parameters = function
        .params
        .iter()
        .map(|parameter| {
            let ty = parameter_type(&parameter.ty, parameter.ownership)
                .ok_or_else(|| error("flat owned-record export parameter is unsupported"))?;
            Ok((parameter.id.as_str().to_owned(), parameter.name.clone(), ty))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    if parameters.len() > super::super::MAX_PUBLIC_API_PARAMETERS {
        return Err(error("flat owned-record export has too many parameters"));
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &function.return_type
    else {
        return Err(error("flat owned-record export result is not a record"));
    };
    if !arguments.is_empty() {
        return Err(error("flat owned-record result must be monomorphic"));
    }
    let record = program
        .types
        .iter()
        .find(|candidate| &candidate.id == declaration)
        .ok_or_else(|| error("flat owned-record result declaration is absent"))?;
    let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(error("flat owned-record result must be an authored record"));
    };
    if !record.type_parameters.is_empty()
        || fields.is_empty()
        || fields.len() > MAX_FLAT_RECORD_FIELDS
    {
        return Err(error("flat owned-record result field inventory is invalid"));
    }
    let record_fact = program
        .declarations
        .declaration(&record.id)
        .ok_or_else(|| error("flat owned-record result lacks declaration metadata"))?;
    if record_fact.identity_origin != IdentityOrigin::Explicit {
        return Err(error("flat owned-record result requires an explicit @id"));
    }
    let mut owned = 0_usize;
    let fields = fields
        .iter()
        .enumerate()
        .map(|(ordinal, field)| {
            if field.index as usize != ordinal {
                return Err(error("flat owned-record field ordinals are not canonical"));
            }
            let fact = program
                .declarations
                .declaration(&field.id)
                .ok_or_else(|| error("flat owned-record field lacks declaration metadata"))?;
            if fact.identity_origin != IdentityOrigin::Explicit {
                return Err(error(
                    "flat owned-record fields require explicit @id values",
                ));
            }
            let ty = match field.ty {
                ResolvedType::I64 => FlatOwnedRecordFieldType::I64,
                ResolvedType::Bool => FlatOwnedRecordFieldType::Bool,
                ResolvedType::Usize => FlatOwnedRecordFieldType::Usize,
                ResolvedType::Bytes => {
                    owned += 1;
                    FlatOwnedRecordFieldType::OwnedBytes
                }
                _ => return Err(error("flat owned-record field type is unsupported")),
            };
            Ok(FlatOwnedRecordField {
                stable_id: field.id.clone(),
                source_name: field.name.clone(),
                host_name: host_field_name(&field.name, field.id.as_str()),
                ordinal: field.index,
                ty,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    if owned != 1 {
        return Err(error(
            "flat owned-record result requires exactly one direct Bytes field",
        ));
    }
    let rust_method_name = rust_method_name(stable_id)?;
    if !rust_methods.insert(rust_method_name.clone()) {
        return Err(error("flat owned-record Rust method identities collide"));
    }
    Ok(FlatOwnedRecordExport {
        stable_id: function.id.clone(),
        typescript_name: stable_id.to_owned(),
        rust_method_name,
        parameters,
        record_id: record.id.clone(),
        record_host_name: host_record_name(&record.name, record.id.as_str()),
        record_source_name: record.name.clone(),
        fields,
    })
}

fn stable_host_name(prefix: &str, stable_id: &str) -> String {
    let digest = Sha256::digest(stable_id.as_bytes());
    let hex = format!("{:x}", crate::digest_hex::LowerHex(digest));
    match prefix {
        "record" => format!("SpxRecordH{hex}"),
        "field" => format!("spx_field_h{hex}"),
        _ => unreachable!("closed host-name family"),
    }
}

fn host_record_name(source_name: &str, stable_id: &str) -> String {
    if source_name
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_uppercase())
        && source_name.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        source_name.to_owned()
    } else {
        stable_host_name("record", stable_id)
    }
}

fn host_field_name(source_name: &str, stable_id: &str) -> String {
    const RUST_KEYWORDS: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn",
    ];
    if source_name
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_')
        && source_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !RUST_KEYWORDS.contains(&source_name)
    {
        source_name.to_owned()
    } else {
        stable_host_name("field", stable_id)
    }
}
