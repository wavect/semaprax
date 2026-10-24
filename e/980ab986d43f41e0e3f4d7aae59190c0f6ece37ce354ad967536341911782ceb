//! Invocation-held empty continuation backing; never HIR proof or a quota.
use super::*;
#[path = "paid_receiving.rs"]
mod paid_receiving;
pub(super) const FRAME_CAPACITY: usize = 516;
// Some(empty) is an idle, never-acquired top-level slot; None is its lease.
// This replaces the old bool without enlarging the inline Storage carrier.
pub(super) struct Storage<'hir> {
    program: &'hir ResolvedProgram,
    pub(super) frames: Option<Vec<Frame<'hir>>>,
    nested: Option<Box<Nested<'hir>>>,
    receiving_owner: Option<crate::bounded_output::classifier_string::ClassifierLedger>,
    receiving_first: Option<paid_receiving::Failure>,
}
struct Nested<'hir> {
    depth: usize,
    slots: Vec<Option<Vec<Frame<'hir>>>>,
}
pub(super) enum Lease {
    Top,
    Nested(usize),
    Local,
}
impl<'hir> Storage<'hir> {
    pub(super) fn new(program: &'hir ResolvedProgram) -> Self {
        Self {
            program,
            frames: Some(Vec::new()),
            nested: None,
            receiving_owner: None,
            receiving_first: None,
        }
    }
    pub(super) fn matches(&self, program: &dyn crate::cleanup::ReplayContext) -> bool {
        program
            .full_program()
            .is_some_and(|current| std::ptr::eq(current, self.program))
    }
    pub(super) fn acquire(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(Lease, Vec<Frame<'hir>>), Diagnostic> {
        if self.receiving_owner.is_some() {
            self.authenticate(self.program, function, budget)?;
        }
        if self.frames.is_some() {
            let frames = self.frames.as_mut().unwrap();
            debug_assert!(frames.is_empty());
            if frames.capacity() == 0
                || (budget.lent_skeleton && frames.capacity() < FRAME_CAPACITY)
            {
                *frames = if self.receiving_owner.is_some() && !budget.lent_skeleton {
                    fresh_receiving(function, budget)?
                } else {
                    fresh(function, budget)?
                };
            } else {
                budget.reserve_bytes(function, 0, "typed HIR continuation stack")?;
            }
            return Ok((Lease::Top, self.frames.take().unwrap()));
        }
        #[cfg(test)]
        if legacy_nested() {
            return Ok((Lease::Local, fresh(function, budget)?));
        }
        if self.nested.is_none() {
            budget.reserve_array::<Nested<'hir>>(function, 1, "nested continuation carrier")?;
            self.nested = Some(Box::new(Nested {
                depth: 0,
                slots: Vec::new(),
            }));
        }
        let nested = self.nested.as_mut().unwrap();
        let depth = nested.depth;
        let next = depth
            .checked_add(1)
            .ok_or_else(|| replay_error(function, "nested continuation depth overflowed"))?;
        if depth == nested.slots.len() {
            // Replacement is allocated while the old directory remains live.
            // Pay its complete layout, not merely its capacity delta.
            budget.reserve_array::<Option<Vec<Frame<'hir>>>>(
                function,
                next,
                "nested continuation directory",
            )?;
            let mut replacement = Vec::with_capacity(next);
            replacement.append(&mut nested.slots);
            replacement.push(None);
            nested.slots = replacement;
        }
        debug_assert!(nested.slots[depth].as_ref().is_none_or(Vec::is_empty));
        if nested.slots[depth].is_none()
            || (budget.lent_skeleton
                && nested.slots[depth]
                    .as_ref()
                    .is_some_and(|frames| frames.capacity() < FRAME_CAPACITY))
        {
            nested.slots[depth] =
                Some(if self.receiving_owner.is_some() && !budget.lent_skeleton {
                    fresh_receiving(function, budget)?
                } else {
                    fresh(function, budget)?
                });
        } else {
            budget.reserve_bytes(function, 0, "typed HIR continuation stack")?;
        }
        let frames = nested.slots[depth].take().unwrap();
        nested.depth = next;
        #[cfg(test)]
        record_nested_entry(depth, frames.as_ptr() as usize);
        Ok((Lease::Nested(depth), frames))
    }
    pub(super) fn release(&mut self, lease: Lease, frames: Vec<Frame<'hir>>) {
        debug_assert!(frames.is_empty());
        match lease {
            Lease::Top => self.frames = Some(frames),
            Lease::Nested(depth) => {
                let nested = self.nested.as_mut().unwrap();
                debug_assert_eq!(nested.depth, depth + 1);
                nested.depth = depth;
                nested.slots[depth] = Some(frames);
            }
            Lease::Local => {}
        }
    }
}
#[cfg(test)]
thread_local! {
    static ENTRY_TRACE: std::cell::Cell<[(usize, usize); 32]> = const { std::cell::Cell::new([(0, 0); 32]) };
    static ENTRY_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static LEGACY_NESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
fn record_nested_entry(depth: usize, pointer: usize) {
    ENTRY_COUNT.with(|count| {
        let index = count.get();
        // Fixed observation storage never allocates under the live payer.
        if index < 32 {
            ENTRY_TRACE.with(|trace| {
                let mut entries = trace.get();
                entries[index] = (depth, pointer);
                trace.set(entries);
            });
        }
        count.set(index + 1);
    });
}
#[cfg(test)]
fn legacy_nested() -> bool {
    LEGACY_NESTED.with(std::cell::Cell::get)
}
#[cfg(test)]
fn with_legacy_nested<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            LEGACY_NESTED.with(|mode| mode.set(self.0));
        }
    }
    let _restore = Restore(LEGACY_NESTED.with(|mode| mode.replace(true)));
    run()
}
/// Same old lazy requested layout for a missing top-level lease or an active
/// nested guard in the legacy/standalone receiver. Caller prepays allocation.
pub(super) fn fresh<'hir>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<Frame<'hir>>, Diagnostic> {
    budget.reserve_array::<Frame<'hir>>(
        function,
        FRAME_CAPACITY,
        "typed HIR continuation stack",
    )?;
    Ok(Vec::with_capacity(FRAME_CAPACITY))
}
// Exact unchanged Frame/OwnedStep declarations are authenticated in the full preimage.
type Frame<'a> = super::Frame<'a>;

