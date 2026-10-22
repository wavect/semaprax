//! Ordinary replay's lexical full-error receiving protocol (#892).
//!
//! These are producers, not adapters around the returning budget API. Expansion
//! keeps the payer and destination at the caller: no Diagnostic Result is formed
//! by a successful reservation or work debit. Legacy methods remain independent.
use super::*;

pub(super) struct Failure<'a> {
    pub(super) error: Option<Diagnostic>,
    pub(super) function: &'a ResolvedFunction,
    pub(super) budget: *const ReplayBudget,
}
impl<'a> Failure<'a> {
    pub(super) const fn acquisition_bytes() -> usize {
        // One inline owner, its one possible terminal initializer, and the
        // caller's initialized/reset borrowed output. No forwarding factory ABI.
        std::mem::size_of::<Self>()
            + Self::terminal_bytes()
            + 2 * std::mem::size_of::<Option<&Option<CallFact<'_>>>>()
    }
    pub(super) const fn terminal_bytes() -> usize {
        // At most one first failure can initialize the full Diagnostic, Some
        // output, and owned message sum. Backing remains the original prepaid
        // refusal String; no new allowance or allocation is introduced.
        std::mem::size_of::<Diagnostic>()
            + std::mem::size_of::<Option<Diagnostic>>()
            + std::mem::size_of::<std::borrow::Cow<'static, str>>()
    }
}

// A complete first Diagnostic moves to the destination; a second refusal never
// replaces it or consumes its String again. The renderer and backing rules are
// the immediate ordinary budget_refusal's, including its detached legacy arm.
macro_rules! refuse_into {
    ($budget:ident $(.$budget_field:ident)*, $function:ident $(.$function_field:ident)*, $slot:ident, $reason:expr, $phase:expr) => {{
        if $slot.error.is_none() {
            let reason: &str = $reason;
            let phase: &str = $phase;
            #[cfg(test)]
            if let Some(lease) = $budget$(.$budget_field)*.oracle_lease().filter(|lease| !lease.refused()) {
                lease.fail_physical("oracle replay budget refused physical construction");
            }
            if $budget$(.$budget_field)*.payer == $crate::cleanup_plan::replay::AllocationPayer::LegacyUnobserved
                || ($budget$(.$budget_field)*.payer == $crate::cleanup_plan::replay::AllocationPayer::SelectedReservation
                    && $budget$(.$budget_field)*.refusal_text.is_none()
                    && !$budget$(.$budget_field)*.refusal_spent)
            {
                $slot.error = Some(replay_error($function$(.$function_field)*, format_args!("{reason}{phase}")));
            } else {
                $budget$(.$budget_field)*.refusal_spent = true;
                $budget$(.$budget_field)*.remaining = 0;
                $budget$(.$budget_field)*.skeleton_remaining = 0;
                if let Some(physical) = $budget$(.$budget_field)*.physical.as_deref_mut() {
                    physical.reject();
                }
                if let Some(mut text) = $budget$(.$budget_field)*.refusal_text.take() {
                    let phase = if phase.len()
                        <= crate::cleanup_plan::replay::physical_budget::REFUSAL_PHASE_BYTES
                    {
                        phase
                    } else {
                        "internal operation label exceeds the prepaid refusal bound"
                    };
                    let pieces = [
                        "cleanup plan for function `",
                        $function$(.$function_field)*.id.as_str(),
                        "` failed independent replay: ",
                        reason,
                        phase,
                    ];
                    let required = pieces
                        .iter()
                        .try_fold(0usize, |n, part| n.checked_add(part.len()));
                    if required.is_some_and(|n| n <= text.capacity()) {
                        for part in pieces {
                            text.push_str(part);
                        }
                    } else {
                        text.push_str(
                            "cleanup replay internal refusal text exceeds its prepaid backing",
                        );
                    }
                    $slot.error = Some(Diagnostic {
                        code: "SPX-H006",
                        severity: crate::diagnostic::Severity::Error,
                        message: std::borrow::Cow::Owned(text),
                        path: None,
                        span: None,
                        help: None,
                    });
                } else {
                    $slot.error = Some(Diagnostic {
                        code: "SPX-H006",
                        severity: crate::diagnostic::Severity::Error,
                        message: std::borrow::Cow::Owned(String::new()),
                        path: None,
                        span: None,
                        help: None,
                    });
                }
            }
        }
        false
    }};
}

