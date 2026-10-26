#![allow(
    clippy::duplicate_mod,
    reason = "Current and complete immediate-before owners independently compile the original cursor tests."
)]
use super::*;
use crate::test_allocation_observer::measure;

fn program() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module cursor.funding; @id(\"cursor.main\") fn main() -> i64 { 0 }",
            "cursor-funding.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn header_bytes() -> usize {
    size_of::<ArenaCapacity<'_, '_>>() + 2 * size_of::<FactFrame<'_>>() + size_of::<usize>()
}
fn growth_bytes(slots: usize) -> usize {
    2 * size_of::<Vec<FactFrame<'_>>>()
        + size_of::<FactFrame<'_>>()
        + slots * size_of::<FactFrame<'_>>()
}
fn held_charge(owner: &ArenaCapacity<'_, '_>) -> usize {
    match &owner.base.physical {
        Payer::Selected(physical) => physical.charged(),
        _ => unreachable!(),
    }
}
fn check_all_growth_and_release(function: &ResolvedFunction) {
    let mut physical = PhysicalReservation::new(usize::MAX);
    let (_, metrics) = measure(|| {
        let mut owner = ArenaCapacity::new(function, Payer::Selected(&mut physical)).unwrap();
        let mut expected = header_bytes();
        let mut work = 0;
        for slots in [2, 4, 8, 16, 32, 64, 128, 256, 512, 514] {
            owner.grow_frames(slots).unwrap();
            expected += growth_bytes(slots);
            work += slots;
            assert_eq!(held_charge(&owner), expected);
            assert_eq!(owner.base.work, MAX_REPLAY_WORK_UNITS - work);
            assert_eq!(owner.frames.len(), slots);
            if slots > 2 {
                let (cursor, started) = owner.frames[0].unwrap();
                assert!(std::ptr::eq(cursor.expression(), &function.body));
                assert!(!started);
            }
            owner.frames[0] = Some((
                expression_children::ReplayChildCursor::new(&function.body),
                false,
            ));
            let before = held_charge(&owner);
            let before_work = owner.base.work;
            owner.grow_frames(slots).unwrap();
            assert_eq!(held_charge(&owner), before);
            assert_eq!(owner.base.work, before_work);
        }
        assert_eq!(
            owner.grow_frames(515).unwrap_err().detail,
            "HIR expression fact traversal exceeds the admitted depth"
        );
        drop(owner);
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    let slots = 2 + 4 + 8 + 16 + 32 + 64 + 128 + 256 + 512 + 514;
    assert_eq!(metrics.allocated, slots * size_of::<FactFrame<'_>>());
    assert_eq!(metrics.highwater, (512 + 514) * size_of::<FactFrame<'_>>());
}
fn check_short_sticky_and_same_owner_reuse(function: &ResolvedFunction) {
    for at in 0..10 {
        let sizes = [2, 4, 8, 16, 32, 64, 128, 256, 512, 514];
        let exact = header_bytes()
            + sizes[..=at]
                .iter()
                .copied()
                .map(growth_bytes)
                .sum::<usize>();
        let mut physical = PhysicalReservation::new(exact - 1);
        let mut owner = ArenaCapacity::new(function, Payer::Selected(&mut physical)).unwrap();
        for slots in &sizes[..at] {
            owner.grow_frames(*slots).unwrap();
        }
        let pointer = owner.frames.as_ptr();
        let length = owner.frames.len();
        let (_, refusal) = measure(|| {
            assert!(owner.grow_frames(sizes[at]).unwrap_err().physical);
            let remaining = owner.base.work;
            assert!(owner.grow_frames(length).unwrap_err().physical);
            assert_eq!(owner.base.work, remaining);
        });
        assert_eq!(owner.frames.as_ptr(), pointer);
        assert_eq!(owner.frames.len(), length);
        assert!(!refusal.invalid);
        assert_eq!(refusal.allocated, 0);
        drop(owner);
        let mut physical = PhysicalReservation::new(exact);
        let mut owner = ArenaCapacity::new(function, Payer::Selected(&mut physical)).unwrap();
        for slots in &sizes[..=at] {
            owner.grow_frames(*slots).unwrap();
        }
        assert_eq!(held_charge(&owner), exact);
    }
    let mut physical = PhysicalReservation::new(usize::MAX);
    let mut owner = ArenaCapacity::new(function, Payer::Selected(&mut physical)).unwrap();
    for _ in 0..3 {
        let before = held_charge(&owner);
        owner.start(&function.body).unwrap();
        let (cursor, started) = owner.pop().unwrap();
        assert!(std::ptr::eq(cursor.expression(), &function.body));
        assert!(!started);
        assert!(owner.pop().is_none());
        if before > header_bytes() {
            assert_eq!(held_charge(&owner), before);
        }
    }
}
fn check_live_builder_exact_short_and_zero(function: &ResolvedFunction) {
    let exact = header_bytes() + growth_bytes(2);
    for limit in [0, exact - 1, exact] {
        let ((accepted, metrics), refused, used) =
            crate::bounded_output::with_limit_usage(limit, || {
                measure(|| {
                    let Ok(mut owner) = ArenaCapacity::new(function, Payer::ActiveBuilder) else {
                        return false;
                    };
                    if owner.start(&function.body).is_err() {
                        return false;
                    }
                    assert!(owner.pop().is_some());
                    assert!(owner.pop().is_none());
                    true
                })
            });
        assert_eq!(accepted, limit == exact);
        assert_eq!(refused, limit != exact);
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        if accepted {
            assert_eq!(used, exact);
        } else {
            assert_eq!(metrics.allocated, 0);
        }
    }
}
fn check_work_refusal_before_acquisition(function: &ResolvedFunction) {
    let mut physical = PhysicalReservation::new(usize::MAX);
    let mut owner = ArenaCapacity::new(function, Payer::Selected(&mut physical)).unwrap();
    owner.base.work = 0;
    let (_, metrics) = measure(|| {
        assert!(!owner.start(&function.body).unwrap_err().physical);
        assert!(!owner.start(&function.body).unwrap_err().physical);
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocated, 0);
    assert_eq!(owner.base.work, 0);
    assert_eq!(held_charge(&owner), header_bytes());
}
fn check_oracle_epoch_and_exact_funding(function: &ResolvedFunction) {
    use crate::cleanup_plan::replay::allocation::OracleScope;
    let exact = header_bytes() + growth_bytes(2);
    for limit in [0, exact - 1, exact] {
        let scope = OracleScope::open(limit).unwrap();
        let (accepted, metrics) = measure(|| {
            let Ok(mut owner) = ArenaCapacity::new(function, Payer::Oracle(scope.lease())) else {
                return false;
            };
            owner.start(&function.body).is_ok()
        });
        assert_eq!(accepted, limit == exact);
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        let receipt = scope.finish();
        if accepted {
            assert_eq!(receipt.charged, exact);
            assert!(receipt.first_refusal.is_none());
        } else {
            assert!(receipt.first_refusal.is_some());
            assert_eq!(metrics.allocated, 0);
        }
    }
    let original = OracleScope::open(exact).unwrap();
    let mut owner = ArenaCapacity::new(function, Payer::Oracle(original.lease())).unwrap();
    owner.start(&function.body).unwrap();
    owner.pop().unwrap();
    assert_eq!(original.finish().charged, exact);
    let foreign = OracleScope::open(usize::MAX).unwrap();
    let (_, metrics) = measure(|| {
        assert!(owner.start(&function.body).unwrap_err().physical);
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocated, 0);
    assert_eq!(foreign.finish().charged, 0);
    drop(owner);
}
#[test]
fn actual_cursor_backing_overlap_work_and_release_match_every_frontier() {
    let program = program();
    check_all_growth_and_release(&program.functions[0]);
}
#[test]
fn every_short_frontier_is_sticky_and_reuse_stays_with_the_same_owner() {
    let program = program();
    check_short_sticky_and_same_owner_reuse(&program.functions[0]);
}
#[test]
fn live_builder_cursor_funding_has_exact_short_and_zero_boundaries() {
    let program = program();
    check_live_builder_exact_short_and_zero(&program.functions[0]);
}

#[test]
fn exhausted_work_refuses_before_backing_and_stale_oracle_cannot_borrow_a_new_epoch() {
    let program = program();
    check_work_refusal_before_acquisition(&program.functions[0]);
    check_oracle_epoch_and_exact_funding(&program.functions[0]);
}
