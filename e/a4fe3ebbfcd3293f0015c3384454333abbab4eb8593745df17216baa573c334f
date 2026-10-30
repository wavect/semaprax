//! A full real Frame per live unpaid continuation, with a fixed pointer directory.
//! This changes storage only. The shared complete runner and 516-frame authority remain.
//! Push and pop debit no work beyond the runner's own, exactly as the Vec stack does.
use super::*;
struct BoxedFrames<'hir> {
    #[allow(
        clippy::vec_box,
        reason = "Each complete live continuation owns its backing; the pointer directory avoids reserving 516 full Frames for shallow traversals."
    )]
    frames: Vec<Box<Frame<'hir>>>,
}
impl<'hir> FrameStack<'hir> for BoxedFrames<'hir> {
    fn len(&self) -> usize {
        self.frames.len()
    }
    fn capacity(&self) -> usize {
        self.frames.capacity()
    }
    fn reserve_push(
        &mut self,
        function: &ResolvedFunction,
        _: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        Err(replay_error(
            function,
            "typed-HIR skeleton traversal exceeds the admitted depth",
        ))
    }
    fn push_owned(
        &mut self,
        frame: Frame<'hir>,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        budget.reserve_array::<Frame<'hir>>(function, 1, "unpaid continuation full frame box")?;
        budget.reserve_bytes(
            function,
            std::mem::size_of::<Box<Frame<'hir>>>() + std::mem::size_of::<Result<(), Diagnostic>>(),
            "unpaid continuation box controls",
        )?;
        self.frames.push(Box::new(frame));
        Ok(())
    }
    fn pop_owned(
        &mut self,
        _: &ResolvedFunction,
        _: &mut ReplayBudget,
    ) -> Result<Option<Frame<'hir>>, Diagnostic> {
        Ok(self.frames.pop().map(|frame| *frame))
    }
}
pub(super) fn run<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    expression: &'hir ResolvedExpr,
    work: &mut SkeletonWork<'hir, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    // Drop only idle empty backing. Active leases and their owned frames stay live.
    if let Some(storage) = work.continuations.as_mut() {
        if !storage.matches(program) {
            return Err(replay_error(
                function,
                "typed-HIR continuation storage belongs to another program",
            ));
        }
        if let Some(frames) = storage.frames.as_mut() {
            if !frames.is_empty() {
                return Err(replay_error(
                    function,
                    "idle typed-HIR continuation storage is not empty",
                ));
            }
            *frames = Vec::new();
        }
    }
    work.budget.reserve_array::<Box<Frame<'hir>>>(
        function,
        FRAME_CAPACITY,
        "unpaid continuation pointer directory",
    )?;
    work.budget.reserve_bytes(
        function,
        std::mem::size_of::<BoxedFrames<'hir>>()
            + std::mem::size_of::<Result<Option<Frame<'hir>>, Diagnostic>>(),
        "unpaid continuation full frame return controls",
    )?;
    let mut frames = BoxedFrames {
        frames: Vec::with_capacity(FRAME_CAPACITY),
    };
    super::super::expression_skeleton_run(program, function, expression, work, &mut frames)
}
