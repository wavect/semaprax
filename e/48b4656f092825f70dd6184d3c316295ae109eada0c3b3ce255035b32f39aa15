use super::*;
use crate::test_allocation_observer::measure;
fn fixture() -> ResolvedProgram {
    let source = crate::check(
        r#"module loan.storage;
@id("packet") record Packet { @id("payload") payload:Bytes, @id("sibling") sibling:Bytes, }
@id("first") fn first(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.payload);
 byte_len(view)+byte_len(view)+byte_len(view)+byte_len(view)+byte_len(view)
}
@id("second") fn second(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.sibling); byte_len(view)+byte_len(view)
}
@id("identity") fn identity<T>(value:T)->T {value}
@id("main") fn main()->i64 {identity<i64>(0)}
"#,
        "canonical-loan-storage.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
#[test]
fn current_shared_owned_and_unobserved_custody_keep_complete_original_ids_liveness_origins() {
    let program = fixture();
    let expected = Inventory::rebuild(&program).unwrap();
    let (current, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let seed = constructor_seed::ConstructorSeed::authenticate(&program, None).unwrap();
        Lookup::current(&seed).unwrap()
    });
    assert!(!refused);
    assert!(matches!(current, Lookup::Owned(_)));
    for (key, id) in &expected.ids {
        assert_eq!(current.id(key), Some(id));
    }
    for (key, index) in &expected.liveness {
        assert_eq!(current.live(key), Some(index));
        assert_eq!(current.origin(*index), expected.origins.get(*index));
    }
    let seed = constructor_seed::ConstructorSeed::authenticate(&program, None).unwrap();
    let unobserved = Lookup::current(&seed).unwrap();
    assert!(matches!(unobserved, Lookup::UnfundedShared(..)));
    for owner in [&current, &unobserved] {
        let ((same, metrics), refused, paid) = crate::bounded_output::with_limit_usage(0, || {
            measure(|| {
                let cloned = owner.clone();
                let shared = owner.share().unwrap();
                let same = cloned.is_current(&program) && shared.is_current(&program);
                drop(cloned);
                drop(shared);
                same
            })
        });
        assert!(same && !refused && !metrics.invalid);
        assert_eq!(paid, 0);
        assert_eq!(metrics.allocated, 0);
        assert_eq!(metrics.live, 0);
    }
    let legacy = Lookup::Legacy(expected);
    let (((), metrics), _, _) =
        crate::bounded_output::with_limit_usage(usize::MAX, || measure(|| drop(legacy.clone())));
    assert!(metrics.allocated > 0 && !metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
}
#[test]
fn complete_active_compatibility_constructor_is_paid_through_drop_exact_short_zero_and_sticky() {
    let program = fixture();
    let observe = |limit| {
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let result = HirValidator::new(&program);
                let ok = result.is_ok();
                drop(result);
                if !ok {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                ok
            })
        })
    };
    let ((ok, metrics), refused, paid) = observe(usize::MAX);
    assert!(ok && !refused && !metrics.invalid);
    assert!(metrics.allocated <= paid && metrics.highwater <= paid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    let ((ok, metrics), refused, used) = observe(paid);
    assert!(ok && !refused && !metrics.invalid);
    assert_eq!(used, paid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    for limit in [0, paid - 1] {
        let ((ok, metrics), refused, used) = observe(limit);
        assert!(!ok && refused && used <= limit && !metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}
#[test]
fn original_compatibility_constructor_and_current_seed_order_keep_full_diagnostics_and_transplant_refusal(
) {
    let program = fixture();
    let old = HirValidator::original_with_directory(&program, None).unwrap();
    assert!(matches!(old.canonical_loans, Lookup::Legacy(_)));
    let current = HirValidator::new(&program).unwrap();
    assert!(matches!(
        current.canonical_loans,
        Lookup::UnfundedShared(..)
    ));
    let other = program.clone();
    let old = HirValidator::original_with_profiles(
        &other,
        None,
        None,
        current.executable_profiles.clone(),
    )
    .err()
    .unwrap();
    let now = HirValidator::with_profiles(&other, None, None, current.executable_profiles.clone())
        .err()
        .unwrap();
    assert_eq!(now, old);
    let mismatch = HirValidator::with_profiles(
        &other,
        None,
        current.canonical_loans.share(),
        current.executable_profiles.clone(),
    )
    .err()
    .unwrap();
    assert_eq!(
        mismatch,
        hir_error("canonical shared-loan storage does not belong to the current program")
    );
    let mut bad = program.clone();
    bad.entrypoint = DeclarationId::new("bad\0identity");
    assert_eq!(
        HirValidator::new(&bad).err().unwrap(),
        HirValidator::original_with_directory(&bad, None)
            .err()
            .unwrap()
    );
}
#[test]
fn attached_loan_drift_is_not_new_constructor_authority_and_whole_saved_receiver_remains_complete()
{
    let mut program = fixture();
    let function = program
        .functions
        .iter_mut()
        .find(|f| !f.loan_plan.loans.is_empty())
        .unwrap();
    function.loan_plan.loans[0].origin.projections.clear();
    // The inner receiver derives fresh canonical facts and deliberately does
    // not trust or compare the attached plan. Preserve that complete old
    // acceptance independently, then compose its original outer plan finish.
    let inner = HirValidator::original_with_directory(&program, None)
        .unwrap()
        .validate();
    assert!(inner.is_ok());
    let old =
        inner.and_then(|selected| crate::loan_plan::validate_program(&program).map(|()| selected));
    let current = crate::bounded_output::with_limit_usage(usize::MAX, || {
        crate::hir::validate_with_static_text(&program)
    })
    .0;
    assert!(old.is_err());
    assert_eq!(current, old);
}

#[test]
fn complete_old_constructor_inventory_caller_and_closure_source_projections_remain_exact() {
    let current = ci_str!("../canonical_loans.rs");
    let original=ci_str!("../../../../docs/evidence/compatibility-loan-custody/src__hir__validation__canonical_loans.rs.before");
    let marker = "// The ordinary compatibility constructor retains the saved owned directories.";
    assert_eq!(
        current.split_once(marker).unwrap().0,
        original.split_once(marker).unwrap().0
    );
    let mut current = super::super::function_lookup::paid_custody::project_current(
        "src/hir/validation.rs",
        ci_str!("../../validation.rs"),
    );
    // The later held-call receipt is inverted before the complete saved custody comparison.
    fn undo(source: &mut String, new: &str, old: &str) {
        assert_eq!(
            source.matches(new).count(),
            1,
            "exact unique held-call edit"
        );
        *source = source.replacen(new, old, 1);
    }
    undo(
        &mut current,
        r###"                                (params, return_type)
                            };
                            let params = if let Some(parameters) = borrowed_parameters {
                                held_parameters.map_or(
                                    CallParameters::Borrowed(parameters),
                                    CallParameters::Held,
                                )
                            } else {
                                intrinsic_parameters.unwrap_or(CallParameters::Owned(params))
                            };
"###,
        r###"                                (params, return_type)
                            };
                            let params = if let Some(parameters) = borrowed_parameters {
                                CallParameters::Borrowed(parameters)
                            } else {
                                intrinsic_parameters.unwrap_or(CallParameters::Owned(params))
                            };
"###,
    );
    undo(
        &mut current,
        r###"                                    call_signature::ResultType::Owned(op.return_type()),
                                )
                            } else {
                                let retained_target = if self.executable_profiles.is_bound() {
                                    held_parameters =
                                        self.executable_profiles.call(callee, instance.as_ref());
                                    held_parameters.map(|call| call.function())
                                } else {
                                    retained_program.resolve_call_target(callee, instance.as_ref())
                                };
                                let (params, return_type, target_effects) = if let Some(target) =
                                    retained_target
                                {
                                    borrowed_parameters = Some(target.params.as_slice());
                                    (
"###,
        r###"                                    call_signature::ResultType::Owned(op.return_type()),
                                )
                            } else {
                                let (params, return_type, target_effects) = if let Some(target) =
                                    retained_program.resolve_call_target(callee, instance.as_ref())
                                {
                                    borrowed_parameters = Some(target.params.as_slice());
                                    (
"###,
    );
    undo(
        &mut current,
        r###"                            }
                            let mut intrinsic_parameters = None;
                            let mut borrowed_parameters = None;
                            let mut held_parameters = None;
                            let retained_program = self.program;
                            let (params, return_type) = if let Some(signature) =
                                self.intrinsic_signature(callee, type_arguments, instance, args)?
"###,
        r###"                            }
                            let mut intrinsic_parameters = None;
                            let mut borrowed_parameters = None;
                            let retained_program = self.program;
                            let (params, return_type) = if let Some(signature) =
                                self.intrinsic_signature(callee, type_arguments, instance, args)?
"###,
    );
    undo(
        &mut current,
        r###"                Some(_) => return Err(hir_error_static(
                    "executable profile observations do not belong to the current declaration index",
                )),
                None => executable_profiles::Lookup::for_program(seed.program)?,
            },
            functions: seed.functions,
            expression_ids: identity_directory::Set::new(),
"###,
        r###"                Some(_) => return Err(hir_error_static(
                    "executable profile observations do not belong to the current declaration index",
                )),
                None => executable_profiles::Lookup::new(&seed.program.declarations)?,
            },
            functions: seed.functions,
            expression_ids: identity_directory::Set::new(),
