//! Scoped, test-only deterministic work counters for agent-context queries.
//!
//! REF-09 and REF-10 replace repeated work with query-local reuse. Their
//! evidence is a count of the work each owning helper actually performs, not
//! a wall-clock claim. Every `record` call compiles to nothing outside
//! `cfg(test)`; under test it only counts while a `measure` scope on the same
//! thread is active, so concurrently running tests never share a counter.

/// One unit of countable agent-context work, recorded at its owning helper.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Work {
    /// A program-wide callable-membership view was constructed.
    CallableMembershipBuild,
    /// Declarations inserted into such a view.
    CallableMembershipEntry,
    /// A legacy call set was computed by walking a function or template.
    CallSetComputation,
    /// A graph payload schema was selected for the whole program.
    SchemaSelection,
    /// A complete agent-context response string was materialized.
    FullResponseMaterialization,
    /// Bytes copied into materialized complete responses.
    MaterializedResponseBytes,
    /// An exact response size was evaluated.
    ResponseSizeEvaluation,
    /// Envelope fragment bytes built while evaluating response sizes.
    EnvelopeFragmentBytes,
}

#[cfg(test)]
thread_local! {
    static ACTIVE: std::cell::RefCell<Option<std::collections::BTreeMap<Work, usize>>> =
        const { std::cell::RefCell::new(None) };
}

#[inline]
pub(super) fn record(work: Work, amount: usize) {
    #[cfg(test)]
    ACTIVE.with(|active| {
        if let Some(counts) = active.borrow_mut().as_mut() {
            let count = counts.entry(work).or_default();
            *count = count.saturating_add(amount);
        }
    });
    #[cfg(not(test))]
    let _ = (work, amount);
}

/// Run `operation` with a fresh counter scope and return what it recorded.
/// The previous scope, if any, is restored afterwards.
#[cfg(test)]
pub(super) fn measure<T>(
    operation: impl FnOnce() -> T,
) -> (T, std::collections::BTreeMap<Work, usize>) {
    let previous = ACTIVE.with(|active| active.replace(Some(Default::default())));
    let value = operation();
    let counts = ACTIVE
        .with(|active| active.replace(previous))
        .expect("the measured scope is still installed");
    (value, counts)
}

#[cfg(test)]
pub(super) fn count(counts: &std::collections::BTreeMap<Work, usize>, work: Work) -> usize {
    counts.get(&work).copied().unwrap_or(0)
}
