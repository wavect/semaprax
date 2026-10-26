//! Final lookup receiving from independent borrowed CFG rows. No attached row
//! determines capacity, membership, origins, or early semantic diagnostics.
use super::*;
use crate::hir::ResolvedFunction;
use crate::loan_plan::canonical_sink::{self, Source};

// These exact original roles are formed only by the nonempty append path.
// Their counts/tariffs are retained; None never constructs their carriers.
pub(super) const APPEND_PRESENT_BYTES: usize = 2 * std::mem::size_of::<(
    &crate::loan_plan::canonical_sink::Parts<'_>, Payer<'_>,
)>() + 2 * std::mem::size_of::<Result<usize, Diagnostic>>()
    + 4 * std::mem::size_of::<Scratch>()
    + 4 * std::mem::size_of::<Option<(usize, bool)>>()
    + 8 * std::mem::size_of::<usize>()
    + 16 * std::mem::size_of::<bool>();
// Only Option::is_some's actual/formal references, returned/received bool,
// and the branch/receiving usize are new. The original first reserve call
// retains its complete original payer/active/required/Result chain; there is
// no extra reserve invocation and no credit borrowed from another bank.
pub(super) const BRANCH_SELECTION_BYTES: usize = 2 * std::mem::size_of::<
    &Option<&mut crate::loan_plan::canonical_sink::Source<'_, '_>>,
>() + 2 * std::mem::size_of::<bool>() + 2 * std::mem::size_of::<usize>();
const EMPTY_CLEAR_BYTES: usize = 2 * std::mem::size_of::<&mut Vec<(LoanId, usize)>>()
    + 2 * std::mem::size_of::<&mut Vec<(LiveKey, Option<usize>, usize)>>()
    + 2 * std::mem::size_of::<&mut Vec<(IdKey, LoanId, usize)>>();

pub(super) struct Receiver<'a, 's, 'p> {
    pub(super) storage: &'s mut Storage<'a>,
    pub(super) deferred: &'s mut Deferred<'a>,
    pub(super) function: &'a ResolvedFunction,
    pub(super) payer: Payer<'p>,
}
impl<'a, 'p> Receiver<'a, '_, 'p> {
    pub(super) fn receive(
        self,
        source: Option<&mut Source<'a, 'p>>,
        matched: bool,
    ) -> Result<(), Diagnostic> {
        let charge = if source.is_some() { APPEND_PRESENT_BYTES } else { EMPTY_CLEAR_BYTES };
        self.payer.reserve(charge)?;
        let _stage = crate::bounded_output::reservation_stage_guard(
            crate::bounded_output::ReservationStage::LoanCanonicalAppend,
        );
        let Some(source) = source else {
            self.deferred.compare_match(self.function, matched);
            // This is the producer's authenticated complete empty result,
            // after its schema/row comparison. Keep the second old zero check
            // and clear retained scratch without forming an empty plan or any
            // append/detach/probe/growth/sort carrier.
            self.payer.reserve(0)?;
            self.storage._scratch.local.clear();
            self.storage._scratch.liveness.clear();
            self.storage._scratch.ids.clear();
            #[cfg(test)]
            EMPTY_COUNTS.with(|counts| { let (empty, present) = counts.get(); counts.set((empty + 1, present)); });
            return Ok(());
        };
        #[cfg(test)]
        EMPTY_COUNTS.with(|counts| { let (empty, present) = counts.get(); counts.set((empty, present + 1)); });
        if !source.is_current(
            self.storage.program,
            self.function,
            match self.payer {
                Payer::Captured(owner) => owner,
                _ => unreachable!("stream requires captured owner"),
            },
        ) {
            return Err(hir_error(
                "canonical shared-loan source belongs to another program",
            ));
        }
        let mut scratch = std::mem::take(&mut self.storage._scratch);
        let result = append_source(
            self.storage,
            self.deferred,
            self.function,
            source,
            matched,
            self.payer,
            &mut scratch,
        );
        scratch.local.clear();
        scratch.liveness.clear();
        scratch.ids.clear();
        self.storage._scratch = scratch;
        result
    }
}

