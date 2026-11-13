//! Exact cumulative constructor ownership; no lexical admission changes.
use super::*;

fn tiny() -> ResolvedProgram {
    let source = crate::check(
        "module recovery.index; @id(\"a\") fn a()->i64 {1} @id(\"b\") fn main()->i64 {2}",
        "recovery.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn literal_bytes(anchor: &ResolvedFunction) -> usize {
    // Independent actual setup/stack/carrier inventory, not measured usage.
    std::mem::size_of::<PhysicalReservation>()
        + 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>()
        + anchor.id.as_str().len()
        + 512
        + std::mem::size_of::<Session<'_>>()
        + std::mem::size_of::<selected_callees::Context<'_>>()
        + 2 * std::mem::size_of::<&ResolvedFunction>()
}
fn observed(
    context: &dyn ReplayContext,
    anchor: &ResolvedFunction,
    work: usize,
    bytes: usize,
) -> (
    (bool, (usize, usize)),
    crate::test_allocation_observer::Metrics,
) {
    crate::test_allocation_observer::measure(|| {
        let preparation = Preparation::with_limits(anchor, work, bytes).unwrap();
        let account = preparation.budget.physical.as_deref().unwrap() as *const PhysicalReservation;
        let paid_text = preparation.budget.refusal_text.as_ref().unwrap().as_ptr();
        match Session::try_from_preparation(context, anchor, preparation) {
            Ok(session) => {
                assert_eq!(
                    session.budget.physical.as_deref().unwrap() as *const PhysicalReservation,
                    account
                );
                assert_eq!(session.indexed.storage_members().len(), 2);
                assert_eq!(session.indexed.storage_members()[0].id.as_str(), "a");
                assert_eq!(session.indexed.storage_members()[1].id.as_str(), "b");
                let usage = session.usage();
                drop(session);
                (true, usage)
            }
            Err((mut preparation, error)) => {
                assert_eq!(
                    preparation.budget.physical.as_deref().unwrap() as *const PhysicalReservation,
                    account
                );
                assert_eq!(error.message.as_ptr(), paid_text);
                assert_eq!(error.code, "SPX-H006");
                assert!(!error.message.is_empty()); // Actual first report, not a replacement.
                let usage = preparation.usage();
                assert!(preparation.failed);
                assert!(preparation.probe_zero(anchor).is_err());
                assert_eq!(preparation.usage(), usage);
                // First Diagnostic and exact recovered owner both remain observed.
                drop(error);
                drop(preparation);
                (false, usage)
            }
        }
    })
}
#[test]
fn recovering_constructor_exact_work_backing_and_first_report_drop_keep_one_owner() {
    let program = tiny();
    let context = primitive_fixture::Context::new(&program);
    let anchor = &program.functions[0];
    assert_eq!(program.functions.len(), 2);
    assert_eq!(anchor.id.as_str(), "a");
    // Two member probes + one heap comparison + one uniqueness comparison.
    // Both IDs are one byte: each comparison costs two units.
    let work = 2 + 2 + 2;
    let bytes = literal_bytes(anchor);
    for (work, bytes, successful, allocations) in [
        (work, bytes, true, 3),
        (work - 1, bytes, false, 3),
        (work, bytes - 1, false, 2),
    ] {
        let ((success, usage), observed) = observed(&context, anchor, work, bytes);
        assert_eq!(success, successful);
        assert_eq!(usage.0, 0);
        assert!(usage.1 <= bytes);
        if success {
            assert_eq!(usage.1, bytes);
        }
        assert!(!observed.invalid);
        assert_eq!(observed.allocations, allocations);
        assert_eq!(observed.reallocations, 0);
        assert_eq!(observed.live, 0);
        let heap = std::mem::size_of::<PhysicalReservation>()
            + anchor.id.as_str().len()
            + 512
            + if allocations == 3 {
                2 * std::mem::size_of::<&ResolvedFunction>()
            } else {
                0
            };
        assert_eq!(observed.allocated, heap);
        assert_eq!(observed.highwater, heap);
        assert_eq!(observed.freed, heap);
        assert!(observed.allocated <= usage.1);
        assert!(observed.highwater <= usage.1);
    }
}
#[test]
fn recovering_constructor_structural_refusal_keeps_paid_first_diagnostic_and_temporary_drop() {
    let mut program = tiny();
    program.functions[1].id = program.functions[0].id.clone();
    let context = primitive_fixture::Context::new(&program);
    let anchor = &program.functions[0];
    let bytes = literal_bytes(anchor);
    let ((successful, usage), observed) = crate::test_allocation_observer::measure(|| {
        let preparation = Preparation::with_limits(anchor, MAX_REPLAY_WORK_UNITS, bytes).unwrap();
        let (mut preparation, first) =
            match Session::try_from_preparation(&context, anchor, preparation) {
                Ok(_) => panic!("duplicate identities must refuse"),
                Err(refused) => refused,
            };
        assert_eq!(first.code, "SPX-H006");
        assert_eq!(first.message,
            "cleanup plan for function `a` failed independent replay: selected callee inventory repeats a declaration identity");
        let usage = preparation.usage();
        assert!(preparation.probe_zero(anchor).is_err());
        assert_eq!(preparation.usage(), usage);
        assert_eq!(first.code, "SPX-H006"); // Original selected report remains owned.
        drop(first);
        drop(preparation);
        (false, usage)
    });
    assert!(!successful);
    assert_eq!(usage, (0, bytes));
    assert!(!observed.invalid);
    assert_eq!(observed.allocations, 3);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.live, 0);
    let heap = std::mem::size_of::<PhysicalReservation>()
        + anchor.id.as_str().len()
        + 512
        + 2 * std::mem::size_of::<&ResolvedFunction>();
    assert_eq!(observed.allocated, heap);
    assert_eq!(observed.highwater, heap);
    assert_eq!(observed.freed, heap);
    assert!(observed.allocated <= usage.1);
    assert!(observed.highwater <= usage.1);
}
