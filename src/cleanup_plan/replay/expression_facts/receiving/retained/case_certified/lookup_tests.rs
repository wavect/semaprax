use super::*;
use crate::cleanup_plan::replay::*;

fn checked_program(rows: usize) -> ResolvedProgram {
    let mut text = String::from("module retained.rows; @id(\"rows.add\") fn add(x:i64)->i64 requires x>=0 ensures result>=0 {x+1} @id(\"rows.main\") fn main()->i64 {");
    for n in 0..rows {
        text.push_str(&format!("let v{n}=add(0)+3-2*4/2%3;"));
    }
    text.push_str("-1}");
    crate::hir::resolve(&crate::check(&text, "retained-rows.spx").unwrap()).unwrap()
}
fn ordinary(
    program: &ResolvedProgram,
    old: bool,
    bytes: usize,
) -> (Result<(), Diagnostic>, bool, usize) {
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    crate::bounded_output::with_limit_usage(bytes, || {
        let mut budget = ReplayBudget::new();
        // Production reserves the complete skeleton allowance before calling this inner receiver.
        // Keep that real preflight inside the same measured payer for both independent arms.
        reserve_program_skeleton_work(program, std::iter::once(function), &mut budget)?;
        if old {
            lookup_caller_before::validate_structure_with_budget_storage(
                program,
                function,
                &mut budget,
                None,
                None,
            )
        } else {
            validate_structure_with_budget_storage(program, function, &mut budget, None, None)
        }
    })
}
// Complete immediate old helper: intentionally lacks the production preflight.
fn ordinary_without_skeleton_preflight(
    program: &ResolvedProgram,
    old: bool,
    bytes: usize,
) -> (Result<(), Diagnostic>, bool, usize) {
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    crate::bounded_output::with_limit_usage(bytes, || {
        let mut budget = ReplayBudget::new();
        if old {
            lookup_caller_before::validate_structure_with_budget_storage(
                program,
                function,
                &mut budget,
                None,
                None,
            )
        } else {
            validate_structure_with_budget_storage(program, function, &mut budget, None, None)
        }
    })
}
#[test]
fn complete_immediate_old_ordinary_caller_preserves_results_and_funds_real_dense_route() {
    let program = checked_program(64);
    let old = ordinary(&program, true, 64 * 1024 * 1024);
    let current = ordinary(&program, false, 64 * 1024 * 1024);
    assert_eq!(current.0, old.0);
    assert!(current.0.is_ok() && !old.1 && !current.1);
    assert!(
        current.2 < old.2,
        "whole caller must save after ALL owner/selector/query costs"
    );
    let exact = ordinary(&program, false, current.2);
    assert!(exact.0.is_ok() && !exact.1);
    for bytes in [0, current.2 - 1] {
        let short = ordinary(&program, false, bytes);
        assert!(short.0.is_err() && short.1);
    }
}
#[test]
#[allow(
    clippy::useless_conversion,
    reason = "The complete saved control retains its original expressions for independent parity checks."
)]
fn earlier_schema_error_and_query_free_prefix_keep_all_six_fields_and_debits() {
    let mut program = checked_program(64);
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.name == "main")
        .unwrap();
    function.cleanup_plan.schema = "foreign.schema".into();
    assert_eq!(
        ordinary(&program, false, 64 * 1024 * 1024),
        ordinary(&program, true, 64 * 1024 * 1024)
    );
    let scalar = checked_program(0);
    // Generation-specific #885 zero-query fee claim: #892 separately changes
    // ordinary setup. Keep this entire original assertion on the complete
    // immediate old HIR caller; current scalar semantics/costs have a mirror in
    // lookup_tests::receiving's independent 0/1/many composition control.
    crate::cleanup_plan::replay::ordinary_receiving::with_before(|| {
        assert_eq!(
            ordinary(&scalar, false, 64 * 1024 * 1024),
            ordinary(&scalar, true, 64 * 1024 * 1024)
        );
    });
}
#[test]
fn retained_first_frontier_has_no_new_owner_before_original_row_guard() {
    let current = include_str!("../case_certified.rs");
    let guard = current
        .find("if !prefix_owner.insert(function, &source.id, budget)?")
        .unwrap();
    let receive = current
        .find("retained status fact receiving assignment")
        .unwrap();
    let selection = current.find("retained_lookup::SELECT_BYTES").unwrap();
    let acquire = current.find("retained_lookup::complete_rows(").unwrap();
    assert!(guard < receive && receive < selection && selection < acquire);
    let closed_current = include_str!("retained_lookup.rs");
    // Preserve the entire original returning-ABI guard on its registered whole
    // image; independently bind the current output/error-slot continuation.
    let closed = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs",
        "4444834e368473fb3a213c900280cd7777a93fc64c423f88ca9ffe3985199e3d",
        closed_current,
    );
    assert!(closed_current
        .contains("session.fact_into(&source.id.expression, &mut received, &mut failure)"));
    assert!(closed_current.contains("Session::into_query_bytes()"));
    assert!(closed_current.contains("Failure::acquisition_bytes()"));
    assert!(closed_current.contains("prefix_owner.insert(function, &source.id, budget)?"));
    assert!(closed_current.contains("retained status fact receiving assignment"));
    for forbidden in [
        "with_limit(",
        "with_limit_usage(",
        "ACTIVE.with",
        "FnMut",
        "dyn Fn",
        "RefCell",
        "guard_bank",
    ] {
        assert!(
            !closed_current.contains(forbidden),
            "current closed continuation cannot admit {forbidden}"
        );
        assert!(
            !closed.contains(forbidden),
            "closed continuation cannot admit {forbidden}"
        );
    }
    assert!(closed.contains("prefix_owner.insert(function, &source.id, budget)?"));
    assert!(closed.contains("session.fact(&source.id.expression)?"));
    assert!(closed.contains("retained status fact receiving assignment"));
    let owner = include_str!("../../../arena/retained.rs");
    assert!(!owner.contains("*const ReplayBudget"));
    assert!(!owner.contains("RefCell"));
    assert!(owner.contains("budget: &'budget mut ReplayBudget"));
    assert!(owner.contains("RESULTS + Self::query_bytes()"));
}

