//! Test-only independently funded oracle account. A scope is a genuine test
//! input owned by one thread-local owner; it is not a refund, an exemption or
//! a production limit, and nothing falls back to the production ACTIVE ledger.
//! Every charge is explicit, cumulative and fail-closed.
use std::cell::Cell;
use std::marker::PhantomData;

mod diagnostic;
mod invocation;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod zero_tests;

pub(in crate::cleanup_plan::replay) use diagnostic::{
    raw_error_funded, raw_error_metered, replay_error_funded, ReplayFailure,
};
pub(in crate::cleanup_plan::replay) use invocation::{
    authority, authority_for_function, enter, opt_in, opted, InvocationRefusal,
};

/// Observable account state. Reported next to, never merged into, production
/// usage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) struct OracleReceipt {
    pub(in crate::cleanup_plan::replay) allowance: usize,
    pub(in crate::cleanup_plan::replay) charged: usize,
    /// Full receipt of the first refused charge; later refusals are sticky and
    /// never overwrite it.
    pub(in crate::cleanup_plan::replay) first_refusal: Option<OracleFirstRefusal>,
    /// Cumulative charge attempts, including sticky retries.
    pub(in crate::cleanup_plan::replay) calls: usize,
    pub(in crate::cleanup_plan::replay) started: usize,
    pub(in crate::cleanup_plan::replay) completed: usize,
    /// Finite admission-work allowance, in work UNITS and never converted to or
    /// from bytes. `open` grants none; only `open_with_work` grants units.
    pub(in crate::cleanup_plan::replay) work_allowance: usize,
    pub(in crate::cleanup_plan::replay) work: usize,
    /// Cumulative work-charge attempts, including sticky retries.
    pub(in crate::cleanup_plan::replay) work_calls: usize,
    /// First refused work charge (requested units, units left, 1-based attempt).
    /// It makes byte charges sticky too, as a byte refusal makes work sticky.
    pub(in crate::cleanup_plan::replay) first_work_refusal: Option<OracleFirstRefusal>,
    /// First allocator, layout or unfunded-acquisition failure, inline and
    /// independent of the byte allowance. It is sticky for every later charge
    /// and is never read as a semantic verdict; a later one never replaces it.
    pub(in crate::cleanup_plan::replay) physical_failure: Option<&'static str>,
}

impl OracleReceipt {
    fn is_refused(&self) -> bool {
        self.first_refusal.is_some()
            || self.first_work_refusal.is_some()
            || self.physical_failure.is_some()
    }
}

/// Requested bytes, bytes still available and 1-based attempt ordinal of the
/// first refused charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) struct OracleFirstRefusal {
    pub(in crate::cleanup_plan::replay) requested: usize,
    pub(in crate::cleanup_plan::replay) remaining: usize,
    pub(in crate::cleanup_plan::replay) ordinal: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) enum OracleRefusal {
    /// No account is owned by this thread.
    Missing,
    /// The lease belongs to a scope that already ended or was replaced.
    Expired,
    /// An earlier charge was refused; every later charge or start fails.
    Sticky,
    Exhausted {
        requested: usize,
    },
    /// A counter would wrap; nothing is accepted.
    Overflow,
    /// `complete` without a matching earlier `start`.
    Unbalanced,
}

#[derive(Clone, Copy)]
struct State {
    epoch: u64,
    receipt: OracleReceipt,
}

thread_local! {
    static ACCOUNT: Cell<Option<State>> = const { Cell::new(None) };
    static EPOCH: Cell<u64> = const { Cell::new(0) };
}

/// Affine owner. Dropping it, including during unwind, restores the thread to
/// "no account". It is neither Clone nor Send.
#[must_use]
pub(in crate::cleanup_plan::replay) struct OracleScope {
    epoch: u64,
    _thread: PhantomData<*const ()>,
}

/// Copyable authority token bound to exactly one scope epoch. The raw-pointer
/// marker makes it neither Send nor Sync, so it cannot alias another thread's
/// local epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) struct OracleLease {
    epoch: u64,
    _thread: PhantomData<*const ()>,
}

impl OracleScope {
    /// Nested scopes fail closed; the existing account is left untouched.
    pub(in crate::cleanup_plan::replay) fn open(allowance: usize) -> Result<Self, OracleRefusal> {
        Self::open_with_work(allowance, 0)
    }

