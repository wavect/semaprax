//! Complete original help receiver and exact diagnostic branch controls.
use super::*;
fn original_help(maximum: usize) -> String {
    let precharge = BUILDER_PHASE.with(Cell::get) == BuilderPhase::Precharge;
    let modules = PRECHARGE_MODULES.with(Cell::get);
    let forecast = PRECHARGE_BYTES.with(Cell::get);
    let dominant = PRECHARGE_DOMINANT.with(|dominant| dominant.borrow().clone());
    let pending = PRECHARGE_PENDING.with(|pending| pending.borrow().clone());
    if let (true, Some(path)) = (precharge, pending) {
        return crate::bounded_output::budgeted_format(format_args!(
            "the static admission pre-charge refused before the graph builder ran, while forecasting module `{path}` on its own: that one module does not fit the {maximum}-byte budget, which already carries {forecast} bytes for {modules} earlier module(s). Splitting `{path}` into several modules cannot reduce the forecast, because the pre-charge sums every reachable module; reducing its admitted source can."
        ));
    }
    let sequential = PRECHARGE_SEQUENTIAL.with(|sequential| sequential.borrow().clone());
    if let (true, Some(step)) = (precharge, sequential) {
        let SequentialRefusal {
            path,
            earlier,
            retained,
            additional,
            resolving,
        } = step;
        return if resolving {
            crate::bounded_output::budgeted_format(format_args!(
                "the static admission pre-charge refused before the graph builder ran, in its final sequential uncached forecast: {earlier} earlier module(s) retain {retained} bytes of filtered output, and resolving `{path}` adds its complete {additional}-byte forecast, which exceeds the {maximum}-byte budget. Each complete module forecast is charged only while that module resolves; reducing the admitted source of `{path}` or the output retained by earlier modules can reduce this peak."
            ))
        } else {
            crate::bounded_output::budgeted_format(format_args!(
                "the static admission pre-charge refused before the graph builder ran, in its final sequential uncached forecast: {earlier} earlier module(s) retain {retained} bytes of filtered output, and `{path}` retains {additional} more bytes, which exceeds the {maximum}-byte budget. Retained output sums every reachable module, so splitting one module into several cannot reduce it; removing a dependency or reducing total reachable source can."
            ))
        };
    }
    match (precharge, dominant) {
        (true, Some((path, bytes))) => crate::bounded_output::budgeted_format(format_args!(
            "the static admission pre-charge refused before the graph builder ran: forecasting {modules} reachable module(s) already reaches {forecast} bytes of the {maximum}-byte budget, and `{path}` alone forecasts {bytes} bytes of retained resolver state. The pre-charge sums every reachable module, so splitting one module into several cannot reduce it; removing a dependency or reducing total reachable source can."
        )),
        (true, None) => crate::bounded_output::budgeted_format(format_args!(
            "the static admission pre-charge refused before the graph builder ran, and before any single module was fully costed against the {maximum}-byte budget."
        )),
        (false, Some((path, bytes))) => crate::bounded_output::budgeted_format(format_args!(
            "live graph construction, not the static admission pre-charge, exceeded the budget: the pre-charge forecast {forecast} bytes for {modules} module(s) and fit within {maximum}, with `{path}` the largest at {bytes} bytes. Reducing reachable source is the only lever; the forecast itself was not the refusing bound."
        )),
        (false, None) => crate::bounded_output::budgeted_format(format_args!(
            "live graph construction, not the static admission pre-charge, exceeded the {maximum}-byte budget while allocating workspace graph structure."
        )),
    }
}

