//! Whole authenticated entry regressions; no source rewriting or quota change.
use super::*;

fn resolved(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "lexical-activation.spx").unwrap()).unwrap()
}
fn probe(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions()
        .iter()
        .find(|function| function.id.as_str() == "probe")
        .unwrap()
}
fn decisions(count: usize) -> String {
    (0..count)
        .map(|index| format!("let choice_{index}=if n==0 {{0}}else{{1}};\n"))
        .collect()
}
fn source(count: usize, extra_path: bool) -> String {
    // The exact break loop authenticates lexical control and has two normal
    // exits. Copy choices contribute 2^(count-2), and the final two returning
    // arms supply the last factor. An earlier return adds exactly one route.
    let choices = decisions(count.checked_sub(2).unwrap());
    let early = if extra_path {
        "if n==2 {return 0;}else{0}"
    } else {
        ""
    };
    let body =
        format!("{early} while n==3 {{break;}} {choices} if n==1 {{return 0;}}else{{return 0;}}");
    format!(
        "module lexical.activation; @id(\"probe\") fn probe(n:i64)->i64 {{{body}}} @id(\"main\") fn main()->i64 {{0}}"
    )
}

#[test]
fn forced_authenticated_strategies_agree_on_small_full_multisets() {
    for authored in [lexical_plan_tests::WHILE, lexical_plan_tests::GENERATED] {
        let (program, function) = lexical_plan_tests::fixture(authored);
        for strategy in [Strategy::Enumerated, Strategy::LexicalFactored] {
            with_strategy(strategy, || validate_structure(&program, &function)).unwrap();
        }
        // Independent HIR Cartesian expansion and plan-route enumeration are
        // separately retained in lexical_plan_tests, including duplicates.
        lexical_plan_tests::assert_copy_scope_oracle(authored);
    }
}

#[test]
fn exact_65536_and_65537_choose_distinct_routes_without_raising_ceiling() {
    for (extra_path, expected) in [(false, 65_536), (true, 65_537)] {
        let program = resolved(&source(16, extra_path));
        let function = probe(&program);
        assert!(function.cleanup_plan.lexical_control.is_some());
        let before = function.cleanup_plan.clone();
        validate_structure(&program, function).unwrap();
        let mut budget = ReplayBudget::new();
        let candidate = Candidate::reconstruct(&program, function, &mut budget).unwrap();
        assert_eq!(candidate.hir, candidate.plan);
        assert_eq!(
            candidate.hir,
            TerminalCounts {
                success: Number::Known(expected),
                ..TerminalCounts::ZERO
            }
            .census()
        );
        assert_eq!(
            candidate.strategy,
            if extra_path {
                Strategy::LexicalFactored
            } else {
                Strategy::Enumerated
            }
        );
        with_strategy(Strategy::LexicalFactored, || {
            validate_structure(&program, function)
        })
        .unwrap();
        let enumerated = with_strategy(Strategy::Enumerated, || {
            validate_structure(&program, function)
        });
        if extra_path {
            assert_eq!(enumerated.unwrap_err().code, "SPX-H006");
        } else {
            enumerated.unwrap();
        }
        assert_eq!(function.cleanup_plan, before);
    }
}

#[test]
fn above_cap_equal_terminal_totals_do_not_authenticate_reordered_decisions() {
    let program = resolved(&source(17, false));
    let function = probe(&program);
    assert!(function.cleanup_plan.lexical_control.is_some());
    let mut forged = function.clone();
    let mut identities = Vec::new();
    for edge in &forged.cleanup_plan.edges {
        if let EdgeCondition::BooleanResult(expression, _) = &edge.condition {
            if !identities.contains(expression) {
                identities.push(expression.clone());
            }
        }
        if identities.len() == 2 {
            break;
        }
    }
    assert_eq!(identities.len(), 2);
    for edge in &mut forged.cleanup_plan.edges {
        if let EdgeCondition::BooleanResult(expression, _) = &mut edge.condition {
            if expression == &identities[0] {
                *expression = identities[1].clone();
            } else if expression == &identities[1] {
                *expression = identities[0].clone();
            }
        }
    }
    let mut budget = ReplayBudget::new();
    let candidate = Candidate::reconstruct(&program, &forged, &mut budget).unwrap();
    assert_eq!(
        candidate.hir, candidate.plan,
        "hostile graph retains all terminal classes and totals"
    );
    assert!(matches!(
        candidate.hir,
        Census::Exceeded { total: 131_072, .. }
    ));
    assert_eq!(
        validate_structure(&program, &forged).unwrap_err().code,
        "SPX-H006"
    );
}

fn whole_entry(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
) -> (
    Result<(usize, usize), Diagnostic>,
    crate::test_allocation_observer::Metrics,
) {
    let context = primitive_fixture::Context::new(program);
    crate::test_allocation_observer::measure(|| {
        selected_entry::validate_with_limits(&context, function, work, bytes)
    })
}

