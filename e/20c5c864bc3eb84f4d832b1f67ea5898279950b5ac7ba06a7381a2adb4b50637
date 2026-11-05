fn complete_original_sources_tail_a() {
    {
        let mut current = super::current_view::project(
            "src/cleanup_plan/replay/factored/oracle_hook.rs",
            include_str!("../../factored/oracle_hook.rs"),
        );
        undo(&mut current, "/// Ordinary storage sharing changes backing only; explicitly opted oracle\n/// routes keep their complete independent account and never borrow this lease.\npub(super) fn cross_check_with_storage<'hir>(\n    program: &'hir dyn crate::cleanup::ReplayContext,\n    function: &'hir ResolvedFunction,\n    budget: &mut ReplayBudget,\n    storage: Option<&mut continuation_storage::Storage<'hir>>,\n) -> Result<(), Diagnostic> {\n    match super::super::allocation::opted() {\n        Ok(None) => match storage {\n            Some(storage) if storage.has_receiving_owner() => {\n                storage.authenticate(program, function, budget)?;\n                super::continuation_receiver::cross_check(program, function, storage);\n            }\n            _ => super::cross_check(program, function),\n        },\n        Ok(Some(lease)) => cross_check_oracle(program, function, lease),\n        Err(refusal) => panic!(\"opted oracle lease is not live: {refusal:?}\"),\n    }\n    Ok(())\n}\n\n", "");
        undo(
            &mut current,
            r###"
// Classification of an undecided result carries no comparison authority.
fn require_decided(
    function: &ResolvedFunction,
    factored: &Result<Option<bool>, Diagnostic>,
) {
    assert!(
        !matches!(factored, Ok(None)),
        "funded factored skeleton comparison is undecided for `{}`",
        function.id,
    );
}

#[cfg(test)]
mod closure_controls;
"###,
            "",
        );
        undo(
            &mut current,
            "        require_decided(function, &factored);\n",
            "",
        );
        undo(
            &mut current,
            r#"            let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
            paid_sort(lease, &mut enumerating, function, &mut hir)?;
            paid_sort(lease, &mut enumerating, function, &mut plan)?;
            same_paths(function, &mut enumerating, &hir, &plan)
        })
    });
    // A fresh factored receiver runs regardless of the enumeration verdict.
"#,
            r#"            let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
            paid_sort(lease, &mut enumerating, function, &mut hir)?;
            paid_sort(lease, &mut enumerating, function, &mut plan)?;
            Ok(hir == plan)
        })
    });
    // A fresh factored receiver runs regardless of the enumeration verdict.
