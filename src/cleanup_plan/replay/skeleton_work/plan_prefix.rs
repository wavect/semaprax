//! Immutable PLAN histories for the allocation-unobserved legacy constructor.
//! Logical flat-vector metadata retains the original branch selection decisions.
use super::*;
use std::cell::RefCell;
use std::mem::size_of;
use std::rc::Rc;
type Scratch = Rc<RefCell<dag_storage::DestructionScratch>>;

pub(in crate::cleanup_plan::replay) struct History {
    observations: Observations,
    logical_len: usize,
    logical_capacity: usize,
    scratch: Option<Scratch>,
}
// The original Vec::clone constructs an exact-length vector. Immutable content
// sharing changes backing only; logical capacity still follows that old clone.
impl Clone for History {
    fn clone(&self) -> Self {
        Self {
            observations: self.observations.clone(),
            logical_len: self.logical_len,
            logical_capacity: self.logical_len,
            scratch: self.scratch.clone(),
        }
    }
}
impl ClonePayload for History {
    const NEEDS_TYPE_CURSOR: bool = false;
    fn clone_payload_bytes(&self) -> Option<usize> {
        self.logical_len.checked_mul(size_of::<Observation>())
    }
}
impl History {
    pub(in crate::cleanup_plan::replay) fn new() -> Self {
        Self {
            observations: Observations::default(),
            logical_len: 0,
            logical_capacity: 0,
            scratch: None,
        }
    }
    pub(in crate::cleanup_plan::replay) fn len(&self) -> usize {
        self.logical_len
    }
    pub(in crate::cleanup_plan::replay) fn capacity(&self) -> usize {
        self.logical_capacity
    }
    pub(in crate::cleanup_plan::replay) fn push(&mut self, observation: Observation) {
        if self.logical_len == self.logical_capacity {
            // This is the actual Rc-sized Vec growth policy used by the frozen
            // constructor. No logical buffer is allocated by this metadata.
            self.logical_capacity = self.logical_capacity.saturating_mul(2).max(4);
        }
        self.logical_len += 1;
        self.observations.items.push(observation);
    }
    pub(in crate::cleanup_plan::replay) fn freeze(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if self.observations.items.is_empty() {
            return Ok(());
        }
        // All original skeleton work remains in the constructor. Construction
        // and eventual iterative destruction spend additional ordinary work.

        budget.charge(function, 4, "PLAN immutable prefix node and release")?;
        budget.reserve_bytes(
            function,
            physical_clone::rc_backing_bytes::<ObservationDag>(),
            "PLAN immutable prefix node",
        )?;
        if self.scratch.is_none() {
            budget.reserve_bytes(
                function,
                INVOCATION_CONTROLS,
                "PLAN destruction frontier activation receiving",
            )?;
            self.scratch = Some(scratch(function, budget)?);
        }
        let kind = ObservationDagKind::Seq(
            self.observations.prefix.take(),
            std::mem::take(&mut self.observations.items),
        );
        // Every node has at most one child. The held one-slot frontier suffices
        // through the entire chain; no destructor allocates or recurses.
        self.observations.prefix = Some(Rc::new(ObservationDag {
            ordinal: Some(enumerated_prefix::SEQUENCE_MARKER),
            pending_bound: Some(1),
            kind,
            scratch: self.scratch.clone(),
        }));
        Ok(())
    }
}
impl From<History> for Observations {
    fn from(history: History) -> Self {
        history.observations
    }
}
pub(in crate::cleanup_plan::replay) const FREEZE_CONTROLS: usize = 6 * size_of::<History>()
    + 2 * size_of::<ObservationDag>()
    + 2 * size_of::<ObservationDagKind>()
    + 4 * size_of::<Rc<ObservationDag>>()
    + 2 * size_of::<Option<Rc<ObservationDag>>>()
    + 2 * size_of::<Vec<Observation>>()
    + 4 * size_of::<Scratch>()
    + 2 * size_of::<Option<Scratch>>()
    + 2 * size_of::<(
        &mut History,
        &ResolvedFunction,
        &mut ReplayBudget,
        SkeletonObservation,
        &str,
    )>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&ResolvedFunction>()
    + 4 * size_of::<Result<(), Diagnostic>>();
