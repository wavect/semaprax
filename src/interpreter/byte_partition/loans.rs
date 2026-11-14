//! Closed ordinary receiver: one checked function, one invocation account, and
//! one owned runtime loan per plan ID. Copy descriptors never own finalization.
//! The eventual installer must bind each Owner to the plan's actual root and
//! drive `advance` from actual execution; this is not a source admission API.
use super::{authority::LoanCapacity, InvocationRuntime, Owner, Registry, View};
use crate::{
    construction_fusion::{domain::Refusal, partition::Preparation},
    loan_plan::LoanId,
};
use std::alloc::Layout;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewId {
    invocation: u64,
    frame: u64,
    index: u16,
    generation: u64,
}
struct Slot {
    generation: u64,
    view: Option<View>,
}
pub(crate) struct Loans<'p, R: InvocationRuntime> {
    capacity: LoanCapacity<'p>,
    registry: Registry<R>,
    slots: Box<[Slot]>,
    point: u16,
    frame: u64,
    next_frame: u64,
    frames: Vec<frames::Frame<'p>>,
    spare: Option<Box<[Slot]>>,
}
impl<'p, R: InvocationRuntime> Loans<'p, R> {
    /// Header and actual checked local inventory are paid before allocation.
    /// The source-held capacity exists, but no ordinary evaluator installer
    /// can mint runtime roots until its complete receiving proof is joined.
    pub(super) fn metadata_layouts(capacity: &LoanCapacity<'p>) -> Result<[Layout; 3], Refusal> {
        Ok([
            Layout::new::<Self>(),
            Layout::new::<Vec<Slot>>(),
            Layout::array::<Slot>(capacity.function().loan_plan.loans.len())
                .map_err(|_| Refusal::Metadata)?,
        ])
    }
    pub(crate) fn new(capacity: LoanCapacity<'p>, mut runtime: R) -> Result<Self, Refusal> {
        for layout in Self::metadata_layouts(&capacity)? {
            runtime.reserve_metadata(layout)?;
        }
        let registry = Registry::new(runtime)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacity.function().loan_plan.loans.len())
            .expect("ordinary partition loan allocation invariant failure");
        assert_eq!(slots.capacity(), capacity.function().loan_plan.loans.len());
        slots.resize_with(capacity.function().loan_plan.loans.len(), || Slot {
            generation: 0,
            view: None,
        });
        Ok(Self {
            capacity,
            registry,
            slots: slots.into_boxed_slice(),
            point: 0,
            frame: 1,
            next_frame: 2,
            frames: Vec::new(),
            spare: None,
        })
    }
    pub(super) fn check_partition(
        &mut self,
        extent: u64,
        count: u64,
        width: u64,
    ) -> Result<(), Preparation> {
        self.registry
            .partition_geometry(extent, count, width)
            .map(|_| ())
            .map_err(|error| self.registry.preparation_failure(error))
    }
    pub(crate) fn partition(
        &mut self,
        owner: R::Owner,
        count: u64,
        width: u64,
    ) -> Result<Vec<Owner>, (Preparation, R::Owner)> {
        self.registry
            .partition_with_capacity(owner, count, width, Some(&self.capacity))
    }
    /// Plan starts are checked against the current endpoint, not node-union
    /// kills/live-before. Alias descriptors copy ViewId; they never clone Loan.
    pub(crate) fn borrow(&mut self, owner: &Owner, id: LoanId) -> Result<ViewId, Refusal> {
        let result = (|| {
            self.registry.active()?;
            let plan = &self.capacity.function().loan_plan;
            let loan = plan
                .loans
                .get(id.0 as usize)
                .filter(|loan| loan.id == id)
                .ok_or(Refusal::Identity)?;
            let endpoint = plan
                .endpoints
                .get(self.point as usize)
                .ok_or(Refusal::Identity)?;
            if loan.start != endpoint.point || !endpoint.starts.contains(&id) {
                return Err(Refusal::Identity);
            }
            let slot = self.slots.get_mut(id.0 as usize).ok_or(Refusal::Identity)?;
            if slot.view.is_some() {
                return Err(Refusal::Borrowed);
            }
            let generation = slot.generation.checked_add(1).ok_or(Refusal::Exhausted)?;
            let view = self.registry.borrow(owner)?;
            slot.generation = generation;
            slot.view = Some(view);
            Ok(ViewId {
                invocation: self.registry.invocation,
                frame: self.frame,
                index: id.0,
                generation,
            })
        })();
        self.registry.select(result)
    }
    pub(crate) fn read<T>(&mut self, id: ViewId, f: impl FnOnce(&[u8]) -> T) -> Result<T, Refusal> {
        let result = (|| {
            self.registry.active()?;
            if id.invocation != self.registry.invocation {
                return Err(Refusal::Identity);
            }
            let view = frames::view(id, self.frame, &self.slots, &self.frames)?;
            self.registry.read(view, f)
        })();
        self.registry.select(result)
    }
    pub(crate) fn read_range<T>(
        &mut self,
        id: ViewId,
        start: usize,
        end: usize,
        f: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Refusal> {
        let result = (|| {
            self.registry.active()?;
            if id.invocation != self.registry.invocation {
                return Err(Refusal::Identity);
            }
            let view = frames::view(id, self.frame, &self.slots, &self.frames)?;
            self.registry.read_range(view, start, end, f)
        })();
        self.registry.select(result)
    }
    pub(super) fn active(&self) -> Result<(), Refusal> {
        self.registry.active()
    }
    #[cfg(test)]
    pub(super) fn endpoint(&self) -> u16 {
        self.point
    }
    pub(super) fn reserve_consumer(&mut self, layout: Layout) -> Result<(), Refusal> {
        self.registry.reserve_consumer(layout)
    }
    pub(super) fn preparation_failure(&mut self, error: Preparation) -> Preparation {
        self.registry.preparation_failure(error)
    }
    pub(super) fn preflight_owner(&mut self, owner: &Owner) -> Result<(), Refusal> {
        self.registry.preflight_owner(owner)
    }
    pub(crate) fn extent(&mut self, owner: &Owner) -> Result<usize, Refusal> {
        self.registry.extent(owner)
    }
    pub(crate) fn moved(&mut self, owner: &mut Owner) -> Result<(), Refusal> {
        self.registry.moved(owner)
    }
    pub(crate) fn deep_clone(&mut self, owner: &Owner) -> Result<R::Owner, Refusal> {
        self.registry.deep_clone(owner)
    }
    pub(crate) fn copy_out_and_settle(&mut self, owner: &mut Owner) -> Result<R::Owner, Refusal> {
        self.registry.copy_out_and_settle(owner)
    }
    /// The actual execution receiver supplies its next dense endpoint. Only an
    /// edge from the held current point can retire loans; an ambiguous edge is
    /// refused. All terminations are selected from exact end_edges.
    pub(crate) fn advance(&mut self, next: u16) -> Result<(), Refusal> {
        let result = (|| {
            self.registry.active()?;
            let plan = &self.capacity.function().loan_plan;
            let key = (self.point, next);
            let edge_index = plan
                .edges
                .partition_point(|edge| (edge.from, edge.to) < key);
            let matches = |index: usize| {
                plan.edges
                    .get(index)
                    .is_some_and(|edge| (edge.from, edge.to) == key)
            };
            if !matches(edge_index) || matches(edge_index + 1) {
                return Err(Refusal::Identity);
            }
            let edge_index = u16::try_from(edge_index).map_err(|_| Refusal::Identity)?;
            for loan in &plan.loans {
                if loan.end_edges.contains(&edge_index) {
                    if let Some(view) = self.slots[loan.id.0 as usize].view.as_mut() {
                        self.registry.end_view(view)?;
                        self.slots[loan.id.0 as usize].view = None;
                    }
                }
            }
            self.point = next;
            Ok(())
        })();
        self.registry.select(result)
    }
    /// Only the direct receiver uses this, across a source-proven straight-line
    /// interval. A branch needs an explicit destination from the actual result.
    pub(super) fn advance_straight_to(&mut self, target: u16) -> Result<(), Refusal> {
        let plan = &self.capacity.function().loan_plan;
        for _ in 0..plan.endpoints.len() {
            if self.point == target {
                return self.registry.active();
            }
            let index = plan.edges.partition_point(|edge| edge.from < self.point);
            let Some(edge) = plan.edges.get(index).filter(|edge| edge.from == self.point) else {
                return self.registry.select(Err(Refusal::Identity));
            };
            if plan
                .edges
                .get(index + 1)
                .is_some_and(|next| next.from == self.point)
            {
                return self.registry.select(Err(Refusal::Identity));
            }
            self.advance(edge.to)?;
        }
        self.registry.select(Err(Refusal::Identity))
    }
    pub(crate) fn store(
        &mut self,
        owner: &mut Owner,
        index: usize,
        bytes: &[u8],
    ) -> Result<(), Refusal> {
        self.registry.store(owner, index, bytes)
    }
    pub(super) fn checked_store(
        &mut self,
        owner: &mut Owner,
        index: u64,
        bytes: &[u8],
    ) -> Result<(), super::StoreError> {
        self.registry.checked_store(owner, index, bytes)
    }
    pub(super) fn preflight_settlement(&mut self, owner: &Owner) -> Result<(), Refusal> {
        self.registry.preflight_settlement(owner)
    }
    pub(crate) fn settle(&mut self, owner: &mut Owner) -> Result<(), Refusal> {
        self.registry.settle(owner)
    }
    /// Administrative scope teardown remains available after sticky refusal.
    /// It is never reported as canonical successful settlement/publication.
    pub(crate) fn abandon_views(&mut self) -> Result<(), Refusal> {
        let mut first = None;
        for slot in &mut self.slots {
            if let Some(mut view) = slot.view.take() {
                if let Err(error) = self.registry.end_view(&mut view) {
                    first.get_or_insert(error);
                }
            }
        }
        for frame in &mut self.frames {
            for slot in &mut frame.slots {
                if let Some(mut view) = slot.view.take() {
                    if let Err(error) = self.registry.end_view(&mut view) {
                        first.get_or_insert(error);
                    }
                }
            }
        }
        first.map_or(Ok(()), Err)
    }
}
impl<R: InvocationRuntime> Drop for Loans<'_, R> {
    fn drop(&mut self) {
        let _ = self.abandon_views();
    }
}

#[cfg(test)]
mod tests;

mod frames;

impl Loans<'_, super::funding::BorrowedAccount<'_>> {
    pub(super) fn account_identity(&self) -> u64 {
        self.registry.runtime.identity()
    }
    pub(super) fn allocation_count(&self) -> u32 {
        self.registry.runtime.counters().0
    }
}

impl<'p, R: InvocationRuntime> Loans<'p, R> {
    pub(crate) fn hold_whole(&mut self, owner: R::Owner) -> Result<Owner, (Refusal, R::Owner)> {
        self.registry.hold_whole(owner, &self.capacity)
    }
    pub(crate) fn take_whole(&mut self, owner: &mut Owner) -> Result<R::Owner, Refusal> {
        self.registry.take_whole(owner)
    }
}
