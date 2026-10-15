//! Actual fresh/hit/miss receivers; complete HIR and live debit equality.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
const CAP: usize = 64 * 1024 * 1024;

fn program(name: &str, vector: bool) -> crate::ast::Program {
    let parameter = if vector { "values:borrow Vec<i64>" } else { "" };
    crate::check(
        &format!("module {name}; @id(\"{name}.read\") fn read({parameter})->i64 {{0}} @id(\"{name}.main\") fn main()->i64 {{0}}"),
        format!("{name}.spx"),
    ).unwrap()
}

#[test]
fn first_cached_module_and_later_miss_pay_shared_base_and_extensions_once() {
    for first_vector in [false, true] {
        for second_vector in [false, true] {
            let programs = [
                program("first", first_vector),
                program("second", second_vector),
            ];
            let first = Input::Owned(programs[0].clone());
            let second = Input::Owned(programs[1].clone());
            let authored = BTreeMap::new();
            let ((cached, cold_right, local), refused, cold_used, refusal) =
                with_limit_usage_refusal(CAP, || {
                    let mut core =
                        crate::workspace_graph::expected_projection::CoreProviderProofs::new(
                            &authored, &programs,
                        )
                        .unwrap();
                    let before = crate::bounded_output::active_remaining().unwrap();
                    let shared_before = core.base_prelude.selected_bytes();
                    let left =
                        resolve_module_with_core(&mut core, None, &programs[0].path, &first, None)
                            .unwrap()
                            .0;
                    let local = local_resolution_bytes(before, shared_before, &core).unwrap();
                    assert!(core.base_prelude.selected_bytes() > shared_before);
                    let right =
                        resolve_module_with_core(&mut core, None, &programs[1].path, &second, None)
                            .unwrap()
                            .0;
                    (left, right, local)
                });
            assert!(!refused && refusal.is_none());
            let (warm_right, refused, warm_used, refusal) = with_limit_usage_refusal(CAP, || {
                let mut core =
                    crate::workspace_graph::expected_projection::CoreProviderProofs::new(
                        &authored, &programs,
                    )
                    .unwrap();
                let before = crate::bounded_output::active_remaining().unwrap();
                let shared_before = core.base_prelude.selected_bytes();
                crate::hir::validate_with_static_text(&cached).unwrap();
                replay_cached_prelude(&mut core, &first).unwrap();
                let replay_local = local_resolution_bytes(before, shared_before, &core).unwrap();
                // Exactly the real root's existing residual prebound. Neither
                // the telemetry nor the cached bill refunds a live byte.
                crate::workspace_graph::charge_builder_prebound(
                    local.checked_sub(replay_local).unwrap(),
                )
                .unwrap();
                resolve_module_with_core(&mut core, None, &programs[1].path, &second, None)
                    .unwrap()
                    .0
            });
            assert!(!refused && refusal.is_none());
            assert_eq!(
                warm_used, cold_used,
                "prefixes {first_vector}/{second_vector}"
            );
            assert_eq!(warm_right, cold_right);
            assert_eq!(
                crate::cache_codec::encode(&warm_right).unwrap(),
                crate::cache_codec::encode(&cold_right).unwrap()
            );
        }
    }
}

#[test]
fn cached_prelude_replay_keeps_foreign_core_refusal_sticky() {
    use crate::hir::prelude_bundle::ProducerFailure;
    use crate::test_allocation_observer::measure;
    let programs = [program("foreign", false)];
    let input = Input::Owned(programs[0].clone());
    let authored = BTreeMap::new();
    for built in [false, true] {
        let observe = |cached: bool| {
            let (outcome, refused, _, first) = with_limit_usage_refusal(CAP, || {
                let mut core =
                    crate::workspace_graph::expected_projection::CoreProviderProofs::new(
                        &authored, &programs,
                    )
                    .unwrap();
                let resolved = if built {
                    Some(
                        resolve_module_with_core(&mut core, None, &programs[0].path, &input, None)
                            .unwrap(),
                    )
                } else {
                    None
                };
                let receipt = core.base_prelude.receipt();
                let selected_bytes = core.base_prelude.selected_bytes();
                assert_eq!(receipt.is_some(), built);
                let call =
                    |core: &mut crate::workspace_graph::expected_projection::CoreProviderProofs<
                        '_,
                    >| {
                        if cached {
                            replay_cached_prelude(core, &input)
                        } else {
                            // This complete ordinary receiver predates cache billing;
                            // its whole original body is preserved in the packet.
                            resolve_module_with_core(core, None, &programs[0].path, &input, None)
                                .map(|_| ())
                        }
                    };
                let expected_parent = crate::bounded_output::ReservationRefusal {
                    requested: 0,
                    remaining: crate::bounded_output::active_remaining().unwrap(),
                    floor: 0,
                    stage: None,
                };
                let acquired = acquisitions();
                let (foreign, child_refused, _, child_first) =
                    with_limit_usage_refusal(CAP, || {
                        let ModuleFailure::Report(report) = call(&mut core).unwrap_err() else {
                            panic!("foreign caller funds the actual report owner")
                        };
                        let pointers = ACQUIRED.with(|cell| cell.get().1.unwrap());
                        assert_eq!(report.as_ptr() as usize, pointers.0);
                        assert_eq!(report.len(), 1);
                        assert_eq!(report[0].message.as_ptr() as usize, pointers.1);
                        assert_eq!(report[0].code, "SPX-H006");
                        assert_eq!(report[0].severity, crate::diagnostic::Severity::Error);
                        assert_eq!(
                            report[0].message,
                            "base prelude producer acquisition refused"
                        );
                        assert_eq!(report[0].path, None);
                        assert_eq!(report[0].span, None);
                        assert_eq!(report[0].help, None);
                        report
                    });
                assert!(!child_refused && child_first.is_none());
                assert_eq!(acquisitions() - acquired, 1);
                let acquired = acquisitions();
                let (restored, physical) = measure(|| call(&mut core).unwrap_err());
                let ModuleFailure::Setup(Refused::Budget(restored)) = restored else {
                    panic!("restored sticky parent cannot acquire another report")
                };
                assert_eq!(restored, expected_parent);
                assert_eq!(acquisitions(), acquired);
                assert_eq!(physical.allocated, 0);
                assert_eq!(physical.live, 0);
                assert!(!physical.invalid);
                assert_eq!(core.base_prelude.receipt(), receipt);
                assert_eq!(core.base_prelude.selected_bytes(), selected_bytes);
                let (selected, retry) = measure(|| {
                    core.base_prelude
                        .ensure(&mut |_| -> Result<(), Refused> {
                            panic!("selected detachment cannot retry acquisition")
                        })
                        .err()
                        .unwrap()
                });
                assert_eq!(
                    selected,
                    InvocationFailure::Producer(ProducerFailure::Detached)
                );
                assert_eq!(retry.allocated, 0);
                assert!(!retry.invalid);
                let ((), disposal) = measure(|| {
                    drop(core);
                    drop(resolved);
                });
                assert_eq!(disposal.allocated, 0);
                assert!(!disposal.invalid);
                (foreign, restored, selected)
            });
            assert!(refused, "the held parent retains its custody refusal");
            assert_eq!(first, Some(outcome.1));
            outcome
        };
        let ordinary = observe(false);
        let cached = observe(true);
        // Same-context complete six-field reports and typed restored receipts,
        // rather than comparing a child-funded report with an unfundable parent.
        assert_eq!(ordinary, cached);
        drop(ordinary);
        drop(cached);
    }
}
