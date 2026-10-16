//! Complete original declaration producer, independently compiled.
use crate::ast::program_source::ProgramSource;
use crate::ast::{Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::source_verify::diagnostics::error;
use crate::source_verify::type_table::{
    classify_nested_owned_byte_record, owned_byte_record_copy_field_is_admitted,
    NestedOwnedRecordAdmission, TypeTable,
};

pub(super) fn check_byte_data_declarations<'p>(
    program: &'p dyn ProgramSource,
    types: &TypeTable<'p>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in program.types() {
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } => {
                // The nested owned-Bytes profile is monomorphic, and a generic
                // declaration has no arguments to substitute into its fields,
                // so classifying its root would read an unsubstituted parameter
                // as owning. Ask each declared field instead: a type parameter
                // resolves to no declaration and so owns nothing, while a
                // `Bytes` field or a field whose record reaches one is refused
                // here. Concrete instances are admitted through HIR type
                // reachability.
                if !declaration.type_parameters.is_empty() {
                    if fields
                        .iter()
                        .any(|field| types.contains_owned_bytes(&field.ty))
                    {
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
                if types.contains_owned_bytes(&root)
                    && !super::super::declared_type::string_record::runtime_admitted(&root, types)
                {
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
            // Site 1 of the five admitted-variant-payload-profile classifiers
            // named in issue #261. The owned-string field admission test just
            // below (`types.is_admitted_owned_string_variant_field`) is the
            // one owning source-level ("Type") predicate this site shares
            // with site 2, `TypeTable::is_flat_owned_string_variant` in
            // `type_table.rs` (`SPX-O002`/`SPX-O104`). The other three sites
            // run after HIR resolution, on `ResolvedType` rather than `Type`,
            // and share their own owning predicate,
            // `type_reachability::is_admitted_owned_string_variant_field`:
            // site 3, `is_admitted_owned_string_variant` in
            // `hir/type_reachability.rs` (`SPX-O117`); site 4, the variant
            // profile gate in `hir/validation.rs` (`SPX-H006`); and site 5,
            // `Interpreter::value_has_type` in `interpreter.rs` (`SPX-F105`),
            // which checks a runtime value rather than a field type but
            // consults the same resolved-level copy-aggregate predicate.
            TypeDeclarationKind::Variant { cases } => {
                let fields = cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .collect::<Vec<_>>();
                let has_direct_bytes = fields.iter().any(|field| field.ty == Type::Bytes);
                let has_direct_string = fields.iter().any(|field| field.ty == Type::String);
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
                if has_direct_string && !has_direct_bytes && !declaration.type_parameters.is_empty()
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T268",
                        format!(
                            "owned-string variant `{}` must be monomorphic in this tranche",
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
                    } else if !has_direct_bytes && types.contains_owned_bytes(&field.ty)
                        && !super::super::declared_type::collection_outcome::runtime_declaration_admitted(types, declaration) {
                        diagnostics.push(error(
                            program,
                            "SPX-T268",
                            format!(
                                "variant `{}` nests owned `Bytes`; this tranche admits only direct `Bytes` payloads",
                                declaration.name
                            ),
                            field.span,
                        ));
                    } else if has_direct_string
                        && !has_direct_bytes
                        && !types.is_admitted_owned_string_variant_field(&field.ty)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-T268",
                            format!(
                                "owned-string variant field `{}.{}` must be direct `string`, a direct Copy scalar, or a drop-free Copy aggregate record",
                                declaration.name, field.name
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