fn append_source<'a, 'p>(
    storage: &mut Storage<'a>,
    deferred: &mut Deferred<'a>,
    function: &'a ResolvedFunction,
    source: &mut Source<'a, 'p>,
    mut matched: bool,
    payer: Payer<'p>,
    scratch: &mut Scratch,
) -> Result<(), Diagnostic> {
    payer.reserve(0)?;
    let parts = source.parts();
    let base = storage.origins.len();
    scratch.local.clear();
    let local = &mut scratch.local;
    ensure_probe(local, source.loan_count(), payer)?;
    for offset in 0..source.loan_count() {
        let loan = source.loan(offset);
        local.push((
            loan.id,
            base.checked_add(offset)
                .ok_or_else(|| hir_error("canonical shared-loan origin directory overflows"))?,
        ));
    }
    local.sort_unstable();
    let origin = |id: &LoanId| {
        let end = local.partition_point(|entry| entry.0 <= *id);
        end.checked_sub(1)
            .and_then(|i| local.get(i))
            .filter(|entry| entry.0 == *id)
            .map(|entry| entry.1)
    };
    scratch.liveness.clear();
    let live_probe = &mut scratch.liveness;
    let upper = selected_capacity(&parts, payer)?;
    ensure_probe(live_probe, upper, payer)?;
    for (node, point) in parts.points.iter().enumerate() {
        // The exact old two merge producers, with their full original vector
        // allocation and malformed-row handling. No union fee is removed.
        let predecessors = &parts.predecessors[parts.predecessor_offsets[node] as usize
            ..parts.predecessor_offsets[node + 1] as usize];
        let live_before = canonical_sink::merge(
            predecessors
                .iter()
                .filter_map(|from| parts.edges.binary_search(&(*from, node as u16)).ok())
                .map(|edge| parts.live[edge].as_slice()),
            match payer {
                Payer::Captured(owner) => owner,
                _ => unreachable!("stream requires captured owner"),
            },
        )?;
        let live_after = canonical_sink::merge(
            (parts.successors[node]..parts.successors[node + 1])
                .map(|edge| parts.edges[edge as usize].1)
                .filter_map(|to| parts.edges.binary_search(&(node as u16, to)).ok())
                .map(|edge| parts.live[edge].as_slice()),
            match payer {
                Payer::Captured(owner) => owner,
                _ => unreachable!("stream requires captured owner"),
            },
        )?;
        // Compare all fields and preserve order even after a prior mismatch.
        if let Some(endpoint) = function.loan_plan.endpoints.get(node) {
            matched &= endpoint.point == *point;
            matched &= endpoint.live_before.as_ref() == live_before.as_slice();
            matched &= endpoint.starts.as_ref() == parts.starts[node].as_slice();
            matched &= endpoint.kills.as_ref() == parts.kills[node].as_slice();
            matched &= endpoint.live_after.as_ref() == live_after.as_slice();
        } else {
            matched = false;
        }
        let live = match point.phase {
            LoanPointPhase::Before => &live_before,
            LoanPointPhase::After => &live_after,
        };
        // Every selected union member belongs to one of the counted rows.
        // Duplicate edge memberships may overreserve; all upper backing was
        // paid once before emission, with no geometric replacement credit.
        for loan in live {
            let ordinal = live_probe.len();
            live_probe.push((
                (point.expression.clone(), point.phase, *loan),
                origin(loan),
                ordinal,
            ));
        }
    }
    for (index, (from, to)) in parts.edges.iter().enumerate() {
        if let Some(edge) = function.loan_plan.edges.get(index) {
            matched &= edge.from == *from;
            matched &= edge.to == *to;
            matched &= edge.live.as_ref() == parts.live[index].as_slice();
        } else {
            matched = false;
        }
    }
    deferred.compare_match(function, matched);
    live_probe.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
    // Select exactly the earliest original source occurrence. Sorting the
    // private lookup index never reorders canonical rows or error precedence.
    let mut first_bad: Option<(usize, bool)> = None;
    for (index, entry) in live_probe.iter().enumerate() {
        let unknown = entry.1.is_none();
        let duplicate =
            storage.live(&entry.0).is_some() || (index != 0 && live_probe[index - 1].0 == entry.0);
        if unknown || duplicate {
            // Additional hostile-selection tuple/Option/comparison roles are
            // paid before forming the new candidate; no diagnostic is funded
            // by the attachment or an unrelated active ledger.
            payer.reserve(
                4 * std::mem::size_of::<Option<(usize, bool)>>()
                    + 4 * std::mem::size_of::<usize>()
                    + 8 * std::mem::size_of::<bool>(),
            )?;
            if first_bad.is_none_or(|prior| entry.2 < prior.0) {
                first_bad = Some((entry.2, unknown));
            }
        }
    }
    if let Some((_, unknown)) = first_bad {
        return Err(hir_error(if unknown {
            "canonical shared-loan endpoint names an unknown loan"
        } else {
            "canonical shared-loan liveness is duplicated"
        }));
    }
    scratch.ids.clear();
    let id_probe = &mut scratch.ids;
    ensure_probe(id_probe, source.loan_count(), payer)?;
    for ordinal in 0..source.loan_count() {
        let loan = source.loan(ordinal);
        id_probe.push(((loan.site.clone(), loan.cause.clone()), loan.id, ordinal));
    }
    id_probe.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
    for ordinal in 0..source.loan_count() {
        let loan = source.loan(ordinal);
        let key = (loan.site.clone(), loan.cause.clone());
        let first = id_probe.partition_point(|entry| entry.0 < key);
        if loan.phase != LoanPointPhase::Before
            || storage.id(&key).is_some()
            || id_probe[first].2 < ordinal
        {
            return Err(hir_error(
                "canonical shared-loan identity is ambiguous or malformed",
            ));
        }
    }
    let id_needed = count(storage.ids.len(), id_probe.len(), payer)?;
    let live_needed = count(storage.liveness.len(), live_probe.len(), payer)?;
    let origin_needed = count(storage.origins.len(), source.loan_count(), payer)?;
    ensure(&mut storage.ids, id_needed, payer)?;
    ensure(&mut storage.liveness, live_needed, payer)?;
    ensure(&mut storage.origins, origin_needed, payer)?;
    storage
        .ids
        .extend(id_probe.drain(..).map(|(key, id, _)| (key, id)));
    storage.liveness.extend(
        live_probe
            .drain(..)
            .map(|(key, ordinal, _)| (key, ordinal.expect("checked every source endpoint"))),
    );
    // All immutable source loans end before consuming owned independent roots.
    source.move_origins(&mut storage.origins)?;
    storage.ids.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    storage.liveness.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    Ok(())
}

