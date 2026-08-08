//! Private retained-module lifetime staging for a future native adapter.
//!
//! The production crate deliberately has no constructor for this type yet.
//! A real constructor must own and validate a platform loader reference; a
//! descriptor or fingerprint alone is not a retained module. Unit tests use a
//! fake retained pin to prove the Rust lifetime and state-machine invariants
//! without making an operating-system unload claim.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "native module loading and callable adapters remain gated"
    )
)]

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

const STATE_OPEN: u8 = 1;
const STATE_DRAINING: u8 = 2;

/// Opaque process incarnation observed by the future loader boundary.
///
/// A PID alone is insufficient because it can be reused. Production creation
/// remains part of the loader/fork integration; tests inject both components.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct NativeProcessIncarnation {
    process_id: u32,
    incarnation: u64,
}

impl NativeProcessIncarnation {
    #[cfg(test)]
    pub(super) fn current_for_test(incarnation: u64) -> Self {
        Self {
            process_id: std::process::id(),
            incarnation,
        }
    }
}

/// One retained reference to an exact loaded-module instance.
///
/// This type intentionally implements neither `Clone` nor formatting traits.
/// Internal code must call [`Self::retain`] so every lifetime extension is an
/// explicit, state-checked operation. Exact identity is the `Arc` allocation,
/// not a path or descriptor fingerprint.
pub(super) struct NativeModuleLease {
    inner: Arc<NativeModulePinInner>,
}

/// Leaf state only: no authority, registry, owner, outcome, callback, or
/// finalizer may be referenced from this allocation.
struct NativeModulePinInner {
    physical_module_fingerprint: [u8; 32],
    origin: NativeProcessIncarnation,
    state: AtomicU8,
    #[cfg(test)]
    release_probe: Arc<FakeRetainedPinProbe>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeModuleLeaseError {
    InvalidIdentity,
    WrongModuleInstance,
    WrongProcessIncarnation,
    Draining,
}

impl NativeModuleLease {
    /// Retain from the currently executing process.
    ///
    /// The opaque incarnation remains fixed for the life of this in-memory
    /// instance; a POSIX fork changes the process ID and therefore fails before
    /// state inspection. PID reuse cannot occur while this allocation remains
    /// alive in the same process.
    pub(super) fn retain_current_process(&self) -> Result<Self, NativeModuleLeaseError> {
        self.retain(NativeProcessIncarnation {
            process_id: std::process::id(),
            incarnation: self.inner.origin.incarnation,
        })
    }

    /// Retain this exact module instance if it is still open and the observed
    /// process incarnation matches the loader's origin.
    ///
    /// The second state read linearizes against `begin_draining`: a retain
    /// either existed before draining committed or discards its temporary
    /// strong reference and rejects.
    pub(super) fn retain(
        &self,
        observed: NativeProcessIncarnation,
    ) -> Result<Self, NativeModuleLeaseError> {
        self.retain_with_after_clone(observed, || {})
    }

    fn retain_with_after_clone(
        &self,
        observed: NativeProcessIncarnation,
        after_clone: impl FnOnce(),
    ) -> Result<Self, NativeModuleLeaseError> {
        self.require_origin(observed)?;
        if self.inner.state.load(Ordering::Acquire) != STATE_OPEN {
            return Err(NativeModuleLeaseError::Draining);
        }

        let inner = Arc::clone(&self.inner);
        after_clone();
        if inner.state.load(Ordering::Acquire) != STATE_OPEN {
            drop(inner);
            return Err(NativeModuleLeaseError::Draining);
        }
        Ok(Self { inner })
    }

    /// Stop future retention for this exact instance.
    ///
    /// Existing leases remain valid lifetime pins and release normally. Real
    /// loader integration must separately quiesce calls, callbacks, and
    /// finalizers before releasing its owned platform handle.
    pub(super) fn begin_draining(
        &self,
        observed: NativeProcessIncarnation,
    ) -> Result<(), NativeModuleLeaseError> {
        self.require_origin(observed)?;
        self.inner
            .state
            .compare_exchange(
                STATE_OPEN,
                STATE_DRAINING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|_| ())
            .map_err(|_| NativeModuleLeaseError::Draining)
    }

    pub(super) fn is_same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub(super) fn physical_module_fingerprint(&self) -> &[u8; 32] {
        &self.inner.physical_module_fingerprint
    }

    fn require_origin(
        &self,
        observed: NativeProcessIncarnation,
    ) -> Result<(), NativeModuleLeaseError> {
        if observed == self.inner.origin {
            Ok(())
        } else {
            Err(NativeModuleLeaseError::WrongProcessIncarnation)
        }
    }

    /// Test-only construction stands in for an owned OS loader reference.
    /// There is intentionally no corresponding production constructor.
    #[cfg(test)]
    pub(super) fn fake_retained(
        physical_module_fingerprint: [u8; 32],
        origin: NativeProcessIncarnation,
        release_probe: Arc<FakeRetainedPinProbe>,
    ) -> Result<Self, NativeModuleLeaseError> {
        if physical_module_fingerprint.iter().all(|byte| *byte == 0)
            || origin.process_id == 0
            || origin.incarnation == 0
        {
            return Err(NativeModuleLeaseError::InvalidIdentity);
        }
        Ok(Self {
            inner: Arc::new(NativeModulePinInner {
                physical_module_fingerprint,
                origin,
                state: AtomicU8::new(STATE_OPEN),
                release_probe,
            }),
        })
    }
}

#[cfg(test)]
pub(super) struct FakeRetainedPinProbe {
    releases: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
impl FakeRetainedPinProbe {
    pub(super) fn new() -> Self {
        Self {
            releases: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub(super) fn releases(&self) -> usize {
        self.releases.load(Ordering::Acquire)
    }
}

#[cfg(test)]
impl Drop for NativeModulePinInner {
    fn drop(&mut self) {
        self.release_probe.releases.fetch_add(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
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
            NativeModuleLease::fake_retained(FINGERPRINT, zero_process, Arc::clone(&probe))
                .map(|_| ()),
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
}
