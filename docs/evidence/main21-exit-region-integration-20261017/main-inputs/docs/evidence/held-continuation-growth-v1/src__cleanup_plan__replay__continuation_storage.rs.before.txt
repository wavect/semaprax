//! Invocation-held empty continuation backing; never HIR proof or a quota.
use super::*;
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
            if frames.capacity() == 0 {
                *frames = fresh(function, budget)?;
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
        if nested.slots[depth].is_none() {
            nested.slots[depth] = Some(fresh(function, budget)?);
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
