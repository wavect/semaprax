use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal_site};
use crate::diagnostic::Severity;
use crate::test_allocation_observer::measure;
use std::fmt::Write as _;

struct Stack {
    bytes: [u8; 8192],
    len: usize,
}
impl Stack {
    fn new() -> Self {
        Self {
            bytes: [0; 8192],
            len: 0,
        }
    }
    fn text(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).unwrap()
    }
}
impl fmt::Write for Stack {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}
fn receipt() -> (crate::bounded_output::ReservationRefusal, ReservationSite) {
    let ((), refused, used, first, site) = with_limit_usage_refusal_site(0, || {
        assert!(!reserve_active_required(1));
        assert!(!reserve_active_required(2));
    });
    assert!(refused);
    assert_eq!(used, 0);
    let first = first.unwrap();
    assert_eq!(first.requested, 1);
    (first, site.unwrap())
}
fn diagnostic(help: bool) -> Diagnostic {
    let mut value = Diagnostic::io("SPX-T947", "primary \"message\"\n").at_path("source\t.spx");
    value.severity = Severity::Warning;
    value.span = Some(crate::ast::Span {
        start: 2,
        end: 7,
        line: 3,
        column: 4,
    });
    if help {
        value = value.with_help("existing help\n\"quoted\"");
    }
    value
}

#[test]
fn sidecar_requires_both_real_first_receipt_and_site_and_moves_all_old_fields() {
    let (first, site) = receipt();
    for (first, site) in [
        (None, None),
        (Some(first), None),
        (None, Some(site)),
        (Some(first), Some(site)),
    ] {
        let original = diagnostic(true);
        let expected = original.clone();
        let mut report = Vec::with_capacity(4);
        report.push(original);
        let (pointer, capacity) = (report.as_ptr(), report.capacity());
        let failure = WorkspaceReportFailure::existing_with_site(report, first, site);
        assert_eq!(
            matches!(&failure.0, Owner::ExistingWithSite { .. }),
            first.is_some() && site.is_some()
        );
        assert!(!failure.has_paid_report());
        assert_eq!(failure.reported().unwrap(), [expected.clone()]);
        assert_eq!(failure.reported().unwrap().as_ptr(), pointer);
        let legacy = failure.into_legacy_unfunded();
        assert_eq!(legacy, [expected]);
        assert_eq!((legacy.as_ptr(), legacy.capacity()), (pointer, capacity));
    }
}

#[test]
fn sidecar_streams_only_first_help_in_json_human_and_display_without_output_allocation() {
    let (first, site) = receipt();
    for has_help in [false, true] {
        let original = vec![
            diagnostic(has_help),
            Diagnostic::io_static("SPX-T948", "later"),
        ];
        let expected = original.clone();
        let failure = WorkspaceReportFailure::existing_with_site(original, Some(first), Some(site));
        let suffix = format!(
            " Reservation caller: {}:{}:{}.",
            site.file(),
            site.line(),
            site.column()
        );
        for json in [false, true] {
            let mut output = Stack::new();
            let (result, observed) = measure(|| failure.write_lines(&mut output, json));
            assert!(result.is_ok());
            assert_eq!(
                (
                    observed.allocations,
                    observed.reallocations,
                    observed.allocated
                ),
                (0, 0, 0)
            );
            assert_eq!(failure.reported().unwrap(), expected.as_slice());
            if json {
                let rows: Vec<serde_json::Value> = output
                    .text()
                    .lines()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect();
                let mut primary: serde_json::Value =
                    serde_json::from_str(&expected[0].json()).unwrap();
                primary["help"] = serde_json::Value::String(format!(
                    "{}{}",
                    expected[0].help.as_deref().unwrap_or(""),
                    suffix
                ));
                assert_eq!(
                    rows,
                    [primary, serde_json::from_str(&expected[1].json()).unwrap()]
                );
            } else {
                let mut display = Stack::new();
                let (result, observed) = measure(|| write!(&mut display, "{failure}"));
                assert!(result.is_ok());
                assert_eq!(
                    (
                        observed.allocations,
                        observed.reallocations,
                        observed.allocated
                    ),
                    (0, 0, 0)
                );
                assert_eq!(output.text().strip_suffix('\n').unwrap(), display.text());
                assert_eq!(output.text().matches(&suffix).count(), 1);
                assert!(output.text().contains("SPX-T948"));
            }
        }
    }
}

