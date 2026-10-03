//! RI-05 private ownership-plan seam; it grants no public SDK surface.

use core::{
    any::{Any, TypeId},
    marker::PhantomData,
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONTEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerRefusal {
    Forged,
    Stale,
    WrongContext,
    WrongType,
    Closed,
}

#[derive(Debug)]
pub(crate) struct Owner<T> {
    context: u64,
    generation: u32,
    slot: u32,
    kind: &'static str,
    marker: PhantomData<std::rc::Rc<T>>,
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
    pub(crate) fn consume<T: 'static>(&mut self, owner: Owner<T>) -> Result<T, OwnerRefusal> {
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
        if slot.kind != owner.kind {
            return Err(OwnerRefusal::WrongType);
        }
        if slot.type_id != TypeId::of::<T>() {
            return Err(OwnerRefusal::WrongType);
        }
        if !slot.live || slot.generation != owner.generation {
            return Err(OwnerRefusal::Stale);
        }
        let value = slot.value.take().ok_or(OwnerRefusal::Stale)?;
        let value = value.downcast::<T>().map_err(|_| OwnerRefusal::WrongType)?;
        slot.live = false;
        slot.generation = slot.generation.checked_add(1).ok_or(OwnerRefusal::Stale)?;
        Ok(*value)
    }
    pub(crate) fn close(&mut self) {
        self.closed = true;
        for slot in &mut self.slots {
            slot.live = false;
            drop(slot.value.take());
        }
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
}
