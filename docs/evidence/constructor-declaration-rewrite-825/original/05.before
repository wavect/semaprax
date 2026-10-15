//! Borrowed original owner; no new ACTIVE handle, budget or report allocation.
use super::{Receiver, Setup};
use crate::workspace_graph::constructor_failure::typed::Failure;
use std::mem::size_of;
impl Receiver {
    pub(in crate::workspace_graph) const fn reservation_controls() -> usize {
        size_of::<Result<(), Setup>>()
            + size_of::<&mut Self>()
            + size_of::<usize>()
            + size_of::<Option<Setup>>()
    }
    pub(in crate::workspace_graph) fn reserve(&mut self, bytes: usize) -> Result<(), Setup> {
        if let Some(first) = self.first {
            return Err(first);
        }
        if self.detached || !self.ledger.current() {
            self.detached = true;
            self.first = Some(Setup::Detached);
            return Err(Setup::Detached);
        }
        // This adapter returns only Copy Setup. Price its declared receiving
        // return/argument/control carriers before any output acquisition.
        let controls = Self::reservation_controls();
        let Some(bytes) = bytes.checked_add(controls) else {
            self.first = Some(Setup::Arithmetic);
            return Err(Setup::Arithmetic);
        };
        if !crate::bounded_output::reserve_active_required(bytes) {
            let first = Setup::Budget {
                maximum: self.maximum,
            };
            self.first = Some(first);
            Err(first)
        } else {
            Ok(())
        }
    }
    /// Full initialized returned Result and borrowed owner argument per entry.
    pub(in crate::workspace_graph) fn entry<T>(&mut self) -> Result<(), Setup> {
        let bytes = size_of::<Result<T, Failure>>()
            .checked_add(size_of::<&mut Self>())
            .and_then(|n| n.checked_add(size_of::<Result<(), Setup>>()))
            .and_then(|n| n.checked_add(size_of::<usize>()));
        let Some(bytes) = bytes else {
            let first = *self.first.get_or_insert(Setup::Arithmetic);
            return Err(first);
        };
        self.reserve(bytes)
    }
    pub(in crate::workspace_graph) fn text(&mut self, value: &str) -> Result<String, Failure> {
        self.entry::<String>()?;
        self.reserve(size_of::<String>())?;
        let text = crate::bounded_output::budgeted_clone(value);
        // budgeted_clone's empty sentinel never licenses subsequent acquisition.
        self.reserve(0)?;
        Ok(text)
    }
    /// The old structural checked_usage cap is a logical verdict, not a
    /// physical reservation receipt. Keep its original captured maximum and
    /// first Setup inline without inventing a sticky ledger refusal.
    pub(in crate::workspace_graph) fn limit(&mut self) -> Setup {
        if let Err(first) = self.reserve(0) {
            return first;
        }
        let first = Setup::Budget {
            maximum: self.maximum,
        };
        self.first = Some(first);
        first
    }
    pub(in crate::workspace_graph) fn sum(
        &mut self,
        used: usize,
        additional: usize,
    ) -> Result<usize, Setup> {
        self.reserve(
            size_of::<Result<usize, Setup>>()
                + size_of::<Option<usize>>()
                + 2 * size_of::<usize>()
                + size_of::<&mut Self>(),
        )?;
        let next = used.checked_add(additional);
        if let Some(next) = next {
            if next <= self.maximum {
                return Ok(next);
            }
        }
        let first = Setup::Budget {
            maximum: self.maximum,
        };
        self.first = Some(first);
        Err(first)
    }
    pub(in crate::workspace_graph) fn physical(&mut self) -> Setup {
        if let Some(first) = self.first {
            return first;
        }
        // Fixed Copy/refusal control footprint was acquired with this owner.
        // Authenticate before the unchanged maximum-size physical request;
        // do not put a new paid probe ahead of its inherited first receipt.
        if self.detached || !self.ledger.current() {
            self.detached = true;
            self.first = Some(Setup::Detached);
            return Setup::Detached;
        }
        crate::bounded_output::refuse_active_required(usize::MAX);
        let first = Setup::Budget {
            maximum: self.maximum,
        };
        self.first = Some(first);
        first
    }
}

#[cfg(test)]
mod tests;
