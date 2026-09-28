//! Actual fresh local Token acquisition. No endpoint attachment is implemented.
//!
//! The private certificate records this closed producer's real cell creation;
//! it is not a checked source factory plan or permission to create an endpoint.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::{
    HostBoundaryRejection, HostOwnerState, HostOwnerToken, HostOwnershipRegistry,
    HostResourceProvenance,
};

pub(crate) const FRESH_TOKEN_PRODUCER: &str = "session.token.acquire.initial.v1";
pub(crate) const TOKEN_RESOURCE: &str = "token.type";
pub(crate) const TOKEN_LIFECYCLE: &str = "token.drop";
const MAX_FRESH_TOKEN_CELLS: usize = 256;
static NEXT_ACQUISITION: AtomicU64 = AtomicU64::new(1);

/// One actual local backing allocation. Only the owning registry retains it.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct FreshTokenCell {
    acquisition: u64,
}

/// Move-only certificate for an actual registered fresh cell, not endpoint authority.
#[must_use = "retain the registry and retire or roll back the actual cell"]
#[derive(Debug)]
pub(crate) struct FreshTokenAcquisition {
    token: HostOwnerToken,
    acquisition: u64,
    provenance: HostResourceProvenance,
}

impl HostOwnershipRegistry {
    pub(crate) fn acquire_fresh_token(
        &mut self,
        provenance: HostResourceProvenance,
    ) -> Result<FreshTokenAcquisition, HostBoundaryRejection> {
        if provenance.resource_type.as_str() != TOKEN_RESOURCE {
            return Err(HostBoundaryRejection::WrongResourceType);
        }
        if provenance.lifecycle.as_str() != TOKEN_LIFECYCLE {
            return Err(HostBoundaryRejection::WrongLifecycle);
        }
        if self.fresh_cells.len() >= MAX_FRESH_TOKEN_CELLS {
            return Err(HostBoundaryRejection::RegistryExhausted);
        }
        let acquisition = NEXT_ACQUISITION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| HostBoundaryRejection::RegistryExhausted)?;
        // Allocation happens before any owner registration. No caller payload
        // or adoption identity can choose this actual cell or its acquisition.
        let cell = Arc::new(FreshTokenCell { acquisition });
        let token = self.register_adapter_owner(provenance.clone(), acquisition)?;
        let previous = self.fresh_cells.insert(token.slot(), cell);
        debug_assert!(
            previous.is_none(),
            "new registry slot cannot replace a cell"
        );
        Ok(FreshTokenAcquisition {
            token,
            acquisition,
            provenance,
        })
    }
}

impl FreshTokenAcquisition {
    /// Descriptive credential coordinates; no owner token is exposed.
    pub(crate) fn slot(&self) -> u64 {
        self.token.slot()
    }
    pub(crate) fn generation(&self) -> u64 {
        self.token.generation()
    }
    pub(crate) fn producer(&self) -> &'static str {
        FRESH_TOKEN_PRODUCER
    }

    pub(crate) fn validate(
        &self,
        registry: &HostOwnershipRegistry,
    ) -> Result<(), HostBoundaryRejection> {
        if registry.poisoned {
            return Err(HostBoundaryRejection::RegistryPoisoned);
        }
        let owner = registry.lookup(self.token)?;
        if owner.state != HostOwnerState::Live {
            return Err(HostBoundaryRejection::OwnerNotLive);
        }
        if owner.provenance != self.provenance || owner.payload != self.acquisition {
            return Err(HostBoundaryRejection::StaleOwner);
        }
        let cell = registry
            .fresh_cells
            .get(&self.token.slot())
            .ok_or(HostBoundaryRejection::UnknownOwner)?;
        if cell.acquisition != self.acquisition {
            return Err(HostBoundaryRejection::StaleOwner);
        }
        Ok(())
    }

    pub(crate) fn rollback(
        &self,
        registry: &mut HostOwnershipRegistry,
    ) -> Result<(), HostBoundaryRejection> {
        self.validate(registry)?;
        registry.rollback_adapter_owner(self.token)?;
        let cell = registry.fresh_cells.remove(&self.token.slot());
        debug_assert!(cell.is_some(), "validated fresh rollback retains its cell");
        Ok(())
    }

    pub(crate) fn retire(
        &self,
        registry: &mut HostOwnershipRegistry,
    ) -> Result<(), HostBoundaryRejection> {
        self.validate(registry)?;
        registry.retire_owner(self.token)?;
        let cell = registry.fresh_cells.remove(&self.token.slot());
        debug_assert!(cell.is_some(), "validated retirement retains its cell");
        Ok(())
    }

    #[cfg(test)]
    fn weak(&self, registry: &HostOwnershipRegistry) -> std::sync::Weak<FreshTokenCell> {
        Arc::downgrade(registry.fresh_cells.get(&self.slot()).unwrap())
    }
}

#[cfg(test)]
mod tests;
