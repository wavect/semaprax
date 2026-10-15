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
    let programs = [program("foreign", false)];
    let input = Input::Owned(programs[0].clone());
    let authored = BTreeMap::new();
    let ((), refused, _, refusal) = with_limit_usage_refusal(CAP, || {
        let mut core = crate::workspace_graph::expected_projection::CoreProviderProofs::new(
            &authored, &programs,
        )
        .unwrap();
        let (foreign, refused, _, refusal) = with_limit_usage_refusal(CAP, || {
            replay_cached_prelude(&mut core, &input)
                .unwrap_err()
                .into_legacy()
        });
        assert!(!refused && refusal.is_none());
        let restored = replay_cached_prelude(&mut core, &input)
            .unwrap_err()
            .into_legacy();
        assert_eq!(foreign, restored);
        assert!(core.base_prelude.receipt().is_none());
        assert_eq!(core.base_prelude.selected_bytes(), 0);
    });
    assert!(!refused && refusal.is_none());
}
