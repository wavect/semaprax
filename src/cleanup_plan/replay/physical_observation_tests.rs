//! Actual allocator witness: source/HIR and all probe inputs are built before
//! observation. Reservation arithmetic is not used as the physical-work oracle.
//! The first selected profile is primitive-only; nominal/dynamic providers stay
//! closed. Diagnostic allocation on refusal is observed and remains a separate
//! reporting obligation, not evidence that failed paths fit the success bound.
use super::*;
use crate::test_allocation_observer;
const SOURCE: &str = r#"
module replay.physical;
@id("physical.loop") fn probe(flag:bool)->i64 {
    let scratch=bytes_zeroed(3usize);
    let scratch_view=bytes_as_slice(scratch);
    let key=str"é\u{0}";
    let mut next=0;
    while next<3 {
        next=next+1;
        let view=str_as_bytes(key);
        if flag && next==1 {continue;}else{0}
        if next==2 {
            return if byte_len(view)==3usize && byte_len(scratch_view)==3usize {7}else{0};
        }else{0}
        0
    }
    9
}
@id("main") fn main()->i64 {probe(false)}
"#;
fn program() -> crate::hir::ResolvedProgram {
    let parsed = crate::check(SOURCE, "replay-physical.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    let reparsed = crate::check(&canonical, "replay-physical.spx").unwrap();
    assert_eq!(crate::format::canonical(&reparsed), canonical);
    crate::hir::resolve(&reparsed).unwrap()
}
#[test]
fn selected_static_text_and_bytes_replay_covers_observed_layouts_at_exact_limit() {
    let program = program();
    let context = super::primitive_fixture::Context::new(&program);
    let function = program
        .functions()
        .iter()
        .find(|function| function.id.as_str() == "physical.loop")
        .unwrap();
    assert!(function.cleanup_plan.lexical_control.is_some());
    let unchanged = function.cleanup_plan.clone();
    let ((wide, charged), refused, outer) =
        crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
            let (result, metrics) = test_allocation_observer::measure(|| {
                selected_entry::validate_retained_lexical_with_reservation(
                    &context,
                    function,
                    PhysicalReservation::new(64 * 1024 * 1024),
                )
                .map(|reservation| reservation.charged())
            });
            (result, metrics)
        });
    let physical = wide.unwrap();
    assert!(physical > 0 && physical <= outer);
    assert!(!refused);
    assert!(!charged.invalid);
    assert!(charged.allocations > 0 && outer > 0);
    assert!(
        charged.allocated <= outer,
        "actual allocated Layout bytes exceed the charged backing"
    );
    assert!(
        charged.highwater <= outer,
        "actual overlapping backing exceeds the charged backing"
    );
    assert_eq!(
        charged.live, 0,
        "successful replay retains no new allocation"
    );
    let ((exact, observed), refused, used) = crate::bounded_output::with_limit_usage(outer, || {
        test_allocation_observer::measure(|| {
            validate_retained_lexical_with_limit(&context, function, physical)
        })
    });
    exact.unwrap();
    assert!(!refused && !observed.invalid);
    assert_eq!(used, outer);
    assert!(observed.allocated <= used && observed.highwater <= used);
    assert_eq!(observed.live, 0);
    assert_eq!(function.cleanup_plan, unchanged);
    // Refusal reporting owns a returned Diagnostic. Its allocation is observed
    // below, not silently subtracted or represented as successful-path backing.
    let ((short, report), _, _) = crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
        test_allocation_observer::measure(|| {
            validate_retained_lexical_with_limit(&context, function, physical - 1)
        })
    });
    assert_eq!(short.unwrap_err().code, "SPX-H006");
    assert!(!report.invalid);
    assert!(report.allocated > 0);
    assert!(report.highwater <= report.allocated);
    assert_eq!(function.cleanup_plan, unchanged);
}
#[test]
fn physical_refusal_survives_work_reset_and_prevents_actual_vector_growth() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|function| function.id.as_str() == "physical.loop")
        .unwrap();
    let mut budget = ReplayBudget::new();
    budget.admission = AdmissionMode::SelectedRetained;
    budget.payer = AllocationPayer::SelectedReservation;
    budget.physical = Some(Box::new(physical_storage::PhysicalReservation::new(8)));
    let (capacity, observed) = test_allocation_observer::measure(|| {
        budget
            .reserve_array::<u8>(function, 8, "test actual vector")
            .unwrap();
        let bytes = Vec::<u8>::with_capacity(8);
        std::hint::black_box(&bytes);
        bytes.capacity()
    });
    assert_eq!(capacity, 8);
    assert!(!observed.invalid);
    assert_eq!(observed.allocated, 8);
    assert_eq!(observed.highwater, 8);
    assert_eq!(observed.live, 0);
    assert!(budget
        .reserve_array::<u8>(function, 1, "refused growth")
        .is_err());
    budget.remaining = MAX_REPLAY_WORK_UNITS;
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let (constructed, report) = test_allocation_observer::measure(|| {
        if budget
            .reserve_array::<u8>(function, 1, "growth after work reset")
            .is_ok()
        {
            std::hint::black_box(Vec::<u8>::with_capacity(1));
            true
        } else {
            false
        }
    });
    assert!(
        !constructed,
        "semantic work reset restored physical allocation permission"
    );
    assert!(!report.invalid);
    // Reporting allocations are retained in the measured totals. There is no
    // assertion that a formatted refusal consumes zero physical backing.
    assert!(report.highwater <= report.allocated);
}