#[test]
fn original_exact_owner_guards_reject_nested_ledger_foreign_budget_and_stale_function() {
    let program = checked_program(32);
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    let mut preparation = ReplayBudget::unobserved();
    let facts =
        crate::cleanup_plan::replay::expression_facts::with_budget(function, &mut preparation)
            .unwrap();
    for mode in 0..3 {
        let observe = |old| {
            crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
                let mut budget = ReplayBudget::new();
                let proof =
                    case_proof::StatusCases::validate(&program, function, &mut budget).unwrap();
                let mut foreign_budget = ReplayBudget::new();
                let stale_function = function.clone();
                let mut invoke = || {
                    let target = if mode == 2 { &stale_function } else { function };
                    let budget = if mode == 1 {
                        &mut foreign_budget
                    } else {
                        &mut budget
                    };
                    let result = if old {
                        lookup_before::validate_status_sources_certified(
                            target, &facts, budget, &proof,
                        )
                    } else {
                        validate_status_sources_certified(target, &facts, budget, &proof)
                    };
                    result
                        .err()
                        .expect("source-bound original guard rejects before lookup acquisition")
                };
                if mode == 0 {
                    crate::bounded_output::with_limit_usage(64 * 1024 * 1024, invoke).0
                } else {
                    invoke()
                }
            })
        };
        assert_eq!(
            observe(false),
            observe(true),
            "all six fields and exact prior debit for guard {mode}"
        );
    }
}

#[test]
fn complete_caller_success_and_refusal_drop_remain_observed_at_unchanged_cap() {
    let program = checked_program(32);
    for bytes in [0, 4096, 64 * 1024 * 1024] {
        // Construct before the measured window; Drop prints only after full
        // value/observer teardown, also when the unchanged assertion unwinds.
        let census =
            crate::cleanup_plan::replay::ordinary_plan_history::PairScopeCensus::whole_caller(
                bytes,
            );
        let (_, metrics) = crate::test_allocation_observer::measure(|| {
            let result = ordinary(&program, false, bytes);
            census.observed(result.0.is_ok(), result.1);
            assert_eq!(
                result.0.is_ok(),
                bytes == 64 * 1024 * 1024,
                "unchanged32-row whole caller at bytes={bytes}: diagnostic={:?}, refused={}, consumed={}",
                result.0,
                result.1,
                result.2,
            );
            drop(result);
        });
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
        drop(census);
    }
}