    pub(in crate::cleanup_plan::replay) fn open_with_work(
        allowance: usize,
        work_allowance: usize,
    ) -> Result<Self, OracleRefusal> {
        if ACCOUNT.with(|account| account.get().is_some()) {
            return Err(OracleRefusal::Sticky);
        }
        // A wrapped epoch could alias a stale lease, so exhaustion refuses.
        let epoch = EPOCH.with(|epoch| {
            let next = epoch.get().checked_add(1).ok_or(OracleRefusal::Overflow)?;
            epoch.set(next);
            Ok::<u64, OracleRefusal>(next)
        })?;
        ACCOUNT.with(|account| {
            account.set(Some(State {
                epoch,
                receipt: OracleReceipt {
                    allowance,
                    charged: 0,
                    first_refusal: None,
                    calls: 0,
                    started: 0,
                    completed: 0,
                    work_allowance,
                    work: 0,
                    work_calls: 0,
                    first_work_refusal: None,
                    physical_failure: None,
                },
            }))
        });
        Ok(Self {
            epoch,
            _thread: PhantomData,
        })
    }

    pub(in crate::cleanup_plan::replay) fn lease(&self) -> OracleLease {
        OracleLease {
            epoch: self.epoch,
            _thread: PhantomData,
        }
    }

    pub(in crate::cleanup_plan::replay) fn receipt(&self) -> OracleReceipt {
        self.lease().receipt().expect("live oracle scope")
    }

    pub(in crate::cleanup_plan::replay) fn finish(self) -> OracleReceipt {
        self.receipt()
    }
}

impl Drop for OracleScope {
    fn drop(&mut self) {
        ACCOUNT.with(|account| {
            if account.get().is_some_and(|state| state.epoch == self.epoch) {
                account.set(None);
            }
        });
    }
}

impl OracleLease {
    fn with<R>(
        self,
        update: impl FnOnce(&mut OracleReceipt) -> Result<R, OracleRefusal>,
    ) -> Result<R, OracleRefusal> {
        ACCOUNT.with(|account| {
            let mut state = account.get().ok_or(OracleRefusal::Missing)?;
            if state.epoch != self.epoch {
                return Err(OracleRefusal::Expired);
            }
            let result = update(&mut state.receipt);
            account.set(Some(state));
            result
        })
    }

    pub(in crate::cleanup_plan::replay) fn receipt(self) -> Result<OracleReceipt, OracleRefusal> {
        self.with(|receipt| Ok(*receipt))
    }

    /// Debit exactly `bytes` before the caller constructs anything. Zero bytes
    /// authenticate the lease and the sticky state only.
    pub(in crate::cleanup_plan::replay) fn charge(self, bytes: usize) -> Result<(), OracleRefusal> {
        self.with(|receipt| {
            // Sticky retries are still attempts; a wrapped count refuses.
            let Some(ordinal) = receipt.calls.checked_add(1) else {
                receipt.first_refusal.get_or_insert(OracleFirstRefusal {
                    requested: bytes,
                    remaining: receipt.allowance - receipt.charged,
                    ordinal: receipt.calls,
                });
                return Err(OracleRefusal::Overflow);
            };
            receipt.calls = ordinal;
            if receipt.is_refused() {
                return Err(OracleRefusal::Sticky);
            }
            let remaining = receipt.allowance - receipt.charged;
            match receipt.charged.checked_add(bytes) {
                Some(total) if total <= receipt.allowance => {
                    receipt.charged = total;
                    Ok(())
                }
                _ => {
                    receipt.first_refusal = Some(OracleFirstRefusal {
                        requested: bytes,
                        remaining,
                        ordinal,
                    });
                    Err(OracleRefusal::Exhausted { requested: bytes })
                }
            }
        })
    }

    // Test-only scalar predicates retain the original small bool receiving
    // values, but never manufacture a Result or Diagnostic-bearing carrier.
    pub(in crate::cleanup_plan::replay) fn refused_zero(self) -> bool {
        ACCOUNT.with(|account| match account.get() {
            Some(state) if state.epoch == self.epoch => state.receipt.is_refused(),
            _ => true,
        })
    }

