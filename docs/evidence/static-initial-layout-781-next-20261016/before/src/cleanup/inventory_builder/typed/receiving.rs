//! Actual per-call failure/output and added bank-borrow receiving. Initial bank
//! failure is the declared acquisition ABI; its token still grants no authority.
use super::{InventoryBuilder, InventoryFailure, Kind, Ledger, Reason};
use crate::cleanup::{
    CleanupEntryState, CleanupFlag, CleanupInventory, CleanupStorageId, CleanupStorageSlot,
    ConditionalVariantEntry,
};
use crate::cleanup_plan::ReplayPhysicalRefusal;
use crate::cleanup_plan::ReplayPhysicalReservation;
use std::cell::{RefCell, RefMut};
use std::mem::size_of;
// Returned/caller Result pair; opaque constructor/pattern pair; finite data;
// real added reservation Result pair; actual debit forwarding Result pair;
// byte amount and checked-sub next; checked-sub Option pair; RefCell borrow
// initializer/callee pair; RefMut return/local pair; Ledger destructure borrow;
// two actual field-binding receivers and reserve/work method receiving refs.
type Controls<'a, T> = (
    Result<T, InventoryFailure>,
    Result<T, InventoryFailure>,
    InventoryFailure,
    InventoryFailure,
    Kind,
    Kind,
    Result<(), ReplayPhysicalRefusal>,
    Result<(), ReplayPhysicalRefusal>,
    Result<(), InventoryFailure>,
    Result<(), InventoryFailure>,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
    &'a RefCell<Ledger<'a>>,
    &'a RefCell<Ledger<'a>>,
    RefMut<'a, Ledger<'a>>,
    RefMut<'a, Ledger<'a>>,
    &'a mut RefMut<'a, Ledger<'a>>,
    &'a mut Ledger<'a>,
    &'a mut &'a mut ReplayPhysicalReservation,
    &'a mut &'a mut usize,
    &'a mut ReplayPhysicalReservation,
    &'a mut ReplayPhysicalReservation,
    &'a mut usize,
    &'a mut usize,
);
// One initializer for each declared receiving field, additional to all original
// traversal/probe/clone work. This never resets or forecasts an allowance.
pub(super) const CONTROL_INITIALIZERS: usize = 32;
// Each method returns immediately after constructing a local leaf failure.
// These are alternative initializer roles, not simultaneous tuple receivers.
// Nested helpers admit their own roles before entry and are not credited here.
pub(super) const REASON: usize = size_of::<Reason>();
pub(super) const PHYSICAL: usize = size_of::<super::Physical>();
const PROFILE: usize =
    size_of::<crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal>();
const TYPE_DROP: usize =
    size_of::<crate::cleanup_plan::renewal_profile::type_drop_refusal::TypeDropRefusal>();
pub(super) const CLASSIFIER: usize = if PROFILE > TYPE_DROP {
    PROFILE
} else {
    TYPE_DROP
};
pub(super) const REASON_OR_CLASSIFIER: usize = if REASON > CLASSIFIER {
    REASON
} else {
    CLASSIFIER
};
pub(super) const REASON_OR_TYPE_DROP: usize = if REASON > TYPE_DROP {
    REASON
} else {
    TYPE_DROP
};
pub(super) const REASON_OR_PHYSICAL: usize = if REASON > PHYSICAL { REASON } else { PHYSICAL };
pub(super) fn controls<T, const PAYLOAD: usize>() -> usize {
    size_of::<Controls<'static, T>>() + PAYLOAD
}
// Initial payment happens before any RefCell or Builder exists. It has
// its own exact primitive/constructor inventory, not the per-method borrow bank.
type Initial<'a> = (
    Result<CleanupInventory, InventoryFailure>,
    Result<CleanupInventory, InventoryFailure>,
    InventoryFailure,
    InventoryFailure,
    Kind,
    Kind,
    Reason,
    Result<(), ReplayPhysicalRefusal>,
    Result<(), ReplayPhysicalRefusal>,
    Result<(), InventoryFailure>,
    Result<(), InventoryFailure>,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
    &'a mut ReplayPhysicalReservation,
    &'a mut ReplayPhysicalReservation,
    &'a mut usize,
    &'a mut usize,
    InventoryBuilder<'a>,
    InventoryBuilder<'a>,
    Ledger<'a>,
    Ledger<'a>,
    RefCell<Ledger<'a>>,
    RefCell<Ledger<'a>>,
    Vec<CleanupStorageSlot>,
    Vec<CleanupStorageSlot>,
    Vec<CleanupFlag>,
    Vec<CleanupFlag>,
    Vec<CleanupStorageId>,
    Vec<CleanupStorageId>,
    Vec<ConditionalVariantEntry>,
    Vec<ConditionalVariantEntry>,
    CleanupInventory,
    CleanupInventory,
    CleanupEntryState,
    CleanupEntryState,
);
pub(super) const INITIAL_CONTROL_INITIALIZERS: usize = 43;
pub(super) fn initial_controls() -> usize {
    size_of::<Initial<'static>>()
}
macro_rules! receiving {
    ($ledger:expr,$value:ty) => {
        receiving!($ledger, $value, 0, 0)
    };
    ($ledger:expr,$value:ty,$payload:expr,$fields:literal) => {{
        const WORK: usize =
            crate::cleanup::inventory_builder::typed::receiving::CONTROL_INITIALIZERS + $fields;
        let bytes =
            crate::cleanup::inventory_builder::typed::receiving::controls::<$value, { $payload }>();
        {
            let mut held = ($ledger).borrow_mut();
            let crate::cleanup::inventory_builder::typed::Ledger {
                physical,
                remaining,
            } = &mut *held;
            match physical.reserve(bytes) {
                Ok(()) => {}
                Err(_) => {
                    return Err(crate::cleanup::inventory_builder::typed::InventoryFailure(
                        crate::cleanup::inventory_builder::typed::Kind::Receiving,
                    ))
                }
            }
            match crate::cleanup::inventory_builder::typed::debit(remaining, WORK) {
                Ok(()) => {}
                Err(failure) => return Err(failure),
            }
            #[cfg(test)]
            crate::cleanup::inventory_builder::typed::note_metadata(WORK);
        }
    }};
}
pub(super) use receiving;