"###,
    );
    undo(
        &mut current,
        r###"mod executable_profiles;
mod generic_record_composition;
mod generic_template;
mod held_profiles;
mod host_command;
mod lexical_exit;
mod native_borrow;
mod nominal_facts;
mod owned_buffer;
mod owned_result_try;
pub(super) mod owner_renewal;
mod ownership_flags;
mod proof_return;
mod record_pattern;
mod record_traversal;
"###,
        r###"mod executable_profiles;
mod generic_record_composition;
mod generic_template;
mod host_command;
mod lexical_exit;
mod native_borrow;
mod nominal_facts;
mod ownership_flags;
mod owned_buffer;
mod owned_result_try;
pub(super) mod owner_renewal;
mod proof_return;
mod record_pattern;
mod record_traversal;
"###,
    );
    let projected=current.replacen("let canonical_loans = canonical_loans::Lookup::current(&seed)?;\n        Self::from_constructor(seed, canonical_loans, None)","let canonical_loans = canonical_loans::Inventory::rebuild(program)?;\n        Self::from_constructor(seed, canonical_loans::Lookup::Legacy(canonical_loans), None)",1)
        .replacen("storage: Option<canonical_loans::Lookup<'a>>", "storage: Option<&'a super::canonical_loan_storage::Storage<'a>>",1)
        .replacen("            storage\n        } else {\n            canonical_loans::Lookup::current(&seed)?", "            canonical_loans::Lookup::Borrowed(storage)\n        } else {\n            canonical_loans::Lookup::Legacy(canonical_loans::Inventory::rebuild(program)?)",1);
    assert_eq!(
        projected,
        ci_str!(
            "../../../../docs/evidence/compatibility-loan-custody/src__hir__validation.rs.before"
        )
    );
    assert_eq!(super::super::function_lookup::paid_custody::project_current("src/hir/validation/closure.rs", ci_str!("../closure.rs")).replacen("self.canonical_loans.share()", "self.canonical_loans.shared()",1),
        ci_str!("../../../../docs/evidence/compatibility-loan-custody/src__hir__validation__closure.rs.before"));
    let old = ci_str!("constructors_before.rs");
    let directory = old
        .split_once("    #[cfg(test)]\n")
        .unwrap()
        .1
        .split_once("    pub(super) fn original_with_profiles(")
        .unwrap()
        .0;
    let directory = "    #[cfg(test)]\n".to_owned() + directory;
    let directory = directory.replacen("original_with_directory(", "with_directory(", 1);
    let archive = ci_str!(
        "../../../../docs/evidence/compatibility-loan-custody/src__hir__validation.rs.before"
    );
    let start = archive
        .find("    #[cfg(test)]\n    pub(super) fn with_directory(")
        .unwrap();
    let end = archive[start..]
        .find("    pub(in crate::hir) fn with_storage(")
        .unwrap()
        + start;
    assert_eq!(directory, &archive[start..end]);
    let saved = ci_str!("constructors_before.rs");
    let (_, tail) = saved
        .split_once("    pub(super) fn original_with_profiles(")
        .unwrap();
    let profiles = "    fn with_profiles(".to_owned() + tail.strip_suffix("}\n").unwrap();
    let start = archive.find("    fn with_profiles(").unwrap();
    let end = archive[start..].find("    fn from_constructor(").unwrap() + start;
    assert_eq!(profiles, &archive[start..end]);
    let old=ci_str!("../../canonical_loan_storage/caller/owned_before.rs").strip_prefix("//! Complete original paid preparation receiver, independently compiled.\nuse super::*;\n").unwrap().replacen("pub(super) fn prepare_original<'a>(","fn prepare_with_comparison<'a>(",1);
    let archive=ci_str!("../../../../docs/evidence/compatibility-loan-custody/src__hir__canonical_loan_storage__caller.rs.before");
    let start = archive.find("fn prepare_with_comparison<'a>(").unwrap();
    let end = archive[start..].find("\nfn append(").unwrap() + start;
    assert_eq!(old, &archive[start..end]);
}