// This separately paid count-only pass needs no union/lookup directory. Each
// selected Before member comes from an incoming row, each selected After
// member from an outgoing row. Summing row lengths therefore bounds the
// deduplicated total even at joins, splits, terminal points and empty graphs.
fn selected_capacity(
    parts: &canonical_sink::Parts<'_>,
    payer: Payer<'_>,
) -> Result<usize, Diagnostic> {
    payer.reserve(
        4 * std::mem::size_of::<std::iter::Enumerate<std::slice::Iter<'_, Vec<LoanId>>>>()
            + 4 * std::mem::size_of::<Option<(usize, &Vec<LoanId>)>>()
            + 12 * std::mem::size_of::<usize>()
            + 8 * std::mem::size_of::<bool>()
            + 4 * std::mem::size_of::<Option<usize>>()
            + 4 * std::mem::size_of::<Result<usize, Diagnostic>>(),
    )?;
    let mut upper = 0usize;
    for (index, row) in parts.live.iter().enumerate() {
        let (from, to) = parts.edges[index];
        if parts.points[to as usize].phase == LoanPointPhase::Before {
            upper = upper
                .checked_add(row.len())
                .ok_or_else(|| payer.capacity(usize::MAX, 0))?;
        }
        if parts.points[from as usize].phase == LoanPointPhase::After {
            upper = upper
                .checked_add(row.len())
                .ok_or_else(|| payer.capacity(usize::MAX, 0))?;
        }
    }
    Ok(upper)
}