#[test]
fn selected_whole_entry_to_drop_obeys_exact_and_one_short_work_and_backing() {
    let (program, _) = lexical_plan_tests::fixture(lexical_plan_tests::WHILE);
    let function = probe(&program);
    let unchanged = function.cleanup_plan.clone();
    let (wide, observed) = whole_entry(&program, function, MAX_REPLAY_WORK_UNITS, 1 << 26);
    let (work, bytes) = wide.unwrap();
    assert!(!observed.invalid && observed.allocated <= bytes && observed.highwater <= bytes);
    assert_eq!(observed.live, 0);
    let (exact, observed) = whole_entry(&program, function, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!observed.invalid && observed.allocated <= bytes && observed.highwater <= bytes);
    assert_eq!(
        observed.live, 0,
        "the whole successful entry and Drop retain no allocation"
    );
    assert_eq!(
        whole_entry(&program, function, work - 1, bytes)
            .0
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(
        whole_entry(&program, function, work, bytes - 1)
            .0
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(function.cleanup_plan, unchanged);
}

fn module_run(program: &ResolvedProgram, work: usize) -> Result<usize, Diagnostic> {
    let mut budget = ReplayBudget::new();
    budget.remaining = work;
    reserve_program_skeleton_work(program, program.functions(), &mut budget)?;
    for function in program.functions() {
        validate_structure_with_budget(program, function, &mut budget, None)?;
    }
    Ok(work - budget.remaining)
}

#[test]
fn deferred_functions_share_one_cumulative_module_work_ledger() {
    let choices = decisions(4);
    let source = format!(
        "module lexical.cumulative;
@id(\"probe\") fn probe(n:i64)->i64 {{{choices} return 0;}}
@id(\"second\") fn second(n:i64)->i64 {{{choices} return 1;}}
@id(\"main\") fn main()->i64 {{0}}"
    );
    let program = resolved(&source);
    let work = module_run(&program, MAX_REPLAY_WORK_UNITS).unwrap();
    assert_eq!(module_run(&program, work).unwrap(), work);
    assert_eq!(module_run(&program, work - 1).unwrap_err().code, "SPX-H006");
}

#[test]
fn deferred_enumerated_reservation_adds_to_held_pool_without_refund() {
    let (program, function) = lexical_plan_tests::fixture(lexical_plan_tests::WHILE);
    let upper = lexical_census::skeleton_work_upper(&program, &function).unwrap();
    let mut budget = ReplayBudget::new();
    budget.reserve_skeleton(&function, 101).unwrap();
    let remaining = budget.remaining;
    reserve_enumerated(&function, &mut budget, upper).unwrap();
    assert_eq!(budget.skeleton_remaining, 101 + upper);
    assert_eq!(budget.remaining, remaining - upper);
    assert_eq!(budget.skeleton_spent, 0);
}

#[test]
fn both_factored_constructors_share_free_work_and_preserve_held_reservations() {
    let (program, function) = lexical_plan_tests::fixture(lexical_plan_tests::WHILE);
    let held = 101;
    let mut wide = ReplayBudget::new();
    wide.reserve_skeleton(&function, held).unwrap();
    let available = wide.remaining;
    let _built = build_comparison(&program, &function, &mut wide, None, None).unwrap();
    let spent = available - wide.remaining;
    assert!(spent > 0);
    assert_eq!(wide.skeleton_remaining, held);
    assert_eq!(wide.skeleton_spent, spent);
    assert!(!wide.lent_skeleton && !wide.merge_paths);

    let mut exact = ReplayBudget::new();
    exact.reserve_skeleton(&function, held).unwrap();
    exact.remaining = spent;
    let _built = build_comparison(&program, &function, &mut exact, None, None).unwrap();
    assert_eq!(exact.remaining, 0);
    assert_eq!(exact.skeleton_remaining, held);
    assert_eq!(exact.skeleton_spent, spent);
    assert!(!exact.lent_skeleton && !exact.merge_paths);

    let mut short = ReplayBudget::new();
    short.reserve_skeleton(&function, held).unwrap();
    short.remaining = spent - 1;
    let error = match build_comparison(&program, &function, &mut short, None, None) {
        Ok(_) => panic!("one-short construction must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(short.skeleton_remaining, held);
    assert_eq!(short.remaining + short.skeleton_spent, spent - 1);
    assert!(!short.lent_skeleton && !short.merge_paths);
}

#[test]
fn immutable_original_shiftsim_forced_enumerated_retains_frozen_refusal() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let example = repository.join("examples/shiftsim-typed-record-successor");
    let wrappers = repository
        .join("tests/project/standard_library/application_json/owned/string_slot_adoption");
    let original = ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/project/standard_library/application_json/owned/shiftsim_schedule_unfactored.txt"
    ));
    let root = std::env::temp_dir().join(format!(
        "semaprax-original-shiftsim-enumerated-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("fixtures")).unwrap();
    std::fs::copy(example.join("semaprax.toml"), root.join("semaprax.toml")).unwrap();
    for name in [
        "request.spx",
        "response.spx",
        "app.spx",
        "app.command.spx",
        "tests.spx",
    ] {
        std::fs::copy(example.join("src").join(name), root.join("src").join(name)).unwrap();
    }
    for (name, wrapper) in [
        ("model.spx", "shiftsim_model_wrapper.txt"),
        ("order.spx", "shiftsim_order_wrapper.txt"),
    ] {
        std::fs::copy(wrappers.join(wrapper), root.join("src").join(name)).unwrap();
    }
    std::fs::write(root.join("src/schedule.spx"), original).unwrap();
    let entered = std::cell::Cell::new(false);
    let errors = with_strategy(Strategy::Enumerated, || {
        crate::project::with_authenticated_project(&root.join("semaprax.toml"), |_| {
            entered.set(true);
            Ok(())
        })
    })
    .expect_err("the immutable original remains above the frozen enumerated ceiling");
    assert!(!entered.get());
    assert!(
        errors.iter().any(|error| error.code == "SPX-H006"
            && error.message.contains("shiftsim.schedule.run")
            && error.message.contains("65537 semantic terminal paths")
            && error.message.contains("65536 path budget")),
        "{errors:?}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("src/schedule.spx")).unwrap(),
        original
    );
    for (name, wrapper) in [
        ("model.spx", "shiftsim_model_wrapper.txt"),
        ("order.spx", "shiftsim_order_wrapper.txt"),
    ] {
        assert_eq!(
            std::fs::read(root.join("src").join(name)).unwrap(),
            std::fs::read(wrappers.join(wrapper)).unwrap()
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