macro_rules! prepare_into {
    ($budget:ident $(.$budget_field:ident)*, $function:ident $(.$function_field:ident)*, $slot:ident) => {{
        'prepare: {
            if $slot.error.is_some() {
                break 'prepare false;
            }
            if !std::ptr::eq($slot.function, $function$(.$function_field)*) || !std::ptr::eq($slot.budget, &*$budget$(.$budget_field)*) {
                break 'prepare refuse_into!(
                    $budget$(.$budget_field)*,
                    $function$(.$function_field)*,
                    $slot,
                    "ordinary replay receiving source or payer is outside its current invocation",
                    ""
                );
            }
            if !$budget$(.$budget_field)*.pays_builder() {
                break 'prepare true;
            }
            if $budget$(.$budget_field)*.refusal_spent
                || match $budget$(.$budget_field)*.payer {
                    #[cfg(test)]
                    $crate::cleanup_plan::replay::AllocationPayer::Oracle(lease) => lease.refused(),
                    _ => crate::bounded_output::active_refused(),
                }
            {
                break 'prepare refuse_into!(
                    $budget$(.$budget_field)*,
                    $function$(.$function_field)*,
                    $slot,
                    "ordinary builder already failed",
                    ""
                );
            }
            let Some(capacity) = $function$(.$function_field)*.id.as_str().len().checked_add(512) else {
                break 'prepare refuse_into!(
                    $budget$(.$budget_field)*,
                    $function$(.$function_field)*,
                    $slot,
                    "ordinary replay refusal layout overflowed",
                    ""
                );
            };
            if $budget$(.$budget_field)*
                .refusal_text
                .as_ref()
                .is_some_and(|text| text.capacity() >= capacity)
            {
                break 'prepare true;
            }
            if !$budget$(.$budget_field)*.reserve_builder(capacity) {
                break 'prepare refuse_into!(
                    $budget$(.$budget_field)*,
                    $function$(.$function_field)*,
                    $slot,
                    "ordinary replay refusal reservation exhausted",
                    ""
                );
            }
            let text = String::with_capacity(capacity);
            if text.capacity() != capacity {
                break 'prepare refuse_into!(
                    $budget$(.$budget_field)*,
                    $function$(.$function_field)*,
                    $slot,
                    "ordinary replay refusal capacity differs",
                    ""
                );
            }
            $budget$(.$budget_field)*.refusal_text = Some(text);
            true
        }
    }};
}

