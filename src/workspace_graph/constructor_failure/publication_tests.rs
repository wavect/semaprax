//! Partial825: real preowned setup output, not zero-budget legacy funding.
use super::{
    attempt, before_publication,
    receiving::{CoreFailure, Setup},
    report::Owner,
};
use crate::bounded_output::{
    active_remaining, clear_active_floor, reserve_active_required, set_active_floor,
    with_limit_usage_refusal,
};
use crate::test_allocation_observer::{current_metrics, measure};
use crate::workspace_graph::diagnostics;

fn setups() -> [Setup; 5] {
    [
        Setup::NoOwner,
        Setup::Allocation,
        Setup::Capacity,
        Setup::Detached,
        Setup::Arithmetic,
    ]
}

#[test]
fn preowned_setup_whole_old_current_all_six_fields_and_full_drop_match() {
    let _scope = diagnostics::invocation_scope();
    for setup in setups() {
        let expected = CoreFailure::Setup(setup).into_legacy_unfunded();
        for old in [true, false] {
            let (((), physical), refused, used, first) =
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        let (result, publication) = if old {
                            before_publication::attempt::run::<()>(usize::MAX, || Err(setup.into()))
                                .into_parts()
                        } else {
                            attempt::run::<()>(usize::MAX, || Err(setup.into())).into_parts()
                        };
                        assert!(publication.is_none());
                        let report = result.unwrap_err().into_legacy_unfunded();
                        // Diagnostic Eq compares every public field, including None.
                        assert_eq!(report, expected);
                        drop(report);
                    })
                });
            assert!(!refused && first.is_none());
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
            assert!(physical.allocated <= used && physical.highwater <= used);
            eprintln!("#825 preowned setup={setup:?} old={old} paid={used} actual={physical:?}");
        }
    }
}

#[test]
fn preowned_setup_public_conversion_after_restore_allocates_nothing_and_never_queries_foreign_payer(
) {
    let (attempt, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        attempt::run::<()>(usize::MAX, || Err(Setup::Capacity.into()))
    });
    assert!(!refused && first.is_none());
    let (result, publication) = attempt.into_parts();
    assert!(publication.is_none());
    let error = result.unwrap_err();
    assert!(matches!(&error, CoreFailure::Received { .. }));
    let ((report, physical), refused, consumed, first) = with_limit_usage_refusal(1, || {
        let before = active_remaining();
        let moved = measure(|| error.into_legacy_unfunded());
        assert_eq!(active_remaining(), before);
        moved
    });
    assert_eq!(physical.allocated, 0);
    assert!(!refused && consumed == 0 && first.is_none());
    assert_eq!(
        report,
        CoreFailure::Setup(Setup::Capacity).into_legacy_unfunded()
    );
    drop(report);
}

#[test]
fn preowned_setup_exact_short_zero_and_sticky_keep_primary_and_do_not_enter_core() {
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut entered = false;
                let (result, publication) = attempt::run::<()>(usize::MAX, || {
                    entered = true;
                    Err(Setup::Allocation.into())
                })
                .into_parts();
                assert!(publication.is_none());
                if entered {
                    assert!(matches!(&result, Err(CoreFailure::Received { .. })));
                    drop(result.unwrap_err().into_legacy_unfunded());
                } else {
                    assert!(matches!(
                        &result,
                        Err(CoreFailure::Setup(Setup::Budget { .. }))
                    ));
                    let at = current_metrics().allocated;
                    assert!(!reserve_active_required(0));
                    drop(result); // The unfunded compatibility API is NOT invoked.
                    assert_eq!(current_metrics().allocated, at);
                }
                entered
            })
        })
    };
    let full = run(usize::MAX);
    assert!(full.0 .0 && !full.1 && full.3.is_none());
    let exact = full.2;
    for limit in [0, exact - 1, exact] {
        let ((entered, physical), refused, used, first) = run(limit);
        assert_eq!(entered, limit == exact);
        assert_eq!(refused, limit != exact);
        assert_eq!(first.is_some(), limit != exact);
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
        assert!(physical.allocated <= used);
        if !entered {
            assert_eq!(physical.allocated, 0);
        }
    }
    let (((), physical), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let (result, publication) = attempt::run::<()>(usize::MAX, || {
                assert!(!reserve_active_required(usize::MAX));
                Err(Setup::Capacity.into())
            })
            .into_parts();
            assert!(matches!(&result, Err(CoreFailure::Setup(Setup::Capacity))));
            let report = publication.expect("existing sticky owner stays authoritative");
            assert_eq!(report[0].code, "SPX-G171");
            drop(result);
            drop(report);
        })
    });
    assert!(refused && first.is_some());
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
}

