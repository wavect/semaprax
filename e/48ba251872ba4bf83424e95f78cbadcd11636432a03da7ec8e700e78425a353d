use super::*;
use crate::cleanup_plan::replay::*;

const RECEIVERS: usize = 2;

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
    ordinary_driver(program, old, false, bytes)
}
// Complete generation counterpart before #903's independently priced scratch
// release. The original #885 equality never includes that later transport.
fn ordinary_driver(
    program: &ResolvedProgram,
    old: bool,
    before_release: bool,
    bytes: usize,
) -> (Result<(), Diagnostic>, bool, usize) {
    let (result, reference) = ordinary_driver_reference(program, old, before_release, bytes);
    eprintln!("ordinary reference max_receivers={RECEIVERS} complete_receipt={reference:?}");
    result
}
fn ordinary_driver_reference(
    program: &ResolvedProgram,
    old: bool,
    before_release: bool,
    bytes: usize,
) -> (
    (Result<(), Diagnostic>, bool, usize),
    crate::cleanup_plan::replay::allocation::OracleReceipt,
) {
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    // The comparison hook is an independent test reference, not part of the
    // ordinary builder invoice. Both real arms retain the original byte limit.
    // One main-function comparison invokes the two original receivers.
    // Physical funding stays at its previous finite declaration; work is the
    // checked product of both original work domains and the two receivers.
    use crate::cleanup_plan::replay::allocation::{opt_in, OracleScope};
    use crate::cleanup_plan::replay::workspace_oracle::{
        reference_work_bound, ReferenceReceiptOnUnwind,
    };
    let reference = OracleScope::open_with_work(1 << 28, reference_work_bound(RECEIVERS))
        .expect("finite independent ordinary replay reference account");
    let trace = ReferenceReceiptOnUnwind::new(&reference, RECEIVERS);
    let selected = opt_in(reference.lease()).expect("one ordinary replay reference lease");
    let result = crate::bounded_output::with_limit_usage(bytes, || {
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
        } else if before_release {
            super::super::before_structure_release::validate_structure_with_budget_storage(
                program,
                function,
                &mut budget,
                None,
                None,
            )
        } else {
            validate_structure_with_budget_storage(program, function, &mut budget, None, None)
        }
    });
    drop(selected);
    drop(trace);
    let reference = reference.finish();
    assert!(reference.started <= RECEIVERS, "{reference:?}");
    assert_eq!(reference.started, reference.completed);
    assert!(
        reference.first_refusal.is_none()
            && reference.first_work_refusal.is_none()
            && reference.physical_failure.is_none(),
        "{reference:?}"
    );
    (result, reference)
}
// Fixed inline observation only; this report outlives the allocation window,
// including an unchanged assertion unwind. It owns no compiler backing.
type NoPreflightSnapshot = (
    bool,
    bool,
    usize,
    usize,
    usize,
    usize,
    crate::cleanup_plan::replay::allocation::OracleReceipt,
    Option<usize>,
    Option<ReplayWorkRefusal>,
);
struct NoPreflightReport {
    old: bool,
    snapshot: std::cell::Cell<Option<NoPreflightSnapshot>>,
}
impl Drop for NoPreflightReport {
    fn drop(&mut self) {
        eprintln!("ordinary without preflight old={} success/refused/paid/work_remaining/skeleton_remaining/skeleton_spent/reference/materializations/work_refusal={:?}", self.old, self.snapshot.get());
    }
}

