//! Complete original fixed-capacity lookup owner.
use super::*;
pub(super) struct CallCursor<'a> {
    frames: Vec<Option<ChildCursor<'a>>>,
}
impl<'a> CallCursor<'a> {
    pub(super) fn new(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        let capacity = 514;
        budget.reserve_array::<Option<ChildCursor<'_>>>(
            function,
            capacity,
            "structural call cursor",
        )?;
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(capacity)
            .map_err(|_| replay_error(function, "structural call cursor allocation refused"))?;
        if frames.capacity() != capacity {
            return Err(replay_error(
                function,
                "structural call cursor capacity differs from reservation",
            ));
        }
        frames.resize(capacity, None);
        Ok(Self { frames })
    }
    fn find(
        &mut self,
        function: &'a ResolvedFunction,
        id: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<&'a ResolvedExpr, Diagnostic> {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            self.frames[0] = Some(ChildCursor::new(root));
            let mut depth = 1;
            while depth != 0 {
                budget.charge(function, 1, "structural call lookup")?;
                let frame = self.frames[depth - 1]
                    .as_mut()
                    .expect("occupied structural call cursor");
                let node = frame.expression();
                if !frame.is_started() && node.id == *id {
                    return Ok(node);
                }
                if let Some(child) = frame.next() {
                    if depth == self.frames.len() {
                        return Err(replay_error(
                            function,
                            "structural call lookup depth exceeds bound",
                        ));
                    }
                    self.frames[depth] = Some(ChildCursor::new(child));
                    depth += 1;
                } else {
                    self.frames[depth - 1] = None;
                    depth -= 1;
                }
            }
        }
        Err(replay_error(function, "call commit expression is absent"))
    }
}

pub(super) fn lookup<'a>(
    cursor: &mut CallCursor<'a>,
    function: &'a ResolvedFunction,
    id: &ExpressionId,
    budget: &mut ReplayBudget,
) -> Result<&'a ResolvedExpr, Diagnostic> {
    cursor.find(function, id, budget)
}
