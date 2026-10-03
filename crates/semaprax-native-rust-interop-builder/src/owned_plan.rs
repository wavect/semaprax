//! RI-05 private ownership-plan seam; it grants no public SDK surface.

use core::{
    any::{Any, TypeId},
    cell::Cell,
    marker::PhantomData,
};
use std::rc::Rc;
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
    LoanActive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerLoanRefusal {
    Owner(OwnerRefusal),
    SharedConflict,
    ExclusiveConflict,
    Reentrant,
    CounterExhausted,
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
    access: Rc<Cell<SlotAccess>>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SlotAccess {
    shared: u32,
    exclusive: bool,
    callback_active: bool,
}

impl SlotAccess {
    fn has_active_access(self) -> bool {
        self.shared != 0 || self.exclusive || self.callback_active
    }
}

/// Invocation-scoped state guard, not an owning reference to the Rust value.
/// Dropping it closes the access state; it does not keep the owner alive.
pub(crate) struct OwnerLoanGuard {
    access: Rc<Cell<SlotAccess>>,
    kind: OwnerLoanKind,
}

/// A private, owner-tied view returned by an admitted Rust operation.
///
/// The owner reference makes the Rust lifetime relation explicit: the view
/// cannot outlive, move, or drop its `Owner`. The guard keeps the matching
/// slot shared for the lifetime of the view, so dynamic owner transitions and
/// conflicting exclusive access still fail closed. This is only the builder's
/// model seam; it does not create a C-ABI view or authorize a generated
/// Regex/Url binding.
pub(crate) struct OwnerView<'owner, T, View: ?Sized> {
    owner: &'owner Owner<T>,
    view: &'owner View,
    _loan: OwnerLoanGuard,
}

impl<T, View: ?Sized> OwnerView<'_, T, View> {
    pub(crate) fn as_ref(&self) -> &View {
        self.view
    }

    pub(crate) fn owner(&self) -> &Owner<T> {
        self.owner
    }
}

#[derive(Clone, Copy)]
enum OwnerLoanKind {
    Shared,
    Exclusive,
}

impl Drop for OwnerLoanGuard {
    fn drop(&mut self) {
        let mut access = self.access.get();
        match self.kind {
            OwnerLoanKind::Shared => access.shared -= 1,
            OwnerLoanKind::Exclusive => access.exclusive = false,
        }
        self.access.set(access);
    }
}

struct OwnerCallbackGuard(Rc<Cell<SlotAccess>>);

impl Drop for OwnerCallbackGuard {
    fn drop(&mut self) {
        let mut access = self.0.get();
        access.callback_active = false;
        self.0.set(access);
    }
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
            access: Rc::new(Cell::new(SlotAccess::default())),
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