"#,
        );
        undo(
            &mut current,
            r#"/// Stable sort scratch is at most one element slot per path; pay that upper
/// bound on the account before the sort can allocate it.
fn paid_sort(
    _lease: OracleLease,
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    paths: &mut [SkeletonPath],
) -> Result<(), Diagnostic> {
    super::oracle_order::sort(paths, function, budget)
}
fn same_paths(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    a: &[SkeletonPath],
    b: &[SkeletonPath],
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "oracle path vector length")?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (x, y) in a.iter().zip(b) {
        budget.charge(function, 1, "oracle path vector member")?;
        if super::oracle_order::path(x, y, function, budget)? != std::cmp::Ordering::Equal {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn cross_check_oracle(
"#,
            r#"/// Stable sort scratch is at most one element slot per path; pay that upper
/// bound on the account before the sort can allocate it.
fn paid_sort(
    lease: OracleLease,
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    paths: &mut [SkeletonPath],
) -> Result<(), Diagnostic> {
    let bytes = paths.len().checked_mul(std::mem::size_of::<SkeletonPath>());
    if bytes.is_none() {
        lease.fail_physical("skeleton sort scratch size overflowed");
    }
    match bytes.map(|bytes| lease.charge(bytes)) {
        Some(Ok(())) => {
            paths.sort();
            Ok(())
        }
        _ => {
            Err(budget.budget_refusal(function, "oracle account refused skeleton sort scratch", ""))
        }
    }
}

pub(super) fn cross_check_oracle(
"#,
        );
        undo(
            &mut current,
            r#"//! Explicitly financed test comparison hook. Ordinary unopted callers retain
//! the complete original enumerating/factored receiver. Owning tests opt into
//! one independent finite account without consuming their live builder ledger.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleLease};

/// Explicit private-test dispatcher. No compiler or resolver call chooses it.
pub(super) fn cross_check(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) {
    match super::super::allocation::opted() {
        Ok(None) => super::cross_check(program, function),
        Ok(Some(lease)) => cross_check_oracle(program, function, lease),
"#,
            r#"//! Pending test-only funded comparison receiver. Live `factored::cross_check`
//! retains its complete original body: neither actual owner control opts in.
//! Explicit private tests exercise this receiver while the documented transitive
//! acquisition audit remains open. It is not a fully funded live-hook claim.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleLease};

/// Explicit private-test dispatcher. No compiler or resolver call chooses it.
fn cross_check(program: &dyn crate::cleanup::ReplayContext, function: &ResolvedFunction) {
    match super::super::allocation::opted() {
        Ok(None) => super::cross_check(program, function),
        Ok(Some(lease)) => cross_check_oracle(program, function, lease),
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__factored__oracle_hook.rs.before"
            ),
            "src/cleanup_plan/replay/factored/oracle_hook.rs"
        );
    }
    {
        let mut current = include_str!("../../nested_shape/paid.rs").to_owned();
        undo(
            &mut current,
            r#"    }
    Ok(shape)
}

#[cfg(test)]
mod oracle;
"#,
            r#"    }
    Ok(shape)
}
"#,
        );
        undo(
            &mut current,
            r#"    physical: &mut ReplayBudget,
    scratch: &mut Scratch,
) -> Result<FieldLivenessShape, Diagnostic> {
    #[cfg(test)]
    if physical.oracle_lease().is_some() {
        return oracle::derive_reusing(
            program, function, root, next_flag, bounds, physical, scratch,
        );
    }
    let (depth, budget) = bounds;
    physical.reserve_clone(function, root, "independent shape root type")?;
    let mut lease = scratch.lease();
"#,
            r#"    physical: &mut ReplayBudget,
    scratch: &mut Scratch,
) -> Result<FieldLivenessShape, Diagnostic> {
    let (depth, budget) = bounds;
    physical.reserve_clone(function, root, "independent shape root type")?;
    let mut lease = scratch.lease();
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__nested_shape__paid.rs.before"
            ),
            "src/cleanup_plan/replay/nested_shape/paid.rs"
        );
    }
    {
        let mut current = include_str!("../../ordinary_inventory/substitute.rs").to_owned();
        undo(
            &mut current,
            r#"                    index,
                } => {
                    if parameter_owner != owner {
                        return Err(hir_error(
                            function,
                            budget,
                            format_args!(
                                "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                            ),
                        ));
                    }
                    if let Some(argument) = usize::try_from(*index)
                        .ok()
"#,
            r#"                    index,
                } => {
                    if parameter_owner != owner {
                        return Err(hir_error(function, budget, format_args!(
                            "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                        )));
                    }
                    if let Some(argument) = usize::try_from(*index)
                        .ok()
"#,
        );
        undo(
            &mut current,
            r#"                "oracle invocation authority refused a raw diagnostic",
            );
        }
        let result = if matches!(allocation::opted(), Ok(Some(current)) if current == lease) {
            allocation::raw_error_metered(lease, function, message)
        } else {
            allocation::raw_error_funded(lease, message)
        };
        return match result {
            Ok(error) => error,
            Err(refusal) => refusal.into_diagnostic(),
        };
"#,
            r#"                "oracle invocation authority refused a raw diagnostic",
            );
        }
        return match allocation::raw_error_funded(lease, message) {
            Ok(error) => error,
            Err(refusal) => refusal.into_diagnostic(),
        };
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__ordinary_inventory__substitute.rs.before"
            ),
            "src/cleanup_plan/replay/ordinary_inventory/substitute.rs"
        );
    }
    {
        let mut current = include_str!("../../paid_type_facts.rs").to_owned();
        undo(
            &mut current,
            r#"mod inventory_lifecycle;

pub(super) mod inventory_summary;

#[cfg(test)]
pub(super) mod oracle_inventory;
"#,
            r#"mod inventory_lifecycle;

pub(super) mod inventory_summary;
"#,
        );
        undo(
            &mut current,
            r#"        }
        return crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| replay_error(function, message));
    }
    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        return oracle_inventory::needs_drop(program, function, ty, budget);
    }
    let mut payer = Payer {
        function,
"#,
            r#"        }
        return crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| replay_error(function, message));
    }
    let mut payer = Payer {
        function,
"#,
        );
        undo(
            &mut current,
            r#"        }
        return Ok(index.type_flags(ty));
    }
    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        return oracle_inventory::flags(index, function, ty, budget);
    }
    index.type_flags_paid(
        ty,
        Payer {
"#,
            r#"        }
        return Ok(index.type_flags(ty));
    }
    index.type_flags_paid(
        ty,
        Payer {
"#,
        );
        // Exact receiving edits; the complete original source remains the final oracle.
        undo(
            &mut current,
            r###"use crate::hir::{DeclarationIndex, TypeFactsPayer};

pub(super) struct Payer<'a> {
    function: &'a ResolvedFunction,
    budget: &'a mut ReplayBudget,
"###,
            r###"use crate::hir::{DeclarationIndex, TypeFactsPayer};

struct Payer<'a> {
    function: &'a ResolvedFunction,
    budget: &'a mut ReplayBudget,
"###,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts.rs"
        );
    }
    {
        let mut current = include_str!("../../paid_type_facts/inventory_encoded.rs").to_owned();
        undo(
            &mut current,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |_, payer: &mut Payer<'_>| {
                inventory_text::current(payer)?;
                Ok(true)
            },
            &mut work,
            false,
        );
    }
    output.raw_with_work(arguments, payer, &mut pay, &mut work)
}

