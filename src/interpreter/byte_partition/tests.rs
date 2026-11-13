use super::{InvocationRuntime, Owner, Registry};
use crate::construction_fusion::{
    domain::owner_core::Runtime,
    domain::Refusal,
    partition::{Fault, Preparation},
};
use std::{alloc::Layout, cell::RefCell, rc::Rc};
#[derive(Default)]
struct Account {
    remaining: usize,
    charged: usize,
    failed: bool,
    events: u32,
    payload: usize,
    releases: usize,
}
#[derive(Clone)]
pub(super) struct Probe(Rc<RefCell<Account>>);
impl Probe {
    pub(super) fn charged(&self) -> usize {
        self.0.borrow().charged
    }
}
impl InvocationRuntime for Probe {
    fn held_account(&self) -> Self {
        self.clone()
    }
}
impl Runtime for Probe {
    type Owner = Box<[u8]>;
    fn reserve_metadata(&mut self, layout: Layout) -> Result<(), Refusal> {
        let mut a = self.0.borrow_mut();
        if a.failed || layout.size() > a.remaining {
            a.failed = true;
            return Err(Refusal::Metadata);
        }
        a.remaining -= layout.size();
        a.charged += layout.size();
        Ok(())
    }
    fn charge_clone(&mut self, extent: usize) -> Result<(), Refusal> {
        {
            let mut a = self.0.borrow_mut();
            if a.failed
                || a.events >= crate::byte_data_capacity::MAX_BYTES_COPY_SITES
                || a.payload.checked_add(extent).is_none_or(|n| {
                    n > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES as usize
                })
            {
                a.failed = true;
                return Err(Refusal::Metadata);
            }
            a.events += 1;
            a.payload += extent;
        }
        self.reserve_metadata(Layout::array::<u8>(extent).map_err(|_| Refusal::Metadata)?)
    }
    fn allocate(&mut self, extent: usize) -> Self::Owner {
        vec![0; extent].into_boxed_slice()
    }
    fn bytes(owner: &Self::Owner) -> &[u8] {
        owner
    }
    fn bytes_mut(owner: &mut Self::Owner) -> &mut [u8] {
        owner
    }
    fn release(&mut self, owner: Self::Owner) {
        drop(owner);
        self.0.borrow_mut().releases += 1;
    }
}
pub(super) fn account(remaining: usize) -> Probe {
    Probe(Rc::new(RefCell::new(Account {
        remaining,
        events: 1,
        payload: 6,
        ..Account::default()
    })))
}
fn read(registry: &mut Registry<Probe>, owner: &Owner) -> Vec<u8> {
    let mut view = registry.borrow(owner).unwrap();
    let result = registry.read(&view, |bytes| bytes.to_vec()).unwrap();
    registry.end_view(&mut view).unwrap();
    result
}
#[test]
fn popped_child_and_internal_return_survive_parent_settlement_with_disjoint_stores() {
    let probe = account(usize::MAX);
    let mut registry = Registry::new(probe.clone()).unwrap();
    let mut returned = {
        let mut parent = registry
            .partition(vec![0, 1, 2, 3, 4, 5].into_boxed_slice(), 3, 2)
            .unwrap();
        let mut returned = parent.pop().unwrap();
        registry.store(&mut returned, 0, &[9, 8]).unwrap();
        assert_eq!(read(&mut registry, &parent[0]), [0, 1]);
        assert_eq!(read(&mut registry, &parent[1]), [2, 3]);
        for sibling in &mut parent {
            registry.settle(sibling).unwrap();
        }
        returned
    };
    assert_eq!(probe.0.borrow().releases, 0);
    registry.moved(&mut returned).unwrap();
    registry.store(&mut returned, 0, &[9, 8]).unwrap();
    assert_eq!(read(&mut registry, &returned), [9, 8]);
    let cloned = registry.deep_clone(&returned).unwrap();
    assert_eq!(cloned.as_ref(), &[9, 8]);
    registry.store(&mut returned, 0, &[7]).unwrap();
    assert_eq!(cloned.as_ref(), &[9, 8]);
    assert_eq!(probe.0.borrow().events, 2);
    assert_eq!(probe.0.borrow().payload, 8);
    let public = registry.copy_out_and_settle(&mut returned).unwrap();
    assert_eq!(public.as_ref(), &[7, 8]);
    assert_eq!(probe.0.borrow().events, 3);
    assert_eq!(probe.0.borrow().payload, 10);
    assert_eq!(probe.0.borrow().releases, 1);
    drop(registry);
    assert_eq!(public.as_ref(), &[7, 8]);
    assert_eq!(probe.0.borrow().releases, 1);
    let probe = account(usize::MAX);
    probe.0.borrow_mut().payload = 12288;
    let mut registry = Registry::new(probe.clone()).unwrap();
    let mut children = registry
        .partition(vec![0; 12288].into_boxed_slice(), 4096, 3)
        .unwrap();
    assert_eq!(children.len(), 4096);
    assert_eq!(probe.0.borrow().events, 1);
    assert_eq!(probe.0.borrow().payload, 12288);
    for child in &mut children {
        registry.settle(child).unwrap();
    }
    assert_eq!(probe.0.borrow().releases, 1);
}
#[test]
fn real_table_allocations_exact_one_short_overlap_and_abandonment_are_monotonic() {
    let probe = account(usize::MAX);
    let mut registry = Registry::new(probe.clone()).unwrap();
    let original = vec![0; 6].into_boxed_slice();
    let ((), observed) = crate::test_allocation_observer::measure(|| {
        let mut owners = registry.partition(original, 3, 2).unwrap();
        for owner in &mut owners {
            registry.settle(owner).unwrap();
        }
        drop(owners);
        drop(registry);
    });
    let exact = probe.0.borrow().charged;
    assert!(!observed.invalid);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.allocations, 8);
    assert!(observed.allocated <= exact && observed.highwater <= exact);
    assert_eq!(observed.live, 0);
    let exact_probe = account(exact);
    let mut exact_registry = Registry::new(exact_probe.clone()).unwrap();
    let mut owners = exact_registry
        .partition(vec![0; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    assert_eq!(exact_probe.0.borrow().remaining, 0);
    for owner in &mut owners {
        exact_registry.settle(owner).unwrap();
    }
    drop(exact_registry);
    assert_eq!(exact_probe.0.borrow().charged, exact);
    let short_probe = account(exact - 1);
    let mut short_registry = Registry::new(short_probe.clone()).unwrap();
    let original = vec![0; 6].into_boxed_slice();
    let pointer = original.as_ptr();
    let (result, observed) =
        crate::test_allocation_observer::measure(|| short_registry.partition(original, 3, 2));
    let (error, returned) = result.err().unwrap();
    assert_eq!(error, Preparation::Authority(Refusal::Metadata));
    assert_eq!(returned.as_ptr(), pointer);
    assert_eq!(
        observed,
        crate::test_allocation_observer::Metrics::default()
    );
    let spent = short_probe.0.borrow().charged;
    assert!(short_registry.partition(returned, 3, 2).is_err());
    assert_eq!(short_probe.0.borrow().charged, spent);
    // Two separately paid cores exercise full replacement while old registry
    // backing remains live; contexts are never reused or refunded.
    let probe = account(usize::MAX);
    let mut registry = Registry::new(probe.clone()).unwrap();
    let first = registry
        .partition(vec![0; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    let prior = probe.0.borrow().charged;
    let second = registry
        .partition(vec![0; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    assert!(probe.0.borrow().charged > prior);
    drop(first);
    drop(second);
    drop(registry);
    assert_eq!(probe.0.borrow().releases, 2); // administrative abandonment only
}
#[test]
fn child_table_remains_charged_until_last_child_settles() {
    let probe = account(usize::MAX);
    let mut registry = Registry::new(probe.clone()).unwrap();
    let mut children = registry
        .partition(vec![0; 8192].into_boxed_slice(), 8192, 1)
        .unwrap();
    let mut last = children.pop().unwrap();
    for child in &mut children {
        registry.settle(child).unwrap();
    }
    drop(children);
    let next = vec![7].into_boxed_slice();
    let next_pointer = next.as_ptr();
    let charged = probe.charged();
    let (error, next) = registry.partition(next, 1, 1).unwrap_err();
    assert_eq!(error, Preparation::Fault(Fault::MetadataCapacity));
    assert_eq!(next.as_ptr(), next_pointer);
    assert_eq!(probe.charged(), charged);
    assert_eq!(probe.0.borrow().releases, 0);

    let stale = Owner {
        invocation: last.invocation,
        context: last.context,
        child: last.child,
        receipt: last.receipt,
    };
    registry.settle(&mut last).unwrap();
    assert_eq!(registry.child_table_slots, 0);
    assert!(registry.contexts[stale.context].is_none());
    assert_eq!(probe.0.borrow().releases, 1);
    assert_eq!(probe.charged(), charged);
    let mut next = registry.partition(next, 1, 1).unwrap();
    assert_eq!(read(&mut registry, &next[0]), [7]);
    assert_eq!(registry.extent(&stale), Err(Refusal::Identity));
    registry.settle(&mut next[0]).unwrap();
    assert_eq!(probe.0.borrow().releases, 2);
    assert_eq!(registry.child_table_slots, 0);
}

#[test]
fn foreign_stale_live_loan_and_clone_exhaustion_refuse_without_publication() {
    let probe = account(usize::MAX);
    let mut left = Registry::new(probe.clone()).unwrap();
    let mut owners = left.partition(vec![0; 6].into_boxed_slice(), 3, 2).unwrap();
    let mut right = Registry::new(probe.clone()).unwrap();
    let foreign = right
        .partition(vec![0; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    assert!(left.borrow(&foreign[0]).is_err());
    assert!(left.borrow(&owners[0]).is_err());
    for owner in &mut owners {
        left.settle(owner).unwrap();
    }
    let mut registry = Registry::new(probe.clone()).unwrap();
    let mut owners = registry
        .partition(vec![0; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    let mut view = registry.borrow(&owners[0]).unwrap();
    assert_eq!(
        registry.store(&mut owners[0], 0, &[1]),
        Err(Refusal::Borrowed)
    );
    registry.end_view(&mut view).unwrap();
    assert!(registry.read(&view, |_| ()).is_err());
    for owner in &mut owners {
        registry.settle(owner).unwrap();
    }
    let probe = account(usize::MAX);
    probe.0.borrow_mut().events = 32;
    let mut registry = Registry::new(probe.clone()).unwrap();
    let mut owners = registry
        .partition(vec![0; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    assert!(registry.deep_clone(&owners[0]).is_err());
    assert_eq!(probe.0.borrow().events, 32);
    assert!(registry.borrow(&owners[0]).is_err());
    for owner in &mut owners {
        registry.settle(owner).unwrap();
    }
    let probe = account(usize::MAX);
    probe.0.borrow_mut().events = 31;
    let mut registry = Registry::new(probe.clone()).unwrap();
    let mut owners = registry
        .partition(vec![0; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    drop(registry.deep_clone(&owners[0]).unwrap());
    assert_eq!(probe.0.borrow().events, 32);
    assert!(registry.deep_clone(&owners[0]).is_err());
    for owner in &mut owners {
        registry.settle(owner).unwrap();
    }
    let probe = account(usize::MAX);
    probe.0.borrow_mut().payload = 2097150;
    let mut registry = Registry::new(probe.clone()).unwrap();
    let mut owners = registry
        .partition(vec![0; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    drop(registry.deep_clone(&owners[0]).unwrap());
    assert_eq!(probe.0.borrow().payload, 2097152);
    assert!(registry.deep_clone(&owners[0]).is_err());
    for owner in &mut owners {
        registry.settle(owner).unwrap();
    }
    let probe = account(usize::MAX);
    let mut registry = Registry::new(probe.clone()).unwrap();
    let mut owners = registry
        .partition(vec![0; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    let stale = Owner {
        invocation: owners[0].invocation,
        context: owners[0].context,
        child: owners[0].child,
        receipt: owners[0].receipt,
    };
    registry.moved(&mut owners[0]).unwrap();
    assert!(registry.borrow(&stale).is_err());
    assert!(registry.borrow(&owners[0]).is_err());
    for owner in &mut owners {
        registry.settle(owner).unwrap();
    }
    let probe = account(usize::MAX);
    let mut registry = Registry::new(probe.clone()).unwrap();
    let mut owners = registry
        .partition(vec![0; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    let mut view = registry.borrow(&owners[0]).unwrap();
    assert_eq!(
        registry.read_range(&view, 1, 2, |bytes| bytes[0]).unwrap(),
        0
    );
    assert!(registry.read_range(&view, 0, 3, |_| ()).is_err());
    assert!(registry.read(&view, |_| ()).is_err());
    registry.end_view(&mut view).unwrap();
    for owner in &mut owners {
        registry.settle(owner).unwrap();
    }
    let probe = account(usize::MAX);
    let mut registry = Registry::new(probe.clone()).unwrap();
    let oversized = vec![0; 131073].into_boxed_slice();
    let pointer = oversized.as_ptr();
    let (result, observed) =
        crate::test_allocation_observer::measure(|| registry.partition(oversized, 1, 131073));
    let (error, returned) = result.err().unwrap();
    assert_eq!(error, Preparation::Authority(Refusal::Range));
    assert_eq!(returned.as_ptr(), pointer);
    assert_eq!(
        observed,
        crate::test_allocation_observer::Metrics::default()
    );
    assert_eq!(probe.0.borrow().charged, 0);
    let otherwise_valid = vec![0; 6].into_boxed_slice();
    let (result, observed) =
        crate::test_allocation_observer::measure(|| registry.partition(otherwise_valid, 3, 2));
    assert_eq!(
        result.err().unwrap().0,
        Preparation::Authority(Refusal::Range)
    );
    assert_eq!(
        observed,
        crate::test_allocation_observer::Metrics::default()
    );
    assert_eq!(probe.0.borrow().charged, 0);
    // A loaned child refuses copy-out before allocation. Cleanup retains
    // authority and the caller's ordinary allocation counters stay intact.
    let probe = account(usize::MAX);
    let mut registry = Registry::new(probe.clone()).unwrap();
    let mut owners = registry
        .partition(vec![7; 6].into_boxed_slice(), 3, 2)
        .unwrap();
    let mut view = registry.borrow(&owners[0]).unwrap();
    let before = probe.charged();
    let (outcome, observed) =
        crate::test_allocation_observer::measure(|| registry.copy_out_and_settle(&mut owners[0]));
    assert_eq!(outcome.unwrap_err(), Refusal::Borrowed);
    assert_eq!(
        observed,
        crate::test_allocation_observer::Metrics::default()
    );
    assert_eq!(probe.charged(), before);
    assert_eq!(probe.0.borrow().events, 1);
    assert_eq!(probe.0.borrow().payload, 6);
    assert_eq!(probe.0.borrow().releases, 0);
    assert!(registry.deep_clone(&owners[1]).is_err());
    registry.end_view(&mut view).unwrap();
    for owner in &mut owners {
        registry.settle(owner).unwrap();
    }
    assert_eq!(probe.0.borrow().releases, 1);
    let probe = account(usize::MAX);
    probe.0.borrow_mut().payload = 0;
    let mut registry = Registry::new(probe.clone()).unwrap();
    assert!(registry
        .partition(Vec::<u8>::new().into_boxed_slice(), 0, u64::MAX)
        .unwrap()
        .is_empty());
}