macro_rules! reserve_into {
    ($budget:ident $(.$budget_field:ident)*, $function:ident $(.$function_field:ident)*, $slot:ident, $bytes:expr, $phase:expr) => {{
        let bytes: usize = $bytes;
        let phase: &str = $phase;
        #[cfg(test)]
        let ticket = crate::cleanup_plan::replay::merge_census::before($budget$(.$budget_field)*, $function$(.$function_field)*, bytes, phase);
        let accepted = 'reserve: {
            if $slot.error.is_some() {
                break 'reserve false;
            }
            // Zero authentication retains its exact non-bootstrap behavior.
            if bytes == 0 {
                if !std::ptr::eq($slot.function, $function$(.$function_field)*)
                || !std::ptr::eq($slot.budget, &*$budget$(.$budget_field)*)
     {
                    break 'reserve refuse_into!($budget$(.$budget_field)*, $function$(.$function_field)*, $slot,
                        "ordinary replay receiving source or payer is outside its current invocation", "");
                }
                let admitted = match $budget$(.$budget_field)*.payer {
                    $crate::cleanup_plan::replay::AllocationPayer::LegacyUnobserved => true,
                    $crate::cleanup_plan::replay::AllocationPayer::ActiveBuilder => {
                        if $budget$(.$budget_field)*.refusal_spent || crate::bounded_output::active_refused() {
                            break 'reserve refuse_into!($budget$(.$budget_field)*, $function$(.$function_field)*, $slot, "ordinary builder already failed", "");
                        }
                        crate::bounded_output::reserve_active_required(0)
                    }
                    #[cfg(test)]
                    $crate::cleanup_plan::replay::AllocationPayer::Oracle(lease) => {
                        if $budget$(.$budget_field)*.refusal_spent || lease.refused() {
                            break 'reserve refuse_into!($budget$(.$budget_field)*, $function$(.$function_field)*, $slot, "ordinary builder already failed", "");
                        }
                        lease.charge(0).is_ok()
                    }
                    $crate::cleanup_plan::replay::AllocationPayer::SelectedReservation => $budget$(.$budget_field)*.physical.as_deref_mut()
                        .expect("selected reservation payer").reserve(0).is_ok(),
                };
                break 'reserve if admitted { true } else {
                    refuse_into!($budget$(.$budget_field)*, $function$(.$function_field)*, $slot,
                        "cleanup replay physical reservation exhausted during ", phase)
                };
            }
            if !prepare_into!($budget$(.$budget_field)*, $function$(.$function_field)*, $slot) {
                break 'reserve false;
            }
            let accepted = match $budget$(.$budget_field)*.payer {
                $crate::cleanup_plan::replay::AllocationPayer::LegacyUnobserved => true,
                $crate::cleanup_plan::replay::AllocationPayer::ActiveBuilder => !$budget$(.$budget_field)*.refusal_spent
                    && !crate::bounded_output::active_refused()
                    && crate::bounded_output::reserve_active_required(bytes),
                #[cfg(test)]
                $crate::cleanup_plan::replay::AllocationPayer::Oracle(lease) => !$budget$(.$budget_field)*.refusal_spent && !lease.refused()
                    && lease.charge(bytes).is_ok(),
                $crate::cleanup_plan::replay::AllocationPayer::SelectedReservation => $budget$(.$budget_field)*.physical.as_deref_mut()
                    .expect("selected reservation payer").reserve(bytes).is_ok(),
            };
            if accepted { true } else {
                refuse_into!($budget$(.$budget_field)*, $function$(.$function_field)*, $slot,
                    "cleanup replay physical reservation exhausted during ", phase)
            }
        };
        #[cfg(test)]
        crate::cleanup_plan::replay::merge_census::after(ticket, $budget$(.$budget_field)*, $function$(.$function_field)*, accepted);
        accepted
    }};
}

