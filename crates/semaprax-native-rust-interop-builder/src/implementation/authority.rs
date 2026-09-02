//! Held object and manifest authority. Bytes are dropped before the
//! authority that admitted them, in a fixed observed order.

use super::*;

struct ObjectAuthority {
    budget: TemporaryBudget,
}

impl ObjectAuthority {
    fn new(mut budget: TemporaryBudget, object_capacity: usize) -> Result<Self, PhaseBLocalError> {
        shrink_phase_b(&mut budget, object_capacity)?;
        if budget.maximum() != object_capacity {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        #[cfg(test)]
        {
            if PHASE_B_OBJECT_AUTHORITY_LIVE.with(|live| live.replace(true)) {
                return Err(PhaseBLocalError::BuilderBudget);
            }
            // The detailed order trace is per authorized object.  Aggregate
            // transfer/drop counters intentionally remain cumulative so tests
            // can also prove that repeated builder invocations release once.
            PHASE_B_OBJECT_DROP_ORDER.with(|order| order.set([0, 0]));
            PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(|length| length.set(0));
            PHASE_B_OBJECT_AUTHORITY_TRANSFERS
                .with(|count| count.set(count.get().saturating_add(1)));
        }
        Ok(Self { budget })
    }

    fn check(&self, object: &[u8], object_capacity: usize) -> Result<(), PhaseBLocalError> {
        if self.budget.maximum() != object_capacity || object.len() > object_capacity {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        Ok(())
    }
}

impl Drop for ObjectAuthority {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            let index = PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
            assert_eq!(index, 1, "object bytes must drop before their authority");
            PHASE_B_OBJECT_DROP_ORDER.with(|order| {
                let mut values = order.get();
                assert_eq!(values[0], 1);
                values[index] = 2;
                order.set(values);
            });
            PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(|length| length.set(index + 1));
            assert!(PHASE_B_OBJECT_AUTHORITY_LIVE.with(|live| live.replace(false)));
            PHASE_B_OBJECT_AUTHORITY_DROPS.with(|count| count.set(count.get().saturating_add(1)));
        }
    }
}

struct ObjectDropGuard;

impl Drop for ObjectDropGuard {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            assert!(PHASE_B_OBJECT_AUTHORITY_LIVE.with(std::cell::Cell::get));
            let index = PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
            assert_eq!(index, 0);
            PHASE_B_OBJECT_DROP_ORDER.with(|order| {
                let mut values = order.get();
                values[index] = 1;
                order.set(values);
            });
            PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(|length| length.set(index + 1));
            PHASE_B_OBJECT_BYTES_DROPS.with(|count| count.set(count.get().saturating_add(1)));
        }
    }
}

struct ObjectBytes {
    bytes: Vec<u8>,
    drop_guard: ObjectDropGuard,
}

pub(super) struct AuthorizedObject {
    object: ObjectBytes,
    authority: ObjectAuthority,
}

impl AuthorizedObject {
    pub(super) fn new(bytes: Vec<u8>, budget: TemporaryBudget) -> Result<Self, PhaseBLocalError> {
        let authority = ObjectAuthority::new(budget, bytes.capacity())?;
        Ok(Self {
            object: ObjectBytes {
                bytes,
                drop_guard: ObjectDropGuard,
            },
            authority,
        })
    }

    pub(super) fn as_slice(&self) -> &[u8] {
        &self.object.bytes
    }

    pub(super) fn check(&self) -> Result<(), PhaseBLocalError> {
        let _ = &self.object.drop_guard;
        self.authority
            .check(self.as_slice(), self.object.bytes.capacity())
    }
}

struct ManifestAuthority {
    budget: TemporaryBudget,
}

