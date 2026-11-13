use super::*;

fn observe<V: PartialEq>(
    bytes: usize,
    work: usize,
    function: &ResolvedFunction,
    expected: Option<&Option<V>>,
    query: impl Fn(&mut Payer<'_>) -> Result<Option<V>, Diagnostic>,
) -> ((bool, bool, usize), Metrics, bool, usize) {
    let ((outcome, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(bytes, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = budget
                    .charge(function, 0, "full constructor receiver setup")
                    .and_then(|()| {
                        budget.remaining = work;
                        query(&mut super::super::super::tests::payer(
                            function,
                            &mut budget,
                        ))
                    });
                let matched =
                    expected.is_none_or(|old| result.as_ref().is_ok_and(|value| value == old));
                let failed = result.is_err();
                let used = work.saturating_sub(budget.remaining);
                if failed {
                    let before = current_metrics();
                    let retry = query(&mut super::super::super::tests::payer(
                        function,
                        &mut budget,
                    ));
                    assert!(retry.is_err());
                    assert_eq!(current_metrics().allocated, before.allocated);
                    drop(retry);
                }
                drop(result);
                drop(budget);
                (matched, failed, used)
            })
        });
    (outcome, observed, refused, paid)
}
fn control<V: PartialEq>(
    function: &ResolvedFunction,
    expected: &Option<V>,
    query: impl Fn(&mut Payer<'_>) -> Result<Option<V>, Diagnostic>,
) {
    let (ample, observed, refused, paid) = observe(
        usize::MAX,
        MAX_REPLAY_WORK_UNITS,
        function,
        Some(expected),
        &query,
    );
    assert!(ample.0 && !ample.1 && !refused && !observed.invalid);
    assert!(observed.allocated <= paid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    let (exact, observed, refused, used) = observe(paid, ample.2, function, Some(expected), &query);
    assert!(exact.0 && !exact.1 && !refused && !observed.invalid);
    assert_eq!(used, paid);
    assert_eq!(observed.live, 0);
    for (bytes, work) in [
        (0, MAX_REPLAY_WORK_UNITS),
        (paid - 1, MAX_REPLAY_WORK_UNITS),
        (paid, 0),
        (paid, ample.2 - 1),
    ] {
        let (short, observed, _, paid) = observe(bytes, work, function, None, &query);
        assert!(short.1 && !observed.invalid && observed.allocated <= paid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
#[test]
fn complete_callable_nominal_identity_zero_one_seventeen_arguments_keep_paid_work_and_drop() {
    let program = fixture();
    let function = &program.functions[0];
    for width in [0, 1, 17] {
        let ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("type.é\0🦀"),
            arguments: vec![
                ResolvedType::Function {
                    parameters: vec![ResolvedType::ArrayU8(u32::MAX), ResolvedType::I64],
                    result: Box::new(ResolvedType::TypeParameter {
                        owner: function.id.clone(),
                        index: 17,
                    }),
                };
                width
            ],
        };
        let (expected, refused) = crate::bounded_output::with_limit(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget
                .charge(function, 0, "complete old key receiver")
                .unwrap();
            program
                .declarations
                .classifier_key_for_test(
                    &ty,
                    super::super::super::tests::payer(function, &mut budget),
                )
                .unwrap()
        });
        assert!(!refused);
        control(function, &expected, |payer| {
            crate::hir::inventory_identity(&ty, payer)
        });
    }
}
#[test]
fn complete_inline_semantic_substitution_refusal_and_nested_argument_clone_keep_original() {
    let program = fixture();
    let function = &program.functions[0];
    let parameter = ResolvedType::TypeParameter {
        owner: function.id.clone(),
        index: 0,
    };
    for width in [0, 1, 17] {
        let arguments = vec![ResolvedType::Nominal {
            declaration: DeclarationId::new("held.argument.é\0suffix"),
            arguments: vec![
                ResolvedType::Function {
                    parameters: vec![ResolvedType::Bytes; width],
                    result: Box::new(ResolvedType::I64),
                };
                width
            ],
        }];
        let template = ResolvedType::Function {
            parameters: vec![parameter.clone(); width],
            result: Box::new(parameter.clone()),
        };
        let (expected, refused) = crate::bounded_output::with_limit(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget
                .charge(function, 0, "complete old substitution receiver")
                .unwrap();
            program
                .declarations
                .classifier_substitute_for_test(
                    &template,
                    &function.id,
                    &arguments,
                    super::super::super::tests::payer(function, &mut budget),
                )
                .unwrap()
        });
        assert!(!refused);
        control(function, &expected, |payer| {
            crate::hir::inventory_substitute(&template, &function.id, &arguments, payer)
        });
    }
    for template in [
        parameter,
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("foreign.é\0owner"),
            index: 0,
        },
    ] {
        let expected = crate::hir::substitute_type(&template, &function.id, &[]).ok();
        assert!(expected.is_none());
        control(function, &expected, |payer| {
            crate::hir::inventory_substitute(&template, &function.id, &[], payer)
        });
    }
}
#[test]
fn complete_prior_key_substitution_and_storage_receivers_remain_independent_byte_exact() {
    fn undo(source: &mut String, new: &str, old: &str) {
        assert_eq!(
            source.matches(new).count(),
            1,
            "exact unique receiving edit"
        );
        *source = source.replacen(new, old, 1);
    }
    let mut current = ci_str!("../../../../../hir.rs").to_owned();
    undo(
        &mut current,
        "pub(crate) use validation::byte_traversal::nested::Slot as ByteTraversalSlot;\n",
        "",
    );
    undo(&mut current, "mod nominal_payload;\n#[path = \"hir/prelude_bundle.rs\"]\npub(crate) mod prelude_bundle;\npub use nominal_payload::{NominalPayload, NominalMutationError};\n", "");
    undo(&mut current, "pub(crate) use declaration_index::{inventory_identity, inventory_substitute, InventoryPayer};\n", "");
    undo(
        &mut current,
        "#[cfg(test)]\npub(crate) use byte_capacity::byte_data_capacity_inputs;\n",
        "",
    );
    // Reconstruct the complete saved root; no original interior is excluded.
    undo(
        &mut current,
        r###"pub(crate) mod borrowed_scalar;
mod byte_capacity;
mod byte_slice_provenance;
mod ordinary_scan;
pub(crate) mod static_text;
pub use static_text::StaticUtf8;
"###,
        r###"pub(crate) mod borrowed_scalar;
mod byte_capacity;
mod ordinary_scan;
mod byte_slice_provenance;
pub(crate) mod static_text;
pub use static_text::StaticUtf8;
"###,
    );
    undo(
        &mut current,
        r###"    is_admitted_concrete_owned_byte_variant, is_admitted_owned_string_variant,
};
pub(crate) fn nested_record_copy_scalar_is_admitted(ty: &ResolvedType) -> bool {
    type_reachability::nested_record_copy_scalar_is_admitted(ty)
}
mod validation;
mod variant_equality;
"###,
        r###"    is_admitted_concrete_owned_byte_variant, is_admitted_owned_string_variant,
};
mod validation;
mod variant_equality;
"###,
    );
    undo(
        &mut current,
        r###"pub(crate) use type_reachability::owned_oracle::owned_reachable_types as owned_reachable_types_oracle;
pub(crate) use type_reachability::{
    authored_nominal_declarations, is_admitted_nested_owned_byte_record, is_flat_owned_byte_record,
    reachable_authored_types, reachable_authored_types_with_roots, reachable_scalar_authored_types,
    BorrowedTypeInventory,
};
pub(crate) use validation::resolved_type_contains_owned_bytes;
#[cfg(test)]
use validation::HirValidator;
pub(crate) use workspace_link::{
    copy_scalar_type, link_https_command_io_workspace, link_https_entry_workspace,
    link_language_command_io_workspace, link_line_command_io_workspace,
    link_network_command_io_workspace, link_network_entry_workspace, link_owned_data_api_workspace,
"###,
        r###"pub(crate) use type_reachability::owned_oracle::owned_reachable_types as owned_reachable_types_oracle;
pub(crate) use type_reachability::{
    BorrowedTypeInventory, authored_nominal_declarations, is_admitted_nested_owned_byte_record,
    is_flat_owned_byte_record, reachable_authored_types, reachable_authored_types_with_roots,
    reachable_scalar_authored_types,
};
#[cfg(test)]
use validation::HirValidator;
pub(crate) use validation::resolved_type_contains_owned_bytes;
pub(crate) use workspace_link::{
    COPY_SCALAR_NAMES, LinkedScalarProjectParts, PACKAGE_SCALAR_NAME, copy_scalar_type,
    link_https_command_io_workspace, link_https_entry_workspace,
    link_language_command_io_workspace, link_line_command_io_workspace,
    link_network_command_io_workspace, link_network_entry_workspace, link_owned_data_api_workspace,
"###,
    );
    undo(
        &mut current,
        r###"    validate_stream_collection_record_program, validate_stream_data_program,
    validate_stream_nested_outcome_program, validate_stream_owned_program,
    validate_stream_record_program, validate_stream_text_program, LinkedScalarProjectParts,
    COPY_SCALAR_NAMES, PACKAGE_SCALAR_NAME,
};

"###,
        r###"    validate_stream_collection_record_program, validate_stream_data_program,
    validate_stream_nested_outcome_program, validate_stream_owned_program,
    validate_stream_record_program, validate_stream_text_program,
};

"###,
    );
    undo(
        &mut current,
        r###"    resolved_scalar_substitutions, same_function_meaning,
};
pub use row_loan::{RowLoan, RowLoanVersion, ROW_LOAN_V1};
pub(crate) use validation::byte_traversal::{
    function_uses as byte_traversal_function_uses, partial_receiver as is_partial_byte_receiver,
"###,
        r###"    resolved_scalar_substitutions, same_function_meaning,
};
pub use row_loan::{ROW_LOAN_V1, RowLoan, RowLoanVersion};
pub(crate) use validation::byte_traversal::{
    function_uses as byte_traversal_function_uses, partial_receiver as is_partial_byte_receiver,
"###,
    );
    undo(
        &mut current,
        r###"    matched
}
pub(crate) use nodes::{
    admitted_owned_byte_prelude_instance, is_refutable_match_scalar, is_scalar_resolved_type,
    LinkedDeclarationFact, LinkedOwnedDataParts, LinkedScalarFunction,
};
pub use nodes::{
    ByteSliceExtent, ByteSliceProvenance, ByteSliceRangeStep, ByteSliceRootKind, Declaration,
"###,
        r###"    matched
}
pub use nodes::{
    ByteSliceExtent, ByteSliceProvenance, ByteSliceRangeStep, ByteSliceRootKind, Declaration,
"###,
    );
    undo(
        &mut current,
        r###"    ResolvedTypeParameterDeclaration, ResolvedVariantCaseDeclaration, ResolvedYieldsClause,
    TypeFacts, VectorFieldProvenance,
};

