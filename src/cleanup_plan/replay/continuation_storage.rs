//! Invocation-held empty continuation backing; never HIR proof or a quota.
use super::*;
#[cfg(test)]
mod oracle_reached;
#[cfg(test)]
mod oracle_reached_tests;
mod paid_receiving;
mod reached;
#[cfg(test)]
#[allow(
    dead_code,
    reason = "Complete independently compiled immediate predecessor owner."
)]
mod reached_before;
#[cfg(test)]
mod reached_tests;
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
    // Only the mandatory unopted test crosscheck ignored this nonreceiving
    // Legacy owner. No live/paid lease is retired, and later actual use must
    // reacquire through the complete original fresh() reservation and carrier.
    #[cfg(test)]
    pub(super) const IDLE_RETIRE_CALL: usize = {
        use std::mem::size_of;
        4 * size_of::<(
            &mut Self,
            &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction,
            &mut ReplayBudget,
        )>() + 4 * size_of::<Result<bool, Diagnostic>>()
            // Authentication's terminal transport and fixed same-program
            // identity loans; no ACTIVE Rc probe on a nonreceiving owner.
            + 4 * size_of::<Diagnostic>() + 4 * size_of::<String>()
            + 2 * size_of::<(&Self, &dyn crate::cleanup::ReplayContext)>()
            + 2 * size_of::<&dyn crate::cleanup::ReplayContext>()
            + 2 * size_of::<Option<&ResolvedProgram>>()
            + 6 * size_of::<&ResolvedProgram>()
            + 2 * size_of::<Result<(), Diagnostic>>()
            + 3 * size_of::<Option<Vec<Frame<'static>>>>()
            + 3 * size_of::<Vec<Frame<'static>>>()
            + 3 * size_of::<Vec<Option<Vec<Frame<'static>>>>>()
            + 4 * size_of::<&Option<Vec<Frame<'static>>>>()
            + 4 * size_of::<&Vec<Frame<'static>>>()
            + 4 * size_of::<&mut Self>()
            + 4 * size_of::<&Self>()
            + 4 * size_of::<&Nested<'static>>()
            + 4 * size_of::<&Option<Box<Nested<'static>>>>()
            + 4 * size_of::<&Box<Nested<'static>>>()
            + 4 * size_of::<&mut Option<Box<Nested<'static>>>>()
            + 4 * size_of::<&mut Box<Nested<'static>>>()
            + 4 * size_of::<&mut Nested<'static>>()
            + 2 * size_of::<std::slice::Iter<'static, Option<Vec<Frame<'static>>>>>()
            + 2 * size_of::<&mut std::slice::Iter<'static, Option<Vec<Frame<'static>>>>>()
            + 2 * size_of::<Option<&Option<Vec<Frame<'static>>>>>()
            + 12 * size_of::<bool>()
            + 4 * size_of::<usize>()
    };
    #[cfg(test)]
    const IDLE_RETIRE_STEP: usize = {
        use std::mem::size_of;
        2 * size_of::<&mut std::slice::Iter<'static, Option<Vec<Frame<'static>>>>>()
            + 2 * size_of::<Option<&Option<Vec<Frame<'static>>>>>()
            + 4 * size_of::<&Option<Vec<Frame<'static>>>>()
            + 2 * size_of::<&Vec<Frame<'static>>>()
            + 2 * size_of::<Option<Vec<Frame<'static>>>>()
            + 3 * size_of::<Vec<Frame<'static>>>()
            + 4 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
            + 4 * size_of::<Result<(), Diagnostic>>()
            + 4 * size_of::<bool>()
    };
    #[cfg(test)]
    pub(super) fn retire_legacy_idle(
        &mut self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        if self.receiving_owner.is_some() {
            return Ok(false);
        }
        if let ReplayBudget {
            payer: AllocationPayer::LegacyUnobserved,
            ..
        } = budget
        {
        } else {
            return Ok(false);
        }
        self.authenticate(program, function, budget)?;
        if !matches!(&self.frames, Some(frames) if frames.is_empty()) {
            return Ok(false);
        }
        if let Some(nested) = &self.nested {
            if nested.depth != 0 {
                return Ok(false);
            }
            for slot in &nested.slots {
                budget.reserve_bytes(
                    function,
                    Self::IDLE_RETIRE_STEP,
                    "idle continuation slot retirement receiving",
                )?;
                budget.charge(function, 1, "idle continuation slot retirement proof")?;
                if !matches!(slot, Some(frames) if frames.is_empty()) {
                    return Ok(false);
                }
            }
        }
        // Every possible leased/nonempty slot was rejected before any Drop.
        // No old reservation is refunded or transferred to a new allocation.
        self.frames = Some(Vec::new());
        if let Some(nested) = &mut self.nested {
            nested.slots = Vec::new();
        }
        Ok(true)
    }

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
        // A nonbinding place-pattern neither copies a payer nor forms a bool.
        if let ReplayBudget {
            payer: AllocationPayer::LegacyUnobserved,
            lent_skeleton: true,
            ..
        } = budget
        {
            budget.reserve_bytes(
                function,
                reached::ACQUIRE_CALL,
                "reached continuation acquisition receiving",
            )?;
            return self.acquire_reached(function, budget);
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
/// Preserve the original constructor for production and Legacy payers. The
/// independently funded test Oracle acquires only backing it actually reaches;
/// its removed 516 allocation carries no old fee and no refund.
pub(super) fn fresh<'hir>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<Frame<'hir>>, Diagnostic> {
    #[cfg(test)]
    if let AllocationPayer::Oracle(_) = budget.payer {
        budget.reserve_bytes(
            function,
            oracle_reached::FRESH_CALL,
            "oracle continuation fresh receivers",
        )?;
        return oracle_reached::fresh(function, budget);
    }
    budget.reserve_array::<Frame<'hir>>(
        function,
        FRAME_CAPACITY,
        "typed HIR continuation stack",
    )?;
    match budget {
        ReplayBudget {
            payer: AllocationPayer::LegacyUnobserved,
            lent_skeleton: true,
            ..
        } => Ok(Vec::with_capacity(2)),
        _ => Ok(Vec::with_capacity(FRAME_CAPACITY)),
    }
}
/// An owning traversal's Step assignment: the match, its renewed binding and
/// the Yield arm block that evaluates the certified Next tail.
pub(super) type OwnedStep<'a> = Option<(
    &'a ResolvedExpr,
    &'a crate::hir::ResolvedBinding,
    &'a ResolvedExpr,
)>;
pub(super) enum Frame<'a> {
    Eval(&'a ResolvedExpr),
    FinishCopyRegion {
        previous: Option<ExpressionId>,
    },
    RenewalPrefix(&'a ResolvedExpr),
    UpcastPassthrough,
    Unary {
        expression: &'a ResolvedExpr,
        op: UnaryOp,
    },
    BinaryLeft {
        expression: &'a ResolvedExpr,
        op: BinaryOp,
        right: &'a ResolvedExpr,
    },
    BinaryRight {
        expression: &'a ResolvedExpr,
        op: BinaryOp,
        left_paths: Vec<ExprSkeletonPath>,
    },
    LazyRight {
        expression: &'a ResolvedExpr,
        op: BinaryOp,
        left: &'a ResolvedExpr,
        left_paths: Vec<ExprSkeletonPath>,
    },
    CallArgument {
        expression: &'a ResolvedExpr,
        params: super::resolved_call::Params<'a>,
        args: &'a [ResolvedExpr],
        index: usize,
        states: Vec<CallSkeletonState>,
    },
    ByteRangeArgument {
        expression: &'a ResolvedExpr,
        index: usize,
        states: Vec<CallSkeletonState>,
    },
    NativeArgument {
        args: &'a [ResolvedExpr],
        index: usize,
        paths: Vec<ExprSkeletonPath>,
    },
    BlockValue {
        expression: &'a ResolvedExpr,
        statements: &'a [ResolvedStatement],
        tail: &'a ResolvedExpr,
        index: usize,
        paths: Vec<ExprSkeletonPath>,
    },
    BlockTail {
        expression: &'a ResolvedExpr,
        paths: Vec<ExprSkeletonPath>,
    },
    WhileAfterCondition {
        prefixes: Vec<ExprSkeletonPath>,
        expression: &'a ResolvedExpr,
        statements: &'a [ResolvedStatement],
        tail: &'a ResolvedExpr,
        index: usize,
        condition: &'a ResolvedExpr,
        body: &'a ResolvedExpr,
    },
    WhileAfterBody {
        expression: &'a ResolvedExpr,
        statements: &'a [ResolvedStatement],
        tail: &'a ResolvedExpr,
        index: usize,
        results: Vec<ExprSkeletonPath>,
        true_prefixes: Vec<ExprSkeletonPath>,
        false_prefixes: Vec<ExprSkeletonPath>,
    },
    LexicalReturn {
        expression: &'a ResolvedExpr,
    },
    ContinueAdvance {
        expression: &'a ResolvedExpr,
        step: OwnedStep<'a>,
    },
    LexicalLoopCondition {
        prefixes: Vec<ExprSkeletonPath>,
        expression: &'a ResolvedExpr,
        statements: &'a [ResolvedStatement],
        tail: &'a ResolvedExpr,
        index: usize,
        condition: &'a ResolvedExpr,
        body: &'a ResolvedExpr,
        latch: &'a crate::hir::LoopLatchKind,
    },
    LexicalLoopBody {
        expression: &'a ResolvedExpr,
        statements: &'a [ResolvedStatement],
        tail: &'a ResolvedExpr,
        index: usize,
        results: Vec<ExprSkeletonPath>,
        true_prefixes: Vec<ExprSkeletonPath>,
        false_prefixes: Vec<ExprSkeletonPath>,
        condition: &'a ResolvedExpr,
    },
    VariantField {
        expression: &'a ResolvedExpr,
        case: &'a DeclarationId,
        fields: &'a [crate::hir::ResolvedFieldInitializer],
        index: usize,
        paths: Vec<ExprSkeletonPath>,
    },
    RecordField {
        expression: &'a ResolvedExpr,
        fields: &'a [crate::hir::ResolvedFieldInitializer],
        index: usize,
        paths: Vec<ExprSkeletonPath>,
    },
    UpdateBase {
        expression: &'a ResolvedExpr,
        base: &'a ResolvedExpr,
        record: &'a DeclarationId,
        fields: &'a [crate::hir::ResolvedFieldInitializer],
    },
    UpdateField {
        expression: &'a ResolvedExpr,
        base: &'a ResolvedExpr,
        record: &'a DeclarationId,
        fields: &'a [crate::hir::ResolvedFieldInitializer],
        index: usize,
        paths: Vec<ExprSkeletonPath>,
        replaced: BTreeSet<DeclarationId>,
        needs_cleanup: bool,
    },
    Try {
        expression: &'a ResolvedExpr,
    },
    TryOption {
        expression: &'a ResolvedExpr,
    },
    Project {
        expression: &'a ResolvedExpr,
        field: &'a DeclarationId,
    },
    IfCondition {
        expression: &'a ResolvedExpr,
        condition: &'a ResolvedExpr,
        then_branch: &'a ResolvedExpr,
        else_branch: &'a ResolvedExpr,
    },
    IfThen {
        expression: &'a ResolvedExpr,
        else_branch: &'a ResolvedExpr,
        true_prefixes: Vec<ExprSkeletonPath>,
        false_prefixes: Vec<ExprSkeletonPath>,
        results: Vec<ExprSkeletonPath>,
    },
    IfElse {
        expression: &'a ResolvedExpr,
        false_prefixes: Vec<ExprSkeletonPath>,
        results: Vec<ExprSkeletonPath>,
    },
    ScalarGuard(scalar_match::Guard<'a>),
    ScalarValue(scalar_match::Value<'a>),
    MatchScrutinee {
        expression: &'a ResolvedExpr,
        scrutinee: &'a ResolvedExpr,
        arms: &'a [ResolvedMatchArm],
    },
    MatchArm {
        expression: &'a ResolvedExpr,
        scrutinee: &'a ResolvedExpr,
        arms: &'a [ResolvedMatchArm],
        index: usize,
        remaining: Vec<ExprSkeletonPath>,
        results: Vec<ExprSkeletonPath>,
        is_record: bool,
    },
}

#[cfg(test)]
mod tests;

// A current invocation-held receiver acquires only the backing it reaches.
// Legacy keeps its original route. The test Oracle has a separately paid
// reached strategy; its complete fixed-layout predecessor remains executable.
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
    if let ReplayBudget {
        payer: AllocationPayer::LegacyUnobserved,
        lent_skeleton: true,
        ..
    } = budget
    {
        budget.reserve_bytes(
            function,
            reached::GROW_CALL,
            "reached continuation growth receiving",
        )?;
        return reached::grow(frames, function, budget);
    }
    #[cfg(test)]
    if let AllocationPayer::Oracle(_) = budget.payer {
        budget.reserve_bytes(
            function,
            oracle_reached::GROW_CALL,
            "oracle continuation growth receivers",
        )?;
        return oracle_reached::grow(frames, function, budget);
    }
    // Every other payer retains its complete original paying/fallback path.
    // This branch never borrows the ordinary receiver's new growth strategy.
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

#[cfg(test)]
mod idle_retirement_tests {
    use super::*;
    #[test]
    fn idle_only_retirement_keeps_leases_and_original_fresh_reacquisition() {
        let program = crate::hir::resolve(
            &crate::check("module idle; @id(\"main\") fn main()->i64 {0}", "idle.spx").unwrap(),
        )
        .unwrap();
        let function = &program.functions[0];
        let mut budget = ReplayBudget::unobserved();
        let mut storage = Storage::new(&program);
        let (lease, frames) = storage.acquire(function, &mut budget).unwrap();
        assert_eq!(frames.capacity(), FRAME_CAPACITY);
        assert!(!storage
            .retire_legacy_idle(&program, function, &mut budget)
            .unwrap());
        assert!(storage.frames.is_none());
        storage.release(lease, frames);
        assert!(storage
            .retire_legacy_idle(&program, function, &mut budget)
            .unwrap());
        assert_eq!(storage.frames.as_ref().unwrap().capacity(), 0);
        let (lease, frames) = storage.acquire(function, &mut budget).unwrap();
        assert_eq!(
            frames.capacity(),
            FRAME_CAPACITY,
            "full original fresh allocation, no credit"
        );
        storage.release(lease, frames);
        let retained = storage.frames.as_ref().unwrap().capacity();
        storage.nested = Some(Box::new(Nested {
            depth: 1,
            slots: vec![None],
        }));
        assert!(!storage
            .retire_legacy_idle(&program, function, &mut budget)
            .unwrap());
        assert_eq!(storage.frames.as_ref().unwrap().capacity(), retained);
        let nested = storage.nested.as_mut().unwrap();
        nested.depth = 0;
        nested.slots[0] = Some(Vec::new());
        assert!(storage
            .retire_legacy_idle(&program, function, &mut budget)
            .unwrap());
        assert_eq!(storage.nested.as_ref().unwrap().slots.capacity(), 0);
        let (lease, frames) = storage.acquire(function, &mut budget).unwrap();
        storage.release(lease, frames);
        let foreign = crate::hir::resolve(
            &crate::check(
                "module foreign; @id(\"main\") fn main()->i64 {0}",
                "foreign.spx",
            )
            .unwrap(),
        )
        .unwrap();
        let capacity = storage.frames.as_ref().unwrap().capacity();
        let mut original = Storage::new(&program);
        let mut original_budget = ReplayBudget::unobserved();
        let expected = original
            .authenticate(&foreign, function, &mut original_budget)
            .unwrap_err();
        let actual = storage
            .retire_legacy_idle(&foreign, function, &mut budget)
            .unwrap_err();
        assert_eq!(
            actual.json(),
            expected.json(),
            "complete original foreign Diagnostic"
        );
        assert_eq!(storage.frames.as_ref().unwrap().capacity(), capacity);
        assert!(
            storage
                .retire_legacy_idle(&program, function, &mut budget)
                .is_err(),
            "foreign refusal remains sticky"
        );
    }
}
