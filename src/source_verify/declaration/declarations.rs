//! Declaration body checks: byte-data storage rules, native Rust imports,
//! resource lifecycles, declared field types, and recursive record layouts.

use crate::ast::{
    ImportDeclaration, ImportFailure, InterfaceDeclaration, ParamMode, Program,
    ResourceLifecycleKind, Type, TypeDeclarationKind,
};
use crate::conformance::STATUS_DOMAIN_MAX_BYTES_V1;
use crate::diagnostic::Diagnostic;
use crate::source_verify::declared_type::{
    check_declared_type, native_rust_status_domain, record_layout_is_recursive,
};
use crate::source_verify::diagnostics::error;
use crate::source_verify::type_table::{
    classify_nested_owned_byte_record, owned_byte_record_copy_field_is_admitted,
    NestedOwnedRecordAdmission, TypeTable,
};
use std::collections::{HashMap, HashSet};

pub(super) fn check_byte_data_declarations<'p>(
    program: &'p Program,
    types: &TypeTable<'p>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &program.types {
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } => {
                // The nested owned-Bytes profile is monomorphic, and a generic
                // declaration has no arguments to substitute into its fields.
                // Classify only its own directly declared `Bytes`, exactly as
                // the variant arm below does; concrete instances are admitted
                // through HIR type reachability.
                if !declaration.type_parameters.is_empty() {
                    if fields.iter().any(|field| field.ty == Type::Bytes) {
                        diagnostics.push(error(
                            program,
                            "SPX-T268",
                            format!(
                                "owned-Bytes record `{}` must be monomorphic in this tranche",
                                declaration.name
                            ),
                            declaration.span,
                        ));
                    }
                    continue;
                }
                let root = Type::Named {
                    name: declaration.name.clone(),
                    arguments: Vec::new(),
                };
                if types.contains_owned_bytes(&root) {
                    match classify_nested_owned_byte_record(types, &root) {
                        NestedOwnedRecordAdmission::Admitted
                        | NestedOwnedRecordAdmission::Recursive => {}
                        NestedOwnedRecordAdmission::NoOwnedBytes => unreachable!(
                            "owned-byte precheck and structural classifier disagree"
                        ),
                        NestedOwnedRecordAdmission::OutsideProfile => diagnostics.push(error(
                            program,
                            "SPX-T268",
                            format!(
                                "owned-Bytes record `{}` must be a monomorphic acyclic record tree with only `Bytes` or direct Copy scalar leaves",
                                declaration.name
                            ),
                            declaration.span,
                        )),
                        NestedOwnedRecordAdmission::LimitExceeded => diagnostics.push(error(
                            program,
                            "SPX-T268",
                            format!(
                                "owned-Bytes record `{}` exceeds the nested record depth, owned-leaf, or field bound",
                                declaration.name
                            ),
                            declaration.span,
                        )),
                    }
                }
            }
            TypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    if field.ty == Type::Bytes || types.contains_owned_bytes(&field.ty) {
                        diagnostics.push(error(
                            program,
                            "SPX-T268",
                            format!(
                                "class field `{}.{}` cannot contain owned `Bytes` directly or transitively",
                                declaration.name, field.name
                            ),
                            field.span,
                        ));
                    }
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                let fields = cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .collect::<Vec<_>>();
                let has_direct_bytes = fields.iter().any(|field| field.ty == Type::Bytes);
                if has_direct_bytes && !declaration.type_parameters.is_empty() {
                    diagnostics.push(error(
                        program,
                        "SPX-T268",
                        format!(
                            "owned-Bytes variant `{}` must be monomorphic in this tranche",
                            declaration.name
                        ),
                        declaration.span,
                    ));
                }
                for field in fields {
                    if has_direct_bytes
                        && field.ty != Type::Bytes
                        && !owned_byte_record_copy_field_is_admitted(&field.ty)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-T268",
                            format!(
                                "owned-Bytes variant field `{}.{}` must be direct `Bytes` or a direct Copy scalar",
                                declaration.name, field.name
                            ),
                            field.span,
                        ));
                    } else if !has_direct_bytes && types.contains_owned_bytes(&field.ty) {
                        diagnostics.push(error(
                            program,
                            "SPX-T268",
                            format!(
                                "variant `{}` nests owned `Bytes`; this tranche admits only direct `Bytes` payloads",
                                declaration.name
                            ),
                            field.span,
                        ));
                    }
                }
            }
            TypeDeclarationKind::Resource { .. } => {}
        }
    }
}

