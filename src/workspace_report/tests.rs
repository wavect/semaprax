//! Full same-invocation carrier transfer; output uses a caller-owned stack.
use super::*;
use crate::cleanup::ReplayContext;
use crate::cleanup_plan::{
    ReplayPhysicalReservation as Physical, SelectedReplayRefused as Refused,
    SelectedReplayReport as Acquired,
};
use crate::test_allocation_observer::measure;
use std::fmt::Write;
const FORM: SelectedReplayReportForm =
    SelectedReplayReportForm::new("SPX-T781", "paid selected replay report owning control");
fn paid() -> usize {
    std::mem::size_of::<Diagnostic>() + FORM.message().len()
}
struct Stack {
    bytes: [u8; 1024],
    len: usize,
}
impl Stack {
    fn new() -> Self {
        Self {
            bytes: [0; 1024],
            len: 0,
        }
    }
}
impl fmt::Write for Stack {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(fmt::Error)?;
        let target = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        target.copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}
fn render(error: &WorkspaceReportFailure) {
    let mut output = Stack::new();
    write!(&mut output, "{error}").unwrap();
    let mut old = Stack::new();
    write!(
        &mut old,
        "{}",
        Diagnostic::io_static(FORM.code(), FORM.message())
    )
    .unwrap();
    assert_eq!(&output.bytes[..output.len], &old.bytes[..old.len]);
}
fn acquire(limit: usize) -> Acquired {
    Acquired::try_acquire(Physical::new(limit), FORM).unwrap_or_else(|_| panic!("paid carrier"))
}
#[test]
fn initial_zero_exact_short_sticky_typed_transfer_display_and_drop_allocate_nothing() {
    for (limit, charged) in [
        (0, 0),
        (std::mem::size_of::<Diagnostic>() - 1, 0),
        (paid() - 1, std::mem::size_of::<Diagnostic>()),
    ] {
        let ((), cost) = measure(|| {
            let refusal = Acquired::try_acquire(Physical::new(limit), FORM)
                .err()
                .expect("initial refusal");
            let mut error = WorkspaceReportFailure::selected(refusal, None);
            assert!(!error.has_paid_report());
            assert!(error.reported().is_none());
            render(&error);
            let ledger = error.selected_owner_mut().unwrap().ledger_mut().unwrap();
            assert_eq!(ledger.charged(), charged);
            let first = ledger.reserve(0).unwrap_err();
            assert_eq!(ledger.reserve(usize::MAX).unwrap_err(), first);
            error = error.into_paid_report().unwrap_err();
            assert_eq!(
                error
                    .selected_owner_mut()
                    .unwrap()
                    .ledger_mut()
                    .unwrap()
                    .reserve(0)
                    .unwrap_err(),
                first
            );
            drop(error);
        });
        assert!(!cost.invalid);
        assert_eq!(
            (
                cost.allocations,
                cost.reallocations,
                cost.allocated,
                cost.live,
                cost.freed
            ),
            (0, 0, 0, 0, 0)
        );
    }
}
#[test]
fn exact_paid_setup_refusal_transfers_original_slot_text_and_ledger_through_drop() {
    let program = crate::hir::resolve(
        &crate::check(
            "module report.setup; @id(\"main\") fn main()->i64{0}",
            "report.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let functions = program.functions();
    let function = functions.get(0).expect("original single-function fixture");
    let ((), cost) = measure(|| {
        let owner = acquire(paid());
        let pointers = owner.backing();
        let refusal = owner.prepare(function).err().expect("no setup backing");
        let mut error = WorkspaceReportFailure::selected(refusal, None);
        render(&error);
        assert!(error.has_paid_report());
        let report = error.reported().unwrap();
        assert_eq!(report.as_ptr(), pointers.0);
        assert_eq!(report[0].message.as_ptr(), pointers.1);
        assert_eq!(report[0].message_owned_capacity(), FORM.message().len());
        let ledger = error.selected_owner_mut().unwrap().ledger_mut().unwrap();
        assert_eq!(ledger.charged(), paid());
        let first = ledger.reserve(0).unwrap_err();
        assert_eq!(ledger.reserve(1).unwrap_err(), first);
        let report = error.into_paid_report().unwrap();
        assert_eq!(report.as_ptr(), pointers.0);
        assert_eq!(report[0].message.as_ptr(), pointers.1);
        drop(report);
    });
    assert!(!cost.invalid);
    assert_eq!((cost.allocations, cost.reallocations), (2, 0));
    assert_eq!(
        (cost.allocated, cost.highwater, cost.live, cost.freed),
        (paid(), paid(), 0, paid())
    );
}
#[test]
fn constructor_receipt_and_same_ledger_survive_typed_owner_and_no_paid_form_remap() {
    let program = crate::hir::resolve(
        &crate::check(
            "module report.construct; @id(\"main\") fn main()->i64{0}",
            "report.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let functions = program.functions();
    let function = functions.get(0).expect("original single-function fixture");
    let (used, cost) = measure(|| {
        let owner = acquire(1 << 26);
        let pointers = owner.backing();
        let mut preparation = owner
            .prepare(function)
            .unwrap_or_else(|_| panic!("ample setup"));
        let own = Refused::constructor("retained constructor refusal");
        let mut address = std::ptr::null();
        let mut charged = 0;
        let refused = preparation
            .construct(function, |ledger, _| -> Result<(), Refused> {
                address = ledger as *const Physical;
                charged = ledger.charged();
                let first = ledger.reserve(usize::MAX).unwrap_err();
                assert_eq!(ledger.reserve(0).unwrap_err(), first);
                Err(own)
            })
            .unwrap_err();
        assert_eq!(refused, own);
        let refusal = preparation.refuse(refused);
        let other = SelectedReplayReportForm::new("SPX-T782", "different requested form");
        let mut error = WorkspaceReportFailure::selected(refusal, Some(other));
        assert!(!error.has_paid_report());
        assert!(error.reported().is_none());
        error = error.into_paid_report().unwrap_err();
        let retained = error.selected_owner_mut().unwrap();
        assert_eq!(retained.refused(), Some(own));
        assert!(retained.receipt().is_none());
        assert_eq!(retained.report().unwrap().as_ptr(), pointers.0);
        assert_eq!(retained.report().unwrap()[0].message.as_ptr(), pointers.1);
        let ledger = retained.ledger_mut().unwrap();
        assert!(std::ptr::eq(&*ledger, address));
        assert_eq!(ledger.charged(), charged);
        let first = ledger.reserve(0).unwrap_err();
        assert_eq!(ledger.reserve(1).unwrap_err(), first);
        drop(error);
        charged
    });
    assert!(!cost.invalid);
    assert!(cost.allocated <= used);
    assert!(cost.highwater <= used);
    assert_eq!(cost.live, 0);
    assert_eq!(cost.freed, cost.allocated);
}
#[test]
fn explicit_legacy_materialization_preserves_full_owned_report_and_original_pass_through() {
    let expected = FORM.legacy();
    let ((), cost) = measure(|| {
        let refusal = Acquired::try_acquire(Physical::new(0), FORM).err().unwrap();
        let report = WorkspaceReportFailure::selected(refusal, None).into_legacy_unfunded();
        assert_eq!(report, expected);
        let pointer = report.as_ptr();
        let text = report[0].message.as_ptr();
        let legacy = WorkspaceReportFailure::from(report);
        assert!(!legacy.has_paid_report());
        render(&legacy);
        let moved = legacy.into_legacy_unfunded();
        assert_eq!(moved.as_ptr(), pointer);
        assert_eq!(moved[0].message.as_ptr(), text);
        drop(moved);
    });
    assert!(!cost.invalid);
    assert_eq!((cost.allocations, cost.reallocations, cost.live), (2, 0, 0));
    assert_eq!(cost.freed, cost.allocated);
}

fn scalar_build() -> (
    crate::workspace_graph::WorkspaceGraphBuild,
    crate::workspace_graph::current_source::SourceFacts,
) {
    // Exact persisted terminated-guard body from the independent scalar owner.
    const BODY: &str = "let mut i=0; let mut total=0; while i<3 { i=i+1; if i==2 {continue;} else {0}; total=total+i; 0 } total";
    let app =
        format!("module calculator.app; @id(\"calculator.app.main\") fn main()->i64 {{ {BODY} }}");
    let sources = [
        ("src/app.spx", app.as_str()),
        (
            "src/tests.spx",
            "module calculator.tests;\n@id(\"calculator.tests.main\") fn main() -> i64 { 0 }\n",
        ),
    ]
    .into_iter()
    .map(|(path, text)| crate::workspace_graph::WorkspaceSource {
        path: path.to_owned(),
        source: crate::format::canonical(&crate::parse(text, std::path::Path::new(path)).unwrap()),
    })
    .collect();
    crate::workspace_graph::build_owned_retaining_sources_for_change(sources, 32 * 1024 * 1024)
        .unwrap()
}
#[test]
fn actual_schema_receiving_caller_keeps_zero_short_exact_paid_owner_until_final_drop() {
    let (build, sources) = scalar_build();
    assert!(
        build.source_graph_schemas_receiving_typed(&sources).is_ok(),
        "full original receiver admission before refusal controls"
    );
    // Independently literal fixed outward form; one slot plus its exact text.
    let form = SelectedReplayReportForm::new(
        "SPX-G410",
        "retained static UTF-8 source-site authority disagrees",
    );
    let bytes = std::mem::size_of::<Diagnostic>() + form.message().len();
    for (limit, expected, requests) in [
        (0, 0, 0),
        (bytes - 1, std::mem::size_of::<Diagnostic>(), 0),
        (bytes, bytes, 2),
    ] {
        let (((), cost), overflowed, used) = crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let mut error = build
                    .source_graph_schemas_receiving_typed(&sources)
                    .err()
                    .expect("same invocation refusal");
                assert_eq!(error.has_paid_report(), limit == bytes);
                let owner = error
                    .selected_owner_mut()
                    .expect("real selected replay owner");
                let ledger = owner.ledger_mut().unwrap();
                assert_eq!(ledger.charged(), expected);
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(usize::MAX).unwrap_err(), first);
                let mut out = Stack::new();
                write!(&mut out, "{error}").unwrap();
                if limit == bytes {
                    let report = error.into_paid_report().unwrap();
                    assert_eq!(report.len(), 1);
                    assert_eq!(report[0].code, form.code());
                    assert_eq!(report[0].message, form.message());
                    drop(report);
                } else {
                    error = error.into_paid_report().unwrap_err();
                    assert!(error.reported().is_none());
                    drop(error);
                }
            })
        });
        assert!(overflowed);
        assert_eq!(used, expected);
        assert!(!cost.invalid);
        assert_eq!(cost.allocations, requests);
        assert_eq!(cost.allocated, if limit == bytes { bytes } else { 0 });
        assert_eq!(cost.live, 0);
        assert_eq!(cost.freed, cost.allocated);
    }
}
#[test]
fn actual_source_drift_keeps_full_prior_literal_and_whole_receiving_debit_drop() {
    let (build, sources) = scalar_build();
    let old_success = build.source_graph_schemas_receiving(&sources).unwrap();
    assert_eq!(
        build
            .source_graph_schemas_receiving_typed(&sources)
            .unwrap(),
        old_success
    );
    // Independent owned hostile proposal; the actual retained metadata
    // already passed both original receiving comparisons above.
    let mut sources: Vec<crate::workspace_graph::WorkspaceSource> = sources
        .iter()
        .map(|source| source.input().clone())
        .collect();
    let before = sources[0].source.clone();
    let drift = "module calculator.app; @id(\"calculator.app.main\") fn main()->i64 { let mut i=0; let mut total=0; while i<3 { i=i+1; if i==2 {continue;} else {0}; total=total+i+1; 0 } total }";
    sources[0].source = crate::format::canonical(
        &crate::parse(drift, std::path::Path::new("src/app.spx")).unwrap(),
    );
    assert_ne!(sources[0].source, before);
    let old = build.source_graph_schemas_receiving(&sources).unwrap_err();
    let (((), cost), overflowed, used) =
        crate::bounded_output::with_limit_usage(32 * 1024 * 1024, || {
            measure(|| {
                let error = build
                    .source_graph_schemas_receiving_typed(&sources)
                    .unwrap_err();
                assert!(error.has_paid_report());
                assert_eq!(error.reported().unwrap(), old.as_slice());
                drop(error);
            })
        });
    assert!(!overflowed);
    assert!(!cost.invalid);
    assert!(cost.allocated <= used);
    assert!(cost.highwater <= used);
    assert_eq!(cost.live, 0);
    assert_eq!(cost.freed, cost.allocated);
}

