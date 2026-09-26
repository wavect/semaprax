//! Agent Stage Semantic Work v1 instrumentation for Core Wasm.
//!
//! Only the private Agent-stage executor selects this, through
//! [`with_semantic_metering`] around one owned-data package build. Every
//! other emission leaves the scoped selection empty and stays byte-identical.
//!
//! The meter charges the same semantic points as the interpreter and native
//! C11: one unit when a metered source function body is entered (before its
//! preconditions) and one when a `while` body is entered after its condition
//! evaluated `true`. A refused charge sets the sticky exhaustion global,
//! settles every live compiler-owned slot of the frame in the same canonical
//! union finalizer order native C11 uses for every failure, and returns the
//! private status [`SEMANTIC_FUEL_STATUS`] through the ordinary status lane;
//! callers then settle through their planned call-failure exits. Performed
//! plan finalizers are appended to exported event globals in execution order.
//! The executor reads these exported globals from the one instance it
//! created; nothing here reaches a host import or a new authority.

use std::cell::RefCell;

use super::*;

/// The private status a refused semantic charge propagates. It lies outside
/// the public `1..=10` arithmetic/contract status range on purpose, so no
/// public status is reused for fuel exhaustion.
pub(crate) const SEMANTIC_FUEL_STATUS: i32 = 12;
/// Fixed capacity of the performed-finalizer event globals. Overflow is
/// sticky and the executor refuses the whole observation.
pub(crate) const SEMANTIC_EVENT_CAPACITY: u32 = 64;
/// Exported global names, in global-index order after the module's own.
pub(crate) const FUEL_USED_EXPORT: &str = "spx_semantic_fuel_used";
pub(crate) const EXHAUSTED_EXPORT: &str = "spx_semantic_fuel_exhausted";
pub(crate) const EVENT_COUNT_EXPORT: &str = "spx_semantic_event_count";
pub(crate) const EVENT_OVERFLOW_EXPORT: &str = "spx_semantic_event_overflow";
pub(crate) const EVENT_EXPORT_PREFIX: &str = "spx_semantic_event_";
const FIXED_GLOBALS: u32 = 4;

/// The metering selected for one owned-data package build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WasmSemanticMetering {
    /// Semantic fuel units the module may charge.
    pub(crate) fuel_limit: u64,
    /// Metered monomorphic source functions and their event ordinals. An
    /// injected executor driver is absent from this map and never charges.
    pub(crate) functions: BTreeMap<DeclarationId, u32>,
}

struct Selection {
    metering: WasmSemanticMetering,
    global_base: Option<u32>,
}

thread_local! {
    static SELECTION: RefCell<Option<Selection>> = const { RefCell::new(None) };
}

struct Scope;

impl Drop for Scope {
    fn drop(&mut self) {
        SELECTION.with(|selection| selection.borrow_mut().take());
    }
}

/// Run `build` with `metering` selected for this thread's owned-data Wasm
/// emission. The selection is cleared on return and on unwind; nesting is
/// refused.
pub(crate) fn with_semantic_metering<T>(
    metering: WasmSemanticMetering,
    build: impl FnOnce() -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    SELECTION.with(|selection| {
        let mut selection = selection.borrow_mut();
        if selection.is_some() {
            return Err(error("semantic metering selection is already active"));
        }
        *selection = Some(Selection {
            metering,
            global_base: None,
        });
        Ok(())
    })?;
    let _scope = Scope;
    build()
}

fn selected<T>(read: impl FnOnce(&Selection) -> T) -> Option<T> {
    SELECTION.with(|selection| selection.borrow().as_ref().map(read))
}

/// Number of globals the selection appends (zero when unmetered).
pub(super) fn global_count() -> u32 {
    selected(|_| FIXED_GLOBALS + SEMANTIC_EVENT_CAPACITY).unwrap_or(0)
}

