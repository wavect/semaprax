//! One invocation-owned immutable structural query entry, never path equality.
use super::*;
use crate::bounded_output::classifier_string::{ClassifierLedger, StorageParent};
use std::mem::size_of;
use std::rc::{Rc, Weak};

// Complete frozen pre-slot layout, independently instantiated by sizeof only.
// No old method forwards to a changed implementation through this passive type.
#[allow(dead_code, reason = "independent complete old Work layout tariff bound")]
#[rustfmt::skip]
struct BeforeWork<'a, 'b> {
    pub(super) continuations: Option<&'b mut continuation_storage::Storage<'a>>,
    pub(super) function: &'a ResolvedFunction,
    pub(super) budget: &'b mut ReplayBudget,
    pub(super) renewal_bindings: crate::cleanup_plan::renewal_profile::RenewalBindings<'a>,
    pub(super) active_copy_region: Option<ExpressionId>,
    pub(super) dag_scratch:
        Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>,
    pub(super) ordinary_scratch:
        Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>,
    pub(super) next_ordinary_dag: usize,
    pub(super) held_renewals: Option<&'a crate::cleanup_plan::renewal_profile::RenewalBindings<'a>>,
    pub(super) ordinary_index: Option<&'b crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    pub(super) ordinary_program: Option<&'a dyn crate::cleanup::ReplayContext>,
    /// Authored same-owner append and direct literal-format String moves.
    pub(super) string_owner_moves: BTreeSet<ExpressionId>,
    pub(super) string_condition_reads: BTreeSet<ExpressionId>,
    /// Authenticated execution identity of a lexical-profile function.
    pub(super) owner: Option<crate::hir::FunctionExecutionId>,
}

const WORK_GROWTH: usize = match size_of::<SkeletonWork<'static, 'static>>()
    .checked_sub(size_of::<BeforeWork<'static, 'static>>())
{
    Some(bytes) => bytes,
    None => panic!("new Work layout unexpectedly shrank"),
};
const RESULT_GROWTH: usize = match size_of::<Result<SkeletonWork<'static, 'static>, Diagnostic>>()
    .checked_sub(size_of::<Result<BeforeWork<'static, 'static>, Diagnostic>>())
{
    Some(bytes) => bytes,
    None => panic!("new Work Result layout unexpectedly shrank"),
};

type Node = Rc<ObservationDag>;
type RootIdentity = Weak<ObservationDag>;
// Retained Weak keeps the allocation/control block reserved against ABA,
// while the actual borrowed Rc supplies liveness. No upgrade or raw pointer
// alone grants a proof: Slot/Proof authenticate function, budget and ledger.
const IDENTITY: usize = 2 * size_of::<&RootIdentity>()
    + 2 * size_of::<&Node>()
    + 10 * size_of::<*const ObservationDag>()
    + 2 * size_of::<bool>();
