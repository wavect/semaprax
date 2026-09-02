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
#[path = "native_module_lease/tests.rs"]
mod tests;
