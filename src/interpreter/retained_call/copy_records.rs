//! Retained-stage-only extension for flat Copy records over the closed scalar leaves.
//! Source/HIR validation and ordinary closure scanning remain mandatory callers.
use super::*;

pub(super) fn admitted_functions(
    program: &hir::ResolvedProgram,
) -> BTreeMap<&str, &ResolvedFunction> {
    let mut functions = admitted_resolved_functions(program);
    functions.extend(
        program
            .functions
            .iter()
            .filter(|function| {
                function.effects.is_empty()
                    && program
                        .declarations
                        .declaration(&function.id)
                        .is_some_and(|item| item.identity_origin == hir::IdentityOrigin::Explicit)
                    && (flat_copy_record(program, &function.return_type)
                        || function
                            .params
                            .iter()
                            .any(|parameter| flat_copy_record(program, &parameter.ty)))
                    && function.params.iter().all(|parameter| {
                        super::super::resolved_data_parameter_is_admitted(
                            &parameter.ty,
                            parameter.ownership,
                            &program.declarations,
                        ) || (matches!(
                            parameter.ownership,
                            hir::OwnershipMode::Value | hir::OwnershipMode::Borrow
                        ) && flat_copy_record(program, &parameter.ty))
                    })
                    && (super::super::resolved_data_result_is_admitted(
                        &function.return_type,
                        &program.declarations,
                    ) || flat_copy_record(program, &function.return_type))
            })
            .map(|function| (function.id.as_str(), function)),
    );
    functions
}

fn flat_copy_record(program: &hir::ResolvedProgram, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if !arguments.is_empty() || !record_construction_is_admitted(&program.declarations, ty) {
        return false;
    }
    let Some(item) = program.declarations.declaration(declaration) else {
        return false;
    };
    if item.kind != hir::DeclarationKind::Record
        || item.identity_origin != hir::IdentityOrigin::Explicit
    {
        return false;
    }
    let Some(record) = program
        .types
        .iter()
        .find(|record| record.id == *declaration)
    else {
        return false;
    };
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return false;
    };
    record.type_parameters.is_empty()
        && !fields.is_empty()
        && fields.iter().all(|field| {
            retained_leaf_is_admitted(&field.ty)
                && program
                    .declarations
                    .declaration(&field.id)
                    .is_some_and(|item| item.identity_origin == hir::IdentityOrigin::Explicit)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_copy_record_calls_return_exact_fields_and_reject_forged_values() {
        let source = r#"module retained.copy.records;
@id("copy.Record")
record CopyRecord { @id("copy.value") value: i64, }
@id("copy.make")
fn make(value: i64) -> CopyRecord { CopyRecord { value: value } }
@id("copy.relay")
fn relay(value: CopyRecord) -> CopyRecord { value }
@id("copy.entry")
fn entry(value: i64) -> CopyRecord { relay(make(value)) }
@id("copy.main")
fn main() -> i64 { 0 }
"#;
        let checked = crate::check(source, "retained-copy.spx").unwrap();
        let program = hir::resolve(&checked).unwrap();
        let call = prepare_retained_call(&program, "copy.entry").unwrap();
        let run =
            evaluate_retained_call(&program, &call, &[RetainedValue::I64(42)], 10_000).unwrap();
        let expected = RetainedValue::Record(RetainedRecord {
            record: DeclarationId::new("copy.Record"),
            fields: vec![RetainedField {
                field: DeclarationId::new("copy.value"),
                value: RetainedValue::I64(42),
            }],
        });
        assert_eq!(run.outcome, RetainedCallOutcome::Returned(expected));
        let call = prepare_retained_call(&program, "copy.relay").unwrap();
        let forged = RetainedValue::Record(RetainedRecord {
            record: DeclarationId::new("copy.Record"),
            fields: vec![RetainedField {
                field: DeclarationId::new("copy.value"),
                value: RetainedValue::Bool(true),
            }],
        });
        assert!(evaluate_retained_call(&program, &call, &[forged], 10_000).is_err());
    }
}