// A current invocation-held receiver acquires only the backing it reaches.
// Legacy and independent oracle receivers retain the complete fixed layout.
pub(super) fn fresh_receiving<'hir>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<Frame<'hir>>, Diagnostic> {
    budget.reserve_bytes(
        function,
        2 * std::mem::size_of::<Vec<Frame<'hir>>>()
            + std::mem::size_of::<Result<Vec<Frame<'hir>>, Diagnostic>>()
            + std::mem::size_of::<bool>(),
        "held continuation initial controls",
    )?;
    let mut frames = Vec::new();
    reserve_push(&mut frames, function, budget)?;
    Ok(frames)
}

/// Grow only before the runner publishes its next frame. Capacity is storage;
/// the original 516-frame bound continues to supply traversal authority.
pub(super) fn reserve_push(
    frames: &mut Vec<Frame<'_>>,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if frames.len() >= FRAME_CAPACITY {
        return Err(replay_error(
            function,
            "typed-HIR skeleton traversal exceeds the admitted depth",
        ));
    }
    budget.reserve_bytes(function, 0, "held continuation growth")?;
    if frames.len() < frames.capacity() {
        return Ok(());
    }
    // Only the invocation-held paying constructor produces a smaller buffer.
    // A missing legacy slot is never turned into a new funded path here.
    if !budget.pays_allocations() {
        return Err(replay_error(
            function,
            "typed-HIR skeleton traversal exceeds the admitted depth",
        ));
    }
    // Actual old/new Vec controls, the result, target and retained/moved frame.
    budget.reserve_bytes(
        function,
        2 * std::mem::size_of::<Vec<Frame<'_>>>()
            + std::mem::size_of::<Result<(), Diagnostic>>()
            + std::mem::size_of::<usize>()
            + std::mem::size_of::<Frame<'_>>(),
        "held continuation replacement controls",
    )?;
    // Eight setup steps and a read/move for every retained live frame.
    budget.charge(
        function,
        8 + 2 * frames.len(),
        "held continuation relocation",
    )?;
    let target = frames
        .capacity()
        .saturating_mul(2)
        .clamp((frames.len() + 1).max(2), FRAME_CAPACITY);
    budget.reserve_array::<Frame<'_>>(function, target, "held continuation replacement backing")?;
    let mut replacement = Vec::with_capacity(target);
    replacement.append(frames);
    *frames = replacement;
    Ok(())
}
