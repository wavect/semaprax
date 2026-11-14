//! Pending ordinary account adapter. It borrows the actual invocation counters;
//! it neither copies them nor creates a second logical payload allowance.
//! A caller supplies real finite physical funding. The layout-derived upper is
//! a forecast, not proof that a host has funded it or source admission authority.
use super::{
    authority::LoanCapacity, loans::Loans, ContextSlot, InvocationRuntime, Owner, Registry, State,
};
use crate::construction_fusion::domain::{
    owner_core::{Core, Runtime},
    Issuer, ModuleIdentity, Refusal,
};
use std::{
    alloc::Layout,
    cell::{Cell, RefCell},
};

mod borrowed;
pub(crate) use borrowed::{AllocationError, BorrowedAccount};

struct Logical<'a> {
    events: &'a mut u32,
    payload: &'a mut u64,
}
static NEXT_ACCOUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
pub(crate) struct Account<'a> {
    identity: u64,
    logical: RefCell<Logical<'a>>,
    remaining: Cell<usize>,
    charged: Cell<usize>,
    failed: Cell<bool>,
    logical_refused: Cell<bool>,
}
#[derive(Clone, Copy)]
pub(crate) struct Handle<'a, 'c>(&'a Account<'c>);
impl<'c> Account<'c> {
    /// This constructor is private receiving groundwork, not a production
    /// installer. `funded_bytes` must come from the actual host account.
    pub(crate) fn funded(
        events: &'c mut u32,
        payload: &'c mut u64,
        funded_bytes: usize,
    ) -> Result<Self, Refusal> {
        if *events > crate::byte_data_capacity::MAX_BYTES_COPY_SITES
            || *payload > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
        {
            return Err(Refusal::Identity);
        }
        let headers = Layout::new::<Self>()
            .size()
            .checked_add(Layout::new::<Handle<'_, '_>>().size())
            .and_then(|bytes| bytes.checked_add(Layout::new::<BorrowedAccount<'_>>().size()))
            .ok_or(Refusal::Metadata)?;
        let remaining = funded_bytes.checked_sub(headers).ok_or(Refusal::Metadata)?;
        let identity = NEXT_ACCOUNT
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |n| n.checked_add(1),
            )
            .map_err(|_| Refusal::Exhausted)?;
        Ok(Self {
            identity,
            logical: RefCell::new(Logical { events, payload }),
            remaining: Cell::new(remaining),
            charged: Cell::new(headers),
            failed: Cell::new(false),
            logical_refused: Cell::new(false),
        })
    }
    pub(in crate::interpreter) fn charged(&self) -> usize {
        self.charged.get()
    }
    pub(crate) fn handle(&self) -> Handle<'_, 'c> {
        Handle(self)
    }
    fn reserve(&self, layout: Layout) -> Result<(), Refusal> {
        if self.failed.get() || layout.size() > self.remaining.get() {
            self.failed.set(true);
            return Err(Refusal::Metadata);
        }
        self.remaining.set(self.remaining.get() - layout.size());
        self.charged.set(
            self.charged
                .get()
                .checked_add(layout.size())
                .ok_or(Refusal::Metadata)?,
        );
        Ok(())
    }
}
impl InvocationRuntime for Handle<'_, '_> {
    fn held_account(&self) -> Self {
        *self
    }
}
impl Runtime for Handle<'_, '_> {
    type Owner = Box<[u8]>;
    fn reserve_metadata(&mut self, layout: Layout) -> Result<(), Refusal> {
        self.0.reserve(layout)
    }
    fn charge_clone(&mut self, extent: usize) -> Result<(), Refusal> {
        if self.0.failed.get() {
            return Err(Refusal::Metadata);
        }
        if extent > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES as usize {
            self.0.failed.set(true);
            return Err(Refusal::Range);
        }
        let mut logical = self.0.logical.borrow_mut();
        let next = (*logical.payload).checked_add(extent as u64);
        if *logical.events >= crate::byte_data_capacity::MAX_BYTES_COPY_SITES
            || next
                .is_none_or(|value| value > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES)
        {
            self.0.failed.set(true);
            return Err(Refusal::Metadata);
        }
        self.0
            .reserve(Layout::array::<u8>(extent).map_err(|_| Refusal::Metadata)?)?;
        *logical.events += 1;
        *logical.payload = next.unwrap();
        Ok(())
    }
    fn allocate(&mut self, extent: usize) -> Self::Owner {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(extent)
            .expect("ordinary partition clone allocation invariant failure");
        assert_eq!(bytes.capacity(), extent);
        bytes.resize(extent, 0);
        bytes.into_boxed_slice()
    }
    fn bytes(owner: &Self::Owner) -> &[u8] {
        owner
    }
    fn bytes_mut(owner: &mut Self::Owner) -> &mut [u8] {
        owner
    }
    fn release(&mut self, owner: Self::Owner) {
        drop(owner);
    }
}
/// Maximum requests for one joined invocation: at most one consuming partition
/// per ordinary backing event, unchanged child/loan bounds, and full geometric
/// registry replacement overlap. One flat container and its single consuming
/// iterator transition are included per partition; the installed callgraph must
/// prove this bound, not reuse these charges for additional container builds.
/// No fabricated byte ceiling is introduced.
/// The eventual factory must prove whole-backing identity/one-time consumption;
/// this computation alone cannot grant that authority.
pub(crate) fn upper(capacity: &LoanCapacity<'_>) -> Result<usize, Refusal> {
    let count = crate::vec_ops::MAX_CAPACITY as usize;
    let contexts = crate::byte_data_capacity::MAX_BYTES_COPY_SITES as usize;
    let mut total = 0usize;
    let mut add = |bytes: usize| {
        total = total.checked_add(bytes).ok_or(Refusal::Metadata)?;
        Ok::<(), Refusal>(())
    };
    add(Layout::new::<Account<'_>>().size())?;
    add(Layout::new::<Handle<'_, '_>>().size())?;
    add(Layout::new::<BorrowedAccount<'_>>().size())?;
    add(Layout::new::<Registry<Handle<'_, '_>>>().size())?;
    for layout in Loans::<Handle<'_, '_>>::metadata_layouts(capacity)? {
        add(layout.size())?;
    }
    let mut replacement = 1;
    while replacement <= contexts {
        add(Layout::array::<ContextSlot<Handle<'_, '_>>>(replacement)
            .map_err(|_| Refusal::Metadata)?
            .size())?;
        replacement = replacement.checked_mul(2).ok_or(Refusal::Metadata)?;
    }
    for _ in 0..contexts {
        for layout in Core::<Handle<'_, '_>>::ordinary_metadata_layouts(count, capacity)? {
            add(layout.size())?;
        }
        add(Layout::new::<Issuer>().size())?;
        add(Layout::new::<ModuleIdentity>().size())?;
        add(Layout::new::<State<Handle<'_, '_>>>().size())?;
        add(Layout::array::<Owner>(count)
            .map_err(|_| Refusal::Metadata)?
            .size())?;
        add(Layout::new::<Vec<Owner>>().size())?;
        add(Layout::new::<super::flat::PartitionVec>().size())?;
        add(Layout::new::<super::flat::PartitionIterator>().size())?;
    }
    add(crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES as usize)?;
    Ok(total)
}