"#,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.raw_with_work(arguments, payer, &mut pay, &mut work)
}

"#,
        );
        undo(
            &mut current,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |bytes, payer: &mut Payer<'_>| payer.classifier_reserve_sink(bytes),
            &mut work,
            true,
        );
    }
    output.strict_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn raw(
"#,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.strict_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn raw(
"#,
        );
        undo(
            &mut current,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |bytes, payer: &mut Payer<'_>| payer.classifier_reserve_sink(bytes),
            &mut work,
            false,
        );
    }
    output.capped_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn strict(
"#,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.capped_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn strict(
"#,
        );
        undo(
            &mut current,
            r#"pub(super) fn acquire(
    payer: &mut Payer<'_>,
) -> Result<ClassifierString, GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return ClassifierString::acquire_oracle_work(payer, &mut pay, &mut work);
    }
    ClassifierString::acquire_with_work(payer, &mut pay, &mut work)
}
pub(super) fn capped(
"#,
            r#"pub(super) fn acquire(
    payer: &mut Payer<'_>,
) -> Result<ClassifierString, GrowthFailure<Diagnostic>> {
    ClassifierString::acquire_with_work(payer, &mut pay, &mut work)
}
pub(super) fn capped(
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts__inventory_encoded.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts/inventory_encoded.rs"
        );
    }
    {
        let mut current =
            include_str!("../../paid_type_facts/inventory_encoded/tests.rs").to_owned();
        undo(
            &mut current,
            r###"}
#[test]
fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    let current = include_str!("../../../../bounded_output/classifier_string.rs")
        .strip_suffix("\n#[cfg(test)]\nmod oracle_work;\n")
        .unwrap();
    let marker = "    /// A test-account caller authenticates its own invocation separately. This\n    /// detached marker contains no ACTIVE handle and grants no ordinary payer.\n    #[cfg(test)]\n    pub(crate) fn oracle_marker() -> Self {\n        Self(None)\n    }\n\n";
    assert_eq!(current.matches(marker).count(), 1);
    let current = current.replace(marker, "");
    assert_eq!(
        current.strip_suffix("\nmod inventory_work;\n").unwrap(),
        include_str!("../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}
"###,
            r#"}
#[test]
fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    assert_eq!(
        include_str!("../../../../../bounded_output/classifier_string.rs")
            .strip_suffix("\nmod inventory_work;\n")
            .unwrap(),
        include_str!("../../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}
"#,
        );
        // Exact receiving edits; the complete original source remains the final oracle.
        undo(
            &mut current,
            r###"fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    assert_eq!(
        include_str!("../../../../../bounded_output/classifier_string.rs")
            .strip_suffix("\nmod inventory_work;\n")
            .unwrap(),
        include_str!("../../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}
"###,
            r###"fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    assert_eq!(
        include_str!("../../../../bounded_output/classifier_string.rs")
            .strip_suffix("\nmod inventory_work;\n")
            .unwrap(),
        include_str!("../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}
"###,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts__inventory_encoded__tests.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts/inventory_encoded/tests.rs"
        );
    }
    {
        let mut current = super::current_view::project(
            "src/cleanup_plan/replay/paid_type_facts/inventory_tests.rs",
            include_str!("../../paid_type_facts/inventory_tests.rs"),
        );
        // Undo only the unique visibility hook before the complete old receiver.
        undo(
            &mut current,
            r###"    assert_eq!(current.matches("pub(super) struct Payer<'a> {").count(), 1);
    let current = current.replacen(
        "pub(super) struct Payer<'a> {",
        "struct Payer<'a> {",
        1,
    );
"###,
            "",
        );
        undo(
            &mut current,
            r###"#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../paid_type_facts.rs");
    let current = current
        .strip_suffix("\n#[cfg(test)]\npub(super) mod oracle_inventory;\n")
        .unwrap();
    let current = current.replace(
        "    #[cfg(test)]\n    if budget.oracle_lease().is_some() {\n        return oracle_inventory::flags(index, function, ty, budget);\n    }\n",
        "",
    );
    let current = current.replace(
        "    #[cfg(test)]\n    if budget.oracle_lease().is_some() {\n        return oracle_inventory::needs_drop(program, function, ty, budget);\n    }\n",
        "",
    );
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
        .unwrap();
"###,
            r#"#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../../paid_type_facts.rs");
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
        .unwrap();
"#,
        );
        // Exact receiving edits; the complete original source remains the final oracle.
        undo(
            &mut current,
            r###"#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../../paid_type_facts.rs");
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
"###,
            r###"#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../paid_type_facts.rs");
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
"###,
        );
        // Exact composed receiving hooks; reconstruct every original byte below.
        undo(
            &mut current,
            r###"    for program in tests::fixtures() {
        for function in &program.functions {
            let (results, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                budget.prepare_builder(function).unwrap();
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                    .unwrap();
"###,
            r###"    for program in tests::fixtures() {
        for function in &program.functions {
            let mut budget = ReplayBudget::new();
            let (results, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                    .unwrap();
"###,
        );
        undo(
            &mut current,
            r###"    assert!(adapter.contains("text::raw(payer, message)"));
}

// The complete prior setup/body stays compiled against its original pure bool
// receiver. It grants no backing authority and makes no funded-owner claim.
mod original_raw_owner {
    use super::*;
    fn inventory_needs_drop(
        program: &dyn crate::cleanup::ReplayContext,
        ty: &ResolvedType,
        _payer: &mut Payer<'_>,
    ) -> Result<bool, Diagnostic> {
        crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| Diagnostic::io("SPX-H006", message))
    }
    #[test]
    fn raw_inventory_classifier_keeps_complete_original_diagnostic_fields() {
        for program in tests::fixtures() {
            for function in &program.functions {
                let mut budget = ReplayBudget::new();
                let (results, refused, _) =
                    crate::bounded_output::with_limit_usage(usize::MAX, || {
                        reserve_program_skeleton_work(
                            &program,
                            std::iter::once(function),
                            &mut budget,
                        )
                        .unwrap();
                        let mut results = Vec::new();
                        for ty in function
                            .params
                            .iter()
                            .map(|p| &p.ty)
                            .chain(std::iter::once(&function.return_type))
                        {
                            results.push((
                                crate::cleanup::type_needs_resource_cleanup(&program, ty)
                                    .map_err(|m| Diagnostic::io("SPX-H006", m)),
                                inventory_needs_drop(
                                    &program,
                                    ty,
                                    &mut tests::payer(function, &mut budget),
                                ),
                            ));
                        }
                        results
                    });
                assert!(!refused);
                for (old, current) in results {
                    assert_eq!(current, old);
                }
            }
        }
    }
}

#[test]
fn complete_prior_raw_and_zero_work_receivers_and_identity_formatter_remain_source_bound() {
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/evidence/paid-classifier-receiving-f8/compiled-original-projection.json"
    ))
    .unwrap();
    let body = proof["compiled_raw_body"].as_str().unwrap();
    assert!(include_str!("inventory_tests.rs").contains(body));
    // Invert only the enclosing module's four-space indentation, then the
    // one complete callback reflow (including rustfmt's trailing call comma).
    let mut projected = body
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    projected.push('\n');
    let reflow = proof["reflow_before"].as_str().unwrap();
    assert_eq!(projected.matches(reflow).count(), 1);
    projected = projected.replacen(reflow, proof["reflow_after"].as_str().unwrap(), 1);
    projected.push('\n');
    assert_eq!(
        projected,
        include_str!(
            "../../../../docs/evidence/paid-classifier-receiving-f8/raw-inventory-owner.before.txt"
        )
    );
    let current = include_str!("../allocation/oracle/tests.rs");
    let old = include_str!(
        "../../../../docs/evidence/paid-classifier-receiving-f8/oracle-fast-owner.before.txt"
    );
    // The complete old zero-backing owner uses the declared prior receiver;
    // the independent funded current owner remains on its real production path.
    let dispatch = "                let received = nested_shape::type_needs_drop(&program, function, ty).unwrap();";
    let original_dispatch = "                let received = crate::cleanup_plan::replay::nested_shape::type_needs_drop(\n                    &program, function, ty,\n                )\n                .unwrap();";
    assert_eq!(current.matches(dispatch).count(), 1);
    let current = current.replacen(dispatch, original_dispatch, 1);
    let alias = "    use original_fast_type_probe as nested_shape;\n";
    assert_eq!(current.matches(alias).count(), 1);
    assert!(current.replacen(alias, "", 1).contains(old));
    let current =
        include_str!("../../../hir/declaration_index/flags/paid/inventory_constructor.rs");
    let hook = "    // A logical sink can select the existing sticky refusal without returning\n    // a physical error. Authenticate before publishing only a successful value;\n    // Storage::work leaves an already selected first error untouched.\n    let _ = storage.work(0, \"inventory identity receiving authority\");\n";
    assert_eq!(current.matches(hook).count(), 1);
    assert_eq!(
        current.replacen(hook, "", 1),
        include_str!(
        "../../../../docs/evidence/paid-classifier-receiving-f8/inventory-constructor.before.txt"
    )
    );
}
"###,
            r###"    assert!(adapter.contains("text::raw(payer, message)"));
}
"###,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts__inventory_tests.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts/inventory_tests.rs"
        );
    }
    {
        let mut current = include_str!("../../skeleton_work/oracle_entry.rs").to_owned();
        undo(
            &mut current,
            r#"                Err(error) => return Err(refused(budget, function, error)),
            }
        };
        let string_owner_moves =
            match crate::literal_format::checked_moving_string_operands_funded_with_work(
                function, &mut units, &mut pay,
            ) {
                Ok(moves) => moves,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let string_condition_reads =
            match crate::string_ops::conditions::function_reads_funded_with_work(
                function, &mut units, &mut pay,
            ) {
                Ok(reads) => reads,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let requires = match crate::hir::lexical_exit::requires_funded_with_work(
            function, &mut pay, &mut units,
        ) {
            Ok(requires) => requires,
            Err(error) => return Err(refused(budget, function, error)),
        };
        // A semantic missing or ambiguous owner is the original absence; a
        // payer refusal never is.
        let owner = if requires {
            match crate::hir::lexical_exit::owner_funded_with_work(
                program, function, &mut pay, &mut units,
            ) {
                Ok(Ok(owner)) => Some(owner),
                Ok(Err(_)) => None,
                Err(error) => return Err(refused(budget, function, error)),
"#,
            r#"                Err(error) => return Err(refused(budget, function, error)),
            }
        };
        let string_owner_moves = match crate::literal_format::checked_moving_string_operands_funded(
            function, &mut pay,
        ) {
            Ok(moves) => moves,
            Err(error) => return Err(refused(budget, function, error)),
        };
        let string_condition_reads =
            match crate::string_ops::conditions::function_reads_funded(function, &mut pay) {
                Ok(reads) => reads,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let requires = match crate::hir::lexical_exit::requires_funded(function, &mut pay) {
            Ok(requires) => requires,
            Err(error) => return Err(refused(budget, function, error)),
        };
        // A semantic missing or ambiguous owner is the original absence; a
        // payer refusal never is.
        let owner = if requires {
            match crate::hir::lexical_exit::owner_funded(program, function, &mut pay) {
                Ok(Ok(owner)) => Some(owner),
                Ok(Err(_)) => None,
                Err(error) => return Err(refused(budget, function, error)),
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__skeleton_work__oracle_entry.rs.before"
            ),
            "src/cleanup_plan/replay/skeleton_work/oracle_entry.rs"
        );
    }
    {
        let mut current = super::current_view::before_production_promotion(
            "src/hir/function_value.rs",
            include_str!("../../../../hir/function_value.rs"),
        );
        undo(
            &mut current,
            r#"    };
    Ok((&INVOKE_ID, args, invocation_params(expression)?))
}

#[cfg(test)]
pub(crate) use funded::work::{walk_funded_paying_with_work, walk_funded_with_work};
"#,
            r#"    };
    Ok((&INVOKE_ID, args, invocation_params(expression)?))
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__function_value.rs.before"
            ),
            "src/hir/function_value.rs"
        );
    }
}