#[cfg(test)]
thread_local! { static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
pub(super) fn before_selected() -> bool {
    BEFORE.with(std::cell::Cell::get)
}
#[cfg(test)]
fn with_before_stream<T>(operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BEFORE.with(|slot| slot.set(self.0));
        }
    }
    let restore = Restore(BEFORE.with(|slot| slot.replace(true)));
    let result = operation();
    drop(restore);
    result
}
// Full 8086 caller AND delegated materializer; never relabel owned V2 as old.
#[cfg(test)]
pub(super) fn with_before<T>(operation: impl FnOnce() -> T) -> T {
    canonical_sink::with_materializer_stage(true, || with_before_stream(operation))
}
// Immediate owned-row producer before streaming, with the same complete old
// canonical caller/append. Force this stage even inside a nested 8086 mode.
#[cfg(test)]
fn with_owned_before<T>(operation: impl FnOnce() -> T) -> T {
    canonical_sink::with_materializer_stage(false, || with_before_stream(operation))
}
#[cfg(test)]
mod tests;

// Complete immediate receiver, including its original None append pipeline.
#[cfg(test)]
pub(super) struct BeforeEmptyReceiver<'a, 's, 'p> {
    pub(super) storage: &'s mut Storage<'a>,
    pub(super) deferred: &'s mut Deferred<'a>,
    pub(super) function: &'a ResolvedFunction,
    pub(super) payer: Payer<'p>,
}
#[cfg(test)]
impl<'a, 'p> BeforeEmptyReceiver<'a, '_, 'p> {
    pub(super) fn receive(
        self,
        source: Option<&mut Source<'a, 'p>>,
        matched: bool,
    ) -> Result<(), Diagnostic> {
        self.payer.reserve(0)?;
        let _stage = crate::bounded_output::reservation_stage_guard(
            crate::bounded_output::ReservationStage::LoanCanonicalAppend,
        );
        let Some(source) = source else {
            self.deferred.compare_match(self.function, matched);
            // Empty append still pays the old zero-request affinity check.
            return append(self.storage, LoanPlan::empty_v1(), self.payer);
        };
        if !source.is_current(
            self.storage.program,
            self.function,
            match self.payer {
                Payer::Captured(owner) => owner,
                _ => unreachable!("stream requires captured owner"),
            },
        ) {
            return Err(hir_error(
                "canonical shared-loan source belongs to another program",
            ));
        }
        let mut scratch = std::mem::take(&mut self.storage._scratch);
        let result = append_source(
            self.storage,
            self.deferred,
            self.function,
            source,
            matched,
            self.payer,
            &mut scratch,
        );
        scratch.local.clear();
        scratch.liveness.clear();
        scratch.ids.clear();
        self.storage._scratch = scratch;
        result
    }
}


#[cfg(test)]
thread_local! {
    static EMPTY_BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static EMPTY_COUNTS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}
#[cfg(test)]
pub(super) fn empty_before_selected() -> bool { EMPTY_BEFORE.with(|flag| flag.get()) }
#[cfg(test)]
fn with_before_empty_terminal<T>(operation: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset { fn drop(&mut self) { EMPTY_BEFORE.with(|flag| flag.set(self.0)); } }
    let reset = Reset(EMPTY_BEFORE.with(|flag| flag.replace(true)));
    let output = operation();
    drop(reset);
    output
}
#[cfg(test)]
fn take_empty_counts() -> (usize, usize) { EMPTY_COUNTS.with(|counts| counts.replace((0, 0))) }
