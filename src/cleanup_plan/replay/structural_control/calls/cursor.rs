//! One bounded lookup stack, grown and paid at the actual traversal depth.
use super::*;
const DEPTH: usize = 514;
type Slot<'a> = Option<ChildCursor<'a>>;

pub(in crate::cleanup_plan::replay::structural_control) struct CallCursor<'a> {
    frames: Vec<Slot<'a>>,
}
impl<'a> CallCursor<'a> {
    pub(in crate::cleanup_plan::replay::structural_control) fn new<E: Policy>(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, E> {
        let _permit = E::enter::<Self>(function, budget)?;
        budget.reserve_array::<Self>(function, 1, "structural call cursor owner")?;
        Ok(Self { frames: Vec::new() })
    }

    fn grow<E: Policy>(
        &mut self,
        function: &ResolvedFunction,
        required: usize,
        budget: &mut ReplayBudget,
    ) -> Result<(), E> {
        let permit = E::enter::<()>(function, budget)?;
        budget.reserve_bytes(function, 0, "structural call cursor")?;
        if required <= self.frames.len() {
            return Ok(());
        }
        // This is the original independent traversal ceiling, never the
        // current allocation's capacity. A deep source cannot grow past it.
        if required > DEPTH {
            return Err(E::static_error(
                &permit,
                Kind::StructuralCallLookupDepthExceedsBound,
            ));
        }
        let target = self.frames.len().saturating_mul(2).clamp(2, DEPTH);
        let target = target.max(required);
        budget.charge(function, target, "structural call cursor initialization")?;
        budget.reserve_array::<Vec<Slot<'a>>>(function, 2, "structural call cursor controls")?;
        budget.reserve_array::<Slot<'a>>(function, target, "structural call cursor")?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(target)
            .map_err(|_| E::static_error(&permit, Kind::StructuralCallCursorAllocationRefused))?;
        if replacement.capacity() != target {
            return Err(E::static_error(
                &permit,
                Kind::StructuralCallCursorCapacityDiffersFromReservation,
            ));
        }
        // Both complete allocations are charged while their backing overlaps.
        // Moving initialized slots and filling new ones costs target visits.
        replacement.append(&mut self.frames);
        replacement.resize(target, None);
        self.frames = replacement;
        Ok(())
    }

    pub(super) fn find<E: Policy>(
        &mut self,
        function: &'a ResolvedFunction,
        id: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<&'a ResolvedExpr, E> {
        let permit = E::enter::<&ResolvedExpr>(function, budget)?;
        self.grow::<E>(function, 1, budget)?;
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
                    if depth == DEPTH {
                        return Err(E::static_error(
                            &permit,
                            Kind::StructuralCallLookupDepthExceedsBound,
                        ));
                    }
                    self.grow::<E>(function, depth + 1, budget)?;
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

#[cfg(test)]
#[allow(
    dead_code,
    reason = "independent complete original fixed-capacity owner"
)]
#[path = "cursor_before.rs"]
mod before;
#[cfg(test)]
mod tests;

#[cfg(test)]
#[allow(
    dead_code,
    reason = "complete immediate677 generic selected-policy cursor remains independent"
)]
#[path = "cursor_before_selected.rs"]
mod before_selected;