    /// Acquire a shared resource loan for the current invocation scope.
    /// Multiple shared loans may coexist, but no exclusive loan may be live.
    pub(crate) fn shared_loan<T: 'static>(
        &self,
        owner: &Owner<T>,
    ) -> Result<OwnerLoanGuard, OwnerLoanRefusal> {
        let access = self
            .owner_slot(owner)
            .map_err(OwnerLoanRefusal::Owner)?
            .access
            .clone();
        let mut state = access.get();
        if state.exclusive {
            return Err(OwnerLoanRefusal::SharedConflict);
        }
        state.shared = state
            .shared
            .checked_add(1)
            .ok_or(OwnerLoanRefusal::CounterExhausted)?;
        access.set(state);
        Ok(OwnerLoanGuard {
            access,
            kind: OwnerLoanKind::Shared,
        })
    }

    /// Acquire an exclusive resource loan for the current invocation scope.
    pub(crate) fn exclusive_loan<T: 'static>(
        &self,
        owner: &Owner<T>,
    ) -> Result<OwnerLoanGuard, OwnerLoanRefusal> {
        let access = self
            .owner_slot(owner)
            .map_err(OwnerLoanRefusal::Owner)?
            .access
            .clone();
        let mut state = access.get();
        if state.exclusive || state.shared != 0 {
            return Err(OwnerLoanRefusal::ExclusiveConflict);
        }
        state.exclusive = true;
        access.set(state);
        Ok(OwnerLoanGuard {
            access,
            kind: OwnerLoanKind::Exclusive,
        })
    }

    /// Enter a callback for this resource. Re-entry is rejected before the
    /// callback closure runs, so a refused call cannot reach the target.
    pub(crate) fn with_callback<T: 'static, R>(
        &self,
        owner: &Owner<T>,
        callback: impl FnOnce() -> R,
    ) -> Result<R, OwnerLoanRefusal> {
        let access = self
            .owner_slot(owner)
            .map_err(OwnerLoanRefusal::Owner)?
            .access
            .clone();
        let mut state = access.get();
        // M1 rejects all callback entry while this resource is loaned. The
        // check is per owner slot, before the callback target can execute.
        if state.has_active_access() {
            return Err(OwnerLoanRefusal::Reentrant);
        }
        state.callback_active = true;
        access.set(state);
        let _guard = OwnerCallbackGuard(access);
        Ok(callback())
    }

    /// Invoke a Rust operation with a scoped `&str` into an admitted String.
    /// The higher-ranked closure can return owned data, but cannot return a
    /// view tied to this invocation. This helper models the loan boundary; it
    /// does not provide a generated C-ABI borrowed view. Rust APIs such as
    /// `Url::as_str` remain unsupported until a returned view carries a
    /// checked borrow of its owner through the generated ABI.
    pub(crate) fn with_borrowed_str<R>(
        &self,
        owner: &Owner<String>,
        invoke: impl for<'loan> FnOnce(&'loan str) -> R,
    ) -> Result<R, OwnerLoanRefusal> {
        let _loan = self.shared_loan(owner)?;
        let value = self.borrow(owner).map_err(OwnerLoanRefusal::Owner)?;
        Ok(invoke(value.as_str()))
    }

    /// Return a zero-copy `str` view tied to this exact admitted String owner.
    ///
    /// Unlike `with_borrowed_str`, this model represents an approved returned
    /// owner relation. The caller cannot retain it past the owner borrow, and
    /// its live shared loan rejects a conflicting exclusive operation or
    /// callback re-entry. Only Rust's checked `String::as_str` conversion is
    /// admitted here; arbitrary pointer/slice reinterpretation is absent.
    pub(crate) fn borrowed_str_view<'owner>(
        &'owner self,
        owner: &'owner Owner<String>,
    ) -> Result<OwnerView<'owner, String, str>, OwnerLoanRefusal> {
        let loan = self.shared_loan(owner)?;
        let value = self.borrow(owner).map_err(OwnerLoanRefusal::Owner)?;
        Ok(OwnerView {
            owner,
            view: value.as_str(),
            _loan: loan,
        })
    }

    pub(crate) fn consume<T: 'static>(&mut self, owner: Owner<T>) -> Result<T, OwnerRefusal> {
        if self.closed {
            return Err(OwnerRefusal::Closed);
        }
        let slot = self.owner_slot_mut(&owner)?;
        if slot.access.get().has_active_access() {
            return Err(OwnerRefusal::LoanActive);
        }
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
        if source.access.get().has_active_access() {
            return refuse(OwnerRefusal::LoanActive, owner);
        }
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
            access: Rc::new(Cell::new(SlotAccess::default())),
        });
        Ok(Owner {
            context: destination.id,
            generation: 1,
            slot,
            kind: source_kind,
            marker: PhantomData,
        })
    }

    /// Best-effort legacy close. Use `try_close` to observe active-loan refusal.
    pub(crate) fn close(&mut self) {
        let _ = self.try_close();
    }

    /// Close and drop every admitted value, refusing to finalize while any
    /// invocation-scoped loan or callback remains active.
    pub(crate) fn try_close(&mut self) -> Result<(), OwnerRefusal> {
        if self
            .slots
            .iter()
            .any(|slot| slot.access.get().has_active_access())
        {
            return Err(OwnerRefusal::LoanActive);
        }
        self.closed = true;
        for slot in &mut self.slots {
            slot.live = false;
            drop(slot.value.take());
        }
        Ok(())
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

    struct RegexFixture {
        needle: &'static str,
        input_pointer: *const u8,
        copied_bytes: Cell<usize>,
    }

    impl RegexFixture {
        fn is_match(&self, input: &str) -> bool {
            if input.as_ptr() != self.input_pointer {
                self.copied_bytes
                    .set(self.copied_bytes.get().saturating_add(input.len()));
            }
            input.contains(self.needle)
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
    fn invocation_loans_enforce_shared_exclusive_and_finalize_boundaries() {
        let count = Rc::new(Cell::new(0));
        let mut source = OwnerContext::new();
        let mut destination = OwnerContext::new();
        let owner = source.admit("count", Count(count.clone())).unwrap();

        let shared_a = source.shared_loan(&owner).unwrap();
        let shared_b = source.shared_loan(&owner).unwrap();
        assert!(matches!(
            source.exclusive_loan(&owner),
            Err(OwnerLoanRefusal::ExclusiveConflict)
        ));
        assert!(matches!(source.try_close(), Err(OwnerRefusal::LoanActive)));
        let (reason, owner) = source.transfer(owner, &mut destination).unwrap_err();
        assert_eq!(reason, OwnerRefusal::LoanActive);
        assert!(source.borrow(&owner).is_ok());
        drop(shared_a);
        assert_eq!(count.get(), 0);
        drop(shared_b);

        let exclusive = source.exclusive_loan(&owner).unwrap();
        assert!(matches!(
            source.shared_loan(&owner),
            Err(OwnerLoanRefusal::SharedConflict)
        ));
        assert!(matches!(source.try_close(), Err(OwnerRefusal::LoanActive)));
        drop(exclusive);

        let moved = source.transfer(owner, &mut destination).unwrap();
        drop(destination.consume(moved).unwrap());
        assert_eq!(count.get(), 1);

        let pending = source.admit("count", Count(count.clone())).unwrap();
        let active = source.shared_loan(&pending).unwrap();
        assert!(matches!(
            source.consume(pending),
            Err(OwnerRefusal::LoanActive)
        ));
        assert_eq!(count.get(), 1);
        drop(active);
        assert_eq!(source.try_close(), Ok(()));
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn callback_reentry_is_refused_before_the_target_runs() {
        let mut context = OwnerContext::new();
        let owner = context.admit("callback", ()).unwrap();
        let calls = Cell::new(0);
        context
            .with_callback(&owner, || {
                calls.set(calls.get() + 1);
                let nested = context.with_callback(&owner, || calls.set(calls.get() + 1));
                assert_eq!(nested, Err(OwnerLoanRefusal::Reentrant));
            })
            .unwrap();
        assert_eq!(calls.get(), 1);
        context
            .with_callback(&owner, || calls.set(calls.get() + 1))
            .unwrap();
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn borrowed_string_reaches_rust_method_without_adapter_copy() {
        let mut context = OwnerContext::new();
        let mut destination = OwnerContext::new();
        let input = String::from("https://example.invalid/🦀");
        let original_pointer = input.as_ptr();
        let input_owner = context.admit("url", input).unwrap();
        let regex = RegexFixture {
            needle: "example.invalid",
            input_pointer: original_pointer,
            copied_bytes: Cell::new(0),
        };

        let matched = context
            .with_borrowed_str(&input_owner, |borrowed| regex.is_match(borrowed))
            .unwrap();
        assert!(matched);
        assert_eq!(regex.copied_bytes.get(), 0);
        assert_eq!(
            context.borrow(&input_owner).unwrap().as_ptr(),
            original_pointer
        );

        // The scoped view has ended, so an affine move is admitted again.
        let moved = context.transfer(input_owner, &mut destination).unwrap();
        assert_eq!(
            destination.borrow(&moved).unwrap().as_ptr(),
            original_pointer
        );
    }

    #[test]
    fn returned_str_view_retains_its_owner_loan_and_rejects_reentry() {
        let mut context = OwnerContext::new();
        let owner = context.admit("url", String::from("https://example.invalid/🦀")).unwrap();
        let calls = Cell::new(0);
        let owner_pointer = context.borrow(&owner).unwrap().as_ptr();
        let view = context.borrowed_str_view(&owner).unwrap();

        assert_eq!(view.as_ref(), "https://example.invalid/🦀");
        assert_eq!(view.as_ref().as_ptr(), owner_pointer);
        assert_eq!(view.owner().slot, owner.slot);
        // An exclusive operation represents storage mutation or reallocation;
        // the live returned view holds the conflicting shared loan.
        assert!(matches!(
            context.exclusive_loan(&owner),
            Err(OwnerLoanRefusal::ExclusiveConflict)
        ));
        // M1 refuses callback entry for this owner before the target runs.
        assert_eq!(
            context.with_callback(&owner, || calls.set(calls.get() + 1)),
            Err(OwnerLoanRefusal::Reentrant)
        );
        assert_eq!(calls.get(), 0);

        drop(view);
        context
            .with_callback(&owner, || calls.set(calls.get() + 1))
            .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(context.consume(owner).unwrap(), "https://example.invalid/🦀");
    }

    #[test]
    fn returned_view_cannot_escape_its_owner_or_survive_owner_move() {
        // This is the same owner-tied shape used by `OwnerView`: both the
        // owner token and returned `str` carry the one borrow lifetime.
        // Compile it as a negative control so a future lifetime weakening is
        // visible even though the private model has no public C ABI yet.
        let root = std::env::temp_dir().join(format!(
            "semaprax-ri06-returned-view-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root).expect("create private rustc fixture directory");
        let source = root.join("returned_view_escape.rs");
        std::fs::write(
            &source,
            r#"
struct Owner(String);
struct OwnerView<'owner> { owner: &'owner Owner, view: &'owner str }
fn borrowed_str_view<'owner>(owner: &'owner Owner) -> OwnerView<'owner> {
    OwnerView { owner, view: owner.0.as_str() }
}
pub fn move_owner() {
    let owner = Owner(String::from("view"));
    let view = borrowed_str_view(&owner);
    drop(owner);
    let _ = view.view;
}
pub fn escape_view() -> OwnerView<'static> {
    let owner = Owner(String::from("view"));
    borrowed_str_view(&owner)
}
"#,
        )
        .expect("write compile-fail fixture");
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = std::process::Command::new(rustc)
            .arg("--crate-type=lib")
            .arg("--emit=metadata")
            .arg("--out-dir")
            .arg(&root)
            .arg(&source)
            .output()
            .expect("run the selected Rust compiler");
        let _ = std::fs::remove_dir_all(&root);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "returned view escaped: {stderr}");
        assert!(
            stderr.contains("cannot move out")
                || stderr.contains("borrowed value does not live long enough")
                || stderr.contains("E0505")
                || stderr.contains("E0515"),
            "expected owner-bound view rejection, got: {stderr}"
        );
    }

    #[test]
    fn invocation_scoped_str_type_cannot_return_a_borrowed_view() {
        // Compile the exact higher-ranked callback shape used by
        // `with_borrowed_str`. The returned `&str` must not be able to borrow
        // from the invocation-only argument.
        let root = std::env::temp_dir().join(format!(
            "semaprax-ri06-borrow-escape-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root).expect("create private rustc fixture directory");
        let source = root.join("borrow_escape.rs");
        std::fs::write(
            &source,
            r#"
fn with_borrowed_str<R>(invoke: impl for<'loan> FnOnce(&'loan str) -> R) -> R {
    invoke("invocation")
}

pub fn escape_view(input: &str) -> &str {
    with_borrowed_str(|view| view)
}
"#,
        )
        .expect("write compile-fail fixture");
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = std::process::Command::new(rustc)
            .arg("--crate-type=lib")
            .arg("--emit=metadata")
            .arg("--out-dir")
            .arg(&root)
            .arg(&source)
            .output()
            .expect("run the selected Rust compiler");
        let _ = std::fs::remove_dir_all(&root);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "borrowed view escaped: {stderr}");
        assert!(
            stderr.contains("lifetime") || stderr.contains("FnOnce"),
            "expected a lifetime-boundary rejection, got: {stderr}"
        );
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
