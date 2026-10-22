//! Fixed diagnostic counters only. They neither acquire storage nor grant reuse.
use super::*;
use std::cell::Cell;
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::cleanup_plan::replay::factored::canonical_shared::inline_singleton) struct Row {
    pub arity: [usize; 3],
    pub unit_drops: usize,
    pub general_drops: usize,
    pub signature_hits: usize,
    pub signature_misses: usize,
    pub complete: bool,
    pub overflowed: bool,
}
#[derive(Clone, Copy)]
struct State {
    active: bool,
    function: usize,
    budget: usize,
    row: Row,
}
thread_local! {
    static STATE: Cell<State> = const { Cell::new(State { active: false, function: 0, budget: 0, row: Row { arity: [0;3], unit_drops: 0, general_drops: 0, signature_hits: 0, signature_misses: 0, complete: false, overflowed: false } }) };
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared::inline_singleton) struct Guard;
pub(in crate::cleanup_plan::replay::factored::canonical_shared::inline_singleton) fn observe(
    function: &ResolvedFunction,
    budget: &ReplayBudget,
) -> Guard {
    STATE.with(|slot| {
        assert!(!slot.get().active);
        slot.set(State {
            active: true,
            function: std::ptr::from_ref(function) as usize,
            budget: std::ptr::from_ref(budget) as usize,
            row: Row::default(),
        });
    });
    Guard
}
fn update(function: &ResolvedFunction, budget: &ReplayBudget, field: usize) {
    STATE.with(|slot| {
        let mut s = slot.get();
        if !s.active {
            return;
        }
        if s.function != std::ptr::from_ref(function) as usize
            || s.budget != std::ptr::from_ref(budget) as usize
        {
            s.row.overflowed = true;
            slot.set(s);
            return;
        }
        let value = match field {
            0..=2 => &mut s.row.arity[field],
            3 => &mut s.row.unit_drops,
            4 => &mut s.row.general_drops,
            5 => &mut s.row.signature_hits,
            _ => &mut s.row.signature_misses,
        };
        if let Some(next) = value.checked_add(1) {
            *value = next;
        } else {
            s.row.overflowed = true;
        }
        slot.set(s);
    });
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared::inline_singleton) fn arity(
    count: usize,
    function: &ResolvedFunction,
    budget: &ReplayBudget,
) {
    update(function, budget, count.min(2));
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared::inline_singleton) fn vector_drop(
    unit: bool,
    function: &ResolvedFunction,
    budget: &ReplayBudget,
) {
    update(function, budget, if unit { 3 } else { 4 });
}
pub(in crate::cleanup_plan::replay::factored::canonical_shared::inline_singleton) fn signature(
    hit: bool,
    function: &ResolvedFunction,
    budget: &ReplayBudget,
) {
    update(function, budget, if hit { 5 } else { 6 });
}

impl Guard {
    pub(in crate::cleanup_plan::replay::factored::canonical_shared::inline_singleton) fn finish(
        &self,
        complete: bool,
    ) -> Row {
        STATE.with(|slot| {
            let mut s = slot.get();
            s.row.complete = complete && !s.row.overflowed;
            slot.set(s);
            s.row
        })
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = STATE.try_with(|slot| {
            let mut s = slot.get();
            s.active = false;
            slot.set(s);
        });
    }
}