type Pending = Vec<Node>;
const PAYMENT: usize = 2 * size_of::<(&ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<Result<(), Diagnostic>>();
// One affine terminal bank lives in Proof until its first authentication
// failure. An error destroys the removed owner instead of returning it to Work;
// successful probes retain this still-unspent bank. Fresh ACTIVE/zero-call
// receivers are separately renewed for EVERY actual probe below.
const TERMINAL: usize = StorageParent::TERMINAL_CONTROLS
    + 8 * size_of::<&usize>()
    + 2 * size_of::<Diagnostic>()
    + 2 * size_of::<String>();
const GUARD: usize = ClassifierLedger::current_receiving_controls()
    + 2 * size_of::<&Proof<'static>>() + 2 * size_of::<&SkeletonWork<'static, 'static>>()
    + 8 * size_of::<&usize>() // actual/formal thin pointer-equality loans
    + 16 * size_of::<bool>();
// Entry additionally observes the sticky zero-byte/work APIs. Their extra
// Results and actual/formal payment arguments are entry roles, not per-node.
const AUTH: usize = GUARD + 2 * PAYMENT + 2 * size_of::<Result<(), Diagnostic>>();
const ENTRY: usize = GUARD
    + 2 * PAYMENT
    + 2 * size_of::<Result<(), Diagnostic>>()
    + 4 * size_of::<&mut Slot<'static>>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<Option<usize>>()
    + 4 * size_of::<Option<Box<Proof<'static>>>>()
    + 4 * size_of::<Box<Proof<'static>>>()
    + 8 * size_of::<bool>();
// Constructor-selected physical carrier growth is separate from the neutral
// Slot payload. Work aggregate/live/Drop and Result return/receive roles are
// independently bounded; six Result growth carriers also cover the checked
// and test-Oracle tail-forwarding constructors. No old frame is a credit.
/// Every real constructor forms the new neutral slot, including saved callers.
pub(in crate::cleanup_plan::replay) const HEADER: usize = ENTRY
    + TERMINAL
    + 3 * WORK_GROWTH
    + 6 * RESULT_GROWTH
    + 3 * size_of::<Slot<'static>>()
    + ClassifierLedger::capture_into_controls()
    + 2 * size_of::<ClassifierLedger>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&ReplayBudget>()
    + 2 * size_of::<allocation::AllocationPayer>()
    + 2 * size_of::<AdmissionMode>()
    + 16 * size_of::<bool>()
    + PAYMENT;
const BEFORE_TAKE_CALL: usize = IDENTITY
    + AUTH
    + 2 * GUARD
    + 3 * PAYMENT
    + 4 * size_of::<&Observations>()
    + 4 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + 2 * size_of::<Result<Box<Proof<'static>>, Diagnostic>>()
    + 2 * size_of::<Result<bool, Diagnostic>>()
    + 4 * size_of::<Option<Box<Proof<'static>>>>()
    + 4 * size_of::<Box<Proof<'static>>>()
    + 4 * size_of::<&mut Option<Box<Proof<'static>>>>()
    + 4 * size_of::<&Proof<'static>>()
    + 4 * size_of::<bool>()
    + 4 * size_of::<&Node>()
    + 2 * size_of::<&Option<Entry>>()
    + 2 * size_of::<&Entry>()
    + 2 * size_of::<usize>();
// Only the take-function Result return/receiver disappears. The lexical
// block yields/receives an owning Box; all other original carriers remain.
pub(in crate::cleanup_plan::replay) const CALL: usize = BEFORE_TAKE_CALL
    - 2 * size_of::<Result<Box<Proof<'static>>, Diagnostic>>()
    + 2 * size_of::<Box<Proof<'static>>>();
const CREATE: usize = TERMINAL
    + 3 * size_of::<Proof<'static>>()
    + 4 * size_of::<Box<Proof<'static>>>()
    + 2 * size_of::<Option<Box<Proof<'static>>>>()
    + 2 * size_of::<ClassifierLedger>()
    + ClassifierLedger::capture_into_controls()
    + 4 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<*const ReplayBudget>()
    + 2 * size_of::<Option<Entry>>()
    + PAYMENT;
const STEP: usize = IDENTITY
    + GUARD
    + 3 * PAYMENT
    + 4 * size_of::<&mut Pending>()
    + 4 * size_of::<Option<Node>>()
    + 4 * size_of::<Node>()
    + 4 * size_of::<&Node>()
    + 2 * size_of::<&ObservationDag>()
    + 2 * size_of::<&ObservationDagKind>()
    + 2 * size_of::<&Option<Entry>>()
    + 2 * size_of::<&Entry>()
    + 2 * size_of::<usize>()
    + 4 * size_of::<Option<usize>>()
    + 4 * size_of::<bool>();
const REPLAY: usize = GUARD + PAYMENT + 4 * size_of::<usize>() + 2 * size_of::<bool>();
const CHILD: usize = GUARD
    + 2 * PAYMENT
    + 2 * size_of::<&mut std::slice::Iter<'static, Node>>()
    + 2 * size_of::<Option<&Node>>()
    + 4 * size_of::<&Node>()
    + 3 * size_of::<Node>()
    + 2 * size_of::<&mut Pending>();
const PUBLISH: usize = GUARD
    + PAYMENT
    + 3 * size_of::<Entry>()
    + 3 * size_of::<Option<Entry>>()
    + 4 * size_of::<RootIdentity>() // downgrade return, move, receiver and Drop
    + 2 * size_of::<&Node>();

struct Entry {
    root: RootIdentity,
    visits: usize,
}
// Private payload plus constructor-captured ledger is the lexical owner.
// Capturing a new current invocation at first query cannot rebind this slot.
pub(in crate::cleanup_plan::replay) struct Slot<'hir> {
    function: &'hir ResolvedFunction,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
    owner: Option<Box<Proof<'hir>>>,
    eligible: bool,
    failed: bool,
}
impl<'hir> Slot<'hir> {
    pub(in crate::cleanup_plan::replay) fn new(
        function: &'hir ResolvedFunction,
        budget: &ReplayBudget,
    ) -> Self {
        let eligible = matches!(
            budget.payer,
            allocation::AllocationPayer::ActiveBuilder
                | allocation::AllocationPayer::LegacyUnobserved
        ) && budget.admission == AdmissionMode::Ordinary;
        let mut ledger = ClassifierLedger::empty_receiving();
        if eligible {
            ledger.capture_current_into();
        }
        Self {
            function,
            budget,
            ledger,
            owner: None,
            eligible,
            failed: false,
        }
    }
    pub(in crate::cleanup_plan::replay) fn preflight(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        bytes: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if !self.eligible
            || self.failed
            || !std::ptr::eq(self.function, function)
            || !std::ptr::eq(self.budget, &*budget)
            || !self.ledger.current()
        {
            self.failed = true;
            drop(self.owner.take());
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        let Some(renewed) = bytes.checked_add(ENTRY) else {
            self.failed = true;
            drop(self.owner.take());
            return Err(Diagnostic::io("SPX-H006", String::new()));
        };
        // The retained first/current guard has already run. This authentic
        // payment funds helper arguments and the NEXT guard before formation.
        let result = budget.reserve_bytes(function, renewed, phase);
        if result.is_err() {
            self.failed = true;
            drop(self.owner.take());
        }
        result
    }
    // The predicate itself also runs on the exact old unsupported route.
    // Pointer-bound constructor loans authenticate that pricing boundary; an
    // unsupported slot grants no sequence-query capability and reads no ACTIVE.
    pub(in crate::cleanup_plan::replay) fn selector(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        bytes: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.failed
            || !std::ptr::eq(self.function, function)
            || !std::ptr::eq(self.budget, &*budget)
            || (self.eligible && !self.ledger.current())
        {
            self.failed = true;
            drop(self.owner.take());
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        let Some(renewed) = bytes.checked_add(ENTRY) else {
            self.failed = true;
            drop(self.owner.take());
            return Err(Diagnostic::io("SPX-H006", String::new()));
        };
        // The retained first/current guard has already run. This authentic
        // payment funds helper arguments and the NEXT guard before formation.
        let result = budget.reserve_bytes(function, renewed, phase);
        if result.is_err() {
            self.failed = true;
            drop(self.owner.take());
        }
        result
    }
    fn current(&self, work: &SkeletonWork<'_, '_>) -> bool {
        self.eligible
            && !self.failed
            && std::ptr::eq(self.function, work.function)
            && std::ptr::eq(self.budget, &*work.budget)
            && self.ledger.current()
    }
}

pub(in crate::cleanup_plan::replay) struct Proof<'hir> {
    function: &'hir ResolvedFunction,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
    last: Option<Entry>,
}

macro_rules! pay {
    ($proof:ident, $work:ident, $bytes:expr, $phase:literal) => {{
        if !$proof.current($work) {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        $work.budget.reserve_bytes($work.function, $bytes, $phase)?;
    }};
}
impl Proof<'_> {
    fn current(&self, work: &SkeletonWork<'_, '_>) -> bool {
        std::ptr::eq(self.function, work.function)
            && std::ptr::eq(self.budget, &*work.budget)
            && self.ledger.current()
    }
    fn authenticate(&self, work: &mut SkeletonWork<'_, '_>) -> Result<(), Diagnostic> {
        if !self.current(work) {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        // Every warm query authenticates live sticky state even when no
        // positive backing reservation will otherwise occur on that query.
        work.budget
            .reserve_bytes(work.function, 0, "structural query sticky authority")?;
        work.budget
            .charge(work.function, 0, "structural query sticky authority")
    }
}
macro_rules! take_owned {
    ($work:ident) => {{
        'query_owner: {
    let owner = $work.queries.owner.take();
    if !$work.queries.current($work) {
        $work.queries.failed = true;
        return Err(Diagnostic::io("SPX-H006", String::new()));
    }
    if let Some(proof) = owner {
        let result = proof.authenticate($work);
        if result.is_err() {
            $work.queries.failed = true;
        }
        result?;
        break 'query_owner proof;
    }
    $work.budget
        .reserve_bytes($work.function, CREATE, "structural query owner construction")?;
    let mut ledger = ClassifierLedger::empty_receiving();
    ledger.capture_current_into();
    let proof = Box::new(Proof {
        function: $work.function,
        budget: $work.budget,
        ledger,
        last: None,
    });
    let result = proof.authenticate($work);
    if result.is_err() {
        $work.queries.failed = true;
    }
    result?;
    proof
        }
    }};
}

#[cfg(test)]
fn take<'hir>(work: &mut SkeletonWork<'hir, '_>) -> Result<Box<Proof<'hir>>, Diagnostic> {
    let owner = work.queries.owner.take();
    if !work.queries.current(work) {
        work.queries.failed = true;
        return Err(Diagnostic::io("SPX-H006", String::new()));
    }
    if let Some(proof) = owner {
        let result = proof.authenticate(work);
        if result.is_err() {
            work.queries.failed = true;
        }
        result?;
        return Ok(proof);
    }
    work.budget
        .reserve_bytes(work.function, CREATE, "structural query owner construction")?;
    let mut ledger = ClassifierLedger::empty_receiving();
    ledger.capture_current_into();
    let proof = Box::new(Proof {
        function: work.function,
        budget: work.budget,
        ledger,
        last: None,
    });
    let result = proof.authenticate(work);
    if result.is_err() {
        work.queries.failed = true;
    }
    result?;
    Ok(proof)
}

// The caller funds CALL before forming helper arguments. No identity/value
// cache is used for equality, ordering, selected replay, or ordinary fallback.
macro_rules! sequence {
    ($observations:expr, $work:ident) => {{
        $work.queries.preflight(
            $work.function,
            &mut *$work.budget,
            $crate::cleanup_plan::replay::skeleton_work::query_proof::CALL,
            "shared sequence query call receivers",
        )?;
        $crate::cleanup_plan::replay::skeleton_work::query_proof::sequence_paid(
            $observations,
            $work,
        )
    }};
}
pub(in crate::cleanup_plan::replay) use sequence;
pub(in crate::cleanup_plan::replay) fn sequence_paid(
    observations: &Observations,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<bool, Diagnostic> {
    let mut proof = take_owned!(work);
    let result = classify(observations, work, &mut proof);
    // Failed/partial classification never republishes its invocation owner.
    if result.is_ok() {
        work.queries.owner = Some(proof);
    } else {
        work.queries.failed = true;
    }
    result
}
fn classify(
    observations: &Observations,
    work: &mut SkeletonWork<'_, '_>,
    proof: &mut Proof<'_>,
) -> Result<bool, Diagnostic> {
    let Some(root) = &observations.prefix else {
        return Ok(true);
    };
    classify_root(root, work, proof)
}
fn classify_root(
    root: &Node,
    work: &mut SkeletonWork<'_, '_>,
    proof: &mut Proof<'_>,
) -> Result<bool, Diagnostic> {
    if let Some(entry) = &proof.last {
        if std::ptr::eq(Weak::as_ptr(&entry.root), Rc::as_ptr(root)) {
            let count = entry.visits;
            let mut left = count;
            loop {
                pay!(
                    proof,
                    work,
                    REPLAY,
                    "shared sequence logical replay receivers"
                );
                if left == 0 {
                    break;
                }
                work.budget
                    .charge(work.function, 1, "complete HIR sequence validation")?;
                left -= 1;
            }
            return Ok(true);
        }
    }
    let mut pending = Pending::new();
    let mut visits = 0usize;
    pay!(
        proof,
        work,
        STEP + 2 * size_of::<Pending>(),
        "shared sequence cold frontier"
    );
    work.budget.reserve_vec(
        work.function,
        &mut pending,
        1,
        "shared sequence cold frontier backing",
    )?;
    pending.push(root.clone());
    loop {
        pay!(proof, work, STEP, "shared sequence query step");
        let Some(node) = pending.pop() else { break };
        if let Some(entry) = &proof.last {
            if std::ptr::eq(Weak::as_ptr(&entry.root), Rc::as_ptr(&node)) {
                let count = entry.visits;
                let mut left = count;
                loop {
                    pay!(
                        proof,
                        work,
                        REPLAY,
                        "shared sequence logical replay receivers"
                    );
                    if left == 0 {
                        break;
                    }
                    work.budget
                        .charge(work.function, 1, "complete HIR sequence validation")?;
                    left -= 1;
                }
                visits = visits.checked_add(count).ok_or_else(|| {
                    work.budget.budget_refusal(
                        work.function,
                        "complete HIR validation visit count overflowed",
                        "",
                    )
                })?;
                continue;
            }
        }
        work.budget
            .charge(work.function, 1, "complete HIR sequence validation")?;
        visits = visits.checked_add(1).ok_or_else(|| {
            work.budget.budget_refusal(
                work.function,
                "complete HIR validation visit count overflowed",
                "",
            )
        })?;
        match &node.kind {
            ObservationDagKind::Alt(_) => return Ok(false),
            ObservationDagKind::Seq(prefix, _) => {
                if let Some(prefix) = prefix {
                    work.budget.reserve_vec(
                        work.function,
                        &mut pending,
                        1,
                        "complete HIR sequence ancestor backing",
                    )?;
                    pending.push(prefix.clone());
                }
            }
            ObservationDagKind::Cat(children) => {
                work.budget.reserve_vec(
                    work.function,
                    &mut pending,
                    children.len(),
                    "complete HIR sequence concatenation backing",
                )?;
                pay!(
                    proof,
                    work,
                    GUARD
                        + PAYMENT
                        + 2 * size_of::<std::slice::Iter<'static, Node>>()
                        + 4 * size_of::<&Vec<Node>>()
                        + 4 * size_of::<&[Node]>(),
                    "shared sequence Cat iterator receivers"
                );
                let mut children = children.iter();
                loop {
                    pay!(proof, work, CHILD, "shared sequence child receivers");
                    let Some(child) = children.next() else { break };
                    pending.push(child.clone());
                }
            }
        }
    }
    pay!(
        proof,
        work,
        PUBLISH,
        "shared sequence successful proof publication"
    );
    proof.last = Some(Entry {
        root: Rc::downgrade(root),
        visits,
    });
    Ok(true)
}

pub(in crate::cleanup_plan::replay) const CLASSIFY_CALL: usize = CALL
    + 2 * size_of::<Result<(bool, usize), Diagnostic>>()
    + 2 * size_of::<(bool, usize)>()
    + 4 * size_of::<&Node>()
    + 2 * size_of::<&Option<Entry>>()
    + 2 * size_of::<&Entry>()
    + PAYMENT;
pub(in crate::cleanup_plan::replay) fn classify_paid(
    root: &Node,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(bool, usize), Diagnostic> {
    let mut proof = take_owned!(work);
    let result = classify_root(root, work, &mut proof);
    let output = match result {
        Ok(true) => Ok((
            true,
            proof
                .last
                .as_ref()
                .expect("completed root published")
                .visits,
        )),
        Ok(false) => Ok((false, 0)),
        Err(error) => Err(error),
    };
    if output.is_ok() {
        work.queries.owner = Some(proof);
    } else {
        work.queries.failed = true;
    }
    output
}

#[cfg(test)]
#[allow(dead_code, reason = "complete independently compiled immediate-old sequence queries")]
#[rustfmt::skip]
#[path = "query_proof/before.rs"]
mod before;
#[cfg(test)]
#[path = "query_proof/tests.rs"]
mod tests;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn before_owner(current: &str, path: &str) -> String {
    assert!(matches!(path, "src/cleanup_plan/replay.rs" | "src/cleanup_plan/replay/skeleton_work.rs"));
    tests::restore(current, path)
}

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn before_emission(current: &str) -> String {
    tests::restore(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs",
    )
}

pub(in crate::cleanup_plan::replay) const SELECTOR: usize = PAYMENT
    + 2 * size_of::<allocation::AllocationPayer>() + 2 * size_of::<AdmissionMode>()
    + 4 * size_of::<&ReplayBudget>() + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&Option<()>>() // same thin layout as the lexical Option loan
    + 4 * size_of::<&AdmissionMode>() + 24 * size_of::<bool>();

// Complete immediate-old entry bodies; unchanged authentication/classification
// helpers and owning physical types are shared byte-for-byte, never a new tariff.
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn before_take_sequence_paid(
    observations: &Observations,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<bool, Diagnostic> {
    let mut proof = take(work)?;
    let result = classify(observations, work, &mut proof);
    // Failed/partial classification never republishes its invocation owner.
    if result.is_ok() {
        work.queries.owner = Some(proof);
    } else {
        work.queries.failed = true;
    }
    result
}
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn before_take_classify_paid(
    root: &Node,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(bool, usize), Diagnostic> {
    let mut proof = take(work)?;
    let result = classify_root(root, work, &mut proof);
    let output = match result {
        Ok(true) => Ok((
            true,
            proof
                .last
                .as_ref()
                .expect("completed root published")
                .visits,
        )),
        Ok(false) => Ok((false, 0)),
        Err(error) => Err(error),
    };
    if output.is_ok() {
        work.queries.owner = Some(proof);
    } else {
        work.queries.failed = true;
    }
    output
}

#[cfg(test)]
pub(in crate::cleanup_plan::replay) const BEFORE_TAKE_CLASSIFY_CALL: usize = BEFORE_TAKE_CALL
    + 2 * size_of::<Result<(bool, usize), Diagnostic>>()
    + 2 * size_of::<(bool, usize)>()
    + 4 * size_of::<&Node>()
    + 2 * size_of::<&Option<Entry>>()
    + 2 * size_of::<&Entry>()
    + PAYMENT;

// OPT952 complete immediate-old validation caller; namespace transports only.
#[cfg(test)]
#[rustfmt::skip]
mod before_take_validation {
//! Invocation-local immutable classification DATA, never content authority.
use super::{ReplayBudget, ResolvedFunction};
const PAYMENT: usize = 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<Result<(), Diagnostic>>();
use super::{
    lexical_skeleton, size_of, ClassifierLedger, Diagnostic, ExprSkeletonPath, ObservationDag,
    ObservationDagKind, Observations, Rc, SkeletonWork,
};

struct Entry {
    root: Rc<ObservationDag>,
    visits: usize,
}

// The exclusive real work loan binds both function and budget without a raw
// address capability. An entry owns its immutable Rc, preventing ABA/mutation.
struct Validation<'scope, 'function, 'budget> {
    work: &'scope mut SkeletonWork<'function, 'budget>,
    ledger: ClassifierLedger,
    last: Option<Entry>,
    failed: bool,
}
type Scope = Validation<'static, 'static, 'static>;
type Pending = Vec<&'static ObservationDag>;
type Paths = std::iter::Chain<
    std::slice::Iter<'static, ExprSkeletonPath>,
    std::slice::Iter<'static, ExprSkeletonPath>,
>;

// Actual/formal loans, live/moved/returned owners, setup iterators and complete
// error/Drop receivers. Dynamic Vec backing is separately acquired below.
pub(in crate::cleanup_plan::replay) const CALL: usize = 8 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + 2 * size_of::<Result<bool, Diagnostic>>()
    + 2 * size_of::<Scope>()
    + 2 * size_of::<ClassifierLedger>()
    + 3 * size_of::<Option<Entry>>()
    + 12 * size_of::<std::slice::Iter<'static, ExprSkeletonPath>>() // two iterator results, chain/IntoIterator owners
    + 2 * size_of::<Paths>()
    + ClassifierLedger::capture_into_controls()
    + AUTH
    + size_of::<bool>() // initialized invocation-local sticky terminal state
    + PAYMENT;
// CALL prepays the first guard. Every successful reserve renews the next
// guard's borrowed receivers and minimal foreign-error return before use.
const AUTH: usize = ClassifierLedger::current_receiving_controls()
    + 2 * size_of::<Diagnostic>()
    + 2 * size_of::<String>()
    + 4 * size_of::<&str>()
    + 2 * size_of::<Result<bool, Diagnostic>>()
    + 3 * size_of::<bool>(); // sticky read, guard, and first terminal publication
const INPUT_STEP: usize = 2 * size_of::<&mut Paths>()
    + 2 * size_of::<Option<&ExprSkeletonPath>>()
    + 2 * size_of::<&ExprSkeletonPath>()
    + 2 * size_of::<bool>()
    + 2 * PAYMENT;
const CHECK: usize = 2 * size_of::<&mut Scope>()
    + 2 * size_of::<&Observations>()
    + 2 * size_of::<&Option<Rc<ObservationDag>>>()
    + 2 * size_of::<Option<&Rc<ObservationDag>>>()
    + 2 * size_of::<&Rc<ObservationDag>>()
    + 2 * size_of::<&Option<Entry>>()
    + 2 * size_of::<&Entry>()
    + 4 * size_of::<&Rc<ObservationDag>>() // ptr_eq actual/formal pair
    + 2 * size_of::<Result<bool, Diagnostic>>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<bool>()
    + 2 * PAYMENT;
const REPLAY: usize = 2 * size_of::<usize>() + 2 * size_of::<bool>() + 2 * PAYMENT;
const COLD: usize = 2 * size_of::<&mut Scope>()
    + 4 * size_of::<&Rc<ObservationDag>>()
    + 3 * size_of::<Pending>()
    + 4 * size_of::<&mut Pending>()
    + 2 * size_of::<&ObservationDag>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<Result<(bool, usize), Diagnostic>>()
    + 2 * PAYMENT; // macro reservation and initial pending backing reservation
const POP: usize = 6 * size_of::<&mut Pending>()
    + 2 * size_of::<Option<&ObservationDag>>()
    + 4 * size_of::<&ObservationDag>()
    + 2 * size_of::<&ObservationDagKind>()
    + 2 * size_of::<&Option<Rc<ObservationDag>>>()
    + 2 * size_of::<Option<&Rc<ObservationDag>>>()
    + 4 * size_of::<&Rc<ObservationDag>>()
    + 4 * size_of::<&Vec<Rc<ObservationDag>>>()
    + 4 * size_of::<&[Rc<ObservationDag>]>()
    + 2 * size_of::<std::slice::Iter<'static, Rc<ObservationDag>>>()
    + 4 * size_of::<usize>()
    + 2 * size_of::<Option<usize>>()
    + 2 * size_of::<bool>()
    + 3 * PAYMENT;
const CHILD: usize = 2 * size_of::<&mut std::slice::Iter<'static, Rc<ObservationDag>>>()
    + 2 * size_of::<Option<&Rc<ObservationDag>>>()
    + 4 * size_of::<&Rc<ObservationDag>>()
    + 2 * size_of::<&ObservationDag>()
    + 2 * size_of::<&mut Pending>()
    + PAYMENT;
const PUBLISH: usize = 2 * size_of::<Entry>()
    + 2 * size_of::<Option<Entry>>() // incoming Some and publication receiver
    + 2 * size_of::<Rc<ObservationDag>>()
    + 2 * size_of::<&Rc<ObservationDag>>()
    + 2 * size_of::<usize>()
    + PAYMENT;

// Lexical expansion avoids an unpaid payment-helper argument/Result boundary.
// Foreign lenders are rejected before any reserve/diagnostic renderer on them.
macro_rules! pay {
    ($scope:ident, $bytes:expr, $phase:literal) => {{
        if $scope.failed {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        if !$scope.ledger.current() {
            $scope.failed = true;
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        match $scope.work.budget.reserve_bytes($scope.work.function, $bytes + AUTH, $phase) {
            Ok(()) => {},
            Err(error) => {
                $scope.failed = true;
                return Err(error);
            }
        }
    }};
}

pub(in crate::cleanup_plan::replay) fn inputs(
    prefixes: &[ExprSkeletonPath],
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<bool, Diagnostic> {
    let mut ledger = ClassifierLedger::empty_receiving();
    ledger.capture_current_into();
    let mut scope = Validation {
        work,
        ledger,
        last: None,
        failed: false,
    };
    let mut paths = prefixes.iter().chain(suffixes.iter());
    loop {
        pay!(scope, INPUT_STEP, "complete HIR memo input receivers");
        let Some(path) = paths.next() else { break };
        scope
            .work
            .budget
            .charge(scope.work.function, 1, "complete HIR fork input selection")?;
        if !matches!(path.continuation, lexical_skeleton::Continuation::Normal) {
            return Ok(false);
        }
        pay!(scope, CHECK, "complete HIR memo validation receivers");
        if !scope.check(&path.observations)? {
            return Ok(false);
        }
    }
    // Scope/Rc owners drop before the caller consumes any prefix owner.
    Ok(true)
}

impl Validation<'_, '_, '_> {
    fn check(&mut self, observations: &Observations) -> Result<bool, Diagnostic> {
        let Some(root) = &observations.prefix else {
            return Ok(true);
        };
        if let Some(entry) = &self.last {
            // One fixed slot deliberately forgoes nonadjacent reuse. This
            // bounds all-disjoint lookup work linearly, without an admission
            // threshold or alternative semantic route.
            self.work
                .budget
                .charge(self.work.function, 1, "complete HIR memo root lookup")?;
            if Rc::ptr_eq(&entry.root, root) {
                #[cfg(test)]
                note(false);
                let mut left = entry.visits;
                loop {
                    pay!(self, REPLAY, "complete HIR memo replay receivers");
                    if left == 0 {
                        break;
                    }
                    self.work.budget.charge(
                        self.work.function,
                        1,
                        "complete HIR sequence validation",
                    )?;
                    left -= 1;
                }
                return Ok(true);
            }
        }
        pay!(self, COLD, "complete HIR memo cold receivers");
        #[cfg(test)]
        note(true);
        let (valid, visits) = self.classify(root)?;
        if !valid {
            return Ok(false);
        }
        pay!(self, PUBLISH, "complete HIR memo publication receivers");
        // The old root remains owned through full cold classification and
        // successful publication payment. Replacing it drops exactly one Rc;
        // caller inputs still own every source until this scope is destroyed.
        self.last = Some(Entry {
            root: root.clone(),
            visits,
        });
        Ok(true)
    }

    fn classify(&mut self, root: &Rc<ObservationDag>) -> Result<(bool, usize), Diagnostic> {
        self.work.queries.selector(self.work.function, &mut *self.work.budget,
            crate::cleanup_plan::replay::skeleton_work::query_proof::SELECTOR,
            "shared sequence cold partition receivers")?;
        if matches!(self.work.budget.payer,
            crate::cleanup_plan::replay::allocation::AllocationPayer::ActiveBuilder
                | crate::cleanup_plan::replay::allocation::AllocationPayer::LegacyUnobserved)
            && self.work.budget.admission == crate::cleanup_plan::replay::AdmissionMode::Ordinary
            && !self.work.budget.lent_skeleton && !self.work.budget.merge_paths
            && self.work.function.cleanup_plan.lexical_control.is_none()
        {
            self.work.queries.preflight(self.work.function, &mut *self.work.budget,
                crate::cleanup_plan::replay::skeleton_work::query_proof::BEFORE_TAKE_CLASSIFY_CALL,
                "shared sequence classifier call receivers")?;
            return crate::cleanup_plan::replay::skeleton_work::query_proof::before_take_classify_paid(root, self.work);
        }
        let mut pending = Vec::new();
        let mut visits = 0usize;
        self.work.budget.reserve_vec(
            self.work.function,
            &mut pending,
            1,
            "complete HIR sequence validation backing",
        )?;
        pending.push(root.as_ref());
        loop {
            pay!(self, POP, "complete HIR memo cold step receivers");
            let Some(node) = pending.pop() else { break };
            self.work
                .budget
                .charge(self.work.function, 1, "complete HIR sequence validation")?;
            visits = visits.checked_add(1).ok_or_else(|| {
                self.work.budget.budget_refusal(
                    self.work.function,
                    "complete HIR validation visit count overflowed",
                    "",
                )
            })?;
            match &node.kind {
                ObservationDagKind::Alt(_) => return Ok((false, visits)),
                ObservationDagKind::Seq(prefix, _) => {
                    if let Some(prefix) = prefix {
                        self.work.budget.reserve_vec(
                            self.work.function,
                            &mut pending,
                            1,
                            "complete HIR sequence ancestor backing",
                        )?;
                        pending.push(prefix.as_ref());
                    }
                }
                ObservationDagKind::Cat(children) => {
                    self.work.budget.reserve_vec(
                        self.work.function,
                        &mut pending,
                        children.len(),
                        "complete HIR sequence concatenation backing",
                    )?;
                    let mut children = children.iter();
                    loop {
                        pay!(self, CHILD, "complete HIR memo child receivers");
                        let Some(child) = children.next() else { break };
                        pending.push(child.as_ref());
                    }
                }
            }
        }
        Ok((true, visits))
    }
}

// Read-only test observations; never consulted by classification/admission.
#[cfg(test)]
thread_local! {
    static COUNTS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}
#[cfg(test)]
fn note(cold: bool) {
    COUNTS.with(|counts| {
        let (classify, hits) = counts.get();
        counts.set((classify + usize::from(cold), hits + usize::from(!cold)));
    });
}

}