pub(super) fn check_native_rust_imports<'p>(
    program: &'p Program,
    types: &TypeTable<'p>,
    native_rust_names: &mut HashSet<&'p str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for interface in &program.interfaces {
        let permits = interface
            .permits
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        for import in &interface.imports {
            if import.native_rust && !native_rust_names.insert(import.name.as_str()) {
                diagnostics.push(error(
                    program,
                    "SPX-B107",
                    "Native Rust Interop declaration set is unsupported: symbol collision",
                    import.span,
                ));
            }
            if import.native_rust
                && program
                    .functions
                    .iter()
                    .any(|function| function.name == import.name)
            {
                diagnostics.push(error(
                    program,
                    "SPX-B107",
                    "Native Rust Interop declaration set is unsupported: symbol collision",
                    import.span,
                ));
            }
            for param in &import.params {
                if param.ty == Type::SliceU8 {
                    diagnostics.push(error(
                        program,
                        "SPX-T268",
                        "`Slice<u8>` cannot cross an import boundary",
                        param.span,
                    ));
                }
                if matches!(param.ty, Type::ArrayU8(_) | Type::Bytes) {
                    diagnostics.push(error(
                        program,
                        "SPX-T268",
                        "fixed arrays and `Bytes` cannot cross an import boundary",
                        param.span,
                    ));
                }
                check_declared_type(
                    program,
                    &param.ty,
                    param.span,
                    types,
                    &HashSet::new(),
                    diagnostics,
                );
            }
            let valid_shape = if import.native_rust {
                import.params.len() <= 8
                    && import.consumes.is_empty()
                    && import.params.iter().all(|parameter| {
                        parameter.mode == ParamMode::Value
                            && matches!(parameter.ty, Type::I64 | Type::Bool)
                    })
            } else {
                import.result == crate::ast::ImportResult::Unit
                    && import.params.len() == 1
                    && import.params[0].mode == ParamMode::Own
                    && types.is_opaque_resource(&import.params[0].ty)
                    && import.consumes == import.params[0].name
            };
            if !valid_shape {
                diagnostics.push(error(
                    program,
                    if import.native_rust { "SPX-B107" } else { "SPX-I404" },
                    if import.native_rust {
                        "Native Rust Interop declaration set is unsupported: scalar value signature required".to_owned()
                    } else {
                        format!(
                            "import `{}.{}` must take one owned resource parameter and consume it always",
                            interface.name, import.name
                        )
                    },
                    import.span,
                ));
            }
            if let ImportFailure::Status { domain_id } = &import.failure {
                if (import.native_rust && !native_rust_status_domain(domain_id))
                    || (!import.native_rust
                        && (domain_id.is_empty()
                            || domain_id.len() > STATUS_DOMAIN_MAX_BYTES_V1
                            || domain_id.contains('\0')))
                {
                    diagnostics.push(error(
                        program,
                        if import.native_rust { "SPX-B107" } else { "SPX-I403" },
                        if import.native_rust {
                            "Native Rust Interop declaration set is unsupported: status domain is invalid".to_owned()
                        } else {
                            format!(
                                "import `{}.{}` has an invalid failure domain",
                                interface.name, import.name
                            )
                        },
                        import.span,
                    ));
                }
            }
            let mut effects = HashSet::new();
            if import.native_rust
                && import
                    .effects
                    .windows(2)
                    .any(|pair| pair[0].as_str() >= pair[1].as_str())
            {
                diagnostics.push(error(
                    program,
                    "SPX-B107",
                    "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                    import.span,
                ));
            }
            for effect in &import.effects {
                if !effects.insert(effect.as_str()) {
                    diagnostics.push(if import.native_rust {
                        error(
                            program,
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                            import.span,
                        )
                    } else {
                        error(
                            program,
                            "SPX-I403",
                            format!(
                                "import `{}.{}` declares duplicate effect `{effect}`",
                                interface.name, import.name
                            ),
                            import.span,
                        )
                    });
                }
                if !permits.contains(effect.as_str()) {
                    diagnostics.push(if import.native_rust {
                        error(
                            program,
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                            import.span,
                        )
                    } else {
                        error(
                            program,
                            "SPX-I404",
                            format!(
                                "import `{}.{}` requires effect `{effect}` outside interface `{}` permits",
                                interface.name, import.name, interface.name
                            ),
                            import.span,
                        )
                    });
                }
            }
        }
    }
}

pub(super) fn check_resource_lifecycles<'p>(
    program: &'p Program,
    import_keys: &HashMap<&'p str, (&'p InterfaceDeclaration, &'p ImportDeclaration)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &program.types {
        let TypeDeclarationKind::Resource { lifecycles } = &declaration.kind else {
            continue;
        };
        if let [lifecycle] = lifecycles.as_slice() {
            if let ResourceLifecycleKind::Imported { import_key } = &lifecycle.kind {
                if import_key.contains('\0') {
                    continue;
                }
                let compatible = import_keys
                    .get(import_key.as_str())
                    .is_some_and(|(_, import)| {
                        !import.native_rust
                            && import.params.len() == 1
                            && import.params[0].mode == ParamMode::Own
                            && import.params[0].ty
                                == (Type::Named {
                                    name: declaration.name.clone(),
                                    arguments: Vec::new(),
                                })
                            && import.consumes == import.params[0].name
                            && matches!(import.failure, ImportFailure::Infallible)
                    });
                if !compatible {
                    let message = if import_keys.contains_key(import_key.as_str()) {
                        format!(
                            "logical import `{import_key}` is incompatible with automatic finalization of `{}`",
                            declaration.name
                        )
                    } else {
                        format!(
                            "resource `{}` references unknown logical import `{import_key}`",
                            declaration.name
                        )
                    };
                    diagnostics.push(error(program, "SPX-O113", message, lifecycle.span));
                }
            }
        }
    }
}