// Complete immediate old helper: intentionally lacks the production preflight.
fn ordinary_without_skeleton_preflight(
    program: &ResolvedProgram,
    old: bool,
    before_selection: bool,
    bytes: usize,
    observed: &std::cell::Cell<Option<NoPreflightSnapshot>>,
) -> (Result<(), Diagnostic>, bool, usize) {
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    // The comparison hook is an independent test reference, not part of the
    // ordinary builder invoice. Both real arms retain the original byte limit.
    // One main-function comparison invokes the two original receivers.
    // Physical funding stays at its previous finite declaration; work is the
    // checked product of both original work domains and the two receivers.
    use crate::cleanup_plan::replay::allocation::{opt_in, OracleScope};
    use crate::cleanup_plan::replay::workspace_oracle::{
        reference_work_bound, ReferenceReceiptOnUnwind,
    };
    const RECEIVERS: usize = 2;
    let reference = OracleScope::open_with_work(1 << 28, reference_work_bound(RECEIVERS))
        .expect("finite independent ordinary replay reference account");
    let trace = ReferenceReceiptOnUnwind::new(&reference, RECEIVERS);
    let selected = opt_in(reference.lease()).expect("one ordinary replay reference lease");
    let initial_materializations = skeleton_materializations();
    let _ = take_replay_work_refusal();
    let mut work = (0, 0, 0);
    let result = crate::bounded_output::with_limit_usage(bytes, || {
        let mut budget = ReplayBudget::new();
        let verdict = if before_selection {
            if old {
                prefitting_lookup::validate_structure_with_budget_storage(
                    program,
                    function,
                    &mut budget,
                    None,
                    None,
                )
            } else {
                prefitting_receiving::before::validate_structure_with_budget_storage(
                    program,
                    function,
                    &mut budget,
                    None,
                    None,
                )
            }
        } else {
            assert!(
                !old,
                "shipping observation never substitutes a historical caller"
            );
            validate_structure_with_budget_storage(program, function, &mut budget, None, None)
        };
        work = (
            budget.remaining,
            budget.skeleton_remaining,
            budget.skeleton_spent,
        );
        verdict
    });
    drop(selected);
    drop(trace);
    let reference = reference.finish();
    assert!(reference.started <= RECEIVERS, "{reference:?}");
    assert_eq!(reference.started, reference.completed);
    assert!(
        reference.first_refusal.is_none()
            && reference.first_work_refusal.is_none()
            && reference.physical_failure.is_none(),
        "{reference:?}"
    );
    observed.set(Some((
        result.0.is_ok(),
        result.1,
        result.2,
        work.0,
        work.1,
        work.2,
        reference,
        skeleton_materializations().checked_sub(initial_materializations),
        take_replay_work_refusal(),
    )));
    result
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
        let ordinary =
            |program: &ResolvedProgram, old, bytes| ordinary_driver(program, old, true, bytes);
        assert_eq!(
            ordinary(&scalar, false, 64 * 1024 * 1024),
            ordinary(&scalar, true, 64 * 1024 * 1024)
        );
    });
    // Actual current receiver also runs, on its own unchanged byte limit. The
    // zero-query path has no retained attempt; its later #903 tuple is paid.
    crate::cleanup_plan::replay::ordinary_receiving::with_before(|| {
        let generation = ordinary_driver(&scalar, false, true, 64 * 1024 * 1024);
        let current = ordinary(&scalar, false, 64 * 1024 * 1024);
        assert_eq!(current.0, generation.0); // all six Diagnostic fields
        assert!(current.0.is_ok() && !current.1 && !generation.1);
        assert_eq!(
            current.2,
            generation
                .2
                .checked_add(super::super::last_use::RELEASE)
                .unwrap(),
            "complete current scalar composition includes the actual scratch transport"
        );
        let exact = ordinary(&scalar, false, current.2);
        assert_eq!(exact.0, current.0); // whole current Result, not a byte-normalized error
        assert!(!exact.1);
        for cap in [0, current.2 - 1] {
            let short = ordinary(&scalar, false, cap);
            assert!(short.0.is_err() && short.1, "current cap={cap}: {short:?}");
            eprintln!("current scalar refusal cap={cap} full_result/refused/paid={short:?}");
        }
        let function = scalar.functions.iter().find(|f| f.name == "main").unwrap();
        for work in [0, 1, 2, 7, 32, MAX_REPLAY_WORK_UNITS] {
            let observe = |before_release| {
                let mut budget = ReplayBudget::unobserved();
                budget.remaining = work;
                let result = match reserve_program_skeleton_work(
                    &scalar,
                    std::iter::once(function),
                    &mut budget,
                ) {
                    Err(error) => Err(error),
                    Ok(_) => {
                        if before_release {
                            super::super::before_structure_release::validate_structure_with_budget_storage(&scalar,function,&mut budget,None,None)
                        } else {
                            validate_structure_with_budget_storage(
                                &scalar,
                                function,
                                &mut budget,
                                None,
                                None,
                            )
                        }
                    }
                };
                (
                    result,
                    budget.remaining,
                    budget.skeleton_remaining,
                    budget.skeleton_spent,
                )
            };
            assert_eq!(
                observe(false),
                observe(true),
                "complete current scalar work/Diagnostic cut={work}"
            );
        }
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
        let (reference, metrics) = crate::test_allocation_observer::measure(|| {
            // Return the complete receipt without harness stderr inside the
            // compiler allocation lifetime being measured.
            let (result, reference) = ordinary_driver_reference(&program, false, false, bytes);
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
            reference
        });
        eprintln!("ordinary reference bytes={bytes} max_receivers={RECEIVERS} complete_receipt={reference:?} physical={metrics:?}");
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
        drop(census);
    }
}

