//! Private structural-cost accumulators for the expected projection.

use crate::diagnostic::Diagnostic;

use super::super::{active_builder_limit, checked_usage};

#[derive(Clone, Copy)]
pub(super) struct ExpandedDefaultCost {
    pub(super) bytes: usize,
    pub(super) identity_slots: usize,
}

#[derive(Clone, Copy)]
pub(super) struct GenericInstanceCost {
    pub(super) bytes: usize,
    pub(super) identity_slots: usize,
}

pub(super) struct StructuralCost(pub(super) usize);

impl StructuralCost {
    pub(super) fn add(&mut self, bytes: usize) -> Result<(), Vec<Diagnostic>> {
        self.0 = checked_usage(self.0, bytes, "builder_bytes", active_builder_limit())?;
        Ok(())
    }

    pub(super) fn value<T>(&mut self, value: &T) -> Result<(), Vec<Diagnostic>> {
        self.add(std::mem::size_of_val(value))
    }

    pub(super) fn string(&mut self, value: &str) -> Result<(), Vec<Diagnostic>> {
        self.add(std::mem::size_of::<String>())?;
        self.add(value.len())
    }
}
