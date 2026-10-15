//! Additive discovery control: separately observed genuine production stages.
//! These independent receipts are not summed into the whole-chain observation.
use super::*;
use crate::test_allocation_observer::measure;

#[test]
fn scalar_production_stages_isolate_actual_backing_without_replacing_whole_owner() {
    let ast = crate::check(
        crate::hir::iterative_validator_tests::SOURCE,
        "function-scope-phases.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert!(function.params.is_empty() && function.effects.is_empty());
    assert!(function.requires.is_empty() && function.ensures.is_empty());
    assert!(function.yields.is_none());
    assert_eq!(function.return_type, ResolvedType::I64);
    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        panic!("literal scalar fixture retains ordinary block");
    };
    assert!(statements.is_empty());
    assert!(matches!(tail.kind, ResolvedExprKind::Int(0)));
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    for phase in [
        "lexical",
        "collection",
        "iterator",
        "renewal",
        "yield",
        "return-type",
        "permits",
        "unsafe",
        "lifecycle",
        "value-id",
        "body",
        "result-binding",
    ] {
        // Identical immutable program and ordinary validator setup. Constructor
        // backing stays outside this isolated stage, as in the whole owner.
        let mut validator = HirValidator::new(&program).unwrap();
        let mut scope = scope_checkpoint::Scope::default();
        if phase == "body" {
            // This is the genuine preceding result-ID registration; acquire it
            // before the isolated body, not during its allocation observation.
            validator.insert_value(&function.result_id).unwrap();
        }
        let (((), actual), refused, paid, receipt) =
            crate::bounded_output::with_limit_usage_refusal(64 * 1024 * 1024, || {
                measure(|| {
                    let result = match phase {
                        "lexical" => crate::hir::lexical_exit::validate_function(
                            &program, function, &execution,
                        ),
                        "collection" => crate::hir::declared_collection::validate_function_funded(
                            &program.declarations,
                            function,
                        ),
                        "iterator" => crate::hir::iterator_loop::validate_function_paid(function),
                        "renewal" => owner_renewal::validate_function_paid(
                            &program,
                            validator.byte_helpers,
                            function,
                        ),
                        "yield" => crate::hir::yield_aggregate::check_bytes_request_site_count(
                            &program.declarations,
                            function,
                        ),
                        "return-type" => validator.validate_type(&function.return_type),
                        "permits" => {
                            let permits = validator.functions.permits_borrowed(&program);
                            assert!(permits.contains("host.echo"));
                            drop(permits);
                            Ok(())
                        }
                        "unsafe" => unsafe_scan::contains_unsafe_boundary_paid(&function.body)
                            .map(|unsafe_boundary| assert!(!unsafe_boundary)),
                        "lifecycle" => (|| -> Result<(), Diagnostic> {
                            let effects = lifecycle::declared_effects(&function.effects)?;
                            let mut required = lifecycle::completed::Collector::new(&program);
                            required.extend(&function.return_type)?;
                            crate::hir::inspection::calls::try_visit_resolved_calls(
                                &function.body,
                                &mut |_, _, _| -> Result<(), Diagnostic> {
                                    panic!("literal fixture has no calls")
                                },
                            )?;
                            assert!(effects.is_empty() && required.iter().next().is_none());
                            drop(required);
                            drop(effects);
                            Ok(())
                        })(),
                        "value-id" => validator.insert_value(&function.result_id),
                        "body" => validator.validate_expr_persistent(
                            &execution,
                            &function.body,
                            &mut scope,
                            "body",
                            true,
                            Some(effect_keys::Allowed::Borrowed(&BTreeSet::new())),
                        ),
                        "result-binding" => (|| -> Result<(), Diagnostic> {
                            let returned = validator
                                .expected_ownership(&function.return_type, OwnershipMode::Own)?;
                            let insertion = scope.prepare_insert()?;
                            scope_checkpoint::charge::clone_type(&function.return_type)?;
                            insertion.insert(
                                function.result_id.clone(),
                                ValidationBinding {
                                    contract_local: false,
                                    mutable: false,
                                    ty: function.return_type.clone(),
                                    ownership: returned,
                                    availability: Availability::Available,
                                    active_loans: BTreeSet::new(),
                                    moved_places: BTreeMap::new(),
                                    definitely_partial: BTreeSet::new(),
                                },
                            )
                        })(),
                        _ => unreachable!(),
                    };
                    assert!(result.is_ok(), "isolated admitted production stage");
                    drop(result);
                    drop(scope);
                    drop(validator);
                })
            });
        assert!(!refused && receipt.is_none() && !actual.invalid);
        assert_eq!(actual.live, 0);
        assert_eq!(actual.allocated, actual.freed);
        // Output is outside the observer. A deficit is reported verbatim,
        // never normalized or concealed by credits or constructor forecasts.
        println!(
            "function {} phase {phase}: actual {actual:?}, paid {paid}",
            function.id
        );
    }
}

#[test]
fn complete_whole_chain_allocation_owner_is_retained_byte_exact() {
    use sha2::{Digest, Sha256};
    let current = ci_str!("tests.rs");
    let original =
        ci_str!("../../../../docs/evidence/function-scope-backing-isolation/tests.rs.before.txt");
    assert_eq!(current, original);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "9668364415acfde0f4f9ab2f78e78113332acd4b5926d1ad27a5051759d7cafa"
    );
}

#[test]
fn production_stage_dispatch_preserves_original_with_only_named_permit_and_registration_hooks() {
    let current = ci_str!("../function_scope.rs");
    let guard = "        // An absent postcondition cannot read a result binding. All result\n        // identity, body type and ownership checks above still precede success.\n        if function.ensures.is_empty() {\n            return scope_checkpoint::charge::reserve(Some(0));\n        }\n\n";
    assert_eq!(current.matches(guard).count(), 1);
    let current = current
        .strip_suffix("\n#[cfg(test)]\nmod empty_ensures;\n")
        .unwrap()
        .replacen(guard, "", 1);
    let original = ci_str!(
        "../../../../docs/evidence/function-scope-backing-isolation/function_scope.rs.before.txt"
    );
    assert_eq!(
        current
            .matches("self.functions.permits_borrowed(self.program)")
            .count(),
        1
    );
    assert_eq!(
        current
            .strip_suffix("\n#[cfg(test)]\nmod phase_tests;\n")
            .unwrap()
            .replacen(
                "self.functions.permits_borrowed(self.program)",
                "self.functions.permits(self.program)",
                1
            ),
        original
    );
}
