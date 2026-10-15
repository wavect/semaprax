//! Complete immediate677 generic selected-policy cursor.
use super::*;

pub(super) struct CallCursor<'a> {
    frames: Vec<Option<ChildCursor<'a>>>,
}
impl<'a> CallCursor<'a> {
    pub(super) fn new<E: Policy>(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, E> {
        let permit = E::enter::<Self>(function, budget)?;
        let capacity = 514;
        budget.reserve_array::<Option<ChildCursor<'_>>>(
            function,
            capacity,
            "structural call cursor",
        )?;
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(capacity)
            .map_err(|_| E::static_error(&permit, Kind::StructuralCallCursorAllocationRefused))?;
        if frames.capacity() != capacity {
            return Err(E::static_error(
                &permit,
                Kind::StructuralCallCursorCapacityDiffersFromReservation,
            ));
        }
        frames.resize(capacity, None);
        Ok(Self { frames })
    }
    pub(super) fn find<E: Policy>(
        &mut self,
        function: &'a ResolvedFunction,
        id: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<&'a ResolvedExpr, E> {
        let permit = E::enter::<&ResolvedExpr>(function, budget)?;
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
                        return Err(E::static_error(
                            &permit,
                            Kind::StructuralCallLookupDepthExceedsBound,
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
        Err(E::static_error(&permit, Kind::CallCommitExpressionIsAbsent))
    }
}
