//! Pure parent-reference controls; these seeds are not authenticated history.
use super::*;
#[test]
fn owned_reduce_parent_effect_refs_use_true_recorded_and_cleanup_sequences() {
    let mut f = FoldV8::empty();
    assert!(effect_refs(&f, true).is_err());
    let mut e = effect_fold::EffectV8::consumed(22);
    e.settlement = Some(24);
    e.recorded = Some(25);
    e.cleanup_started = Some(26);
    e.observed = true;
    f.effect = Some(e);
    assert_eq!(effect_refs(&f, true).unwrap(), (24, 25, 27));
    assert!(effect_refs(&f, false).is_err());
    f.effect.as_mut().unwrap().recorded = None;
    assert!(effect_refs(&f, true).is_err());
    f.effect.as_mut().unwrap().recorded = Some(25);
    f.effect.as_mut().unwrap().cleanup_started = Some(u32::MAX);
    assert!(effect_refs(&f, true).is_err());
}
#[test]
fn owned_reduce_parent_records_consumption_with_checked_aggregate_arithmetic() {
    let mut f = FoldV8::empty();
    recorded(&mut f, 9).unwrap();
    recorded(&mut f, 0).unwrap();
    assert_eq!(f.consumed_recorded, 9);
    f.consumed_recorded = u64::MAX;
    assert_eq!(recorded(&mut f, 1), Err(SourceJournalError::Capacity));
}
