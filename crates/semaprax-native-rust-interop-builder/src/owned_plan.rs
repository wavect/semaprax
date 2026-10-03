//! RI-05 private ownership-plan seam; it grants no public SDK surface.

use core::marker::PhantomData;

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
    marker: PhantomData<fn() -> T>,
}

#[derive(Debug)]
struct Slot {
    generation: u32,
    kind: &'static str,
    live: bool,
}

#[derive(Debug)]
pub(crate) struct OwnerContext {
    id: u64,
    closed: bool,
    slots: Vec<Slot>,
}

impl OwnerContext {
    pub(crate) fn new(id: u64) -> Self {
        Self {
            id,
            closed: false,
            slots: Vec::new(),
        }
    }
    pub(crate) fn admit<T>(&mut self, kind: &'static str) -> Result<Owner<T>, OwnerRefusal> {
        if self.closed {
            return Err(OwnerRefusal::Closed);
        }
        let slot = u32::try_from(self.slots.len()).map_err(|_| OwnerRefusal::Forged)?;
        self.slots.push(Slot {
            generation: 1,
            kind,
            live: true,
        });
        Ok(Owner {
            context: self.id,
            generation: 1,
            slot,
            kind,
            marker: PhantomData,
        })
    }
    pub(crate) fn consume<T>(&mut self, owner: Owner<T>) -> Result<(), OwnerRefusal> {
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
        if !slot.live || slot.generation != owner.generation {
            return Err(OwnerRefusal::Stale);
        }
        slot.live = false;
        slot.generation = slot.generation.checked_add(1).ok_or(OwnerRefusal::Stale)?;
        Ok(())
    }
    pub(crate) fn close(&mut self) {
        self.closed = true;
        for slot in &mut self.slots {
            slot.live = false;
        }
    }
}

pub(crate) fn render_drop_fixture() -> &'static str {
    "struct Owned(std::rc::Rc<std::cell::Cell<u32>>); impl Drop for Owned { fn drop(&mut self) { self.0.set(self.0.get()+1) } }\n"
}
