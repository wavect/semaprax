//! One ordinary record-shape producer and its exact diagnostic dependency.
//! Recovery grants no type, ownership, executable admission or backend authority.
use crate::ast::program_source::ProgramSource;
use crate::ast::{Function, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::Binding;
use crate::source_verify::diagnostics::error;
use crate::source_verify::recovery::Recovery;
use crate::source_verify::type_table::{
    classify_nested_owned_byte_record, NestedOwnedRecordAdmission, TypeTable,
};
use std::collections::HashMap;

pub(super) fn diagnostic(
    program: &dyn ProgramSource,
    types: &TypeTable<'_>,
    declaration: &TypeDeclaration,
) -> Option<Diagnostic> {
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    // Generic roots have no arguments to substitute. Only an independently
    // owning declared field receives this original declaration diagnostic.
    if !declaration.type_parameters.is_empty() {
        return fields
            .iter()
            .any(|field| types.contains_owned_bytes(&field.ty))
            .then(|| {
                error(
                    program,
                    "SPX-T268",
                    format!(
                        "owned-Bytes record `{}` must be monomorphic in this tranche",
                        declaration.name,
                    ),
                    declaration.span,
                )
            });
    }
    let root = Type::Named {
        name: declaration.name.clone(),
        arguments: Vec::new(),
    };
    if !types.contains_owned_bytes(&root)
        || crate::source_verify::declared_type::string_record::runtime_admitted(&root, types)
    {
        return None;
    }
    match classify_nested_owned_byte_record(types, &root) {
        NestedOwnedRecordAdmission::Admitted | NestedOwnedRecordAdmission::Recursive => None,
        NestedOwnedRecordAdmission::NoOwnedBytes => unreachable!(
            "owned-byte precheck and structural classifier disagree"
        ),
        NestedOwnedRecordAdmission::OutsideProfile => Some(error(
            program, "SPX-T268", format!(
                "owned-Bytes record `{}` must be a monomorphic acyclic record tree with only `Bytes` or direct Copy scalar leaves", declaration.name,
            ), declaration.span,
        )),
        NestedOwnedRecordAdmission::LimitExceeded => Some(error(
            program, "SPX-T268", format!(
                "owned-Bytes record `{}` exceeds the nested record depth, owned-leaf, or field bound", declaration.name,
            ), declaration.span,
        )),
    }
}

pub(in crate::source_verify) fn origin_named(
    program: &dyn ProgramSource,
    types: &TypeTable<'_>,
    name: &str,
    diagnostics: &[Diagnostic],
) -> Option<usize> {
    // Cold successful verification never repeats declaration classification.
    if !diagnostics
        .iter()
        .any(|d| d.code == "SPX-T268" && d.severity.is_error())
    {
        return None;
    }
    let declaration = types.declaration(name)?;
    if !declaration.type_parameters.is_empty()
        || !program
            .types()
            .iter()
            .any(|item| std::ptr::eq(item, declaration))
    {
        return None;
    }
    let refusal = diagnostic(program, types, declaration)?;
    // All six fields bind this live producer, including exact raw message/path,
    // source span and help. A same-code or same-span foreign receipt is not enough.
    diagnostics.iter().position(|existing| {
        existing.code == refusal.code
            && existing.severity == refusal.severity
            && existing.message == refusal.message
            && existing.path == refusal.path
            && existing.span == refusal.span
            && existing.help == refusal.help
    })
}

/// Continue ordinary checking only when every refused storage declaration has
/// the exact concrete-record producer understood by expression recovery.
/// Other storage refusals keep their complete prior early-return route.
pub(in crate::source_verify) fn all_storage_refusals_recoverable(
    program: &dyn ProgramSource,
    types: &TypeTable<'_>,
    diagnostics: &[Diagnostic],
) -> bool {
    let mut refused = false;
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if !diagnostic.severity.is_error() {
            continue;
        }
        refused = true;
        if !program.types().iter().any(|declaration| {
            // Bound refusals keep the previous signature-only frontier. In
            // particular, generic signatures can add T223/T224 after this gate.
            let root = Type::Named {
                name: declaration.name.clone(),
                arguments: Vec::new(),
            };
            matches!(
                classify_nested_owned_byte_record(types, &root),
                NestedOwnedRecordAdmission::OutsideProfile
            ) && origin_named(program, types, &declaration.name, diagnostics) == Some(index)
        }) {
            return false;
        }
    }
    refused
}

pub(in crate::source_verify) fn origin(
    program: &dyn ProgramSource,
    types: &TypeTable<'_>,
    ty: &Type,
    diagnostics: &[Diagnostic],
) -> Option<usize> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    origin_named(program, types, name, diagnostics)
}