struct Restore {
    phase: BuilderPhase,
    modules: usize,
    bytes: usize,
    dominant: Option<(String, usize)>,
    pending: Option<String>,
    sequential: Option<SequentialRefusal>,
}
impl Restore {
    fn capture() -> Self {
        Self {
            phase: BUILDER_PHASE.with(Cell::get),
            modules: PRECHARGE_MODULES.with(Cell::get),
            bytes: PRECHARGE_BYTES.with(Cell::get),
            dominant: PRECHARGE_DOMINANT.with(|v| v.borrow().clone()),
            pending: PRECHARGE_PENDING.with(|v| v.borrow().clone()),
            sequential: PRECHARGE_SEQUENTIAL.with(|v| v.borrow().clone()),
        }
    }
}
impl Drop for Restore {
    fn drop(&mut self) {
        BUILDER_PHASE.with(|v| v.set(self.phase));
        PRECHARGE_MODULES.with(|v| v.set(self.modules));
        PRECHARGE_BYTES.with(|v| v.set(self.bytes));
        PRECHARGE_DOMINANT.with(|v| *v.borrow_mut() = self.dominant.take());
        PRECHARGE_PENDING.with(|v| *v.borrow_mut() = self.pending.take());
        PRECHARGE_SEQUENTIAL.with(|v| *v.borrow_mut() = self.sequential.take());
    }
}
fn fixture(case: usize) {
    begin_precharge_pass();
    BUILDER_PHASE.with(|v| {
        v.set(if case == 0 || case == 6 {
            BuilderPhase::Live
        } else {
            BuilderPhase::Precharge
        })
    });
    PRECHARGE_MODULES.with(|v| v.set(3));
    PRECHARGE_BYTES.with(|v| v.set(12345));
    if case > 1 {
        PRECHARGE_DOMINANT.with(|v| *v.borrow_mut() = Some(("dominant.λ\0🙂".to_owned(), 1234)));
    }
    if case == 3 || case == 7 {
        PRECHARGE_PENDING.with(|v| *v.borrow_mut() = Some("pending.λ\0🙂".to_owned()));
    }
    if case == 4 || case == 5 || case == 7 {
        PRECHARGE_SEQUENTIAL.with(|v| {
            *v.borrow_mut() = Some(SequentialRefusal {
                path: "sequential.λ\0🙂".to_owned(),
                earlier: 2,
                retained: 777,
                additional: 888,
                resolving: case != 5,
            })
        });
    }
}
fn original_limit_error(field: &'static str, maximum: usize) -> Diagnostic {
    let error = graph_error(
        "SPX-G171",
        crate::bounded_output::budgeted_format(format_args!(
            "Workspace Semantic Graph `{field}` exceeds {maximum}"
        )),
    );
    if field != "builder_bytes" {
        return error;
    }
    let help = original_help(maximum);
    if help.is_empty() {
        return error;
    }
    error.with_help(help)
}

#[test]
fn every_help_branch_keeps_full_original_diagnostic_and_actual_complete_drop() {
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;
    let _restore = Restore::capture();
    for case in 0..8 {
        for empty_paths in [false, true] {
            fixture(case);
            if empty_paths {
                PRECHARGE_DOMINANT.with(|v| {
                    if let Some((path, _)) = v.borrow_mut().as_mut() {
                        path.clear();
                    }
                });
                PRECHARGE_PENDING.with(|v| {
                    if let Some(path) = v.borrow_mut().as_mut() {
                        path.clear();
                    }
                });
                PRECHARGE_SEQUENTIAL.with(|v| {
                    if let Some(step) = v.borrow_mut().as_mut() {
                        step.path.clear();
                    }
                });
            }
            // The original clones dominant and pending before its pending return.
            // It clones sequential only when that early return is not selected.
            let pending_returns = BUILDER_PHASE.with(Cell::get) == BuilderPhase::Precharge
                && PRECHARGE_PENDING.with(|v| v.borrow().is_some());
            let original_cloned_path_bytes = PRECHARGE_DOMINANT
                .with(|v| v.borrow().as_ref().map_or(0, |(path, _)| path.len()))
                + PRECHARGE_PENDING.with(|v| v.borrow().as_ref().map_or(0, String::len))
                + if pending_returns {
                    0
                } else {
                    PRECHARGE_SEQUENTIAL
                        .with(|v| v.borrow().as_ref().map_or(0, |step| step.path.len()))
                };
            let (old, refused, _, first) = with_limit_usage_refusal(1024 * 1024, || {
                original_limit_error("builder_bytes", 65536)
            });
            assert!(!refused && first.is_none());
            let (((), actual), refused, _, first) = with_limit_usage_refusal(1024 * 1024, || {
                measure(|| {
                    let current = limit_error("builder_bytes", 65536);
                    assert_eq!(current, old);
                    drop(current);
                })
            });
            assert!(!refused && first.is_none());
            assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
            let ((), old_actual) = measure(|| {
                let old_again = original_limit_error("builder_bytes", 65536);
                assert_eq!(old_again, old);
                drop(old_again);
            });
            assert!(
                !old_actual.invalid
                    && old_actual.live == 0
                    && old_actual.allocated == old_actual.freed
            );
            if original_cloned_path_bytes > 0 {
                assert!(
                    actual.allocated < old_actual.allocated,
                    "case {case}, empty_paths {empty_paths}: original cloned nonempty paths"
                );
            } else {
                assert_eq!(
                    actual.allocated, old_actual.allocated,
                    "case {case}, empty_paths {empty_paths}: original cloned no path backing"
                );
            }
        }
    }
}
#[test]
fn zero_and_sticky_help_do_not_clone_paths_or_replace_first_refusal() {
    use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
    use crate::test_allocation_observer::measure;
    let _restore = Restore::capture();
    for case in 0..8 {
        fixture(case);
        let (old, old_refused, old_paid, old_first) = with_limit_usage_refusal(0, || {
            assert!(!reserve_active_required(1));
            original_limit_error("builder_bytes", 65536)
        });
        let (((), actual), refused, paid, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                assert!(!reserve_active_required(1));
                let current = limit_error("builder_bytes", 65536);
                assert_eq!(current, old);
                drop(current);
                assert!(!reserve_active_required(0));
            })
        });
        assert!(old_refused && refused && old_paid == 0 && paid == 0);
        assert_eq!(first, old_first);
        assert_eq!(first.unwrap().requested, 1);
        assert!(!actual.invalid && actual.allocated == 0 && actual.live == 0);
    }
}
#[test]
fn complete_borrowed_help_source_restores_full_original_without_normalization() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/borrowed-refusal-help-824/diagnostics.before.txt"
            ))))
        ),
        "c480b975897767ea2516b73112dd1547ea107841cbbe0537ed09b1de87b4b8bc"
    );
    let record: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/borrowed-refusal-help-824/exact-source-inverse.json"
    )))
    .unwrap();
    let mut current = include_str!("../diagnostics.rs").to_owned();
    for edit in record["edits"].as_array().unwrap().iter().rev() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(current.matches(after).count(), 1);
        current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/borrowed-refusal-help-824/diagnostics.before.txt"
        ))
    );
    let saved = include_str!("borrowed_help_tests.rs");
    // The primary formatter's placeholder boundaries are allocation behavior.
    // Bind the complete saved wrapper, not a byte-equivalent hardcoded variant.
    let wrapper_start = saved.find("fn original_limit_error(").unwrap();
    let wrapper_end = saved[wrapper_start..].find("\n#[test]").unwrap() + wrapper_start;
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/borrowed-refusal-help-824/diagnostics.before.txt"
    ));
    let original_start = archive.find("pub(super) fn limit_error(").unwrap();
    let original_end = archive[original_start..].find("/// Which of").unwrap() + original_start;
    assert_eq!(
        saved[wrapper_start..wrapper_end]
            .trim_end()
            .replacen("fn original_limit_error(", "pub(super) fn limit_error(", 1)
            .replacen(
                "let help = original_help(maximum);",
                "let help = builder_bytes_help(maximum);",
                1
            ),
        archive[original_start..original_end].trim_end()
    );
    let start = saved
        .find("fn original_help(maximum: usize) -> String {")
        .unwrap();
    let end = saved[start..].find("\nstruct Restore").unwrap() + start;
    assert_eq!(
        saved[start..end]
            .trim_end()
            .replacen("fn original_help(", "fn builder_bytes_help(", 1),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/borrowed-refusal-help-824/formatter.before.txt"
        ))
        .trim_end()
    );
}