"###,
        r###"    ResolvedTypeParameterDeclaration, ResolvedVariantCaseDeclaration, ResolvedYieldsClause,
    TypeFacts, VectorFieldProvenance,
};
pub(crate) use nodes::{
    LinkedDeclarationFact, LinkedOwnedDataParts, LinkedScalarFunction,
    admitted_owned_byte_prelude_instance, is_refutable_match_scalar, is_scalar_resolved_type,
};

"###,
    );
    undo(
        &mut current,
        r###"    // The environment proof belongs to this exact current source receiver.
    current: &'a dyn ProgramSource,
    program: &'a dyn ProgramSource,
    resolved: &'a ResolvedProgram,
    costs: &'a BTreeMap<String, usize>,
"###,
        r###"    // The environment proof belongs to this exact current source receiver.
    current: &'a dyn ProgramSource,
    program: &'a Program,
    resolved: &'a ResolvedProgram,
    costs: &'a BTreeMap<String, usize>,
"###,
    );
    undo(
        &mut current,
        r###"        let invalid_oracle = full_verdict(&allocating, None);
        assert_eq!(invalid_oracle.as_ref().unwrap_err()[0].code, "SPX-T252");
        assert!(invalid_oracle.as_ref().unwrap_err()[0]
            .message
            .contains("call `leaf` is not admitted in loop bodies"));
        assert_eq!(
            full_verdict(&allocating, Some((&previous, &old, &costs)))
"###,
        r###"        let invalid_oracle = full_verdict(&allocating, None);
        assert_eq!(invalid_oracle.as_ref().unwrap_err()[0].code, "SPX-T252");
        assert!(
            invalid_oracle.as_ref().unwrap_err()[0]
                .message
                .contains("call `leaf` is not admitted in loop bodies")
        );
        assert_eq!(
            full_verdict(&allocating, Some((&previous, &old, &costs)))
"###,
    );
    undo(
        &mut current,
        r###"#[path = "hir/provisional_result_borrow_tests.rs"]
mod provisional_result_borrow_tests;

mod function_reuse_source;
"###,
        r###"#[path = "hir/provisional_result_borrow_tests.rs"]
mod provisional_result_borrow_tests;
"###,
    );
    assert_eq!(
        current,
        ci_str!("../../../../../../docs/evidence/schema-summary/hir.rs.before")
    );
    let paid_nominal_input = ci_str!("../../../../../hir/declaration_index/flags/paid.rs");
    let paid_nominal_registration = r####"#[path = "../nominal_paid_facts.rs"]
mod nominal_constructor;
"####;
    assert_eq!(
        paid_nominal_input
            .matches(paid_nominal_registration)
            .count(),
        1
    );
    let paid_nominal_original = paid_nominal_input.replacen(paid_nominal_registration, "", 1);
    assert_eq!(paid_nominal_original
        .strip_suffix("\nmod inventory_constructor;\npub(crate) use inventory_constructor::{\n    identity as inventory_identity, substitute as inventory_substitute, InventoryPayer,\n};\n").unwrap(),
        ci_str!("../../../../../../docs/evidence/schema-summary/flags-paid.rs.before"));
    assert_eq!(
        ci_str!("../../../../../hir/declaration_index/flags/paid/key.rs"),
        ci_str!("../../../../../../docs/evidence/schema-summary/flags-paid-key.rs.before")
    );
    assert_eq!(
        ci_str!("../../../../../hir/declaration_index/flags/paid/substitute.rs"),
        ci_str!("../../../../../../docs/evidence/schema-summary/flags-paid-substitute.rs.before")
    );
    assert_eq!(
        ci_str!("../../../../../hir/declaration_index/flags/paid/storage.rs"),
        ci_str!("../../../../../../docs/evidence/schema-summary/flags-paid-storage.rs.before")
    );
}