#[test]
fn complete_caller_without_skeleton_preflight_keeps_exact_root_refusal_and_drop() {
    let program = checked_program(32);
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    for old in [false, true] {
        let (_, metrics) = crate::test_allocation_observer::measure(|| {
            let result = ordinary_without_skeleton_preflight(&program, old, 64 * 1024 * 1024);
            assert_eq!(
                result.0,
                Err(replay_error(
                    function,
                    "cleanup replay work budget exhausted during HIR root path",
                ))
            );
            assert!(
                !result.1,
                "work refusal must not be relabeled as a byte refusal"
            );
            drop(result);
        });
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}

#[test]
fn complete_old_prefix_dispatch_and_ordinary_caller_remain_source_bound() {
    let current = include_str!("../../../arena/prefix.rs");
    let current = current
        .split("\n#[cfg(test)]\n#[path = \"prefix/retained_before.rs\"]")
        .next()
        .unwrap();
    let strip_test_modules = |text: &str| {
        let registration = "#[cfg(test)]\nmod tests;\n\n#[cfg(test)]\n#[path = \"prefix/source_main8a.rs\"]\nmod source_main8a;\n\n#[cfg(test)]\n#[path = \"prefix/source_maina3.rs\"]\nmod source_maina3;\n";
        assert_eq!(text.matches(registration).count(), 1);
        text.replacen(registration, "", 1)
    };
    let old = include_str!("../../../arena/prefix/retained_before.rs");
    let old = old
        .split("\n// Complete immediate-old Arena::fact dispatch")
        .next()
        .unwrap();
    assert_eq!(
        strip_test_modules(current).trim(),
        old.replace(
            "pub(in crate::cleanup_plan::replay::expression_facts) fn find_receiving",
            "pub(super) fn find_receiving"
        )
        .trim()
    );
    let replay = include_str!("../../../../../replay.rs");
    let replay = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
        "src/cleanup_plan/replay.rs",
        "6cdfce6c20964d62f360cd147d0c6ca8b4d9cb86f671d6d5b5773d8c6e446ae7",
        replay,
    );
    let start = replay
        .find("fn validate_structure_with_budget_storage<")
        .unwrap();
    let end = replay[start..].find("\nstruct CfgReplayBounds").unwrap() + start;
    let old = include_str!("lookup_caller_before.rs");
    let old = old[old
        .find("pub(super) fn validate_structure_with_budget_storage<")
        .unwrap()..]
        .replacen("pub(super) fn", "fn", 1)
        .replacen(
            "crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20",
            "super::CLEANUP_PLAN_SCHEMA_V20",
            1,
        );
    assert_eq!(replay[start..end].trim(), old.trim());
}

#[test]
fn status_directory_outlives_independent_short_fact_payloads() {
    let program = checked_program(64);
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    // This owner is deliberately independent of the original function/proof.
    // Its facts and their payload source are dropped before the returned
    // StatusSources is read. No Facts-inner:'hir bound is available here.
    let payload_function = function.clone();
    let (count, refused, _) = crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
        let mut budget = ReplayBudget::new();
        let proof = case_proof::StatusCases::validate(&program, function, &mut budget).unwrap();
        assert!(function.cleanup_plan.status_sources.len() >= retained_lookup::MIN_ROWS);
        let sources = {
            let facts = crate::cleanup_plan::replay::expression_facts::with_budget(
                &payload_function,
                &mut budget,
            )
            .unwrap();
            let sources =
                validate_status_sources_certified(function, &facts, &mut budget, &proof).unwrap();
            drop(facts);
            sources
        };
        drop(payload_function);
        let count = sources.ids.as_ref().unwrap().len();
        assert_eq!(count, function.cleanup_plan.status_sources.len());
        drop(sources);
        drop(proof);
        count
    });
    assert!(!refused && count > 0);
}

#[test]
fn selector_site_ledger_uses_actual_borrowed_roles_and_preserves_old_inverse() {
    let selector = include_str!("retained_lookup.rs");
    let selector = selector.split("const A:").next().unwrap();
    assert!(
        !selector.contains("Option<usize>"),
        "no unformed donor options"
    );
    for typed_role in [
        "PAYER_TAG",
        "&crate::cleanup_plan::replay::allocation::AllocationPayer",
        "ROW_LENGTH",
        "&Vec<crate::cleanup_plan::StatusSource>",
        "ROW_BOUND",
        "SHORT_CIRCUIT",
        "FACTS_ARM",
        "&Facts<'_>",
        "NONEMPTY",
        "Session::arena_len_bytes()",
        "PAYMENT_AND_RETURN",
        "PAYMENT_TRANSPORT",
        "CONTINUATION",
    ] {
        assert!(
            selector.contains(typed_role),
            "missing typed site {typed_role}"
        );
    }
    let caller = include_str!("../case_certified.rs");
    let frontier = caller
        .split("retained_lookup::SELECT_BYTES")
        .nth(1)
        .unwrap();
    assert!(frontier.contains("matches!(\n                &budget.payer,"));
    let old = include_str!("lookup_caller_before.rs");
    assert_eq!(
        old.matches("crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20")
            .count(),
        1
    );
    assert!(!old.contains("super::CLEANUP_PLAN_SCHEMA_V20"));
}

mod receiving;