pub(super) fn check_declared_fields<'p>(
    program: &'p Program,
    types: &TypeTable<'p>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &program.types {
        if let TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } =
            &declaration.kind
        {
            let parameters = declaration
                .type_parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<HashSet<_>>();
            let is_class = matches!(declaration.kind, TypeDeclarationKind::Class { .. });
            for field in fields {
                // Class Inheritance v1: owned strings inside class members are
                // closed. The cleanup plan deliberately keeps strings out of
                // the resource-lifecycle inventory, so an aggregate carrying
                // one has no finalizer representation yet; classes fail closed
                // here instead of at cleanup-plan construction.
                if is_class && types.contains_string(&field.ty) {
                    diagnostics.push(error(
                        program,
                        "SPX-T234",
                        format!(
                            "class `{}` member `{}` carries `string`; string-bearing members are outside Class Inheritance v1",
                            declaration.name, field.name
                        ),
                        field.span,
                    ));
                }
                check_declared_type(
                    program,
                    &field.ty,
                    field.span,
                    types,
                    &parameters,
                    diagnostics,
                );
                if !parameters.is_empty() {
                    let is_parameter = matches!(
                        &field.ty,
                        Type::Named { name, arguments }
                            if arguments.is_empty() && parameters.contains(name.as_str())
                    );
                    let is_unknown_parameter = matches!(
                        &field.ty,
                        Type::Named { name, arguments }
                            if arguments.is_empty() && types.declaration(name).is_none()
                    );
                    if !matches!(field.ty, Type::I64 | Type::Bool)
                        && !is_parameter
                        && !is_unknown_parameter
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-T223",
                            format!(
                                "generic record field `{}.{}` must have direct `i64`, `bool`, or an in-scope record type parameter",
                                declaration.name, field.name
                            ),
                            field.span,
                        ));
                    }
                } else if matches!(
                    &field.ty,
                    Type::Named { name, arguments }
                        if !arguments.is_empty()
                            && matches!(
                                types.declaration(name).map(|item| &item.kind),
                                Some(TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. })
                            )
                ) {
                    diagnostics.push(error(
                        program,
                        "SPX-T223",
                        format!(
                            "record field `{}.{}` cannot nest a generic record instance in this slice",
                            declaration.name, field.name
                        ),
                        field.span,
                    ));
                }
            }
        }
        if let TypeDeclarationKind::Variant { cases } = &declaration.kind {
            let parameters = declaration
                .type_parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<HashSet<_>>();
            let owned_byte_variant = parameters.is_empty()
                && cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .any(|field| field.ty == Type::Bytes);
            for case in cases {
                for field in &case.fields {
                    check_declared_type(
                        program,
                        &field.ty,
                        field.span,
                        types,
                        &parameters,
                        diagnostics,
                    );
                    let is_parameter = matches!(
                        &field.ty,
                        Type::Named { name, arguments }
                            if arguments.is_empty() && parameters.contains(name.as_str())
                    );
                    let is_unknown_parameter = matches!(
                        &field.ty,
                        Type::Named { name, arguments }
                            if arguments.is_empty()
                                && !parameters.is_empty()
                                && types.declaration(name).is_none()
                    );
                    if !owned_byte_variant
                        && !matches!(field.ty, Type::I64 | Type::Bool)
                        && !is_parameter
                        && !is_unknown_parameter
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-T215",
                            format!(
                                "case field `{}::{}.{}` must have direct `i64`, `bool`, or an in-scope variant type parameter in Copy Variants v1",
                                declaration.name, case.name, field.name
                            ),
                            field.span,
                        ));
                    }
                }
            }
        }
    }
}

pub(super) fn check_record_layouts<'p>(
    program: &'p Program,
    types: &TypeTable<'p>,
    checked_layouts: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &program.types {
        if matches!(
            declaration.kind,
            TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. }
        ) && record_layout_is_recursive(
            declaration.name.as_str(),
            types,
            &mut HashSet::new(),
            checked_layouts,
        ) {
            diagnostics.push(error(
                program,
                "SPX-T217",
                format!(
                    "record `{}` has an illegal recursive by-value layout",
                    declaration.name
                ),
                declaration.span,
            ));
            break;
        }
    }
}