    pub(in crate::cleanup_plan::replay) fn authenticate_zero(self) -> bool {
        ACCOUNT.with(|account| {
            let Some(mut state) = account.get() else {
                return false;
            };
            if state.epoch != self.epoch {
                return false;
            }
            // Preserve charge(0)'s attempt count even on sticky retries, and
            // its exact first receipt when the counter itself cannot advance.
            let Some(ordinal) = state.receipt.calls.checked_add(1) else {
                state
                    .receipt
                    .first_refusal
                    .get_or_insert(OracleFirstRefusal {
                        requested: 0,
                        remaining: state.receipt.allowance - state.receipt.charged,
                        ordinal: state.receipt.calls,
                    });
                account.set(Some(state));
                return false;
            };
            state.receipt.calls = ordinal;
            if state.receipt.is_refused() {
                account.set(Some(state));
                return false;
            }
            // checked_add(0) always succeeds and leaves charged unchanged.
            account.set(Some(state));
            true
        })
    }

    /// Debit admission WORK UNITS against the account's finite work allowance
    /// before the caller proceeds. A refused or wrapped charge is recorded and
    /// every later work or byte charge is sticky; a wrapped attempt count or
    /// total never aliases a smaller one. Zero units authenticate only.
    pub(in crate::cleanup_plan::replay) fn charge_work(
        self,
        units: usize,
    ) -> Result<(), OracleRefusal> {
        self.with(|receipt| {
            let remaining = receipt.work_allowance - receipt.work;
            let Some(ordinal) = receipt.work_calls.checked_add(1) else {
                receipt
                    .first_work_refusal
                    .get_or_insert(OracleFirstRefusal {
                        requested: units,
                        remaining,
                        ordinal: receipt.work_calls,
                    });
                return Err(OracleRefusal::Overflow);
            };
            receipt.work_calls = ordinal;
            if receipt.is_refused() {
                return Err(OracleRefusal::Sticky);
            }
            match receipt.work.checked_add(units) {
                Some(total) if total <= receipt.work_allowance => {
                    receipt.work = total;
                    Ok(())
                }
                total => {
                    receipt.first_work_refusal = Some(OracleFirstRefusal {
                        requested: units,
                        remaining,
                        ordinal,
                    });
                    Err(if total.is_none() {
                        OracleRefusal::Overflow
                    } else {
                        OracleRefusal::Exhausted { requested: units }
                    })
                }
            }
        })
    }

    pub(in crate::cleanup_plan::replay) fn refused(self) -> bool {
        self.with(|receipt| Ok(receipt.is_refused()))
            .unwrap_or(true)
    }

    /// Record an allocator, layout or unfunded-acquisition failure. The first
    /// reason is kept; the account is then sticky even with bytes left.
    pub(in crate::cleanup_plan::replay) fn fail_physical(self, reason: &'static str) {
        let _ = self.with(|receipt| {
            receipt.physical_failure.get_or_insert(reason);
            Ok(())
        });
    }

    /// The physical failure alone, never a logical refusal.
    pub(in crate::cleanup_plan::replay) fn physical_failure(self) -> Option<&'static str> {
        self.with(|receipt| Ok(receipt.physical_failure))
            .ok()
            .flatten()
    }

    pub(in crate::cleanup_plan::replay) fn start(self) -> Result<(), OracleRefusal> {
        self.with(|receipt| {
            if receipt.is_refused() {
                return Err(OracleRefusal::Sticky);
            }
            receipt.started = receipt
                .started
                .checked_add(1)
                .ok_or(OracleRefusal::Overflow)?;
            Ok(())
        })
    }

    /// Completion may follow a refusal, but never exceeds the starts.
    pub(in crate::cleanup_plan::replay) fn complete(self) -> Result<(), OracleRefusal> {
        self.with(|receipt| {
            if receipt.completed >= receipt.started {
                return Err(OracleRefusal::Unbalanced);
            }
            receipt.completed += 1;
            Ok(())
        })
    }
}

