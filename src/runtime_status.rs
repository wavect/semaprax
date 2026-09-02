//! Reference model for invocation-scoped SEMAPRAX status arenas.
//!
//! ABI status tokens are opaque, context-local, one-based `u32` indices.
//! Token zero is success and never identifies a status record.  Arena
//! exhaustion is a harness/runtime-capacity error: it must never be converted
//! into a language-visible [`NormalizedStatus`].

use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cleanup_plan::{ContractPhase, StatusCase};
use crate::conformance::NormalizedStatus;

static NEXT_ARENA_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// The raw status value carried by the internal ABI.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatusToken(u32);

impl StatusToken {
    pub const SUCCESS: Self = Self(0);

    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn is_success(self) -> bool {
        self.0 == 0
    }
}

/// Host-assigned identity of one invocation-scoped status context.
///
/// The reference model does not allocate identities from process-global state;
/// a host, test harness, or generated wrapper supplies the invocation nonce.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatusContextId(u64);

impl StatusContextId {
    pub const fn new(nonce: u64) -> Self {
        Self(nonce)
    }

    pub const fn nonce(self) -> u64 {
        self.0
    }
}

/// A reference-execution token paired with its context provenance.
///
/// Generated ABIs carry only [`Self::raw`].  Keeping the context alongside it
/// in the reference executor makes accidental cross-context resolution
/// detectable without changing the target ABI.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScopedStatusToken {
    context: StatusContextId,
    arena_instance: u64,
    token: StatusToken,
}

impl ScopedStatusToken {
    pub const fn context(self) -> StatusContextId {
        self.context
    }

    pub const fn token(self) -> StatusToken {
        self.token
    }

    pub const fn raw(self) -> u32 {
        self.token.raw()
    }
}

/// Exact compiler-owned normalization for checked arithmetic failures.
pub fn normalize_arithmetic(case: StatusCase) -> NormalizedStatus {
    NormalizedStatus::arithmetic(case)
}

/// Exact compiler-owned normalization for a false contract expression.
///
/// Contract ordinal and expression identity remain source/trace metadata. They
/// deliberately do not alter the stable semantic status code.
pub fn normalize_contract(phase: ContractPhase) -> NormalizedStatus {
    NormalizedStatus::contract(phase)
}

/// Invocation-local immutable status storage.
#[derive(Debug, Eq, PartialEq)]
pub struct StatusArena {
    context: StatusContextId,
    arena_instance: u64,
    capacity: u32,
    records: Vec<NormalizedStatus>,
}

impl StatusArena {
    pub fn new(context: StatusContextId, capacity: u32) -> Result<Self, StatusArenaError> {
        let arena_instance = NEXT_ARENA_INSTANCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                (next != u64::MAX).then_some(next + 1)
            })
            .map_err(|_| StatusArenaError::ArenaIdentityExhausted)?;
        Ok(Self {
            context,
            arena_instance,
            capacity,
            // Do not eagerly reserve attacker- or harness-controlled capacity.
            records: Vec::new(),
        })
    }

    pub const fn context(&self) -> StatusContextId {
        self.context
    }

    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn record(
        &mut self,
        status: NormalizedStatus,
    ) -> Result<ScopedStatusToken, StatusArenaError> {
        if self.records.len() >= self.capacity as usize {
            return Err(StatusArenaError::Exhausted {
                capacity: self.capacity,
            });
        }
        let raw = u32::try_from(self.records.len())
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or(StatusArenaError::TokenSpaceExhausted)?;
        self.records.push(status);
        Ok(ScopedStatusToken {
            context: self.context,
            arena_instance: self.arena_instance,
            token: StatusToken(raw),
        })
    }

    pub fn record_arithmetic(
        &mut self,
        case: StatusCase,
    ) -> Result<ScopedStatusToken, StatusArenaError> {
        self.record(normalize_arithmetic(case))
    }

    pub fn record_contract(
        &mut self,
        phase: ContractPhase,
    ) -> Result<ScopedStatusToken, StatusArenaError> {
        self.record(normalize_contract(phase))
    }

    pub fn resolve(
        &self,
        scoped: ScopedStatusToken,
    ) -> Result<&NormalizedStatus, StatusArenaError> {
        if scoped.context != self.context {
            return Err(StatusArenaError::WrongContext {
                expected: self.context,
                actual: scoped.context,
            });
        }
        if scoped.arena_instance != self.arena_instance {
            return Err(StatusArenaError::ForeignArena {
                context: self.context,
            });
        }
        self.resolve_local(scoped.token)
    }

    /// Resolve a raw ABI token already known to belong to this context.
    pub fn resolve_local(&self, token: StatusToken) -> Result<&NormalizedStatus, StatusArenaError> {
        if token.is_success() {
            return Err(StatusArenaError::SuccessHasNoRecord);
        }
        let index = usize::try_from(token.raw() - 1)
            .map_err(|_| StatusArenaError::UnknownToken { token })?;
        self.records
            .get(index)
            .ok_or(StatusArenaError::UnknownToken { token })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusArenaError {
    ArenaIdentityExhausted,
    /// Reference harness capacity failed; this is never a language status.
    Exhausted {
        capacity: u32,
    },
    TokenSpaceExhausted,
    SuccessHasNoRecord,
    UnknownToken {
        token: StatusToken,
    },
    WrongContext {
        expected: StatusContextId,
        actual: StatusContextId,
    },
    ForeignArena {
        context: StatusContextId,
    },
}

impl fmt::Display for StatusArenaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArenaIdentityExhausted => {
                formatter.write_str("status arena instance identity space is exhausted")
            }
            Self::Exhausted { capacity } => {
                write!(formatter, "status arena capacity {capacity} is exhausted")
            }
            Self::TokenSpaceExhausted => formatter.write_str("status token space is exhausted"),
            Self::SuccessHasNoRecord => {
                formatter.write_str("success token zero has no status record")
            }
            Self::UnknownToken { token } => {
                write!(formatter, "status token {} is not allocated", token.raw())
            }
            Self::WrongContext { expected, actual } => write!(
                formatter,
                "status token belongs to context {}, not context {}",
                actual.nonce(),
                expected.nonce()
            ),
            Self::ForeignArena { context } => write!(
                formatter,
                "status token belongs to another arena for context {}",
                context.nonce()
            ),
        }
    }
}

impl Error for StatusArenaError {}

#[cfg(test)]
#[path = "runtime_status/tests.rs"]
mod tests;