impl ManifestAuthority {
    fn check(&self, manifest: &String) -> Result<(), PhaseBLocalError> {
        if self.budget.maximum() != manifest.capacity()
            || manifest.len() > self.budget.maximum()
            || manifest.capacity() != MAX_MANIFEST_BYTES
        {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        Ok(())
    }
}

impl Drop for ManifestAuthority {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            let index = PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
            assert_eq!(index, 1, "manifest bytes must drop before their authority");
            PHASE_B_MANIFEST_DROP_ORDER.with(|order| {
                let mut values = order.get();
                assert_eq!(values[0], 1);
                values[index] = 2;
                order.set(values);
            });
            PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(|length| length.set(index + 1));
            assert!(PHASE_B_MANIFEST_AUTHORITY_LIVE.with(|live| live.replace(false)));
            PHASE_B_MANIFEST_AUTHORITY_DROPS.with(|count| count.set(count.get().saturating_add(1)));
        }
    }
}

struct ManifestDropGuard;

impl Drop for ManifestDropGuard {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            assert!(PHASE_B_MANIFEST_AUTHORITY_LIVE.with(std::cell::Cell::get));
            let index = PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
            assert_eq!(index, 0);
            PHASE_B_MANIFEST_DROP_ORDER.with(|order| {
                let mut values = order.get();
                values[index] = 1;
                order.set(values);
            });
            PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(|length| length.set(index + 1));
            PHASE_B_MANIFEST_BYTES_DROPS.with(|count| count.set(count.get().saturating_add(1)));
        }
    }
}

pub(super) struct ManifestBytes {
    pub(super) bytes: String,
    drop_guard: ManifestDropGuard,
}

pub(super) struct AuthorizedManifest {
    pub(super) manifest: ManifestBytes,
    authority: ManifestAuthority,
}

impl AuthorizedManifest {
    pub(super) fn new(bytes: String, budget: TemporaryBudget) -> Result<Self, PhaseBLocalError> {
        if bytes.capacity() != budget.maximum() || bytes.capacity() != MAX_MANIFEST_BYTES {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        #[cfg(test)]
        {
            if PHASE_B_MANIFEST_AUTHORITY_LIVE.with(|live| live.replace(true)) {
                return Err(PhaseBLocalError::BuilderBudget);
            }
            PHASE_B_MANIFEST_DROP_ORDER.with(|order| order.set([0, 0]));
            PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(|length| length.set(0));
            PHASE_B_MANIFEST_AUTHORITY_TRANSFERS
                .with(|count| count.set(count.get().saturating_add(1)));
        }
        Ok(Self {
            manifest: ManifestBytes {
                bytes,
                drop_guard: ManifestDropGuard,
            },
            authority: ManifestAuthority { budget },
        })
    }

    pub(super) fn as_str(&self) -> &str {
        &self.manifest.bytes
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        self.as_str().as_bytes()
    }

    pub(super) fn check(&self) -> Result<(), PhaseBLocalError> {
        let _ = &self.manifest.drop_guard;
        self.authority.check(&self.manifest.bytes)
    }
}

pub(super) struct BuildStageFacts {
    pub(super) object_name: &'static str,
    pub(super) object: AuthorizedObject,
    pub(super) manifest: AuthorizedManifest,
    pub(super) inventory_exact: (platform::PreparedInventoryExact<7>, TemporaryBudget),
}

impl BuildStageFacts {
    pub(super) fn observe_object_authority_for_manifest(&self) -> Result<(), PhaseBLocalError> {
        self.object.check()?;
        self.manifest.check()?;
        #[cfg(test)]
        {
            if !PHASE_B_OBJECT_AUTHORITY_LIVE.with(std::cell::Cell::get) {
                return Err(PhaseBLocalError::BuilderBudget);
            }
            PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS
                .with(|count| count.set(count.get().saturating_add(1)));
        }
        Ok(())
    }

    pub(super) fn observe_object_authority_for_publish(&self) -> Result<(), PhaseBLocalError> {
        self.object.check()?;
        self.manifest.check()?;
        #[cfg(test)]
        {
            if !PHASE_B_OBJECT_AUTHORITY_LIVE.with(std::cell::Cell::get) {
                return Err(PhaseBLocalError::BuilderBudget);
            }
            PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS
                .with(|count| count.set(count.get().saturating_add(1)));
        }
        Ok(())
    }
}