#[test]
fn exhausted_default_builder_message_is_complete_without_backing_or_refusal_drift() {
    use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
    use crate::test_allocation_observer::measure;
    let _restore = Restore::capture();
    for case in 0..8 {
        fixture(case);
        for limit in [0, 1] {
            let (old, old_refused, old_paid, old_first) = with_limit_usage_refusal(limit, || {
                assert!(!reserve_active_required(limit + 1));
                original_limit_error("builder_bytes", MAX_BUILDER_BYTES)
            });
            assert!(
                old.message.is_empty(),
                "complete original exhausted-message witness"
            );
            let ((current, actual), refused, paid, first) = with_limit_usage_refusal(limit, || {
                assert!(!reserve_active_required(limit + 1));
                measure(|| limit_error("builder_bytes", MAX_BUILDER_BYTES))
            });
            let mut expected = old;
            expected.message = std::borrow::Cow::Borrowed(
                "Workspace Semantic Graph `builder_bytes` exceeds 67108864",
            );
            assert_eq!(current, expected);
            assert!(matches!(current.message, std::borrow::Cow::Borrowed(_)));
            assert!(old_refused && refused);
            assert_eq!((paid, first), (old_paid, old_first));
            assert_eq!(first.unwrap().requested, limit + 1);
            assert!(!actual.invalid);
            assert_eq!((actual.allocated, actual.freed, actual.live), (0, 0, 0));
        }
        let (old, old_refused, old_paid, old_first) = with_limit_usage_refusal(1 << 20, || {
            original_limit_error("builder_bytes", MAX_BUILDER_BYTES)
        });
        let (current, refused, paid, first) =
            with_limit_usage_refusal(1 << 20, || limit_error("builder_bytes", MAX_BUILDER_BYTES));
        assert_eq!(current, old);
        assert!(!old_refused && !refused);
        assert_eq!((paid, first), (old_paid, old_first));
    }
}
