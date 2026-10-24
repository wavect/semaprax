//! Precharged DAG construction and allocation-free iterative destruction.
//!
//! Each node registers every child edge with one shared destruction frontier
//! before the node is allocated. Its maximum pending size cannot exceed the
//! cumulative registered edges. Growing that backing charges old/new overlap.
//! These helpers do not select lexical factoring or authenticate eligibility.
use super::*;
use std::cell::RefCell;
use std::rc::Rc;

pub(in crate::cleanup_plan::replay) struct DestructionScratch {
    pub(super) pending: Vec<Rc<ObservationDag>>,
    pub(super) registered_edges: usize,
    // One checked construction sequence belongs to this shared frontier.
    // Children must share it; a detached/other-session memo hit still checks Rc.
    pub(super) next_ordinal: usize,
}

impl ObservationDagKind {
    pub(super) fn edges(&self) -> usize {
        match self {
            Self::Seq(prefix, _) => usize::from(prefix.is_some()),
            Self::Cat(parts) | Self::Alt(parts) => parts.len(),
        }
    }
}

impl SkeletonWork<'_, '_> {
    fn charged_dag(&mut self, kind: ObservationDagKind) -> Result<Rc<ObservationDag>, Diagnostic> {
        let units = kind
            .edges()
            .checked_mul(2)
            .and_then(|units| units.checked_add(2))
            .ok_or_else(|| replay_error(self.function, "factored DAG work bound overflowed"))?;
        // Cover construction/registration and the later destruction traversal.
        self.charge(units, "factored observation DAG node and release")?;
        if !self.budget.selected() {
            return self.ordinary_dag(kind);
        }
        if self.dag_scratch.is_none() {
            self.budget.reserve_bytes(
                self.function,
                physical_clone::rc_backing_bytes::<RefCell<DestructionScratch>>(),
                "factored destruction scratch header",
            )?;
            self.dag_scratch = Some(Rc::new(RefCell::new(DestructionScratch {
                pending: Vec::new(),
                registered_edges: 0,
                next_ordinal: 0,
            })));
        }
        let scratch = self.dag_scratch.as_ref().expect("DAG scratch held").clone();
        let same_session = |child: &Rc<ObservationDag>| {
            child
                .scratch
                .as_ref()
                .is_some_and(|held| Rc::ptr_eq(held, &scratch))
        };
        let children_held = match &kind {
            ObservationDagKind::Seq(prefix, _) => prefix.as_ref().is_none_or(same_session),
            ObservationDagKind::Cat(parts) | ObservationDagKind::Alt(parts) => {
                parts.iter().all(same_session)
            }
        };
        if !children_held {
            return Err(replay_error(
                self.function,
                "factored DAG crosses its destruction reservation",
            ));
        }
        let pending_bound = {
            let mut frontier = scratch.borrow_mut();
            let edges = frontier
                .registered_edges
                .checked_add(kind.edges())
                .ok_or_else(|| {
                    replay_error(
                        self.function,
                        "factored destruction frontier length overflowed",
                    )
                })?;
            self.budget.reserve_vec(
                self.function,
                &mut frontier.pending,
                edges,
                "factored destruction frontier",
            )?;
            frontier.registered_edges = edges;
            // Every selected descendant was published in this same session,
            // and every actual edge was checked and registered beforehand.
            // This immutable cumulative edge count is a structural upper bound
            // even when the session has other live or previously dropped nodes.
            // Empty nodes need no pending slot. Ordinary parents may retain
            // excess capacity for a foreign selected child; they pay it in full.
            if kind.edges() == 0 {
                0
            } else {
                edges
            }
        };
        self.budget.reserve_bytes(
            self.function,
            physical_clone::rc_backing_bytes::<ObservationDag>(),
            "factored observation DAG header",
        )?;
        let ordinal = {
            self.charge(1, "factored DAG construction ordinal")?;
            let mut held = scratch.borrow_mut();
            let ordinal = held.next_ordinal;
            held.next_ordinal = ordinal.checked_add(1).ok_or_else(|| {
                replay_error(
                    self.function,
                    "factored DAG construction ordinal overflowed",
                )
            })?;
            ordinal
        };
        Ok(Rc::new(ObservationDag {
            ordinal: Some(ordinal),
            pending_bound: Some(pending_bound),
            kind,
            scratch: Some(scratch),
        }))
    }

    /// Detect only a real factored suffix fanout; charge the inspected carriers.
    pub(in crate::cleanup_plan::replay) fn shares_prefix_tail(
        &mut self,
        suffixes: &[ExprSkeletonPath],
    ) -> Result<bool, Diagnostic> {
        if !self.budget.merge_paths || suffixes.len() < 2 {
            return Ok(false);
        }
        self.charge(suffixes.len(), "factored suffix fanout inspection")?;
        Ok(suffixes
            .iter()
            .any(|suffix| suffix.observations.prefix.is_some()))
    }