// Helper-owned metadata for ONE additional ordinary Oracle charge. This is a
// bounded maximum over success/missing/expired/sticky/exhausted arms, not an
// account, permission, refund or substitute for performing the original call.
// Private State includes the epoch; a receipt-only stand-in loses real owners.
impl OracleLease {
    const WITH_UPDATE_CONTROLS: usize = {
        use std::mem::size_of;
        2 * size_of::<OracleLease>() // with actual/formal self
        + 2 * size_of::<&usize>() // moved update closure, actual/formal capture
        + 2 * size_of::<(&u64, &usize)>() // TLS closure captures epoch loan + update
        + 2 * size_of::<&std::thread::LocalKey<Cell<Option<State>>>>()
        + 6 * size_of::<&Cell<Option<State>>>() // TLS argument, get self, set self pairs
        + 5 * size_of::<Option<State>>() // get result/ok_or receiver; Some initializer + set actual/formal
        + 2 * size_of::<Result<State, OracleRefusal>>() // ok_or return/receiving match
        + 4 * size_of::<State>() // bound Ok owner, mutable local, Some move, selected receiver
        + 2 * size_of::<&mut OracleReceipt>() // update actual/formal loan
        + 8 * size_of::<Result<(), OracleRefusal>>() // update, TLS, with, outer return/receiving pairs
        + 4 * size_of::<OracleRefusal>() // missing/expired selected constructors and matches
        + 4 * size_of::<u64>() + 2 * size_of::<bool>() // epoch values and equality/branch
    };
    const REFUSAL_PREDICATE_CONTROLS: usize = {
        use std::mem::size_of;
        2 * size_of::<&OracleReceipt>()
        + 4 * size_of::<&Option<OracleFirstRefusal>>()
        + 2 * size_of::<&Option<&str>>()
        + 8 * size_of::<bool>()
    };
    const ARITHMETIC_UPDATE_CONTROLS: usize = {
        use std::mem::size_of;
        2 * size_of::<(OracleLease, usize)>() // public charge actual/formal tuple
        + 4 * size_of::<Option<usize>>() // checked attempt/total result and matches
        + 14 * size_of::<usize>() // inputs, remaining, ordinals, total and publication
        + 2 * size_of::<OracleFirstRefusal>() // initializer/insert parameter
        + 2 * size_of::<Option<OracleFirstRefusal>>() // selected Some publication
        + 2 * size_of::<&mut Option<OracleFirstRefusal>>()
        + 2 * size_of::<&mut OracleFirstRefusal>()
        + 4 * size_of::<OracleRefusal>() // overflow/sticky/exhausted selected results
        + 8 * size_of::<bool>() // comparison, selection and overflow predicates
    };
    // refused() uses the same epoch-bearing State producer, but its update
    // closure is zero-sized and the TLS closure captures only &self. Its
    // actual returned R is bool, not unit. No receipt/state proxy is used.
    const REFUSED_CALL_CONTROLS: usize = Self::WITH_UPDATE_CONTROLS
        - 8 * std::mem::size_of::<Result<(), OracleRefusal>>()
        + 8 * std::mem::size_of::<Result<bool, OracleRefusal>>()
        - 2 * std::mem::size_of::<&usize>()
        - 2 * std::mem::size_of::<(&u64, &usize)>()
        + 2 * std::mem::size_of::<&OracleLease>()
        + Self::REFUSAL_PREDICATE_CONTROLS;
    // A new work call first runs prepare_builder -> builder_refused -> refused,
    // then the complete charge_work update. Both real State paths are paid.
    pub(in crate::cleanup_plan::replay) const WORK_CALL_CONTROLS: usize =
        Self::WITH_UPDATE_CONTROLS + Self::REFUSAL_PREDICATE_CONTROLS
        + Self::ARITHMETIC_UPDATE_CONTROLS + Self::REFUSED_CALL_CONTROLS;
    // A new positive byte call performs BOTH readiness checks: prepare_builder
    // checks builder_refused, then reserve_compact_bytes checks refused again
    // before charge. Neither old call is credited or removed by the new bank.
    pub(in crate::cleanup_plan::replay) const BYTE_CALL_CONTROLS: usize =
        Self::WITH_UPDATE_CONTROLS + Self::REFUSAL_PREDICATE_CONTROLS
        + Self::ARITHMETIC_UPDATE_CONTROLS + 2 * Self::REFUSED_CALL_CONTROLS;
}