#[test]
fn preowned_setup_foreign_restored_and_floor_paths_keep_affine_authority() {
    let (((), physical), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let owner = Owner::acquire(usize::MAX, 0).unwrap();
            let remaining = active_remaining().unwrap();
            assert!(set_active_floor(remaining));
            let published = owner
                .seal_setup(Setup::Arithmetic)
                .unwrap_or_else(|_| panic!("preowned floor output"));
            assert_eq!(active_remaining(), Some(remaining));
            clear_active_floor();
            drop(published.into_report());
        })
    });
    assert!(!refused && first.is_none());
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    let ((), original_refused, _, original_first) = with_limit_usage_refusal(usize::MAX, || {
        let owner = Owner::acquire(usize::MAX, 0).unwrap();
        let ((unpublished, physical), foreign_refused, spent, first) =
            with_limit_usage_refusal(3, || {
                measure(|| match owner.seal_setup(Setup::Capacity) {
                    Err(unpublished) => unpublished,
                    Ok(_) => panic!("foreign publication admitted"),
                })
            });
        assert!(matches!(unpublished.refusal, Setup::Detached));
        assert_eq!(physical.allocated, 0);
        assert!(!foreign_refused && spent == 0 && first.is_none());
        let (again, physical) = measure(|| unpublished.owner.seal_setup(Setup::Allocation));
        let unpublished = match again {
            Err(value) => value,
            Ok(_) => panic!("restored publication revived"),
        };
        assert!(matches!(unpublished.refusal, Setup::Detached));
        assert_eq!(physical.allocated, 0);
        drop(unpublished.owner);
    });
    assert!(original_refused);
    assert_eq!(original_first.unwrap().requested, 0);
}

#[test]
fn preowned_setup_dynamic_budget_and_legacy_many_remain_explicit_residuals() {
    let _scope = diagnostics::invocation_scope();
    let _phase = diagnostics::precharge_scope();
    diagnostics::begin_precharge_pass();
    let (attempt, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        attempt::run::<()>(usize::MAX, || {
            diagnostics::begin_precharge_module("late.λ\0🙂.spx");
            Err(Setup::Budget {
                maximum: usize::MAX,
            }
            .into())
        })
    });
    assert!(!refused && first.is_none());
    let (result, publication) = attempt.into_parts();
    assert!(publication.is_none());
    assert!(matches!(
        &result,
        Err(CoreFailure::Setup(Setup::Budget { .. }))
    ));
    // Explicit ordinary compatibility remains allocating; it is not priced by
    // the discarded report slot or described as funded by this successor.
    let expected = diagnostics::limit_error("builder_bytes", usize::MAX);
    let report = result.unwrap_err().into_legacy_unfunded();
    assert_eq!(report.as_slice(), std::slice::from_ref(&expected));
    assert!(report[0].help.as_ref().unwrap().contains("late.λ\0🙂.spx"));
    drop(report);
    let legacy = vec![expected];
    let pointer = legacy.as_ptr();
    let (attempt, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        attempt::run::<()>(usize::MAX, || Err(CoreFailure::Legacy(legacy)))
    });
    assert!(!refused && first.is_none());
    let (result, publication) = attempt.into_parts();
    assert!(publication.is_none());
    assert!(matches!(&result, Err(CoreFailure::Legacy(_))));
    let report = result.unwrap_err().into_legacy_unfunded();
    assert_eq!(report.as_ptr(), pointer);
    drop(report);
}

#[test]
fn preowned_setup_complete_old_public_conversions_and_compiled_report_attempt_remain_exact() {
    let old_report = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/preowned-setup-publication-825/report.rs.before.txt"
    ));
    let compiled = include_str!("before_publication/report.rs")
        .replace("#[path = \"../report/stages.rs\"]\n", "")
        .replace("#[path = \"../report/tests.rs\"]\n", "");
    assert_eq!(compiled, old_report);
    assert_eq!(
        include_str!("before_publication/attempt.rs"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/preowned-setup-publication-825/attempt.rs.before.txt"
        ))
    );
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/preowned-setup-publication-825/receiving.rs.before.txt"
    ));
    let current = include_str!("receiving.rs");
    let declaration =
        "    pub(in crate::workspace_graph) fn into_legacy_unfunded(self) -> Vec<Diagnostic> {";
    let finish = "\n}\nimpl Receiver {";
    assert_eq!(
        &current[current.find(declaration).unwrap()..current.find(finish).unwrap()],
        &old[old.find(declaration).unwrap()..old.find(finish).unwrap()]
    );
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/preowned-setup-publication-825/constructor_failure.rs.before.txt"
    ));
    let current = include_str!("../constructor_failure.rs");
    let declaration = "    pub(super) fn into_legacy_unfunded(self) -> Vec<Diagnostic> {";
    let finish = "\n}\nimpl From<Vec<Diagnostic>> for Failure {";
    assert_eq!(
        &current[current.find(declaration).unwrap()..current.find(finish).unwrap()],
        &old[old.find(declaration).unwrap()..old.find(finish).unwrap()]
    );
    let union = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/workspace/semantic_graph/source_inventory.rs"
    ));
    for path in [
        "before_publication.rs",
        "before_publication/report.rs",
        "before_publication/attempt.rs",
        "publication_tests.rs",
    ] {
        let include = format!("src/workspace_graph/constructor_failure/{path}\"");
        assert_eq!(
            union.matches(&include).count(),
            1,
            "complete workspace-owned child joins"
        );
    }
}