// PUBLIC_BOUNDARY_SOURCE_INVERSE_GUARD
#[test]
fn complete_public_boundary_composition_preserves_original_owner_and_kat_source_bytes() {
    let records: serde_json::Value = serde_json::from_str(ci_str!(
        "../../../../docs/evidence/public-boundary-fixture-repair/exact-source-inverses.json"
    ))
    .unwrap();
    let sources = [
        (ci_str!("ownership_tests.rs"), ci_str!("../../../../docs/evidence/public-boundary-fixture-repair/ownership-tests.before.txt")),
        (ci_str!("../../../../tests/workspace/semantic_change.rs"), ci_str!("../../../../docs/evidence/public-boundary-fixture-repair/tests__workspace__semantic_change.rs.before.txt")),
        (ci_str!("../../../../tests/workspace/semantic_structural_change.rs"), ci_str!("../../../../docs/evidence/public-boundary-fixture-repair/tests__workspace__semantic_structural_change.rs.before.txt")),
        (ci_str!("../../../../tests/workspace/semantic_change/reporting.rs"), ci_str!("../../../../docs/evidence/public-boundary-fixture-repair/tests__workspace__semantic_change__reporting.rs.before.txt")),
        (ci_str!("../../../../tests/workspace/semantic_structural_change/reporting.rs"), ci_str!("../../../../docs/evidence/public-boundary-fixture-repair/tests__workspace__semantic_structural_change__reporting.rs.before.txt")),
    ];
    assert_eq!(records.as_array().unwrap().len(), sources.len());
    for (ordinal, (current, original)) in sources.into_iter().enumerate() {
        let projected;
        let current = if ordinal == 0 {
            projected = super::super::function_lookup::paid_custody::project_current(
                "src/hir/validation/canonical_loans/ownership_tests.rs",
                current,
            );
            projected
                .split_once("\n// PUBLIC_BOUNDARY_SOURCE_INVERSE_GUARD\n")
                .unwrap()
                .0
        } else {
            current
        };
        let mut restored = current.to_owned();
        for edit in records[ordinal]["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            let before = edit["before"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1, "unique named inverse");
            restored = restored.replacen(after, before, 1);
        }
        assert_eq!(
            restored, original,
            "complete original source owner {ordinal}"
        );
    }
}
