//! Declaration and interface identity checks: reserved and duplicate names,
//! duplicate stable identities, and the logical import key table.

use crate::ast::{
    ImportDeclaration, InterfaceDeclaration, Program, ResourceLifecycleKind, Type,
    TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;
use crate::source_verify::diagnostics::{
    error, invalid_stable_id, reject_reserved_host_id, source_identifier,
};
use std::collections::{HashMap, HashSet};

pub(super) fn check_type_identities<'p>(
    program: &'p Program,
    ids: &mut HashSet<&'p str>,
    type_names: &mut HashSet<&'p str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &program.types {
        if crate::prelude::is_reserved_type_name(&declaration.name) {
            diagnostics.push(error(
                program,
                "SPX-S113",
                format!(
                    "type name `{}` is reserved by compiler prelude `{}`",
                    declaration.name,
                    crate::prelude::SCHEMA_V1
                ),
                declaration.name_span,
            ));
        }
        if matches!(declaration.kind, TypeDeclarationKind::Resource { .. })
            && !declaration.type_parameters.is_empty()
        {
            diagnostics.push(error(
                program,
                "SPX-T223",
                "only record and variant declarations may declare generic parameters in this slice",
                declaration.type_parameters[0].span,
            ));
        }
        let mut parameter_names = HashSet::new();
        for parameter in &declaration.type_parameters {
            if !source_identifier(&parameter.name)
                || !parameter_names.insert(parameter.name.as_str())
            {
                diagnostics.push(error(
                    program,
                    "SPX-T220",
                    format!(
                        "invalid or duplicate type parameter `{}` on `{}`",
                        parameter.name, declaration.name
                    ),
                    parameter.span,
                ));
            }
        }
        if !source_identifier(&declaration.name) {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => "resource",
                TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. } => "record",
                TypeDeclarationKind::Variant { .. } => "variant",
            };
            diagnostics.push(error(
                program,
                "SPX-S106",
                format!("`{}` is not a valid {kind} identifier", declaration.name),
                declaration.name_span,
            ));
        }
        if !type_names.insert(declaration.name.as_str()) {
            let duplicate = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => "resource",
                TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. } => "type",
                TypeDeclarationKind::Variant { .. } => "type",
            };
            diagnostics.push(error(
                program,
                "SPX-S107",
                format!("duplicate {duplicate} `{}`", declaration.name),
                declaration.span,
            ));
        }
        reject_reserved_host_id(
            program,
            &declaration.stable_id,
            "type declaration",
            declaration.span,
            diagnostics,
        );
        if declaration.stable_id.contains('\0') {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => "resource",
                TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. } => "record",
                TypeDeclarationKind::Variant { .. } => "variant",
            };
            diagnostics.push(invalid_stable_id(
                program,
                "SPX-S102",
                format!("{kind} `{}`", declaration.name),
                declaration.span,
            ));
        } else if !ids.insert(declaration.stable_id.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-S102",
                format!("duplicate stable id `{}`", declaration.stable_id),
                declaration.span,
            ));
        }
        if !declaration.explicit_id {
            let (subject, help) = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => ("resource", "your.namespace.resource"),
                TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. } => {
                    ("record", "your.namespace.record")
                }
                TypeDeclarationKind::Variant { .. } => ("variant", "your.namespace.variant"),
            };
            diagnostics.push(
                Diagnostic::warning(
                    "SPX-S108",
                    format!(
                        "{subject} `{}` has an automatic identity that changes when renamed",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .at_path(&program.path)
                .with_help(format!("add @id(\"{help}\") before the declaration")),
            );
        }
        if let TypeDeclarationKind::Resource { lifecycles } = &declaration.kind {
            if lifecycles.is_empty() {
                diagnostics.push(
                    error(
                        program,
                        "SPX-O112",
                        format!(
                            "resource `{}` must declare exactly one destruction strategy",
                            declaration.name
                        ),
                        declaration.name_span,
                    )
                    .with_help(
                        "declare an explicitly identified `drop trivial` or `drop import` strategy",
                    ),
                );
            } else if lifecycles.len() > 1 {
                diagnostics.push(error(
                    program,
                    "SPX-O113",
                    format!(
                        "resource `{}` declares more than one destruction strategy",
                        declaration.name
                    ),
                    lifecycles[1].span,
                ));
            }
            for lifecycle in lifecycles {
                if let ResourceLifecycleKind::Imported { import_key } = &lifecycle.kind {
                    if import_key.contains('\0') {
                        diagnostics.push(error(
                            program,
                            "SPX-O113",
                            format!(
                                "resource lifecycle `{}.drop` has an invalid logical import key; persistent identities forbid NUL",
                                declaration.name
                            ),
                            lifecycle.span,
                        ));
                    }
                }
                match lifecycle.stable_id.as_deref() {
                    Some(id) if !id.is_empty() => {
                        reject_reserved_host_id(
                            program,
                            id,
                            "resource lifecycle",
                            lifecycle.span,
                            diagnostics,
                        );
                        if id.contains('\0') {
                            diagnostics.push(invalid_stable_id(
                                program,
                                "SPX-O113",
                                format!("resource lifecycle `{}.drop`", declaration.name),
                                lifecycle.span,
                            ));
                        } else if !ids.insert(id) {
                            diagnostics.push(error(
                                program,
                                "SPX-S102",
                                format!("duplicate stable id `{id}`"),
                                lifecycle.span,
                            ));
                        }
                    }
                    _ => diagnostics.push(
                        error(
                            program,
                            "SPX-O113",
                            format!(
                                "resource lifecycle `{}.drop` requires an explicit @id",
                                declaration.name
                            ),
                            lifecycle.span,
                        )
                        .with_help("add @id(\"your.namespace.resource.drop\") before `drop`"),
                    ),
                }
            }
        }
        if let TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } =
            &declaration.kind
        {
            let mut field_names = HashSet::new();
            let mut field_ids = HashSet::new();
            for field in fields {
                if field.ty == Type::Str {
                    diagnostics.push(error(
                        program,
                        "SPX-O116",
                        format!(
                            "aggregate field `{}.{}` cannot store borrowed `str`",
                            declaration.name, field.name
                        ),
                        field.span,
                    ));
                }
                if field.ty == Type::SliceU8 {
                    diagnostics.push(error(
                        program,
                        "SPX-T264",
                        format!(
                            "aggregate field `{}.{}` cannot store borrowed `Slice<u8>`",
                            declaration.name, field.name
                        ),
                        field.span,
                    ));
                }
                if !source_identifier(&field.name) {
                    diagnostics.push(error(
                        program,
                        "SPX-S110",
                        format!("`{}` is not a valid field identifier", field.name),
                        field.name_span,
                    ));
                }
                if !field_names.insert(field.name.as_str())
                    || !field_ids.insert(field.stable_id.as_str())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-S111",
                        format!(
                            "duplicate field `{}` in record `{}`",
                            field.name, declaration.name
                        ),
                        field.span,
                    ));
                }
                reject_reserved_host_id(
                    program,
                    &field.stable_id,
                    "record field",
                    field.span,
                    diagnostics,
                );
                if field.stable_id.contains('\0') {
                    diagnostics.push(invalid_stable_id(
                        program,
                        "SPX-S102",
                        format!("field `{}.{}`", declaration.name, field.name),
                        field.span,
                    ));
                } else if !ids.insert(field.stable_id.as_str()) {
                    diagnostics.push(error(
                        program,
                        "SPX-S102",
                        format!("duplicate stable id `{}`", field.stable_id),
                        field.span,
                    ));
                }
                if !field.explicit_id {
                    diagnostics.push(
                        Diagnostic::warning(
                            "SPX-S112",
                            format!(
                                "field `{}.{}` has an automatic identity that changes when renamed",
                                declaration.name, field.name
                            ),
                            field.name_span,
                        )
                        .at_path(&program.path)
                        .with_help("add @id(\"your.namespace.record.field\") before the field"),
                    );
                }
            }
        }
        if let TypeDeclarationKind::Variant { cases } = &declaration.kind {
            if cases.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T215",
                    format!(
                        "variant `{}` must declare at least one case",
                        declaration.name
                    ),
                    declaration.span,
                ));
            }
            let mut case_names = HashSet::new();
            let mut case_ids = HashSet::new();
            for case in cases {
                if !source_identifier(&case.name) {
                    diagnostics.push(error(
                        program,
                        "SPX-S110",
                        format!("`{}` is not a valid variant case identifier", case.name),
                        case.name_span,
                    ));
                }
                if !case_names.insert(case.name.as_str())
                    || !case_ids.insert(case.stable_id.as_str())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-S111",
                        format!(
                            "duplicate case `{}` in variant `{}`",
                            case.name, declaration.name
                        ),
                        case.span,
                    ));
                }
                reject_reserved_host_id(
                    program,
                    &case.stable_id,
                    "variant case",
                    case.span,
                    diagnostics,
                );
                if case.stable_id.contains('\0') {
                    diagnostics.push(invalid_stable_id(
                        program,
                        "SPX-S102",
                        format!("case `{}::{}`", declaration.name, case.name),
                        case.span,
                    ));
                } else if !ids.insert(case.stable_id.as_str()) {
                    diagnostics.push(error(
                        program,
                        "SPX-S102",
                        format!("duplicate stable id `{}`", case.stable_id),
                        case.span,
                    ));
                }
                if !case.explicit_id {
                    diagnostics.push(
                        Diagnostic::warning(
                            "SPX-S112",
                            format!(
                                "case `{}::{}` has an automatic identity that changes when renamed",
                                declaration.name, case.name
                            ),
                            case.name_span,
                        )
                        .at_path(&program.path)
                        .with_help("add @id(\"your.namespace.variant.case\") before the case"),
                    );
                }
                let mut field_names = HashSet::new();
                let mut field_ids = HashSet::new();
                for field in &case.fields {
                    if field.ty == Type::Str {
                        diagnostics.push(error(
                            program,
                            "SPX-O116",
                            format!(
                                "variant field `{}::{}.{}` cannot store borrowed `str`",
                                declaration.name, case.name, field.name
                            ),
                            field.span,
                        ));
                    }
                    if field.ty == Type::SliceU8 {
                        diagnostics.push(error(
                            program,
                            "SPX-T264",
                            format!(
                                "variant field `{}::{}.{}` cannot store borrowed `Slice<u8>`",
                                declaration.name, case.name, field.name
                            ),
                            field.span,
                        ));
                    }
                    if matches!(field.ty, Type::ArrayU8(_)) {
                        diagnostics.push(error(
                            program,
                            "SPX-T268",
                            format!(
                                "variant field `{}::{}.{}` cannot store fixed arrays",
                                declaration.name, case.name, field.name
                            ),
                            field.span,
                        ));
                    }
                    if !source_identifier(&field.name) {
                        diagnostics.push(error(
                            program,
                            "SPX-S110",
                            format!("`{}` is not a valid case field identifier", field.name),
                            field.name_span,
                        ));
                    }
                    if !field_names.insert(field.name.as_str())
                        || !field_ids.insert(field.stable_id.as_str())
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-S111",
                            format!(
                                "duplicate field `{}` in case `{}::{}`",
                                field.name, declaration.name, case.name
                            ),
                            field.span,
                        ));
                    }
                    reject_reserved_host_id(
                        program,
                        &field.stable_id,
                        "variant case field",
                        field.span,
                        diagnostics,
                    );
                    if field.stable_id.contains('\0') {
                        diagnostics.push(invalid_stable_id(
                            program,
                            "SPX-S102",
                            format!(
                                "case field `{}::{}.{}`",
                                declaration.name, case.name, field.name
                            ),
                            field.span,
                        ));
                    } else if !ids.insert(field.stable_id.as_str()) {
                        diagnostics.push(error(
                            program,
                            "SPX-S102",
                            format!("duplicate stable id `{}`", field.stable_id),
                            field.span,
                        ));
                    }
                    if !field.explicit_id {
                        diagnostics.push(
                            Diagnostic::warning(
                                "SPX-S112",
                                format!(
                                    "case field `{}::{}.{}` has an automatic identity that changes when renamed",
                                    declaration.name, case.name, field.name
                                ),
                                field.name_span,
                            )
                            .at_path(&program.path)
                            .with_help(
                                "add @id(\"your.namespace.variant.case.field\") before the field",
                            ),
                        );
                    }
                }
            }
        }
    }
}