    /// Move one authentic prefix tail into one immutable node before fanout.
    /// This changes representation only: path ownership, continuation, failure
    /// and pending commits remain carried separately by every sibling.
    pub(in crate::cleanup_plan::replay) fn share_prefix_tail(
        &mut self,
        observations: &mut Observations,
    ) -> Result<(), Diagnostic> {
        if observations.items.is_empty() {
            return Ok(());
        }
        let prefix = observations.prefix.take();
        let items = std::mem::take(&mut observations.items);
        // Existing Vec backing moves with the sequence; no clone allocation.
        observations.prefix = Some(self.charged_dag(ObservationDagKind::Seq(prefix, items))?);
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn freeze_observations(
        &mut self,
        observations: &Observations,
    ) -> Result<Option<Rc<ObservationDag>>, Diagnostic> {
        if observations.items.is_empty() {
            return Ok(observations.prefix.clone());
        }
        self.budget
            .reserve_clone(self.function, observations, "factored sequence tail clone")?;
        let kind = ObservationDagKind::Seq(observations.prefix.clone(), observations.items.clone());
        self.charged_dag(kind).map(Some)
    }

    /// Consumed members transfer their paid tail without a clone buffer.
    fn freeze_owned_observations(
        &mut self,
        observations: Observations,
    ) -> Result<Option<Rc<ObservationDag>>, Diagnostic> {
        if observations.items.is_empty() {
            return Ok(observations.prefix);
        }
        self.charged_dag(ObservationDagKind::Seq(
            observations.prefix,
            observations.items,
        ))
        .map(Some)
    }

    pub(in crate::cleanup_plan::replay) fn union_observations(
        &mut self,
        members: Vec<Observations>,
    ) -> Result<Observations, Diagnostic> {
        self.charge(members.len(), "factored observation alternatives")?;
        self.budget.reserve_array::<Rc<ObservationDag>>(
            self.function,
            members.len(),
            "factored alternative members",
        )?;
        let mut alternatives = Vec::with_capacity(members.len());
        for member in members {
            let node = match self.freeze_owned_observations(member)? {
                Some(node) => node,
                None => self.charged_dag(ObservationDagKind::Seq(None, Vec::new()))?,
            };
            // Preserve duplicate members: this is a multiset, never a set.
            alternatives.push(node);
        }
        Ok(Observations {
            prefix: Some(self.charged_dag(ObservationDagKind::Alt(alternatives))?),
            items: Vec::new(),
        })
    }

    #[cfg(test)]
    pub(super) fn extend_factored_observations(
        &mut self,
        target: &mut Observations,
        suffix: &Observations,
    ) -> Result<(), Diagnostic> {
        let Some(suffix_prefix) = &suffix.prefix else {
            self.budget.reserve_vec(
                self.function,
                &mut target.items,
                suffix.items.len(),
                "factored flat suffix",
            )?;
            target.items.extend_from_slice(&suffix.items);
            return Ok(());
        };
        let prefix = match self.freeze_observations(target)? {
            None => suffix_prefix.clone(),
            Some(head) => {
                self.budget.reserve_array::<Rc<ObservationDag>>(
                    self.function,
                    2,
                    "factored concatenation members",
                )?;
                self.charged_dag(ObservationDagKind::Cat(vec![head, suffix_prefix.clone()]))?
            }
        };
        self.budget
            .reserve_clone(self.function, suffix, "factored suffix tail clone")?;
        target.prefix = Some(prefix);
        target.items = suffix.items.clone();
        Ok(())
    }

    pub(super) fn extend_owned_factored_observations(
        &mut self,
        mut target: Observations,
        suffix: &Observations,
    ) -> Result<Observations, Diagnostic> {
        let Some(suffix_prefix) = &suffix.prefix else {
            self.budget.reserve_vec(
                self.function,
                &mut target.items,
                suffix.items.len(),
                "factored flat suffix",
            )?;
            target.items.extend_from_slice(&suffix.items);
            return Ok(target);
        };
        let prefix = match self.freeze_owned_observations(target)? {
            None => suffix_prefix.clone(),
            Some(head) => {
                self.budget.reserve_array::<Rc<ObservationDag>>(
                    self.function,
                    2,
                    "factored concatenation members",
                )?;
                self.charged_dag(ObservationDagKind::Cat(vec![head, suffix_prefix.clone()]))?
            }
        };
        self.budget
            .reserve_clone(self.function, suffix, "factored suffix tail clone")?;
        Ok(Observations {
            prefix: Some(prefix),
            items: suffix.items.clone(),
        })
    }

    pub(super) fn prepend_factored_observation(
        &mut self,
        observations: &mut Observations,
        observation: Observation,
    ) -> Result<(), Diagnostic> {
        let Some(prefix) = &observations.prefix else {
            self.budget.reserve_vec(
                self.function,
                &mut observations.items,
                1,
                "factored flat prepend",
            )?;
            observations.items.insert(0, observation);
            return Ok(());
        };
        self.budget
            .reserve_array::<Observation>(self.function, 1, "factored prepend head")?;
        let head = self.charged_dag(ObservationDagKind::Seq(None, vec![observation]))?;
        self.budget.reserve_array::<Rc<ObservationDag>>(
            self.function,
            2,
            "factored prepend concatenation",
        )?;
        let joined = self.charged_dag(ObservationDagKind::Cat(vec![head, prefix.clone()]))?;
        observations.prefix = Some(joined);
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
impl SkeletonWork<'_, '_> {
    pub(super) fn selected_dag_for_foreign_control(
        &mut self,
        kind: ObservationDagKind,
    ) -> Result<Rc<ObservationDag>, Diagnostic> {
        self.charged_dag(kind)
    }
}
