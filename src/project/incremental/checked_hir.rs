//! Immutable existing checked-module owner, never a validation lease.
use super::CheckedModule;
use crate::hir::ResolvedProgram;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct CheckedModuleLease(Arc<CheckedModule>);
impl CheckedModuleLease {
    pub(super) fn new(owner: Arc<CheckedModule>) -> Self {
        Self(owner)
    }
    /// Confirm that grouped views borrow the same existing allocation.
    pub(crate) fn same_backing(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Deref for CheckedModuleLease {
    type Target = ResolvedProgram;
    fn deref(&self) -> &Self::Target {
        &self.0.resolved
    }
}
// Keep the owned fallback inline: its actual carrier delta is prepaid by
// checked_retention::reserve_input_carriers. Boxing adds a new allocation.
#[allow(clippy::large_enum_variant)]
#[cfg_attr(test, derive(Clone))]
pub(crate) enum ResolvedModuleInput {
    Owned(ResolvedProgram),
    Checked(CheckedModuleLease),
}
impl Deref for ResolvedModuleInput {
    type Target = ResolvedProgram;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(program) => program,
            Self::Checked(owner) => owner,
        }
    }
}
impl From<ResolvedProgram> for ResolvedModuleInput {
    fn from(program: ResolvedProgram) -> Self {
        Self::Owned(program)
    }
}

#[cfg(test)]
impl std::ops::DerefMut for ResolvedModuleInput {
    fn deref_mut(&mut self) -> &mut ResolvedProgram {
        if let Self::Checked(owner) = self {
            *self = Self::Owned((**owner).clone());
        }
        let Self::Owned(program) = self else {
            unreachable!()
        };
        program
    }
}

#[cfg(test)]
pub(crate) mod tests;
