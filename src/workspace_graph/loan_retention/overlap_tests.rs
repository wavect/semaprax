//! Cold whole-public windows include source setup, validation, serialization
//! parity and final owner Drop. Input source and expected wire bytes preexist.
//! These are allocation observations, never an admitted retained forecast.
use crate::test_allocation_observer::{measure_with_capacity, with_phase_trace};

const SOURCE: &str = r#"module loan.overlap;
@id("first") fn first(packet:own Bytes)->usize {
 let view=bytes_as_slice(packet);
 byte_len(view)+byte_len(view)+byte_len(view)
}
@id("second") fn second(packet:own Bytes)->usize {
 let view=bytes_as_slice(packet); byte_len(view)+byte_len(view)
}
@id("identity") fn identity<T>(value:T)->T {value}
@id("main") fn main()->i64 {identity<i64>(0)}
"#;

#[test]
fn cold_public_setup_validate_projection_and_drop_exposes_real_local_overlap_without_debits() {
    let checked = crate::check(SOURCE, "loan-overlap.spx").unwrap();
    let resolved = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let expected = crate::cache_codec::encode(&resolved).unwrap();
    let expected_functions = resolved.functions.len();
    let expected_instances = resolved.function_instances.len();
    assert!(
        expected_instances > 0,
        "generic output keeps the complete fallback"
    );
    drop(resolved);
    drop(checked);
    let operation = || {
        // Each probe runs in its own fresh thread: no prior per-thread
        // compiler/phase state is supplied as cold construction evidence.
        let checked = crate::check(SOURCE, "loan-overlap.spx").unwrap();
        let resolved = crate::hir::resolve(&checked).unwrap();
        crate::hir::validate(&resolved).unwrap();
        let wire = crate::cache_codec::encode(&resolved).unwrap();
        assert_eq!(
            wire, expected,
            "complete HIR bytes, including attached plans"
        );
        assert_eq!(resolved.functions.len(), expected_functions);
        assert_eq!(resolved.function_instances.len(), expected_instances);
        drop(wire);
        drop(resolved);
        drop(checked);
    };
    let (plain, (observed, trace)) = std::thread::scope(|scope| {
        let plain = scope
            .spawn(|| measure_with_capacity(1 << 16, operation))
            .join()
            .unwrap();
        let traced = scope
            .spawn(|| with_phase_trace(|| measure_with_capacity(1 << 16, operation)))
            .join()
            .unwrap();
        (plain, traced)
    });
    assert_eq!(
        plain, observed,
        "fixed tracing changes no allocation or semantic result"
    );
    let metrics = observed.1;
    assert!(!metrics.invalid);
    assert!(!trace.overflow);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    for owner in [
        "hir::validate_selected_with_static_text",
        "hir::canonical_loan_storage::prepare_sealed",
        "hir::canonical_loan_storage::next_function",
        "hir::canonical_loan_storage::append",
        "loan_plan::Reachability::build_rows_and_queue",
    ] {
        let (entry, local_peak, exit) = trace
            .function_peak(owner)
            .unwrap_or_else(|| panic!("the whole-public window did not exercise {owner}"));
        assert!(
            entry.live > 0,
            "live HIR/prior inventory remains beside {owner}"
        );
        assert!(local_peak >= entry.live && local_peak >= exit.live);
        assert!(local_peak <= metrics.highwater);
        assert!(entry.allocated <= exit.allocated && entry.freed <= exit.freed);
        eprintln!("OPT903 cold whole-public owner={owner} entry={entry:?} local_simultaneous_peak={local_peak} exit={exit:?}; inclusive actual requested-layout overlap, not additive scratch or RSS");
    }
    eprintln!("OPT903 cold public setup+resolve+validate+complete HIR bytes+Drop metrics={metrics:?} functions={expected_functions} instances={expected_instances}; no retained-forecast certification");
}
