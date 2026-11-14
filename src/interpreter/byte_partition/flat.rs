//! Affine flat partition containers. No Arc control block or second value array.
//! Actual language result/iterator installation remains sealed and closed.
use super::{loans::Loans, InvocationRuntime, Owner};
use crate::construction_fusion::{domain::Refusal, partition::Preparation};
use std::{alloc::Layout, num::NonZeroU32, vec::IntoIter};

#[derive(Debug, PartialEq)]
pub(crate) struct PartitionVec {
    owners: Vec<Owner>,
    capacity: usize,
    allocation: Option<NonZeroU32>,
}
#[derive(Debug)]
pub(crate) struct PartitionIterator {
    owners: IntoIter<Owner>,
    allocation: Option<NonZeroU32>,
}
impl PartitionVec {
    /// Numeric faults precede all reservations. The header and original output
    /// array are paid on the same account before any child becomes observable.
    pub(crate) fn partition<R: InvocationRuntime>(
        receiver: &mut Loans<'_, R>,
        owner: R::Owner,
        count: u64,
        width: u64,
    ) -> Result<Self, (Preparation, R::Owner)> {
        if let Err(error) = receiver.check_partition(R::bytes(&owner).len() as u64, count, width) {
            return Err((error, owner));
        }
        if let Err(error) = receiver.reserve_consumer(Layout::new::<Self>()) {
            return Err((Preparation::Authority(error), owner));
        }
        let owners = receiver.partition(owner, count, width)?;
        let capacity = owners.len();
        Ok(Self {
            owners,
            capacity,
            allocation: None,
        })
    }
    pub(super) fn with_allocation(mut self, allocation: NonZeroU32) -> Self {
        self.allocation = Some(allocation);
        self
    }
    pub(crate) fn len(&self) -> usize {
        self.owners.len()
    }
    pub(crate) fn capacity(&self) -> usize {
        self.capacity
    }
    pub(crate) fn owner(&self, index: usize) -> Option<&Owner> {
        self.owners.get(index)
    }
    /// Private affine extraction, not a new ordinary vec_pop declaration.
    /// Publication follows exact child authentication and successful renewal.
    pub(crate) fn take_last<R: InvocationRuntime>(
        &mut self,
        receiver: &mut Loans<'_, R>,
    ) -> Result<Option<Owner>, Refusal> {
        receiver.active()?;
        let Some(owner) = self.owners.last_mut() else {
            return Ok(None);
        };
        receiver.moved(owner)?;
        Ok(self.owners.pop())
    }
    pub(crate) fn clear<R: InvocationRuntime>(
        &mut self,
        receiver: &mut Loans<'_, R>,
    ) -> Result<(), Refusal> {
        receiver.active()?;
        self.settle(receiver)
    }
    /// Exact Vec<Bytes> container lifecycle: ascending rows, one owned leaf per
    /// row. Canonical callers invoke this only at their actual finalizer action.
    /// Cleanup remains usable after a selected failure; it grants no publication.
    pub(crate) fn settle<R: InvocationRuntime>(
        &mut self,
        receiver: &mut Loans<'_, R>,
    ) -> Result<(), Refusal> {
        for owner in &self.owners {
            receiver.preflight_settlement(owner)?;
        }
        for owner in &mut self.owners {
            receiver.settle(owner)?;
        }
        self.owners.clear();
        Ok(())
    }
    /// IntoIter retains the same paid array without allocation or owner aliases.
    pub(crate) fn into_iter<R: InvocationRuntime>(
        self,
        receiver: &mut Loans<'_, R>,
    ) -> Result<PartitionIterator, (Refusal, Self)> {
        for owner in &self.owners {
            if let Err(error) = receiver.preflight_owner(owner) {
                return Err((error, self));
            }
        }
        if let Err(error) = receiver.reserve_consumer(Layout::new::<PartitionIterator>()) {
            return Err((error, self));
        }
        Ok(PartitionIterator {
            owners: self.owners.into_iter(),
            allocation: self.allocation,
        })
    }
}
impl PartitionIterator {
    pub(crate) fn remaining(&self) -> usize {
        self.owners.len()
    }
    /// Typed child transfer retains the whole producer's logical allocation.
    pub(super) fn next_bytes<R: InvocationRuntime>(
        &mut self,
        receiver: &mut Loans<'_, R>,
    ) -> Result<Option<crate::interpreter::OwnedBytesValue>, Refusal> {
        let allocation = self.allocation.ok_or(Refusal::Identity)?;
        self.next(receiver).map(|child| {
            child.map(|owner| {
                crate::interpreter::OwnedBytesValue::partition(allocation.get(), owner)
            })
        })
    }
    /// The actual language IterStep constructor remains a separate checked
    /// receiving boundary. This method returns only the internal affine leaf.
    pub(crate) fn next<R: InvocationRuntime>(
        &mut self,
        receiver: &mut Loans<'_, R>,
    ) -> Result<Option<Owner>, Refusal> {
        receiver.active()?;
        let Some(owner) = self.owners.as_mut_slice().first_mut() else {
            return Ok(None);
        };
        receiver.moved(owner)?;
        Ok(self.owners.next())
    }
    pub(crate) fn settle<R: InvocationRuntime>(
        &mut self,
        receiver: &mut Loans<'_, R>,
    ) -> Result<(), Refusal> {
        for owner in self.owners.as_slice() {
            receiver.preflight_settlement(owner)?;
        }
        for owner in self.owners.as_mut_slice() {
            receiver.settle(owner)?;
        }
        // Tokens have no independent Drop finalizer. Drop of this paid array
        // follows leaf settlement; registry abandonment is not canonical exit.
        self.owners.by_ref().for_each(drop);
        Ok(())
    }
}
#[cfg(test)]
mod tests;

impl PartialEq for PartitionIterator {
    fn eq(&self, other: &Self) -> bool {
        self.allocation == other.allocation && self.owners.as_slice() == other.owners.as_slice()
    }
}