macro_rules! charge_into {
    ($budget:ident $(.$budget_field:ident)*, $function:ident $(.$function_field:ident)*, $slot:ident, $units:expr, $phase:expr) => {{
        'charge: {
            if !prepare_into!($budget$(.$budget_field)*, $function$(.$function_field)*, $slot) {
                break 'charge false;
            }
            if $budget$(.$budget_field)*.refusal_spent {
                break 'charge refuse_into!(
                    $budget$(.$budget_field)*,
                    $function$(.$function_field)*,
                    $slot,
                    "selected replay budget already failed",
                    ""
                );
            }
            let units: usize = $units;
            let phase: &str = $phase;
            #[cfg(test)]
            if let $crate::cleanup_plan::replay::AllocationPayer::Oracle(lease) = $budget$(.$budget_field)*.payer {
                if lease.charge_work(units).is_err() {
                    break 'charge refuse_into!(
                        $budget$(.$budget_field)*,
                        $function$(.$function_field)*,
                        $slot,
                        "oracle account work allowance refused during ",
                        phase
                    );
                }
            }
            if $budget$(.$budget_field)*.lent_skeleton {
                let Some(spent) = $budget$(.$budget_field)*.skeleton_spent.checked_add(units) else {
                    break 'charge refuse_into!(
                        $budget$(.$budget_field)*,
                        $function$(.$function_field)*,
                        $slot,
                        "cleanup replay skeleton debit count overflowed",
                        ""
                    );
                };
                let Some(remaining) = $budget$(.$budget_field)*.skeleton_remaining.checked_sub(units) else {
                    break 'charge refuse_into!(
                        $budget$(.$budget_field)*,
                        $function$(.$function_field)*,
                        $slot,
                        "cleanup replay work budget exhausted during ",
                        phase
                    );
                };
                $budget$(.$budget_field)*.skeleton_remaining = remaining;
                $budget$(.$budget_field)*.skeleton_spent = spent;
            } else if let Some(remaining) = $budget$(.$budget_field)*.remaining.checked_sub(units) {
                $budget$(.$budget_field)*.remaining = remaining;
            } else {
                #[cfg(test)]
                REPLAY_WORK_REFUSAL.with(|record| {
                    record.set(Some(ReplayWorkRefusal {
                        caller: std::panic::Location::caller(),
                        requested: units,
                        remaining: $budget$(.$budget_field)*.remaining,
                        status_sources: $function$(.$function_field)*.cleanup_plan.status_sources.len(),
                        first_status_bytes: $function$(.$function_field)*
                            .cleanup_plan
                            .status_sources
                            .first()
                            .map_or(0, |source| source.id.expression.as_str().len()),
                        payer: match $budget$(.$budget_field)*.payer {
                            $crate::cleanup_plan::replay::AllocationPayer::LegacyUnobserved => "unobserved",
                            $crate::cleanup_plan::replay::AllocationPayer::ActiveBuilder => "builder",
                            $crate::cleanup_plan::replay::AllocationPayer::SelectedReservation => "selected",
                            $crate::cleanup_plan::replay::AllocationPayer::Oracle(_) => "oracle",
                        },
                    }))
                });
                break 'charge refuse_into!(
                    $budget$(.$budget_field)*,
                    $function$(.$function_field)*,
                    $slot,
                    "cleanup replay work budget exhausted during ",
                    phase
                );
            }
            true
        }
    }};
}

pub(super) use {charge_into, prepare_into, refuse_into, reserve_into};

#[cfg(test)]
mod caller_before;

#[cfg(test)]
thread_local! {
    static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(super) fn saved() -> bool {
    BEFORE.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn with_before<T>(operation: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            BEFORE.with(|flag| flag.set(self.0));
        }
    }
    let _reset = Reset(BEFORE.with(|flag| flag.replace(true)));
    operation()
}
#[cfg(test)]
pub(super) use caller_before::hir_skeleton_paths_with_index_storage_before;

// Detached test observations, never a producer guard, budget or replay proof.
// A saturated observation is explicitly incomplete and cannot support totals.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Census {
    pub(super) reached: [usize; 4], // setup, lookup owner, status query, edge query
    pub(super) incomplete: bool,
}
#[cfg(test)]
thread_local! {
    static CENSUS: std::cell::Cell<Census> = const { std::cell::Cell::new(Census {
        reached: [0; 4], incomplete: false,
    }) };
}
#[cfg(test)]
pub(super) fn observe_site(site: usize) {
    CENSUS.with(|cell| {
        let mut value = cell.get();
        match value.reached[site].checked_add(1) {
            Some(count) => value.reached[site] = count,
            None => value.incomplete = true,
        }
        cell.set(value);
    });
}
#[cfg(test)]
pub(super) fn peek_census() -> Census {
    CENSUS.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn take_census() -> Census {
    CENSUS.with(|cell| cell.replace(Census::default()))
}

#[cfg(test)]
mod tests;
