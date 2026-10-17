//! Ordinary destruction storage is prepaid before a new node is published.
//! Capacity bounds traversal scratch only; it grants no authority to children.
use super::*;
use std::cell::RefCell;

impl ObservationDagKind {
    pub(in super::super) fn pending_bound(&self) -> Option<usize> {
        match self {
            Self::Seq(None, _) => Some(0),
            Self::Seq(Some(child), _) => Some(child.pending_bound?.max(1)),
            Self::Cat(children) | Self::Alt(children) => {
                let mut bound = children.len();
                for (ordinal, child) in children.iter().enumerate() {
                    bound = bound.max(ordinal.checked_add(child.pending_bound?)?);
                }
                Some(bound)
            }
        }
    }
}

impl SkeletonWork<'_, '_> {
    pub(in crate::cleanup_plan::replay::skeleton_work) fn ordinary_frontier(
        &mut self,
        required: usize,
    ) -> Result<Option<Rc<RefCell<dag_storage::DestructionScratch>>>, Diagnostic> {
        // Sticky admission is checked even for a leaf or retained-capacity hit.
        self.budget
            .reserve_bytes(self.function, 0, "ordinary destruction frontier")?;
        if required == 0 {
            return Ok(None);
        }
        if self.ordinary_scratch.is_none() {
            self.budget.reserve_bytes(
                self.function,
                physical_clone::rc_backing_bytes::<RefCell<dag_storage::DestructionScratch>>(),
                "ordinary destruction scratch header",
            )?;
            self.ordinary_scratch = Some(Rc::new(RefCell::new(dag_storage::DestructionScratch {
                pending: Vec::new(),
                registered_edges: 0,
                next_ordinal: 0,
            })));
        }
        let scratch = self
            .ordinary_scratch
            .as_ref()
            .expect("funded scratch header")
            .clone();
        {
            let mut frontier = scratch.borrow_mut();
            if !frontier.pending.is_empty() {
                return Err(self.budget.budget_refusal(
                    self.function,
                    "ordinary destruction frontier is active",
                    "",
                ));
            }
            if required > frontier.pending.capacity() {
                let capacity =
                    physical_storage::replay_growth_capacity(frontier.pending.capacity(), required)
                        .ok_or_else(|| {
                            self.budget.budget_refusal(
                                self.function,
                                "ordinary destruction frontier capacity overflowed",
                                "",
                            )
                        })?;
                // The complete old allocation stays live during this acquisition.
                self.budget.reserve_array::<Rc<ObservationDag>>(
                    self.function,
                    capacity,
                    "ordinary destruction frontier",
                )?;
                let mut replacement = Vec::new();
                replacement.try_reserve_exact(capacity).map_err(|_| {
                    self.budget.budget_refusal(
                        self.function,
                        "ordinary destruction frontier allocation failed",
                        "",
                    )
                })?;
                if replacement.capacity() != capacity {
                    return Err(self.budget.budget_refusal(
                        self.function,
                        "ordinary destruction frontier capacity disagrees with its reservation",
                        "",
                    ));
                }
                frontier.pending = replacement;
            }
        }
        Ok(Some(scratch))
    }
}