#[test]
fn prepaid_first_budget_receipt_moves_with_affine_typed_owner_without_reformatting() {
    let program = crate::hir::resolve(
        &crate::check(
            "module report.receipt; @id(\"main\") fn main()->i64{0}",
            "report.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let functions = program.functions();
    let function = functions.get(0).expect("original single-function fixture");
    let (used, cost) = measure(|| {
        let owner = acquire(1 << 26);
        let report_pointers = owner.backing();
        let mut preparation = owner
            .prepare(function)
            .unwrap_or_else(|_| panic!("ample setup"));
        let refused = preparation
            .try_charge(function, usize::MAX, "first owning work refusal")
            .unwrap_err();
        let receipt = preparation.receipt().expect("original prepaid Diagnostic");
        let pointer = receipt.message.as_ptr();
        let capacity = receipt.message_owned_capacity();
        assert_eq!(
            preparation.try_charge(function, 0, "later refusal"),
            Err(Refused::FAILED)
        );
        assert_eq!(preparation.receipt().unwrap().message.as_ptr(), pointer);
        let mut error = WorkspaceReportFailure::selected(preparation.refuse(refused), None);
        assert!(error.has_paid_report());
        assert_eq!(error.reported().unwrap().as_ptr(), report_pointers.0);
        let retained = error.selected_owner_mut().unwrap();
        assert_eq!(retained.refused(), Some(refused));
        let receipt = retained.receipt().unwrap();
        assert_eq!(receipt.message.as_ptr(), pointer);
        assert_eq!(receipt.message_owned_capacity(), capacity);
        let ledger = retained.ledger_mut().unwrap();
        let charged = ledger.charged();
        let first = ledger.reserve(0).unwrap_err();
        assert_eq!(ledger.reserve(1).unwrap_err(), first);
        render(&error);
        drop(error);
        charged
    });
    assert!(!cost.invalid);
    assert!(cost.allocated <= used);
    assert!(cost.highwater <= used);
    assert_eq!(cost.live, 0);
    assert_eq!(cost.freed, cost.allocated);
}

#[path = "tests/caller.rs"]
mod caller;
