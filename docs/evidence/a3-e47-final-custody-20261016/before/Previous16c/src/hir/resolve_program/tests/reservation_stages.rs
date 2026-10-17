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
