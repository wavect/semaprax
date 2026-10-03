//! RI-05 private ownership-plan seam; it grants no public SDK surface.

use core::{
    any::{Any, TypeId},
    marker::PhantomData,
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONTEXT: AtomicU64 = AtomicU64::new(1);
const MAX_OWNER_SLOTS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerRefusal {
    Forged,
    Stale,
    WrongContext,
    WrongType,
    Closed,
    Capacity,
    GenerationExhausted,
}

pub(crate) struct Owner<T> {
    context: u64,
    generation: u32,
    slot: u32,
    kind: &'static str,
    marker: PhantomData<std::rc::Rc<T>>,
}

impl<T> core::fmt::Debug for Owner<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Owner")
            .field("context", &self.context)
            .field("generation", &self.generation)
            .field("slot", &self.slot)
            .field("kind", &self.kind)
            .finish()
    }
}

#[derive(Debug)]
struct Slot {
    generation: u32,
    kind: &'static str,
    live: bool,
    type_id: TypeId,
    value: Option<Box<dyn Any>>,
}

#[derive(Debug)]
pub(crate) struct OwnerContext {
    id: u64,
    closed: bool,
    slots: Vec<Slot>,
}

impl OwnerContext {
    pub(crate) fn new() -> Self {
        let id = NEXT_CONTEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("opaque owner context id exhausted");
        Self {
            id,
            closed: false,
            slots: Vec::new(),
        }
    }
    pub(crate) fn admit<T: 'static>(
        &mut self,
        kind: &'static str,
        value: T,
    ) -> Result<Owner<T>, OwnerRefusal> {
        if self.closed {
            return Err(OwnerRefusal::Closed);
        }
        if self.slots.len() == MAX_OWNER_SLOTS || self.slots.try_reserve(1).is_err() {
            return Err(OwnerRefusal::Capacity);
        }
        let slot = u32::try_from(self.slots.len()).map_err(|_| OwnerRefusal::Forged)?;
        self.slots.push(Slot {
            generation: 1,
            kind,
            live: true,
            type_id: TypeId::of::<T>(),
            value: Some(Box::new(value)),
        });
        Ok(Owner {
            context: self.id,
            generation: 1,
            slot,
            kind,
            marker: PhantomData,
        })
    }
    pub(crate) fn borrow<T: 'static>(&self, owner: &Owner<T>) -> Result<&T, OwnerRefusal> {
        let slot = self.owner_slot(owner)?;
        slot.value
            .as_deref()
            .and_then(|value| value.downcast_ref::<T>())
            .ok_or(OwnerRefusal::WrongType)
    }

    pub(crate) fn consume<T: 'static>(&mut self, owner: Owner<T>) -> Result<T, OwnerRefusal> {
        if self.closed {
            return Err(OwnerRefusal::Closed);
        }
        let slot = self.owner_slot_mut(&owner)?;
        // Check exhaustion before taking the value so the context retains the
        // Rust value for its mandatory close cleanup on a failed transition.
        let next_generation = slot
            .generation
            .checked_add(1)
            .ok_or(OwnerRefusal::GenerationExhausted)?;
        let value = slot.value.take().ok_or(OwnerRefusal::Stale)?;
        let value = match value.downcast::<T>() {
            Ok(value) => value,
            Err(value) => {
                slot.value = Some(value);
                return Err(OwnerRefusal::WrongType);
            }
        };
        slot.live = false;
        slot.generation = next_generation;
        Ok(*value)
    }

    /// Move one real Rust value between invocation contexts without exposing
    /// its representation. On refusal, the original owner remains usable.
    pub(crate) fn transfer<T: 'static>(
        &mut self,
        owner: Owner<T>,
        destination: &mut OwnerContext,
    ) -> Result<Owner<T>, (OwnerRefusal, Owner<T>)> {
        let refuse = |reason, owner| Err((reason, owner));
        if self.closed {
            return refuse(OwnerRefusal::Closed, owner);
        }
        if destination.closed {
            return refuse(OwnerRefusal::Closed, owner);
        }
        if core::ptr::eq(self, destination) {
            return refuse(OwnerRefusal::WrongContext, owner);
        }
        if destination.slots.len() == MAX_OWNER_SLOTS || destination.slots.try_reserve(1).is_err() {
            return refuse(OwnerRefusal::Capacity, owner);
        }
        let source = match self.owner_slot_mut(&owner) {
            Ok(slot) => slot,
            Err(reason) => return refuse(reason, owner),
        };
        if source.generation.checked_add(1).is_none() {
            return refuse(OwnerRefusal::GenerationExhausted, owner);
        }
        let source_kind = source.kind;
        let source_type = source.type_id;
        let next_generation = source.generation + 1;
        let boxed = match source.value.take() {
            Some(value) => value,
            None => return refuse(OwnerRefusal::Stale, owner),
        };
        let value = match boxed.downcast::<T>() {
            Ok(value) => value,
            Err(boxed) => {
                source.value = Some(boxed);
                return refuse(OwnerRefusal::WrongType, owner);
            }
        };
        source.live = false;
        source.generation = next_generation;

        let slot = destination.slots.len() as u32;
        destination.slots.push(Slot {
            generation: 1,
            kind: source_kind,
            live: true,
            type_id: source_type,
            value: Some(value),
        });
        Ok(Owner {
            context: destination.id,
            generation: 1,
            slot,
            kind: source_kind,
            marker: PhantomData,
        })
    }

    pub(crate) fn close(&mut self) {
        self.closed = true;
        for slot in &mut self.slots {
            slot.live = false;
            drop(slot.value.take());
        }
    }

    fn owner_slot<T: 'static>(&self, owner: &Owner<T>) -> Result<&Slot, OwnerRefusal> {
        if self.closed {
            return Err(OwnerRefusal::Closed);
        }
        if owner.context != self.id {
            return Err(OwnerRefusal::WrongContext);
        }
        let slot = self
            .slots
            .get(owner.slot as usize)
            .ok_or(OwnerRefusal::Forged)?;
        if slot.kind != owner.kind || slot.type_id != TypeId::of::<T>() {
            return Err(OwnerRefusal::WrongType);
        }
        if !slot.live || slot.generation != owner.generation {
            return Err(OwnerRefusal::Stale);
        }
        Ok(slot)
    }

    fn owner_slot_mut<T: 'static>(&mut self, owner: &Owner<T>) -> Result<&mut Slot, OwnerRefusal> {
        if self.closed {
            return Err(OwnerRefusal::Closed);
        }
        if owner.context != self.id {
            return Err(OwnerRefusal::WrongContext);
        }
        let slot = self
            .slots
            .get_mut(owner.slot as usize)
            .ok_or(OwnerRefusal::Forged)?;
        if slot.kind != owner.kind || slot.type_id != TypeId::of::<T>() {
            return Err(OwnerRefusal::WrongType);
        }
        if !slot.live || slot.generation != owner.generation {
            return Err(OwnerRefusal::Stale);
        }
        Ok(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    struct Count(Rc<Cell<u32>>);
    impl Drop for Count {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    #[test]
    fn consume_and_close_drop_real_values_once() {
        let count = Rc::new(Cell::new(0));
        let mut context = OwnerContext::new();
        let owner = context.admit("count", Count(count.clone())).unwrap();
        drop(context.consume(owner).unwrap());
        assert_eq!(count.get(), 1);
        let owner = context.admit("count", Count(count.clone())).unwrap();
        context.close();
        assert_eq!(count.get(), 2);
        assert!(matches!(context.consume(owner), Err(OwnerRefusal::Closed)));
        assert_eq!(count.get(), 2);
    }
    #[test]
    fn refusals_preserve_a_live_owner() {
        let count = Rc::new(Cell::new(0));
        let mut first = OwnerContext::new();
        let mut second = OwnerContext::new();
        let owner = first.admit("count", Count(count.clone())).unwrap();
        let foreign = Owner::<Count> {
            context: second.id,
            generation: owner.generation,
            slot: owner.slot,
            kind: owner.kind,
            marker: PhantomData,
        };
        assert!(matches!(
            first.consume(foreign),
            Err(OwnerRefusal::WrongContext)
        ));
        let wrong = Owner::<u64> {
            context: owner.context,
            generation: owner.generation,
            slot: owner.slot,
            kind: owner.kind,
            marker: PhantomData,
        };
        assert!(matches!(first.consume(wrong), Err(OwnerRefusal::WrongType)));
        drop(first.consume(owner).unwrap());
        assert_eq!(count.get(), 1);
        assert!(second.admit("count", Count(count.clone())).is_ok());
    }

    #[test]
    fn forged_and_stale_tokens_cannot_consume_another_value() {
        let count = Rc::new(Cell::new(0));
        let mut context = OwnerContext::new();
        let owner = context.admit("count", Count(count.clone())).unwrap();
        let forged = Owner::<Count> {
            context: owner.context,
            generation: owner.generation,
            slot: owner.slot + 1,
            kind: owner.kind,
            marker: PhantomData,
        };
        assert!(matches!(context.consume(forged), Err(OwnerRefusal::Forged)));
        assert_eq!(count.get(), 0);
        let stale = Owner::<Count> {
            context: owner.context,
            generation: owner.generation,
            slot: owner.slot,
            kind: owner.kind,
            marker: PhantomData,
        };
        drop(context.consume(owner).unwrap());
        assert!(matches!(context.consume(stale), Err(OwnerRefusal::Stale)));
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn scoped_borrow_and_transfer_preserve_single_drop() {
        let mut source = OwnerContext::new();
        let mut destination = OwnerContext::new();
        let count = Rc::new(Cell::new(0));
        let owner = source.admit("count", Count(count.clone())).unwrap();
        assert_eq!(source.borrow(&owner).unwrap().0.get(), 0);

        let moved = source.transfer(owner, &mut destination).unwrap();
        assert!(matches!(
            source.borrow(&moved),
            Err(OwnerRefusal::WrongContext)
        ));
        assert_eq!(destination.borrow(&moved).unwrap().0.get(), 0);

        drop(destination.consume(moved).unwrap());
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn refused_transfer_preserves_owner_and_bounded_capacity() {
        let count = Rc::new(Cell::new(0));
        let mut source = OwnerContext::new();
        let mut destination = OwnerContext::new();
        let owner = source.admit("count", Count(count.clone())).unwrap();
        destination.close();
        let (reason, owner) = source.transfer(owner, &mut destination).unwrap_err();
        assert_eq!(reason, OwnerRefusal::Closed);
        assert_eq!(source.borrow(&owner).unwrap().0.get(), 0);
        drop(source.consume(owner).unwrap());
        assert_eq!(count.get(), 1);

        let mut bounded = OwnerContext::new();
        for _ in 0..MAX_OWNER_SLOTS {
            bounded.admit("unit", ()).unwrap();
        }
        assert!(matches!(
            bounded.admit("unit", ()),
            Err(OwnerRefusal::Capacity)
        ));

        let mut source = OwnerContext::new();
        let mut full_destination = OwnerContext::new();
        let owner = source.admit("count", Count(count.clone())).unwrap();
        for _ in 0..MAX_OWNER_SLOTS {
            full_destination.admit("unit", ()).unwrap();
        }
        let (reason, owner) = source
            .transfer(owner, &mut full_destination)
            .unwrap_err();
        assert_eq!(reason, OwnerRefusal::Capacity);
        assert_eq!(source.borrow(&owner).unwrap().0.get(), 1);
        drop(source.consume(owner).unwrap());
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn generation_exhaustion_keeps_value_for_close_cleanup() {
        let count = Rc::new(Cell::new(0));
        let mut context = OwnerContext::new();
        let mut owner = context.admit("count", Count(count.clone())).unwrap();
        context.slots[0].generation = u32::MAX;
        owner.generation = u32::MAX;

        assert!(matches!(
            context.consume(owner),
            Err(OwnerRefusal::GenerationExhausted)
        ));
        assert_eq!(count.get(), 0);
        assert!(context.slots[0].live);
        context.close();
        assert_eq!(count.get(), 1);
    }
}