#[test]
fn sidecar_mapping_keeps_real_receipt_site_and_original_hint_mapper_result() {
    let (first, site) = receipt();
    let report = vec![diagnostic(true)];
    let pointer = report.as_ptr();
    let failure = WorkspaceReportFailure::existing_with_site(report, Some(first), Some(site));
    let mut calls = 0;
    let failure = failure.map_existing_diagnostics(|mut report| {
        calls += 1;
        assert_eq!(report.as_ptr(), pointer);
        report[0].help.as_mut().unwrap().push_str(" mapped hint");
        report
    });
    assert_eq!(calls, 1);
    assert_eq!(failure.reported().unwrap().as_ptr(), pointer);
    let mut output = Stack::new();
    failure.write_lines(&mut output, true).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.text()).unwrap();
    assert!(parsed["help"]
        .as_str()
        .unwrap()
        .contains(" mapped hint Reservation caller: "));
    assert!(
        matches!(failure.0, Owner::ExistingWithSite { site: retained, .. } if retained == site)
    );
}

#[test]
fn sidecar_active_budget_and_unlock_override_use_entire_original_writer() {
    let (first, site) = receipt();
    let report = vec![diagnostic(true)];
    let original = report.clone();
    let failure = WorkspaceReportFailure::existing_with_site(report, Some(first), Some(site));
    for json in [false, true] {
        let ((actual, expected), refused, _, receipt, location) =
            with_limit_usage_refusal_site(usize::MAX, || {
                let mut actual = Stack::new();
                let mut expected = Stack::new();
                failure.write_lines(&mut actual, json).unwrap();
                crate::diagnostic::writer::write_lines(&mut expected, &original, json).unwrap();
                (actual, expected)
            });
        assert!(!refused && receipt.is_none() && location.is_none());
        assert_eq!(actual.text(), expected.text());
    }
    let override_report = vec![Diagnostic::io_static("SPX-T949", "unlock selected")];
    let expected = override_report.clone();
    let failure = failure.with_unlock_override(override_report);
    assert_eq!(failure.retained_report().unwrap(), original.as_slice());
    for json in [false, true] {
        let mut actual = Stack::new();
        let mut original = Stack::new();
        failure.write_lines(&mut actual, json).unwrap();
        crate::diagnostic::writer::write_lines(&mut original, &expected, json).unwrap();
        assert_eq!(actual.text(), original.text());
    }
    assert_eq!(failure.into_legacy_unfunded(), expected);
}

#[test]
fn sidecar_writer_failure_is_immediate_and_never_changes_selected_diagnostics() {
    struct Refuse(usize);
    impl fmt::Write for Refuse {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            self.0 += 1;
            Err(fmt::Error)
        }
    }
    let (first, site) = receipt();
    for json in [false, true] {
        let report = vec![diagnostic(true), Diagnostic::io_static("SPX-T948", "later")];
        let expected = report.clone();
        let failure = WorkspaceReportFailure::existing_with_site(report, Some(first), Some(site));
        let mut sink = Refuse(0);
        let (result, observed) = measure(|| failure.write_lines(&mut sink, json));
        assert!(result.is_err());
        assert_eq!(sink.0, 1);
        assert_eq!(
            (
                observed.allocations,
                observed.reallocations,
                observed.allocated
            ),
            (0, 0, 0)
        );
        assert_eq!(failure.reported().unwrap(), expected.as_slice());
    }
}
