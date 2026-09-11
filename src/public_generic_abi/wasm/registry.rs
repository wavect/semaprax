//! Host-side handle-safety bookkeeping for the Core Wasm adapter.
//!
//! Wasm itself has no concept of a typed, owned handle: a numeric handle is
//! just an integer a foreign caller can forge, replay, or hand back for the
//! wrong slot. [`super::native`]'s C adapter answers this by scanning a
//! process-wide registry for the exact pointer value before ever
//! dereferencing it; this module reuses the identical pattern for Wasm's
//! numeric-handle world rather than reinventing it: every handle this
//! adapter ever mints is recorded here first, and every use looks the
//! handle up here before touching linear memory, so a foreign, stale,
//! wrong-kind, or wrong-provider handle is rejected without ever computing
//! an offset from attacker-controlled bytes.

use std::collections::HashMap;

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::carrier::Handle;

/// A handle is not live in this provider's registry: foreign, stale,
/// already consumed/released, forged, or presented against a provider that
/// never minted it.
pub const HANDLE_INVALID: &str = "SPX-PG915";
/// A handle was presented where a different structural position or role was
/// required (root vs. leaf, input value vs. result), independent of the
/// handle's own lifecycle state.
pub const WRONG_KIND: &str = "SPX-PG916";

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(HANDLE_INVALID, message.into())
}

fn wrong_kind(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(WRONG_KIND, message.into())
}

/// Which structural position and which side of the boundary a registered
/// handle names. Orthogonal from the handle's own [`crate::public_generic_abi::carrier::CarrierState`]:
/// this is identity, not lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandleRole {
    InputRoot,
    InputLeaf,
    ResultRoot,
    ResultLeaf,
}

impl HandleRole {
    fn is_leaf(self) -> bool {
        matches!(self, Self::InputLeaf | Self::ResultLeaf)
    }
}

/// One live registry entry: where the handle's bytes live in linear memory,
/// and which structural role it was minted for.
#[derive(Clone, Copy, Debug)]
pub struct RegistryEntry {
    pub role: HandleRole,
    pub offset: u32,
    pub len: u32,
}

/// The live-handle table for one [`super::provider::WasmProvider`] instance.
/// Never shared across providers: two providers hold two independent
/// registries, so a handle minted by one is simply absent from the other's
/// table — the same [`HANDLE_INVALID`] outcome a forged handle gets, with no
/// separate code needed for "foreign provider."
#[derive(Clone, Debug, Default)]
pub struct HandleRegistry {
    entries: HashMap<Handle, RegistryEntry>,
}

impl HandleRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Register a freshly minted handle. Panics if `handle` is already
    /// registered — an adapter-internal invariant violation (double-mint of
    /// the same `(id, generation)` pair), never a caller-reachable path,
    /// since generations are minted monotonically by
    /// [`super::provider::WasmProvider`] and never reused.
    pub fn insert(&mut self, handle: Handle, entry: RegistryEntry) {
        let previous = self.entries.insert(handle, entry);
        debug_assert!(
            previous.is_none(),
            "SEMAPRAX Core Wasm adapter: minted the same handle twice"
        );
    }

    /// Look up `handle`, requiring it to be live and to have exactly `role`.
    /// Fails closed for a foreign, stale, released, or wrong-kind handle
    /// without ever computing a memory offset from it first.
    pub fn get(&self, handle: Handle, role: HandleRole) -> Result<RegistryEntry, Diagnostic> {
        let entry =
            self.entries.get(&handle).copied().ok_or_else(|| {
                invalid(format!("handle {handle:?} is not live in this registry"))
            })?;
        if entry.role != role {
            return Err(wrong_kind(format!(
                "handle {handle:?} is {:?}, not the required {role:?}",
                entry.role
            )));
        }
        Ok(entry)
    }

    /// Remove and return a live, correctly-kinded handle's entry — the
    /// lookup half of a release, so a release call cannot free the same
    /// handle twice.
    pub fn remove(
        &mut self,
        handle: Handle,
        role: HandleRole,
    ) -> Result<RegistryEntry, Diagnostic> {
        let entry = self.get(handle, role)?;
        self.entries.remove(&handle);
        Ok(entry)
    }

    /// Every currently live handle with the given role, leaf-only entries in
    /// insertion (structural) order — used to drive release in reverse
    /// obligation order without trusting a caller-submitted sequence.
    pub fn live_leaves_in_order(&self, leaf_role: HandleRole) -> Vec<(Handle, RegistryEntry)> {
        debug_assert!(leaf_role.is_leaf());
        let mut leaves: Vec<(Handle, RegistryEntry)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.role == leaf_role)
            .map(|(handle, entry)| (*handle, *entry))
            .collect();
        leaves.sort_by_key(|(handle, _)| handle.id);
        leaves
    }
}

#[cfg(test)]
mod tests;
