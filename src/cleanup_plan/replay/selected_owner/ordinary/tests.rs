//! Full original search remains the independent result/diagnostic oracle.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

const SOURCE: &str = r#"module ordinary.owner;
@id("first") fn first()->i64 {0}
@id("identity") fn identity<T>(value:T)->T {value}
@id("main") fn main()->i64 {identity<i64>(1)}
"#;

fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "ordinary-owner.spx").unwrap()).unwrap()
}

#[test]
fn cold_original_search_keeps_every_owner_six_field_error_and_actual_paid_drop() {
    let program = program();
    assert!(!program.function_instances.is_empty());
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        let expected = crate::hir::lexical_exit::owner(&program, function);
        assert!(expected.is_ok());
        let observe = |limit, floor| {
            with_limit_usage_refusal(limit, || {
                assert!(crate::bounded_output::set_active_floor(floor));
                let result = measure(|| {
                    let mut budget = ReplayBudget::new();
                    let before = (budget.remaining, budget.skeleton_remaining);
                    let actual = owner(&program, function, &mut budget);
                    let success = actual.is_ok();
                    if success {
                        assert_eq!(actual, expected);
                        assert_eq!((budget.remaining, budget.skeleton_remaining), before);
                    } else {
                        assert_eq!(actual.as_ref().unwrap_err().code, "SPX-H006");
                    }
                    drop(actual);
                    drop(budget);
                    success
                });
                let ledger = crate::bounded_output::classifier_string::ClassifierLedger::capture();
                let first = ledger.first_refusal();
                if first.is_some() {
                    let remaining = crate::bounded_output::active_remaining();
                    for retry in [0, 1] {
                        assert!(!crate::bounded_output::reserve_active_required(retry));
                        assert_eq!(ledger.first_refusal(), first);
                        assert_eq!(crate::bounded_output::active_remaining(), remaining);
                    }
                }
                crate::bounded_output::clear_active_floor();
                result
            })
        };
        let full = observe(usize::MAX, 0);
        assert!(full.0.0 && !full.1 && full.3.is_none());
        for (limit, floor, success) in [
            (0, 0, false),
            (full.2 - 1, 0, false),
            (full.2, 0, true),
            (full.2, 1, false),
        ] {
            let actual = observe(limit, floor);
            assert_eq!(actual.0.0, success);
            assert_eq!(actual.1, !success);
            assert_eq!(actual.3.is_some(), !success);
            let metrics = actual.0.1;
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
            assert!(metrics.allocated <= actual.2 && actual.2 <= limit);
        }
    }
}

#[test]
fn missing_and_ambiguous_results_keep_complete_original_diagnostics_without_work_changes() {
    let original = program();
    let mut missing = original
        .functions
        .iter()
        .find(|f| f.name == "main")
        .unwrap()
        .clone();
    missing.body.id = ExpressionId::from_owned("forged.body".to_owned());
    let mut repeated = original.clone();
    repeated.functions.push(repeated.functions[0].clone());
    for (program, function) in [(&original, &missing), (&repeated, &repeated.functions[0])] {
        let expected = crate::hir::lexical_exit::owner(program, function);
        assert!(expected.is_err());
        let (((), metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let before = (budget.remaining, budget.skeleton_remaining);
                let actual = owner(program, function, &mut budget);
                assert_eq!(actual, expected); // all six Diagnostic fields
                assert_eq!((budget.remaining, budget.skeleton_remaining), before);
                drop(actual);
                drop(budget);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= paid);
    }
}

#[test]
fn exact_original_owner_helper_and_both_original_adapters_remain_source_bound() {
    use sha2::{Digest, Sha256};
    let digest = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let original = include_str!("owner_before.rs.txt");
    let current = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/hir/lexical_exit/owner.rs"
    ));
    assert_eq!(current, original);
    assert_eq!(
        digest(current.as_bytes()),
        "3011442d86bb6ffa452684b58a7ebb78c60d36ba383588f0704729ac38749eec"
    );
    // Owner identity comparisons are the unchanged streaming, allocation-free
    // helpers, not a reconstructed identity String hidden behind the search.
    assert_eq!(
        digest(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/hir/ids.rs"
        ))),
        "1eceacba63ce7ce7cbeb99ebe1ecfa05b3bdc25b679c6f74fadaa0b5b3a0284e"
    );
    let wrapper = include_str!("../ordinary.rs");
    assert_eq!(
        wrapper
            .matches("crate::hir::lexical_exit::owner(program, function)")
            .count(),
        1
    );
    assert!(FINAL_ERROR >= "lexical function execution owner is ambiguous".len());
    for (path, compiled, before) in [
        (
            "src/cleanup_plan/replay/selected_owner.rs",
            include_str!("../../selected_owner.rs"),
            include_str!("selected_owner_before.rs.txt"),
        ),
        (
            "src/cleanup_plan/replay/selected_owner/paid.rs",
            include_str!("../paid.rs"),
            include_str!("paid_before.rs.txt"),
        ),
        (
            "src/cleanup_plan/replay/path_join/frontier_tests/custody.rs",
            include_str!("../../path_join/frontier_tests/custody.rs"),
            include_str!("frontier_custody_before.rs.txt"),
        ),
    ] {
        let restored = super::custody(path, compiled.as_bytes());
        assert_eq!(restored, before.as_bytes());
        assert!(
            std::panic::catch_unwind(|| super::custody(
                path,
                &(compiled.to_owned() + " ").into_bytes()
            ))
            .is_err()
        );
        assert!(std::panic::catch_unwind(|| super::custody(path, before.as_bytes())).is_err());
    }
}
