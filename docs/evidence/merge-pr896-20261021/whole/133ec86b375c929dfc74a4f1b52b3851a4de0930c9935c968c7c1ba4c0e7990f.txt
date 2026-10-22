use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationStage};

#[test]
fn actual_resolver_lowering_stage_keeps_complete_hir_and_first_refusal() {
    let program = parsed(
        "module stage; @id(\"stage.main\") fn main() -> i64 { let x = 3; x + 4 }",
        "stage.spx",
    );
    let ordinary = resolver(&program).resolve().unwrap().0;
    let prepared = resolver(&program);
    let (bounded, overflow, _, refusal) =
        with_limit_usage_refusal(64 * 1024 * 1024, || prepared.resolve());
    assert!(!overflow);
    assert_eq!(refusal, None);
    let bounded = bounded.unwrap().0;
    assert_eq!(
        crate::cache_codec::encode(&ordinary).unwrap(),
        crate::cache_codec::encode(&bounded).unwrap()
    );
    let prepared = resolver(&program);
    let (_, _, charged, refusal) = with_limit_usage_refusal(0, || prepared.resolve());
    assert_eq!(charged, 0);
    assert_eq!(refusal.unwrap().stage, Some(ReservationStage::HirLowering));
}

#[test]
fn template_selected_name_minimum_and_static_refusal_remain_complete() {
    let program = parsed(
        "module name.funding; @id(\"name.identity\") fn identity<T>(value:T)->T {value} @id(\"name.main\") fn main()->i64 {0}",
        "name-funding.spx",
    );
    let prepared = resolver(&program);
    let function = &program.functions[0];
    let ordinary = prepared.resolve_function_template(function).unwrap();
    let (bounded, refused, minimum, first) = with_limit_usage_refusal(64 * 1024 * 1024, || {
        prepared.resolve_function_template(function)
    });
    assert!(!refused && first.is_none());
    assert_eq!(bounded.unwrap(), ordinary);
    assert!(minimum > function.name.len());
    for limit in [0, minimum - 1, minimum] {
        let baseline =
            with_limit_usage_refusal(limit, || prepared.resolve_function_template(function));
        let observed = with_limit_usage_refusal(limit, || {
            let result = prepared.resolve_function_template(function);
            if result.is_err() {
                assert!(!crate::bounded_output::reserve_active(0));
            }
            result
        });
        assert_eq!(observed, baseline);
        let (result, refused, charged, first) = observed;
        if limit == minimum {
            assert!(!refused && first.is_none());
            assert_eq!(charged, minimum);
            assert_eq!(result.unwrap(), ordinary);
        } else {
            assert!(refused && first.is_some());
            let diagnostic = result.unwrap_err();
            assert_eq!(diagnostic.code, "SPX-H006");
            assert_eq!(diagnostic.message_owned_capacity(), 0);
            assert_eq!(diagnostic.path, None);
            assert_eq!(diagnostic.span, None);
            assert_eq!(diagnostic.help, None);
            assert_eq!(diagnostic.severity, crate::diagnostic::Severity::Error);
        }
    }
}
