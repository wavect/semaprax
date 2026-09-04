//! Private structural-cost accumulators for the expected projection.

use crate::diagnostic::Diagnostic;

use super::super::{active_builder_limit, checked_usage};

#[derive(Clone, Copy)]
pub(super) struct ExpandedDefaultCost {
    pub(super) bytes: usize,
    pub(super) string_bytes: usize,
    pub(super) identity_slots: usize,
}

#[derive(Clone, Copy)]
pub(super) struct GenericInstanceCost {
    pub(super) bytes: usize,
    pub(super) string_bytes: usize,
    pub(super) identity_slots: usize,
}

/// Bytes the resolver clones from one source tree, split into the fixed node
/// footprints that expand by the structural factor and the string contents
/// that expand by the copy factor. `string_bytes` is always part of `total`.
pub(super) struct StructuralCost {
    pub(super) total: usize,
    pub(super) string_bytes: usize,
}

impl StructuralCost {
    pub(super) const fn new() -> Self {
        Self {
            total: 0,
            string_bytes: 0,
        }
    }

    pub(super) const fn structure(bytes: usize) -> Self {
        Self {
            total: bytes,
            string_bytes: 0,
        }
    }

    pub(super) fn add(&mut self, bytes: usize) -> Result<(), Vec<Diagnostic>> {
        self.total = checked_usage(self.total, bytes, "builder_bytes", active_builder_limit())?;
        Ok(())
    }

    /// Absorbs a cost whose `string_bytes` are already part of `bytes`.
    pub(super) fn add_split(
        &mut self,
        bytes: usize,
        string_bytes: usize,
    ) -> Result<(), Vec<Diagnostic>> {
        self.add(bytes)?;
        self.string_bytes = checked_usage(
            self.string_bytes,
            string_bytes,
            "builder_bytes",
            active_builder_limit(),
        )?;
        Ok(())
    }

    pub(super) fn value<T>(&mut self, value: &T) -> Result<(), Vec<Diagnostic>> {
        self.add(std::mem::size_of_val(value))
    }

    pub(super) fn string(&mut self, value: &str) -> Result<(), Vec<Diagnostic>> {
        self.add(std::mem::size_of::<String>())?;
        self.add_split(value.len(), value.len())
    }
}
