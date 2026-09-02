use std::sync::{Barrier, Weak};
use std::thread;

use super::*;

macro_rules! assert_not_impl {
    ($type:ty, $trait:path) => {{
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<T: ?Sized> AmbiguousIfImplemented<()> for T {}
        struct Implemented;
        impl<T: ?Sized + $trait> AmbiguousIfImplemented<Implemented> for T {}
        let _ = <$type as AmbiguousIfImplemented<_>>::probe;
    }};
}

const FINGERPRINT: [u8; 32] = [0xa5; 32];
const ORIGIN: NativeProcessIncarnation = NativeProcessIncarnation {
    process_id: 17,
    incarnation: 23,
};

fn fixture() -> (NativeModuleLease, Arc<FakeRetainedPinProbe>) {
    let probe = Arc::new(FakeRetainedPinProbe::new());
    let lease = NativeModuleLease::fake_retained(FINGERPRINT, ORIGIN, Arc::clone(&probe))
        .expect("fixture identity is valid");
    (lease, probe)
}

#[test]
fn identical_fingerprints_do_not_conflate_loaded_instances() {
    let (first, first_probe) = fixture();
    let (second, second_probe) = fixture();
    let retained_first = first.retain(ORIGIN).unwrap();

    assert_eq!(first.physical_module_fingerprint(), &FINGERPRINT);
    assert_eq!(second.physical_module_fingerprint(), &FINGERPRINT);
    assert!(first.is_same_instance(&retained_first));
    assert!(!first.is_same_instance(&second));

    drop(first);
    assert_eq!(first_probe.releases(), 0);
    drop(retained_first);
    drop(second);
    assert_eq!(first_probe.releases(), 1);
    assert_eq!(second_probe.releases(), 1);
}

#[test]
fn draining_rejects_new_retention_without_revoking_existing_leases() {
    let (lease, probe) = fixture();
    let retained = lease.retain(ORIGIN).unwrap();

    assert_eq!(lease.begin_draining(ORIGIN), Ok(()));
    assert!(lease.is_same_instance(&retained));
    assert_eq!(
        retained.retain(ORIGIN).map(|_| ()),
        Err(NativeModuleLeaseError::Draining)
    );
    assert_eq!(
        lease.begin_draining(ORIGIN),
        Err(NativeModuleLeaseError::Draining)
    );

    drop(lease);
    assert_eq!(probe.releases(), 0);
    drop(retained);
    assert_eq!(probe.releases(), 1);
}

#[test]
fn drain_committing_after_temporary_clone_forces_retain_to_reject() {
    let (lease, probe) = fixture();
    let worker_lease = lease.retain(ORIGIN).unwrap();
    let cloned = Arc::new(Barrier::new(2));
    let drain_committed = Arc::new(Barrier::new(2));
    let worker = {
        let cloned = Arc::clone(&cloned);
        let drain_committed = Arc::clone(&drain_committed);
        thread::spawn(move || {
            worker_lease
                .retain_with_after_clone(ORIGIN, || {
                    cloned.wait();
                    drain_committed.wait();
                })
                .map(|_| ())
        })
    };

    cloned.wait();
    lease.begin_draining(ORIGIN).unwrap();
    drain_committed.wait();
    assert_eq!(
        worker.join().unwrap(),
        Err(NativeModuleLeaseError::Draining)
    );
    assert_eq!(probe.releases(), 0);
    drop(lease);
    assert_eq!(probe.releases(), 1);
}

#[test]
fn wrong_process_incarnation_precedes_state_and_cannot_start_drain() {
    let (lease, probe) = fixture();
    let wrong_process = NativeProcessIncarnation {
        process_id: ORIGIN.process_id + 1,
        incarnation: ORIGIN.incarnation,
    };
    let wrong_incarnation = NativeProcessIncarnation {
        process_id: ORIGIN.process_id,
        incarnation: ORIGIN.incarnation + 1,
    };

    assert_eq!(
        lease.retain(wrong_process).map(|_| ()),
        Err(NativeModuleLeaseError::WrongProcessIncarnation)
    );
    assert_eq!(
        lease.begin_draining(wrong_incarnation),
        Err(NativeModuleLeaseError::WrongProcessIncarnation)
    );
    let retained = lease
        .retain(ORIGIN)
        .expect("wrong-incarnation attempts must leave the instance open");

    drop(lease);
    drop(retained);
    assert_eq!(probe.releases(), 1);
}

#[test]
fn concurrent_last_releases_trigger_the_fake_pin_exactly_once() {
    const THREADS: usize = 16;
    let (lease, probe) = fixture();
    let barrier = Arc::new(Barrier::new(THREADS + 1));
    let mut workers = Vec::with_capacity(THREADS);

    for _ in 0..THREADS {
        let retained = lease.retain(ORIGIN).unwrap();
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            drop(retained);
        }));
    }
    drop(lease);
    assert_eq!(probe.releases(), 0);
    barrier.wait();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(probe.releases(), 1);
}

#[test]
fn leaf_pin_has_no_retention_backedge() {
    let (lease, probe) = fixture();
    let weak: Weak<NativeModulePinInner> = Arc::downgrade(&lease.inner);

    assert!(weak.upgrade().is_some());
    drop(lease);
    assert!(weak.upgrade().is_none());
    assert_eq!(probe.releases(), 1);
}

#[test]
fn fake_construction_rejects_uninitialized_identity_without_releasing() {
    let probe = Arc::new(FakeRetainedPinProbe::new());
    assert_eq!(
        NativeModuleLease::fake_retained([0; 32], ORIGIN, Arc::clone(&probe)).map(|_| ()),
        Err(NativeModuleLeaseError::InvalidIdentity)
    );
    let zero_process = NativeProcessIncarnation {
        process_id: 0,
        incarnation: ORIGIN.incarnation,
    };
    assert_eq!(
        NativeModuleLease::fake_retained(FINGERPRINT, zero_process, Arc::clone(&probe)).map(|_| ()),
        Err(NativeModuleLeaseError::InvalidIdentity)
    );
    let zero_incarnation = NativeProcessIncarnation {
        process_id: ORIGIN.process_id,
        incarnation: 0,
    };
    assert_eq!(
        NativeModuleLease::fake_retained(FINGERPRINT, zero_incarnation, Arc::clone(&probe))
            .map(|_| ()),
        Err(NativeModuleLeaseError::InvalidIdentity)
    );
    assert_eq!(probe.releases(), 0);
}

#[test]
fn lease_traits_are_deliberate() {
    fn assert_send_and_sync<T: Send + Sync>() {}
    assert_send_and_sync::<NativeModuleLease>();
    assert_not_impl!(NativeModuleLease, Clone);
    assert_not_impl!(NativeModuleLease, std::fmt::Debug);
    assert_not_impl!(NativeModuleLease, std::fmt::Display);
    assert_not_impl!(NativeModuleLease, Default);
}