/// Append the meter globals at `base`, the module's next global index.
pub(super) fn append_globals(globals: &mut Vec<u8>, base: u32) {
    let appended = SELECTION.with(|selection| {
        let mut selection = selection.borrow_mut();
        let Some(selection) = selection.as_mut() else {
            return false;
        };
        selection.global_base = Some(base);
        true
    });
    if !appended {
        return;
    }
    globals.extend([I64, 0x01, 0x42, 0x00, 0x0b]);
    for _ in 1..FIXED_GLOBALS {
        globals.extend([I32, 0x01, 0x41, 0x00, 0x0b]);
    }
    for _ in 0..SEMANTIC_EVENT_CAPACITY {
        globals.extend([I64, 0x01, 0x42, 0x00, 0x0b]);
    }
}

/// Number of exports the selection appends (zero when unmetered).
pub(super) fn export_count() -> u32 {
    global_count()
}

/// Export every meter global by its fixed name.
pub(super) fn append_exports(exports: &mut Vec<u8>) {
    let Some(Some(base)) = selected(|selection| selection.global_base) else {
        return;
    };
    for (offset, name) in [
        FUEL_USED_EXPORT,
        EXHAUSTED_EXPORT,
        EVENT_COUNT_EXPORT,
        EVENT_OVERFLOW_EXPORT,
    ]
    .into_iter()
    .enumerate()
    {
        write_name(exports, name);
        exports.push(0x03);
        write_u32(exports, base + offset as u32);
    }
    for index in 0..SEMANTIC_EVENT_CAPACITY {
        write_name(exports, &format!("{EVENT_EXPORT_PREFIX}{index}"));
        exports.push(0x03);
        write_u32(exports, base + FIXED_GLOBALS + index);
    }
}

impl Emitter<'_> {
    /// The metered ordinal and global base of the function being emitted.
    fn semantic_site(&self) -> Result<Option<(u32, u32)>, Diagnostic> {
        let Some((ordinal, base)) = selected(|selection| {
            (
                selection.metering.functions.get(&self.function.id).copied(),
                selection.global_base,
            )
        }) else {
            return Ok(None);
        };
        let Some(ordinal) = ordinal else {
            return Ok(None);
        };
        let base = base.ok_or_else(|| error("semantic metering globals were not appended"))?;
        Ok(Some((ordinal, base)))
    }

    /// Charge one semantic unit at the current point of a metered function.
    pub(super) fn semantic_charge(&mut self) -> Result<(), Diagnostic> {
        let Some((_, base)) = self.semantic_site()? else {
            return Ok(());
        };
        let limit = selected(|selection| selection.metering.fuel_limit)
            .ok_or_else(|| error("semantic metering selection disappeared"))?;
        let limit = i64::try_from(limit).map_err(|_| error("semantic fuel limit overflows i64"))?;
        let (used, exhausted) = (base, base + 1);
        self.output.push(0x23);
        write_u32(self.output, exhausted);
        self.output.push(0x23);
        write_u32(self.output, used);
        self.output.push(0x42);
        write_i64(self.output, limit);
        self.output.extend([0x5a, 0x72]); // i64.ge_u, i32.or
        self.output.extend([0x04, 0x40]);
        self.output.extend([0x41, 0x01, 0x24]);
        write_u32(self.output, exhausted);
        self.output.push(0x41);
        write_i64(self.output, i64::from(SEMANTIC_FUEL_STATUS));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        let actions = union_finalizer_order(self.cleanup_plan)?;
        self.emit_cleanup_actions(&actions)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        self.output.push(0x23);
        write_u32(self.output, used);
        self.output.extend([0x42, 0x01, 0x7c, 0x24]);
        write_u32(self.output, used);
        Ok(())
    }

    /// Append one performed finalizer event. Emitted inside the finalizer's
    /// own liveness guard, so only an executed finalizer is recorded.
    pub(super) fn semantic_cleanup_event(&mut self, flag: u32) -> Result<(), Diagnostic> {
        let Some((ordinal, base)) = self.semantic_site()? else {
            return Ok(());
        };
        let count = base + 2;
        let overflow = base + 3;
        let event = i64::try_from((u64::from(ordinal) << 32) | u64::from(flag))
            .map_err(|_| error("semantic cleanup event overflows i64"))?;
        let capacity = SEMANTIC_EVENT_CAPACITY;
        self.output.push(0x23);
        write_u32(self.output, count);
        self.output.push(0x41);
        write_i64(self.output, i64::from(capacity));
        self.output.extend([0x4f, 0x04, 0x40, 0x41, 0x01, 0x24]); // ge_u, if, overflow = 1
        write_u32(self.output, overflow);
        self.output.push(0x05); // else
        for _ in 0..=capacity {
            self.output.extend([0x02, 0x40]);
        }
        self.output.push(0x23);
        write_u32(self.output, count);
        self.output.push(0x0e);
        write_u32(self.output, capacity);
        for label in 0..capacity {
            write_u32(self.output, label);
        }
        write_u32(self.output, capacity - 1);
        for index in 0..capacity {
            self.output.push(0x0b); // end of case block `index`
            self.output.push(0x42);
            write_i64(self.output, event);
            self.output.push(0x24);
            write_u32(self.output, base + FIXED_GLOBALS + index);
            self.output.push(0x0c);
            write_u32(self.output, capacity - 1 - index);
        }
        self.output.push(0x0b); // end of the completion block
        self.output.push(0x23);
        write_u32(self.output, count);
        self.output.extend([0x41, 0x01, 0x6a, 0x24]);
        write_u32(self.output, count);
        self.output.push(0x0b); // end if
        Ok(())
    }
}