#[test]
fn complete_caller_without_skeleton_preflight_keeps_exact_root_refusal_and_drop() {
    let program = checked_program(32);
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    authenticate_prefitting_ordinary();
    for old in [false, true] {
        let report = NoPreflightReport {
            old,
            snapshot: std::cell::Cell::new(None),
        };
        let (_, metrics) = crate::test_allocation_observer::measure(|| {
            let result = ordinary_without_skeleton_preflight(
                &program,
                old,
                true,
                64 * 1024 * 1024,
                &report.snapshot,
            );
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

    // The shipping route keeps its real64MiB payer and complete finalDrop.
    // New representation selection is observed separately from the old refusal.
    let report = NoPreflightReport {
        old: false,
        snapshot: std::cell::Cell::new(None),
    };
    let (_, metrics) = crate::test_allocation_observer::measure(|| {
        let result = ordinary_without_skeleton_preflight(
            &program,
            false,
            false,
            64 * 1024 * 1024,
            &report.snapshot,
        );
        assert_eq!(result.0, Ok(()));
        assert!(!result.1);
        drop(result);
    });
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    let snapshot = report.snapshot.get().expect("complete current accounting");
    assert!(snapshot.2 <= 64 * 1024 * 1024 && snapshot.3 > 0);
    assert!(snapshot.6.started > 0 && snapshot.6.started == snapshot.6.completed);
    assert!(
        snapshot.6.first_refusal.is_none()
            && snapshot.6.first_work_refusal.is_none()
            && snapshot.6.physical_failure.is_none()
    );
    assert!(snapshot.8.is_none());
    assert!(metrics.allocated <= snapshot.2.checked_add(snapshot.6.charged).unwrap());
    let cfg_paths = branch_sensitive_cfg_bounds(function)
        .unwrap()
        .terminal_paths;
    let semantic_paths = hir_terminal_path_bound(function).unwrap();
    // After full observations, authenticate the old nonselection and current route.
    let mut old_selector = ReplayBudget::unobserved();
    assert!(!factored::prefitting_selected_paid(
        function,
        cfg_paths,
        semantic_paths,
        &mut old_selector
    )
    .unwrap());
    assert_eq!(old_selector.remaining, MAX_REPLAY_WORK_UNITS);
    assert!(function.cleanup_plan.lexical_control.is_none());
    let builder = cfg_paths > 128 || semantic_paths > 128;
    let fitting = !builder
        && cfg_paths <= 4096
        && semantic_paths <= 4096
        && function.cleanup_plan.status_sources.len() >= 32
        && function.cleanup_plan.slots.is_empty()
        && function
            .cleanup_plan
            .entry_state
            .live_owned_parameters
            .is_empty()
        && function
            .cleanup_plan
            .entry_state
            .conditional_owned_parameters
            .is_empty();
    assert!(
        builder || fitting,
        "actual current representation must have an admitted selector"
    );
    eprintln!("ordinary no-preflight route cfg={cfg_paths} semantic={semantic_paths} builder={builder} fitting={fitting}");
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

use super::super::super::regions_entry;
#[path = ""]
#[rustfmt::skip]
mod prefitting_receiving {
    use super::*;
    use crate::cleanup_plan::replay::expression_facts::receiving::{helpers, require_status_paid};
    use crate::cleanup_plan::replay::expression_facts::receiving::retained::{exit_prefix, validate_owned_edge_unique};
    use super::super as case_certified;
    use super::prefitting_typed_skeleton as validate_typed_control_skeleton_with_index;
    #[path = "../last_use/before.rs"]
    pub(super) mod before;
}
#[rustfmt::skip]
mod prefitting_lookup {
//! Immediate 25d6fc178 ordinary composition; only certified status lookup is frozen.
use crate::cleanup_plan::replay::*;
use super::prefitting_typed_skeleton as validate_typed_control_skeleton_with_index;
mod expression_facts {
    pub(super) use crate::cleanup_plan::replay::expression_facts::with_budget;
    pub(super) mod receiving {
        pub(in crate::cleanup_plan::replay) use super::super::super::lookup_before::validate_status_sources_certified;
        pub(in crate::cleanup_plan::replay) use crate::cleanup_plan::replay::expression_facts::receiving::*;
    }
}
pub(super) fn validate_structure_with_budget_storage<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayStructure,
    );
    let plan = &function.cleanup_plan;
    if helpers.is_some_and(|index| {
        !program
            .full_program()
            .is_some_and(|full| index.matches_program(full))
    }) {
        return Err(replay_error(
            function,
            "cleanup replay helper index belongs to a different typed program",
        ));
    }
    // Reconstruct the ordinary schema first, preserving all its current-source
    // probes/error order. A private source-held projection adds allocation
    // branches; neither its attached v20 string nor an ordinary context selects it.
    let ordinary_schema =
        schema::paid_summary::selected_with_budget(program, function, helpers, budget)?;
    let expected_schema = if allocation_paths::current(program, function).is_some() {
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20
    } else {
        ordinary_schema
    };
    if plan.schema != expected_schema {
        return Err(replay_error(
            function,
            format_args!(
                "uses cleanup-plan schema `{}` instead of HIR-derived `{expected_schema}`",
                plan.schema
            ),
        ));
    }

    if lexical_cert::requires_lexical(function) {
        return lexical_replay::validate_with_index(program, function, budget, helpers);
    }
    if plan.lexical_control.is_some() {
        return Err(replay_error(
            function,
            "function without lexical exits carries a lexical-control certificate",
        ));
    }
    validate_replay_size_budget(function)?;
    budget.charge(
        function,
        plan_structure_units(plan),
        "structural validation",
    )?;

    let expression_facts = expression_facts::with_budget(function, budget)?;
    ordinary_inventory::validate_inventory_coverage(program, function, budget)?;
    let status_cases =
        expression_facts::receiving::StatusCases::validate(program, function, budget)?;
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    let statuses = expression_facts::receiving::validate_status_sources_certified(
        function,
        &expression_facts,
        budget,
        &status_cases,
    )?;
    expression_facts::receiving::validate_regions(function, &storage, budget)?;
    expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;
    expression_facts::receiving::validate_blocks_and_edges_retained(
        program,
        function,
        &expression_facts,
        &storage,
        &leaves,
        helpers,
        budget,
        &statuses,
    )?;
    expression_facts::receiving::validate_exits_retained(
        program,
        function,
        &storage,
        &leaves,
        &BTreeSet::new(),
        budget,
        &statuses,
    )?;
    structural_maps::validate_reference_coverage(function, budget)?;
    structural_control::validate_ordinary_cfg(function, budget)?;
    if !cleanup_inert_path_product_can_be_summarized(function)?
        && !status_only_paths_can_be_summarized(function)
        && !cleanup_inert_large_decisions_can_be_summarized(function)
    {
        validate_path_states::<Diagnostic>(
            program, function, &storage, &leaves, budget, None, helpers,
        )?;
        validate_typed_control_skeleton_with_index(
            program,
            function,
            budget,
            helpers,
            continuations,
        )?;
    }
    Ok(())
}

}

#[rustfmt::skip]
fn prefitting_typed_skeleton<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    mut continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::OtherReplay);
    if function.cleanup_plan.lexical_control.is_none()
        && factored::prefitting_validate(
            program,
            function,
            budget,
            helpers,
            continuations.as_deref_mut(),
        )?
    {
        return Ok(());
    }
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::EnumeratedHIR);
    let mut expected =
        hir_skeleton_paths_with_index_storage(program, function, budget, helpers, continuations)?;
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::OtherReplay);
    // Match the existing received reference, with no new payer/tag/flag carrier.
    if let ReplayBudget {
        payer: AllocationPayer::ActiveBuilder,
        ..
    } = budget
    {
        budget.reserve_bytes(
            function,
            ordinary_plan_history::SELECT_CALL_CONTROLS,
            "ordinary plan history selector call receivers",
        )?;
        if ordinary_plan_history::select(function, budget)? {
            #[cfg(test)]
            let _debit_phase = debit_routes::phase(debit_routes::Phase::HistoryPLAN);
            budget.reserve_bytes(
                function,
                ordinary_plan_history::PRODUCE_CALL_CONTROLS,
                "ordinary plan history producer call receivers",
            )?;
            let actual = ordinary_plan_history::produce(function, budget)?;
            #[cfg(test)]
            let _debit_phase = debit_routes::phase(debit_routes::Phase::PathComparison);
            budget.reserve_bytes(
                function,
                skeleton_work::enumerated_prefix::ENTRY_CALL,
                "enumerated HIR sort entry receivers",
            )?;
            skeleton_work::enumerated_prefix::sort(&mut expected, function, budget)?;
            budget.reserve_bytes(
                function,
                ordinary_plan_history::COMPARE_CALL_CONTROLS,
                "ordinary plan history owned comparison call receivers",
            )?;
            return ordinary_plan_history::compare(function, budget, actual, &expected);
        }
    }
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::FlatPLAN);
    let mut actual = plan_skeleton_paths(function, budget)?;
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::PathComparison);
    #[cfg(feature = "unstable-workflow-profiling")]
    let _paid_comparison = budget.paid_phase(PaidPhase::Comparison);
    if match budget {
        ReplayBudget {
            payer: AllocationPayer::ActiveBuilder,
            admission: AdmissionMode::Ordinary,
            lent_skeleton: false,
            ..
        } => {
            budget.reserve_bytes(
                function,
                2 * skeleton_work::enumerated_prefix::ENTRY_CALL,
                "enumerated HIR sort and equality entry receivers",
            )?;
            skeleton_work::enumerated_prefix::sort(&mut expected, function, budget)?;
            actual.sort();
            !skeleton_work::enumerated_prefix::equal(&expected, &actual, function, budget)?
        }
        _ => {
            expected.sort();
            actual.sort();
            actual != expected
        }
    } {
        if std::env::var_os("SPX_PROBE_SKELETON").is_some() {
            for (index, path) in expected.iter().enumerate() {
                eprintln!("EXP[{index}] {path:?}");
            }
            for (index, path) in actual.iter().enumerate() {
                eprintln!("ACT[{index}] {path:?}");
            }
        }
        return Err(replay_error(
            function,
            "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
        ));
    }
    Ok(())
}

