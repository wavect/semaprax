use std::collections::{BTreeMap, BTreeSet};

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
    if !(1..=super::super::MAX_PUBLIC_API_CLOSURE_FUNCTIONS).contains(&program.functions.len()) {
        return Err(error(
            "flat owned-record linked executable inventory must contain 1..=256 functions",
        ));
    }
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
    let mut remaining_content = MAX_FLAT_RECORD_DESCRIPTOR_BYTES;
    for stable_id in selected {
        let function = functions
            .get(&DeclarationId::new(stable_id.clone()))
            .ok_or_else(|| error("flat owned-record export is absent"))?;
        exports.push(derive_export(
            program,
            function,
            stable_id,
            &mut rust_methods,
            &mut remaining_content,
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
    validate_descriptor_size(&descriptor)?;
    Ok(descriptor)
}

fn validate_descriptor_size(descriptor: &FlatOwnedRecordApiDescriptor) -> Result<(), Diagnostic> {
    if descriptor.canonical_bytes().len() > MAX_FLAT_RECORD_DESCRIPTOR_BYTES {
        return Err(error("flat owned-record descriptor exceeds its byte limit"));
    }
    Ok(())
}

fn derive_export(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    stable_id: &str,
    rust_methods: &mut BTreeSet<String>,
    remaining_content: &mut usize,
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
    if function.params.len() > super::super::MAX_PUBLIC_API_PARAMETERS {
        return Err(error("flat owned-record export has too many parameters"));
    }
    let parameters = function
        .params
        .iter()
        .map(|parameter| {
            charge_content(remaining_content, parameter.id.as_str(), 1)?;
            charge_content(remaining_content, &parameter.name, 1)?;
            let ty = parameter_type(&parameter.ty, parameter.ownership)
                .ok_or_else(|| error("flat owned-record export parameter is unsupported"))?;
            Ok((parameter.id.as_str().to_owned(), parameter.name.clone(), ty))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
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
    charge_content(remaining_content, record.id.as_str(), 3)?;
    charge_content(remaining_content, &record.name, 1)?;
    let mut owned = 0_usize;
    let fields = fields
        .iter()
        .enumerate()
        .map(|(ordinal, field)| {
            charge_content(remaining_content, field.id.as_str(), 3)?;
            charge_content(remaining_content, &field.name, 1)?;
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
                host_name: host_field_name(field.id.as_str()),
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
        record_host_name: host_record_name(record.id.as_str()),
        record_source_name: record.name.clone(),
        fields,
    })
}

// A lower bound on final canonical content, not a second wire grammar or a
// peak-heap budget. Identity bytes occur once as JSON text and twice as hex in
// host names. Charge repeated export facts again before cloning/hex expansion.
// JSON escaping and fixed framing only add bytes; the exact final check stays
// authoritative, so this cannot reject a formerly in-budget descriptor.
fn charge_content(remaining: &mut usize, value: &str, copies: usize) -> Result<(), Diagnostic> {
    let bytes = value
        .len()
        .checked_mul(copies)
        .ok_or_else(|| error("flat owned-record descriptor exceeds its byte limit"))?;
    *remaining = remaining
        .checked_sub(bytes)
        .ok_or_else(|| error("flat owned-record descriptor exceeds its byte limit"))?;
    Ok(())
}

fn stable_host_name(prefix: &str, stable_id: &str) -> String {
    let mut output = match prefix {
        "record" => String::from("SpxRecordId"),
        "field" => String::from("spx_field_id_"),
        _ => unreachable!("closed host-name family"),
    };
    for byte in stable_id.bytes() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn host_record_name(stable_id: &str) -> String {
    stable_host_name("record", stable_id)
}

fn host_field_name(stable_id: &str) -> String {
    stable_host_name("field", stable_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_size_and_content_precheck_preserve_exact_bound() {
        let fact = format!("sha256:{}", "1".repeat(64));
        let mut value = FlatOwnedRecordApiDescriptor {
            project_revision: fact.clone(),
            workspace_revision: fact.clone(),
            project_graph_digest: fact,
            exports: Vec::new(),
        };
        // Private model exercises the byte guard, not semantic admission.
        let overhead = value.canonical_bytes().len();
        value
            .project_revision
            .push_str(&"x".repeat(MAX_FLAT_RECORD_DESCRIPTOR_BYTES - overhead));
        assert_eq!(
            value.canonical_bytes().len(),
            MAX_FLAT_RECORD_DESCRIPTOR_BYTES
        );
        validate_descriptor_size(&value).unwrap();
        value.project_revision.push('x');
        assert!(validate_descriptor_size(&value).is_err());
        let mut remaining = 6;
        charge_content(&mut remaining, "λ", 3).unwrap();
        assert_eq!(remaining, 0);
        assert!(charge_content(&mut remaining, "x", 1).is_err());
    }
}