/// The canonical union order of every terminal exit's finalizers: the same
/// deterministic precedence-preserving topological order native C11 uses for
/// its shared failure epilogue. Ties break by cleanup place.
fn union_finalizer_order(
    plan: &crate::cleanup_plan::CleanupPlan,
) -> Result<Vec<crate::cleanup_plan::FinalizeAction>, Diagnostic> {
    use crate::cleanup_plan::{CleanupPlace, ExitContinuation, FinalizeAction};
    let mut nodes = BTreeMap::<CleanupPlace, FinalizeAction>::new();
    let mut successors = BTreeMap::<CleanupPlace, BTreeSet<CleanupPlace>>::new();
    let mut indegree = BTreeMap::<CleanupPlace, usize>::new();
    for exit in &plan.exits {
        if matches!(exit.continuation, ExitContinuation::Continue(_)) {
            continue;
        }
        for action in &exit.finalize_in_order {
            if let Some(existing) = nodes.get(&action.source) {
                if existing != action {
                    return Err(error("semantic cleanup place has divergent finalizers"));
                }
            }
            nodes.insert(action.source.clone(), action.clone());
            indegree.entry(action.source.clone()).or_insert(0);
        }
        for pair in exit.finalize_in_order.windows(2) {
            if successors
                .entry(pair[0].source.clone())
                .or_default()
                .insert(pair[1].source.clone())
            {
                *indegree.entry(pair[1].source.clone()).or_insert(0) += 1;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(place, degree)| (*degree == 0).then_some(place.clone()))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(place) = ready.pop_first() {
        for successor in successors.get(&place).into_iter().flatten() {
            let degree = indegree
                .get_mut(successor)
                .ok_or_else(|| error("semantic finalizer precedence node is missing"))?;
            *degree = degree
                .checked_sub(1)
                .ok_or_else(|| error("semantic finalizer precedence underflow"))?;
            if *degree == 0 {
                ready.insert(successor.clone());
            }
        }
        order.push(nodes[&place].clone());
    }
    if order.len() != nodes.len() {
        return Err(error(
            "semantic cleanup exits contain contradictory precedence",
        ));
    }
    Ok(order)
}
