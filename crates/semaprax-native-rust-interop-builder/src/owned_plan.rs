//! RI-05 private ownership-plan seam; it grants no public SDK surface.

use core::{
    any::{Any, TypeId},
    marker::PhantomData,
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONTEXT: AtomicU64 = AtomicU64::new(1);
const MAX_OWNER_SLOTS: usize = 256;
const MAX_OWNER_PAYLOAD_BYTES: usize = 1_048_576;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerConversionRefusal {
    Owner(OwnerRefusal),
    PayloadTooLarge,
    InvalidUtf8,
    InvalidOptionTag,
    InvalidResultTag,
    WrongResultArm,
    NonzeroReserved,
    MissingPayload,
    UnexpectedPayload,
    MultiplePayloads,
    SignedOverflow,
    UnsignedOverflow,
    TargetWidthOverflow,
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
        let slot = self.reserve_slot()?;
        Ok(self.store_reserved(slot, kind, value))
    }

    /// Convert checked UTF-8 bytes into an opaque Rust-owned String. Every
    /// refusal returns the original allocation so cleanup remains explicit.
    pub(crate) fn admit_utf8(
        &mut self,
        kind: &'static str,
        bytes: Vec<u8>,
    ) -> Result<Owner<String>, (OwnerConversionRefusal, Vec<u8>)> {
        if bytes.len() > MAX_OWNER_PAYLOAD_BYTES {
            return Err((OwnerConversionRefusal::PayloadTooLarge, bytes));
        }
        let slot = match self.reserve_slot() {
            Ok(slot) => slot,
            Err(reason) => return Err((OwnerConversionRefusal::Owner(reason), bytes)),
        };
        let value = match String::from_utf8(bytes) {
            Ok(value) => value,
            Err(error) => return Err((OwnerConversionRefusal::InvalidUtf8, error.into_bytes())),
        };
        Ok(self.store_reserved(slot, kind, value))
    }

    /// Transfer bounded bytes without copying them into a second allocation.
    pub(crate) fn admit_bytes(
        &mut self,
        kind: &'static str,
        bytes: Vec<u8>,
    ) -> Result<Owner<Vec<u8>>, (OwnerConversionRefusal, Vec<u8>)> {
        if bytes.len() > MAX_OWNER_PAYLOAD_BYTES {
            return Err((OwnerConversionRefusal::PayloadTooLarge, bytes));
        }
        let slot = match self.reserve_slot() {
            Ok(slot) => slot,
            Err(reason) => return Err((OwnerConversionRefusal::Owner(reason), bytes)),
        };
        Ok(self.store_reserved(slot, kind, bytes))
    }

    fn reserve_slot(&mut self) -> Result<u32, OwnerRefusal> {
        if self.closed {
            return Err(OwnerRefusal::Closed);
        }
        if self.slots.len() == MAX_OWNER_SLOTS || self.slots.try_reserve(1).is_err() {
            return Err(OwnerRefusal::Capacity);
        }
        u32::try_from(self.slots.len()).map_err(|_| OwnerRefusal::Forged)
    }

    fn store_reserved<T: 'static>(&mut self, slot: u32, kind: &'static str, value: T) -> Owner<T> {
        self.slots.push(Slot {
            generation: 1,
            kind,
            live: true,
            type_id: TypeId::of::<T>(),
            value: Some(Box::new(value)),
        });
        Owner {
            context: self.id,
            generation: 1,
            slot,
            kind,
            marker: PhantomData,
        }
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

/// Decode the closed 0=None / 1=Some option tag. A malformed frame returns
/// the original owner, if present, rather than consuming or dropping it.
fn decode_option<T>(
    tag: u8,
    reserved: [u8; 7],
    value: Option<T>,
) -> Result<Option<T>, (OwnerConversionRefusal, Option<T>)> {
    if reserved != [0; 7] {
        return Err((OwnerConversionRefusal::NonzeroReserved, value));
    }
    match (tag, value) {
        (0, None) => Ok(None),
        (0, Some(value)) => Err((OwnerConversionRefusal::UnexpectedPayload, Some(value))),
        (1, None) => Err((OwnerConversionRefusal::MissingPayload, None)),
        (1, Some(value)) => Ok(Some(value)),
        (_, value) => Err((OwnerConversionRefusal::InvalidOptionTag, value)),
    }
}

/// Decode the closed 0=Ok / 1=Err result tag. Invalid frames preserve both
/// alternatives so a caller can run normal Rust cleanup or retry admission.
fn decode_result<T, E>(
    tag: u8,
    reserved: [u8; 7],
    success: Option<T>,
    failure: Option<E>,
) -> Result<Result<T, E>, (OwnerConversionRefusal, Option<T>, Option<E>)> {
    if reserved != [0; 7] {
        return Err((OwnerConversionRefusal::NonzeroReserved, success, failure));
    }
    match (tag, success, failure) {
        (0, Some(value), None) => Ok(Ok(value)),
        (1, None, Some(error)) => Ok(Err(error)),
        (0, None, None) | (1, None, None) => {
            Err((OwnerConversionRefusal::MissingPayload, None, None))
        }
        (0 | 1, success, failure) if success.is_some() && failure.is_some() => {
            Err((OwnerConversionRefusal::MultiplePayloads, success, failure))
        }
        (0, None, Some(error)) => Err((OwnerConversionRefusal::WrongResultArm, None, Some(error))),
        (1, Some(value), None) => Err((OwnerConversionRefusal::WrongResultArm, Some(value), None)),
        (0 | 1, success, failure) => {
            Err((OwnerConversionRefusal::UnexpectedPayload, success, failure))
        }
        (_, success, failure) => Err((OwnerConversionRefusal::InvalidResultTag, success, failure)),
    }
}

fn signed_to_i64(value: i128) -> Result<i64, OwnerConversionRefusal> {
    i64::try_from(value).map_err(|_| OwnerConversionRefusal::SignedOverflow)
}

fn unsigned_to_u64(value: u128) -> Result<u64, OwnerConversionRefusal> {
    u64::try_from(value).map_err(|_| OwnerConversionRefusal::UnsignedOverflow)
}

fn signed_i64_to_i32(value: i64) -> Result<i32, OwnerConversionRefusal> {
    i32::try_from(value).map_err(|_| OwnerConversionRefusal::SignedOverflow)
}

fn unsigned_u64_to_u32(value: u64) -> Result<u32, OwnerConversionRefusal> {
    u32::try_from(value).map_err(|_| OwnerConversionRefusal::UnsignedOverflow)
}

fn unsigned_u64_to_usize(value: u64) -> Result<usize, OwnerConversionRefusal> {
    usize::try_from(value).map_err(|_| OwnerConversionRefusal::TargetWidthOverflow)
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
        let (reason, owner) = source.transfer(owner, &mut full_destination).unwrap_err();
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

    #[test]
    fn utf8_and_byte_admission_are_bounded_and_preserve_rejected_payloads() {
        let mut context = OwnerContext::new();
        let text = context
            .admit_utf8("text", "regex 🦀".as_bytes().to_vec())
            .unwrap();
        assert_eq!(context.borrow(&text).unwrap(), "regex 🦀");

        let invalid = vec![b'a', 0xff, b'b'];
        let (reason, returned) = context.admit_utf8("text", invalid.clone()).unwrap_err();
        assert_eq!(reason, OwnerConversionRefusal::InvalidUtf8);
        assert_eq!(returned, invalid);

        let oversized = vec![0x55; MAX_OWNER_PAYLOAD_BYTES + 1];
        let (reason, returned) = context.admit_bytes("bytes", oversized).unwrap_err();
        assert_eq!(reason, OwnerConversionRefusal::PayloadTooLarge);
        assert_eq!(returned.len(), MAX_OWNER_PAYLOAD_BYTES + 1);

        let bytes = vec![0, 1, 2, 255];
        let original_ptr = bytes.as_ptr();
        let owned = context.admit_bytes("bytes", bytes).unwrap();
        assert_eq!(context.borrow(&owned).unwrap().as_ptr(), original_ptr);
        assert_eq!(context.borrow(&owned).unwrap(), &[0, 1, 2, 255]);
    }

    #[test]
    fn option_and_result_tags_are_closed_and_refusals_return_owners() {
        let count = Rc::new(Cell::new(0));
        let mut context = OwnerContext::new();
        assert!(matches!(
            decode_option::<Owner<Count>>(0, [0; 7], None),
            Ok(None)
        ));
        assert!(matches!(
            decode_result::<Owner<Count>, Owner<u64>>(0, [0; 7], None, None),
            Err((OwnerConversionRefusal::MissingPayload, None, None))
        ));

        let owner = context.admit("count", Count(count.clone())).unwrap();
        let (reason, owner) = decode_option(2, [0; 7], Some(owner)).unwrap_err();
        assert_eq!(reason, OwnerConversionRefusal::InvalidOptionTag);
        assert_eq!(context.borrow(owner.as_ref().unwrap()).unwrap().0.get(), 0);

        let success = context.admit("count", Count(count.clone())).unwrap();
        let failure = context.admit("failure", 41_u64).unwrap();
        let (reason, success, failure) =
            decode_result(0, [0; 7], Some(success), Some(failure)).unwrap_err();
        assert_eq!(reason, OwnerConversionRefusal::MultiplePayloads);
        assert_eq!(
            context.borrow(success.as_ref().unwrap()).unwrap().0.get(),
            0
        );
        assert_eq!(*context.borrow(failure.as_ref().unwrap()).unwrap(), 41);

        let wrong_arm = context.admit("failure", 43_u64).unwrap();
        let (reason, success, failure) =
            decode_result::<Owner<Count>, Owner<u64>>(0, [0; 7], None, Some(wrong_arm))
                .unwrap_err();
        assert_eq!(reason, OwnerConversionRefusal::WrongResultArm);
        assert!(success.is_none());
        assert_eq!(*context.borrow(&failure.unwrap()).unwrap(), 43);

        let (reason, failure) =
            decode_option::<Owner<Count>>(1, [1, 0, 0, 0, 0, 0, 0], None).unwrap_err();
        assert_eq!(reason, OwnerConversionRefusal::NonzeroReserved);
        assert!(failure.is_none());

        let (reason, success, failure) =
            decode_result::<Owner<Count>, Owner<u64>>(7, [0; 7], None, None).unwrap_err();
        assert_eq!(reason, OwnerConversionRefusal::InvalidResultTag);
        assert!(success.is_none() && failure.is_none());
        drop(context);
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn integer_width_conversions_check_both_signed_and_unsigned_bounds() {
        assert_eq!(signed_to_i64(i64::MIN as i128), Ok(i64::MIN));
        assert_eq!(signed_to_i64(i64::MAX as i128), Ok(i64::MAX));
        assert_eq!(
            signed_to_i64(i64::MIN as i128 - 1),
            Err(OwnerConversionRefusal::SignedOverflow)
        );
        assert_eq!(
            signed_to_i64(i64::MAX as i128 + 1),
            Err(OwnerConversionRefusal::SignedOverflow)
        );
        assert_eq!(unsigned_to_u64(u64::MAX as u128), Ok(u64::MAX));
        assert_eq!(
            unsigned_to_u64(u64::MAX as u128 + 1),
            Err(OwnerConversionRefusal::UnsignedOverflow)
        );
        assert_eq!(signed_i64_to_i32(i32::MIN as i64), Ok(i32::MIN));
        assert_eq!(signed_i64_to_i32(i32::MAX as i64), Ok(i32::MAX));
        assert_eq!(
            signed_i64_to_i32(i32::MAX as i64 + 1),
            Err(OwnerConversionRefusal::SignedOverflow)
        );
        assert_eq!(unsigned_u64_to_u32(u32::MAX as u64), Ok(u32::MAX));
        assert_eq!(
            unsigned_u64_to_u32(u32::MAX as u64 + 1),
            Err(OwnerConversionRefusal::UnsignedOverflow)
        );
        assert_eq!(
            unsigned_u64_to_usize(u64::MAX),
            if usize::BITS == 64 {
                Ok(u64::MAX as usize)
            } else {
                Err(OwnerConversionRefusal::TargetWidthOverflow)
            }
        );
    }
}