pub(in crate::source_verify) fn seed<'p>(
    recovery: &mut Recovery<'p>,
    program: &'p dyn ProgramSource,
    current: &'p Function,
    variables: &HashMap<String, Binding>,
    types: &TypeTable<'p>,
    diagnostics: &[Diagnostic],
) {
    for parameter in &current.params {
        if variables
            .get(&parameter.name)
            .is_none_or(|binding| binding.ty != parameter.ty || binding.mode != parameter.mode)
        {
            continue;
        }
        if let Some(origin) = origin(program, types, &parameter.ty, diagnostics) {
            recovery.refused_annotation(0, &parameter.name, &parameter.ty, origin);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::record_shape_before as before;
    use super::*;
    use std::path::Path;

    #[test]
    fn full_original_declaration_producer_keeps_every_diagnostic_field_and_order() {
        for declaration in [
            "record Packet { payload: Bytes, text: string, }",
            "record Packet { payload: Bytes, text: string, unsupported: [u8; 1], }",
            "record Packet { payload: Bytes, unsupported: [u8; 1], }",
            "record Packet<T> { payload: Bytes, item: T, }",
            "record Packet { payload: Bytes, unsupported: [u8; 1], } class Other { data: Bytes, }",
        ] {
            let source = format!("module t; {declaration} fn main()->i64{{0}}");
            let parsed = crate::parse(&source, Path::new("shape-producer.spx")).unwrap();
            let types = TypeTable::new(&parsed);
            let mut old = Vec::new();
            let mut current = Vec::new();
            before::check_byte_data_declarations(&parsed, &types, &mut old);
            super::super::declarations::check_byte_data_declarations(&parsed, &types, &mut current);
            assert_eq!(current, old);
        }
    }

    #[test]
    fn shape_origin_refuses_each_foreign_diagnostic_field_and_valid_record() {
        let parsed = crate::parse(
            "module t; record Packet { payload: Bytes, text: string, unsupported: [u8; 1], } record Valid { payload: Bytes, } fn main()->i64{0}",
            Path::new("shape-origin.spx"),
        ).unwrap();
        let types = TypeTable::new(&parsed);
        let mut receipts = Vec::new();
        before::check_byte_data_declarations(&parsed, &types, &mut receipts);
        assert_eq!(receipts.len(), 1);
        assert_eq!(origin_named(&parsed, &types, "Packet", &receipts), Some(0));
        assert_eq!(origin_named(&parsed, &types, "Valid", &receipts), None);
        assert_eq!(origin_named(&parsed, &types, "Missing", &receipts), None);
        for field in 0..6 {
            let mut foreign = receipts[0].clone();
            match field {
                0 => foreign.code = "SPX-T999",
                1 => foreign.severity = crate::diagnostic::Severity::Warning,
                2 => foreign.message = "foreign record shape".into(),
                3 => foreign.path = Some("foreign.spx".to_owned()),
                4 => foreign.span = None,
                5 => foreign.help = Some("foreign help".to_owned()),
                _ => unreachable!(),
            }
            assert_eq!(origin_named(&parsed, &types, "Packet", &[foreign]), None);
        }
    }

    #[test]
    fn storage_recovery_requires_every_exact_live_record_refusal() {
        let source = "module recovery; record Packet { payload: Bytes, text: string, unsupported: [u8; 1], } fn main()->i64{0}";
        let parsed = crate::parse(source, Path::new("storage-recovery.spx")).unwrap();
        let types = TypeTable::new(&parsed);
        let mut diagnostics = Vec::new();
        before::check_byte_data_declarations(&parsed, &types, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(all_storage_refusals_recoverable(
            &parsed,
            &types,
            &diagnostics
        ));
        assert!(!all_storage_refusals_recoverable(&parsed, &types, &[]));
        for field in 0..6 {
            let mut foreign = diagnostics[0].clone();
            match field {
                0 => foreign.code = "SPX-T999",
                1 => foreign.severity = crate::diagnostic::Severity::Warning,
                2 => foreign.message.to_mut().push_str(" foreign"),
                3 => foreign.path = Some("foreign.spx".to_owned()),
                4 => foreign.span = None,
                5 => foreign.help = Some("foreign help".to_owned()),
                _ => unreachable!(),
            }
            assert!(!all_storage_refusals_recoverable(
                &parsed,
                &types,
                &[foreign]
            ));
        }
        let fields = (0..257)
            .map(|index| format!("leaf_{index}: Bytes,"))
            .collect::<String>();
        let bounded = crate::parse(
            &format!("module bounded; record Bag {{{fields}}} fn main()->i64{{0}}"),
            Path::new("storage-recovery-bound.spx"),
        )
        .unwrap();
        let bounded_types = TypeTable::new(&bounded);
        let mut bounded_diagnostics = Vec::new();
        before::check_byte_data_declarations(&bounded, &bounded_types, &mut bounded_diagnostics);
        assert_eq!(bounded_diagnostics.len(), 1);
        assert!(!all_storage_refusals_recoverable(
            &bounded,
            &bounded_types,
            &bounded_diagnostics
        ));
        let mixed = crate::parse(
            &format!("{source} class Other {{ payload: Bytes, }}"),
            Path::new("storage-recovery-mixed.spx"),
        )
        .unwrap();
        let mixed_types = TypeTable::new(&mixed);
        let mut mixed_diagnostics = Vec::new();
        before::check_byte_data_declarations(&mixed, &mixed_types, &mut mixed_diagnostics);
        assert!(mixed_diagnostics.len() > 1);
        assert!(!all_storage_refusals_recoverable(
            &mixed,
            &mixed_types,
            &mixed_diagnostics
        ));
    }

    #[test]
    fn complete_original_declaration_producer_body_is_byte_exact() {
        let original = include_str!("../../../docs/evidence/opt859-record-shape-recovery-20261016/before/src__source_verify__declaration__declarations.rs");
        let end = original
            .find("\npub(super) fn check_native_rust_imports")
            .unwrap();
        let start = original
            .find("pub(super) fn check_byte_data_declarations")
            .unwrap();
        let current = include_str!("record_shape_before.rs");
        let current_start = current
            .find("pub(super) fn check_byte_data_declarations")
            .unwrap();
        assert_eq!(&current[current_start..], &original[start..end]);
    }
}