pub(in crate::cleanup_plan::replay) const INVOCATION_CONTROLS: usize = FREEZE_CONTROLS
    + 2 * size_of::<Scratch>()
    + 2 * size_of::<dag_storage::DestructionScratch>()
    + 2 * size_of::<Result<Scratch, Diagnostic>>()
    + physical_clone::rc_backing_bytes::<RefCell<dag_storage::DestructionScratch>>()
    + size_of::<Rc<ObservationDag>>();
pub(in crate::cleanup_plan::replay) fn scratch(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Scratch, Diagnostic> {
    // This route cannot acquire a paid/selected/oracle account by accident.
    assert!(matches!(budget.payer, AllocationPayer::LegacyUnobserved));
    budget.reserve_bytes(
        function,
        FREEZE_CONTROLS
            + 2 * size_of::<Scratch>()
            + 2 * size_of::<dag_storage::DestructionScratch>()
            + physical_clone::rc_backing_bytes::<RefCell<dag_storage::DestructionScratch>>()
            + size_of::<Rc<ObservationDag>>(),
        "PLAN immutable prefix invocation and destruction frontier",
    )?;
    Ok(Rc::new(RefCell::new(dag_storage::DestructionScratch {
        pending: Vec::with_capacity(1),
        registered_edges: 0,
        next_ordinal: 0,
    })))
}
pub(in crate::cleanup_plan::replay) fn push(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    target: &mut History,
    value: SkeletonObservation,
    phase: &str,
) -> Result<(), Diagnostic> {
    // Exactly the original push work and conversion, before the same Rc value.
    budget.charge_skeleton(function, 1, phase)?;
    note_skeleton_materialization();
    budget.reserve_vec(function, &mut target.observations.items, 1, phase)?;
    budget.reserve_bytes(
        function,
        physical_clone::conversion_bytes::<Observation, _>(&value),
        phase,
    )?;
    target.push(value.into());
    Ok(())
}

// Admission precedes every newly formed actual/formal History receiver. These
// lexical adapters introduce no unpaid helper argument or Result forwarding.
macro_rules! freeze {
    ($target:ident, $function:ident, $budget:ident) => {{
        $budget.reserve_bytes(
            $function,
            $crate::cleanup_plan::replay::skeleton_work::plan_prefix::FREEZE_CONTROLS,
            "PLAN immutable prefix receiving",
        )?;
        $target.freeze($function, $budget)?;
    }};
}
pub(in crate::cleanup_plan::replay) use freeze;
macro_rules! push {
    ($budget:ident, $function:ident, $target:expr, $value:expr, $phase:expr $(,)?) => {{
        $budget.reserve_bytes(
            $function,
            $crate::cleanup_plan::replay::skeleton_work::plan_prefix::FREEZE_CONTROLS,
            "PLAN mutable suffix receiving",
        )?;
        $crate::cleanup_plan::replay::skeleton_work::plan_prefix::push(
            $budget, $function, $target, $value, $phase,
        )
    }};
}
pub(in crate::cleanup_plan::replay) use push;

macro_rules! clone_history {
    ($budget:ident, $function:ident, $history:ident) => {{
        // The original clone's complete authorization precedes any new node.
        $budget.charge_skeleton($function, 1, "cleanup-plan skeleton branch clone")?;
        $budget.reserve_clone($function, &$history, "cleanup-plan skeleton branch clone")?;
        $crate::cleanup_plan::replay::note_skeleton_materialization();
        $crate::cleanup_plan::replay::skeleton_work::plan_prefix::freeze!(
            $history, $function, $budget
        );
        $history.clone()
    }};
}
pub(in crate::cleanup_plan::replay) use clone_history;