pub(super) fn check_interface_identities<'p>(
    program: &'p Program,
    ids: &mut HashSet<&'p str>,
    interface_names: &mut HashSet<&'p str>,
    import_keys: &mut HashMap<&'p str, (&'p InterfaceDeclaration, &'p ImportDeclaration)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for interface in &program.interfaces {
        if !source_identifier(&interface.name) {
            diagnostics.push(error(
                program,
                "SPX-I403",
                format!("`{}` is not a valid interface identifier", interface.name),
                interface.name_span,
            ));
        }
        if !interface_names.insert(interface.name.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-I403",
                format!("duplicate interface `{}`", interface.name),
                interface.span,
            ));
        }
        if !interface.explicit_id || interface.stable_id.is_empty() {
            diagnostics.push(
                error(
                    program,
                    "SPX-I403",
                    format!("interface `{}` requires an explicit @id", interface.name),
                    interface.name_span,
                )
                .with_help("add @id(\"your.namespace.interface\") before the interface"),
            );
        }
        reject_reserved_host_id(
            program,
            &interface.stable_id,
            "interface",
            interface.span,
            diagnostics,
        );
        if interface.stable_id.contains('\0') {
            diagnostics.push(invalid_stable_id(
                program,
                "SPX-I403",
                format!("interface `{}`", interface.name),
                interface.span,
            ));
        } else if !ids.insert(interface.stable_id.as_str()) {
            diagnostics.push(error(
                program,
                "SPX-S102",
                format!("duplicate stable id `{}`", interface.stable_id),
                interface.span,
            ));
        }
        let mut permitted = HashSet::new();
        for effect in &interface.permits {
            if !permitted.insert(effect.as_str()) {
                diagnostics.push(error(
                    program,
                    "SPX-I403",
                    format!(
                        "interface `{}` declares duplicate permit `{effect}`",
                        interface.name
                    ),
                    interface.span,
                ));
            }
        }
        let mut import_names = HashSet::new();
        for import in &interface.imports {
            if !source_identifier(&import.name) {
                diagnostics.push(error(
                    program,
                    "SPX-I403",
                    format!("`{}` is not a valid import identifier", import.name),
                    import.name_span,
                ));
            }
            if crate::host_io_ops::by_name(&import.name).is_some()
                || crate::command_io_ops::by_name(&import.name).is_some()
            {
                diagnostics.push(error(
                    program,
                    "SPX-S113",
                    format!(
                        "import `{}.{}` aliases a compiler-owned host I/O operation",
                        interface.name, import.name
                    ),
                    import.name_span,
                ));
            }
            reject_reserved_host_id(
                program,
                &import.stable_id,
                "import",
                import.span,
                diagnostics,
            );
            if !import_names.insert(import.name.as_str()) {
                diagnostics.push(error(
                    program,
                    "SPX-I403",
                    format!("duplicate import `{}.{}`", interface.name, import.name),
                    import.span,
                ));
            }
            if !import.explicit_id || import.stable_id.is_empty() {
                if import.native_rust {
                    diagnostics.push(error(
                        program,
                        "SPX-B107",
                        "Native Rust Interop declaration set is unsupported: explicit persistent ID required",
                        import.name_span,
                    ));
                } else {
                    diagnostics.push(
                        error(
                            program,
                            "SPX-I403",
                            format!(
                                "import `{}.{}` requires an explicit @id",
                                interface.name, import.name
                            ),
                            import.name_span,
                        )
                        .with_help(
                            "the v1 import @id is also its target-neutral logical import key",
                        ),
                    );
                }
            }
            let import_identity_is_valid = !import.stable_id.contains('\0');
            if !import_identity_is_valid {
                diagnostics.push(invalid_stable_id(
                    program,
                    "SPX-I403",
                    format!("import `{}.{}`", interface.name, import.name),
                    import.span,
                ));
            } else if !ids.insert(import.stable_id.as_str()) {
                diagnostics.push(error(
                    program,
                    "SPX-S102",
                    format!("duplicate stable id `{}`", import.stable_id),
                    import.span,
                ));
            }
            if import_identity_is_valid
                && import_keys
                    .insert(import.stable_id.as_str(), (interface, import))
                    .is_some()
            {
                diagnostics.push(error(
                    program,
                    "SPX-I403",
                    format!("duplicate logical import key `{}`", import.stable_id),
                    import.span,
                ));
            }
        }
    }
}
