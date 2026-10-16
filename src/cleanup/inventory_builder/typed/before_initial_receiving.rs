//! Complete original60ad initial layout and acquisition constants.
use super::{InventoryBuilder, InventoryFailure, Kind, Ledger, Reason};
use crate::cleanup::{
    CleanupEntryState, CleanupFlag, CleanupInventory, CleanupStorageId, CleanupStorageSlot,
    ConditionalVariantEntry,
};
use crate::cleanup_plan::{ReplayPhysicalRefusal, ReplayPhysicalReservation};
use std::cell::RefCell;
use std::mem::size_of;

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
// Static layout creates no runtime size-helper result or named bytes local.
// Payment keeps its CONST argument and primitive reserve parameter receivers;
// initial primitive/failure publication is still an unreceived acquisition ABI.
pub(in super::super) const INITIAL_CONTROL_INITIALIZERS: usize = 42;
pub(in super::super) const INITIAL_BYTES: usize = size_of::<Initial<'static>>();