fn authenticate_prefitting_ordinary() {
    use sha2::{Digest, Sha256};
    fn definition<'a>(source: &'a str, marker: &str) -> &'a str {
        let start = source.find(marker).expect("full source definition");
        let end = start + source[start..].find("\n}").unwrap() + 2;
        &source[start..end]
    }
    fn digest(raw: &str) -> String {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(raw.as_bytes()))
        )
    }
    factored::authenticate_prefitting_dispatch();
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d"
    ));
    assert_eq!(original.len(), 326426);
    assert_eq!(
        digest(original),
        "5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d"
    );
    let current = include_str!("lookup_tests.rs");
    assert_eq!(
        definition(current, "fn prefitting_typed_skeleton<'hir>(")
            .replacen(
                "fn prefitting_typed_skeleton<'hir>(",
                "fn validate_typed_control_skeleton_with_index<'hir>(",
                1
            )
            .replacen(
                "factored::prefitting_validate(",
                "factored::validate_with_index_storage(",
                1
            ),
        definition(
            original,
            "fn validate_typed_control_skeleton_with_index<'hir>("
        )
    );
    let lookup = include_str!("lookup_caller_before.rs");
    let compiled = current
        .split_once("mod prefitting_lookup {\n")
        .unwrap()
        .1
        .split_once("\n}\n\n#[rustfmt::skip]\nfn prefitting_typed_skeleton")
        .unwrap()
        .0;
    assert_eq!(compiled.replacen("use super::prefitting_typed_skeleton as validate_typed_control_skeleton_with_index;\n","",1).trim(),lookup.trim());
    let receiving = include_str!("../last_use/before.rs");
    assert_eq!(receiving.len(), 86308);
    assert_eq!(
        digest(receiving),
        "755306bc0932240e81f62d01f1ec556235756c816d013d5d7f81417c4ba40417"
    );
    let paired = include_str!("../../../../factored/paired.rs");
    assert_eq!(paired.len(), 27714);
    assert_eq!(
        digest(paired),
        "b71f44bb3ca6e1feccfdbad9128d33267881a147766583397634c4d9348cf7ca"
    );
    let dispatch = definition(
        include_str!("../../../../factored.rs"),
        "pub(super) fn validate_with_index_storage<'hir>(",
    );
    assert_eq!(dispatch.len(), 3774);
    assert_eq!(
        digest(dispatch),
        "e84402053f4da6ffd1e6c9224104069d1c38399ebd0fa3f67981933e7ae272d2"
    );
}
