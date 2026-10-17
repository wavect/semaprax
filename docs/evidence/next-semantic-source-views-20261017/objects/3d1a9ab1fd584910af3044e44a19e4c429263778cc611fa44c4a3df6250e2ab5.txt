//! Ordinary construction ordinals order storage only; the memo checks Rc identity.
//! This keeps the original destructor and semantic observation ordering. New nodes
//! and every new tail/member carrier are paid on the existing replay ledger.
use super::*;
use std::rc::Rc;
mod frontier;
#[cfg(test)]
#[path = "ordinary_dag/saved_receiver.rs"]
mod saved;

impl SkeletonWork<'_, '_> {
    pub(super) fn ordinary_dag(
        &mut self,
        kind: ObservationDagKind,
    ) -> Result<Rc<ObservationDag>, Diagnostic> {
        self.charge(1, "ordinary DAG construction ordinal")?;
        let ordinal = self.next_ordinary_dag;
        let next = ordinal.checked_add(1).ok_or_else(|| {
            replay_error(
                self.function,
                "ordinary DAG construction ordinal overflowed",
            )
        })?;
        // The incoming kind remains owned by this frame throughout admission.
        // Any refusal releases the frontier borrow before dropping its children.
        let release = kind
            .edges()
            .checked_mul(2)
            .and_then(|edges| edges.checked_add(1))
            .ok_or_else(|| {
                self.budget.budget_refusal(
                    self.function,
                    "ordinary DAG metadata and release work overflowed",
                    "",
                )
            })?;
        self.charge(release, "ordinary DAG bound fold and prepaid release")?;
        let pending_bound = kind.pending_bound().ok_or_else(|| {
            self.budget.budget_refusal(
                self.function,
                "observation DAG destruction bound overflowed",
                "",
            )
        })?;
        let scratch = self.ordinary_frontier(pending_bound)?;
        self.budget.reserve_bytes(
            self.function,
            physical_clone::rc_backing_bytes::<ObservationDag>(),
            "ordinary observation DAG node",
        )?;
        let node = Rc::new(ObservationDag {
            ordinal: Some(ordinal),
            kind,
            pending_bound: Some(pending_bound),
            scratch,
        });
        self.next_ordinary_dag = next;
        Ok(node)
    }

    #[cfg(test)]
    fn ordinary_freeze(
        &mut self,
        observations: &Observations,
    ) -> Result<Option<Rc<ObservationDag>>, Diagnostic> {
        if observations.items.is_empty() {
            return Ok(observations.prefix.clone());
        }
        self.budget.reserve_clone(
            self.function,
            &observations.items,
            "ordinary DAG sequence tail",
        )?;
        self.ordinary_dag(ObservationDagKind::Seq(
            observations.prefix.clone(),
            observations.items.clone(),
        ))
        .map(Some)
    }

    /// Owned union members already paid for their tail backing.
    fn ordinary_freeze_owned(
        &mut self,
        observations: Observations,
    ) -> Result<Option<Rc<ObservationDag>>, Diagnostic> {
        if observations.items.is_empty() {
            return Ok(observations.prefix);
        }
        self.ordinary_dag(ObservationDagKind::Seq(
            observations.prefix,
            observations.items,
        ))
        .map(Some)
    }

    pub(super) fn ordinary_union(
        &mut self,
        members: Vec<Observations>,
    ) -> Result<Observations, Diagnostic> {
        self.budget.reserve_array::<Rc<ObservationDag>>(
            self.function,
            members.len(),
            "ordinary DAG alternatives",
        )?;
        let mut nodes = Vec::with_capacity(members.len());
        for member in members {
            let node = match self.ordinary_freeze_owned(member)? {
                Some(node) => node,
                None => self.ordinary_dag(ObservationDagKind::Seq(None, Vec::new()))?,
            };
            nodes.push(node);
        }
        Ok(Observations {
            prefix: Some(self.ordinary_dag(ObservationDagKind::Alt(nodes))?),
            items: Vec::new(),
        })
    }

    #[cfg(test)]
    pub(super) fn ordinary_extend(
        &mut self,
        target: &mut Observations,
        suffix: &Observations,
    ) -> Result<(), Diagnostic> {
        let Some(prefix) = &suffix.prefix else {
            self.budget.reserve_vec(
                self.function,
                &mut target.items,
                suffix.items.len(),
                "ordinary flat suffix",
            )?;
            target.items.extend_from_slice(&suffix.items);
            return Ok(());
        };
        let joined = match self.ordinary_freeze(target)? {
            None => prefix.clone(),
            Some(head) => {
                self.budget.reserve_array::<Rc<ObservationDag>>(
                    self.function,
                    2,
                    "ordinary DAG concatenation",
                )?;
                self.ordinary_dag(ObservationDagKind::Cat(vec![head, prefix.clone()]))?
            }
        };
        self.budget
            .reserve_clone(self.function, &suffix.items, "ordinary DAG suffix tail")?;
        let items = suffix.items.clone();
        target.prefix = Some(joined);
        target.items = items;
        Ok(())
    }

    pub(super) fn ordinary_extend_owned(
        &mut self,
        mut target: Observations,
        suffix: &Observations,
    ) -> Result<Observations, Diagnostic> {
        let Some(prefix) = &suffix.prefix else {
            self.budget.reserve_vec(
                self.function,
                &mut target.items,
                suffix.items.len(),
                "ordinary flat suffix",
            )?;
            target.items.extend_from_slice(&suffix.items);
            return Ok(target);
        };
        let joined = match self.ordinary_freeze_owned(target)? {
            None => prefix.clone(),
            Some(head) => {
                self.budget.reserve_array::<Rc<ObservationDag>>(
                    self.function,
                    2,
                    "ordinary DAG concatenation",
                )?;
                self.ordinary_dag(ObservationDagKind::Cat(vec![head, prefix.clone()]))?
            }
        };
        self.budget
            .reserve_clone(self.function, &suffix.items, "ordinary DAG suffix tail")?;
        Ok(Observations {
            prefix: Some(joined),
            items: suffix.items.clone(),
        })
    }

    pub(super) fn ordinary_prepend(
        &mut self,
        target: &mut Observations,
        observation: Observation,
    ) -> Result<(), Diagnostic> {
        let prefix = target
            .prefix
            .as_ref()
            .expect("caller observed prefix")
            .clone();
        self.budget
            .reserve_array::<Observation>(self.function, 1, "ordinary DAG prepend head")?;
        let head = self.ordinary_dag(ObservationDagKind::Seq(None, vec![observation]))?;
        self.budget.reserve_array::<Rc<ObservationDag>>(
            self.function,
            2,
            "ordinary DAG prepend concatenation",
        )?;
        let joined = self.ordinary_dag(ObservationDagKind::Cat(vec![head, prefix]))?;
        target.prefix = Some(joined);
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod frontier_tests;
