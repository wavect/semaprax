use semaprax::workflow_profile::plan_comparison::{Binding, Capture};
fn main() {
    let input = std::env::args().nth(1).expect("manifest path");
    let raw = std::fs::read(&input).unwrap();
    use sha2::{Digest, Sha256};
    let product = |result: &Result<(), Vec<semaprax::diagnostic::Diagnostic>>| {
        let diagnostics = result
            .as_ref()
            .err()
            .map(|values| {
                values
                    .iter()
                    .map(|value| value.json())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        format!(
            "{{\"ok\":{},\"diagnostics\":[{}]}}",
            result.is_ok(),
            diagnostics
        )
    };
    if std::env::args().nth(2).as_deref() == Some("plain") {
        let result = semaprax::project::with_authenticated_project(
            std::path::Path::new(&input),
            |snapshot| snapshot.check(),
        );
        println!("product-json:{}", product(&result));
        return;
    }
    let capture = Capture::begin(Binding::caller_labelled(
        &input,
        Sha256::digest(&raw).into(),
    ))
    .unwrap();
    let result =
        semaprax::project::with_authenticated_project(std::path::Path::new(&input), |snapshot| {
            snapshot.check()
        });
    let report = capture.finish().unwrap();
    println!("product-json:{}", product(&result));
    eprintln!("result: {:?}", result);
    use semaprax::workflow_profile::plan_comparison::{PaidPhase, PhaseOrigin, PhasePayer};
    assert!(report.paid_phases.balanced && !report.paid_phases.overflow);
    for phase in [
        PaidPhase::CleanupValidation,
        PaidPhase::CanonicalRebuild,
        PaidPhase::ReplayStructure,
        PaidPhase::FactoredComparison,
        PaidPhase::FactoredConstruction,
        PaidPhase::HirPaths,
        PaidPhase::PlanPaths,
        PaidPhase::Comparison,
    ] {
        let row = report
            .paid_phases
            .row(phase, PhasePayer::ActiveBuilder, PhaseOrigin::Shipping);
        println!("inclusive paid phase {phase:?}: {row:?}");
    }
    let core = report.resolved_core;
    assert!(
        core.complete && !core.invalid && !core.lost && !core.overflow,
        "incomplete debit census"
    );
    assert_eq!(core.algorithm, semaprax::workflow_profile::plan_comparison::CoreDebitAlgorithm::InstrumentedProductionAlgorithm);
    assert!(core.stored_attempts > 0 && core.stored_attempts == core.entered_attempts);

    println!(
        "complete={} invalid={} lost={} overflow={} entered={} stored={} final={:?}",
        core.complete,
        core.invalid,
        core.lost,
        core.overflow,
        core.entered_attempts,
        core.stored_attempts,
        core.final_attempt_ordinal
    );
    for a in &core.attempts[..core.stored_attempts] {
        println!("attempt={} parent={:?} mode={} initial={} remaining={} consumed={} nested={} foreign={} outcome={:?} complete={} invalid={} refused={} first={:?}",a.ordinal,a.parent_ordinal,a.mode,a.initial,a.remaining,a.consumed,a.nested_ledger_debits,a.foreign_ledger_debits,a.outcome,a.complete,a.invalid,a.refused,a.first_refusal);
        for (family, row) in semaprax::workflow_profile::plan_comparison::PATH_FAMILIES
            .iter()
            .zip(a.path_census)
        {
            println!("path-census:{{\"schema\":\"semaprax.path-family-census.v1\",\"family\":\"{}\",\"entries\":{},\"successes\":{},\"errors\":{},\"unwinds\":{},\"refusals\":{},\"requests\":{},\"requested_bytes\":{},\"admitted_requests\":{},\"admitted_bytes\":{},\"debits\":{},\"debit_bytes\":{},\"terminal_paths\":{},\"successful_paths\":{},\"events\":{:?},\"event_labels\":{:?},\"foreign_events\":{},\"descendant_requests\":{},\"descendant_requested_bytes\":{},\"descendant_admitted_requests\":{},\"descendant_admitted_bytes\":{},\"descendant_entries\":{},\"descendant_events\":{:?},\"physical_allocator_census\":false}}",
                family,row.entries,row.successes,row.errors,row.unwinds,row.refusals,row.requests,row.requested_bytes,row.admitted_requests,row.admitted_bytes,row.debits,row.debit_bytes,row.terminal_paths,row.successful_paths,row.events,semaprax::workflow_profile::plan_comparison::PATH_EVENTS,row.foreign_events,row.descendant_requests,row.descendant_requested_bytes,row.descendant_admitted_requests,row.descendant_admitted_bytes,row.descendant_entries,row.descendant_events);
        }
        for (label, value) in core.stage_labels.iter().zip(a.stage_debits) {
            if value != 0 {
                println!("stage {label}: {value}");
            }
        }
        for (label, value) in core.operation_labels.iter().zip(a.operation_debits) {
            if value != 0 {
                println!("operation {label}: {value}");
            }
        }
    }
}
