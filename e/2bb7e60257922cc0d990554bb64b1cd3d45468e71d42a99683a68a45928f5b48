//! Whole immediate 0f9 canonical receiver; selected Groups route remains independent.
use super::inline_weighted::SmallWeighted;
use super::{
    app_census, arena_tests, borrowed_labels, closure_receiving, empty_closure, empty_memo,
    empty_signature, intern_arena, lexical_child_scratch, ordered_alias, ordered_weights,
    paid_many, physical_clone, replay_error, selected_labels, tests, AllocationPayer, Automaton,
    BTreeMap, Diagnostic, Label, Rc, ReplayBudget, ResolvedFunction, Weighted,
};
// Invoked only after preflight proved an unbound, nonfailed Memo. This is the
// original cold activation body at its caller: no authenticate Result or
// forwarded method arguments are constructed. Nested reservation Results,
// first-refusal order, full Memo/Binding storage and capture remain real.
macro_rules! activate_closure {
    ($this:ident, $function:ident, $budget:ident, $error:ident, $failure:block) => {{
        if let Err($error) =
            $budget.reserve_bytes($function, 0, "factored closure memo authentication")
        {
            $this.closures.failed = true;
            $failure
        }
        if $budget.pays_allocations() {
            if let Err($error) = $budget.reserve_bytes(
                $function,
                std::mem::size_of::<empty_memo::Memo<Rc<ordered_weights::OrderedWeights>>>()
                    + std::mem::size_of::<Option<empty_memo::Binding>>(),
                "factored closure memo controls",
            ) {
                $this.closures.failed = true;
                $failure
            }
        }
        let binding = Some(empty_memo::Binding {
            function: std::ptr::from_ref($function),
            budget: std::ptr::from_ref($budget),
            ledger: crate::bounded_output::classifier_string::ClassifierLedger::capture(),
        });
        $this.closures.binding = binding;
    }};
}

// Lexical unit publication deliberately introduces no forwarding borrowed
// receiver: preflight replaces the old RetainedMemo auth method boundary, and
// the old zero-auth call borrows the already-live invocation refusal field here.
macro_rules! authenticate_closure {
    ($this:ident, $function:ident, $budget:ident, $receiving:ident) => {{
        // The returned branch predicate replaces the former controls_paid /
        // binding branch; no second Option<Binding> borrow or named local exists.
        if $this.closures.preflight($function, $budget) {
            if $this.closures.failed() {
                $receiving.refusal = Some(Diagnostic::io("SPX-H006", String::new()));
            } else {
                $budget.authenticate_zero_into(
                    $function,
                    "factored closure memo authentication",
                    &mut $receiving.refusal,
                );
                if $receiving.refusal.is_some() {
                    $this.closures.failed = true;
                }
            }
        } else if $this.closures.failed() {
            $receiving.refusal = Some(Diagnostic::io("SPX-H006", String::new()));
        } else {
            'activation: {
                activate_closure!($this, $function, $budget, error, {
                    $receiving.refusal = Some(error);
                    break 'activation;
                });
            }
        }
    }};
}
macro_rules! publish_closure {
    ($this:ident, $function:ident, $budget:ident, $receiving:ident, $operation:expr) => {{
        authenticate_closure!($this, $function, $budget, $receiving);
        if $receiving.refusal.is_some() {
            $receiving.ready = true;
            return;
        }
        if let Err(error) = $operation {
            $this.closures.failed = true;
            $receiving.refuse(error);
            return;
        }
    }};
}

type SharedWeighted = Rc<SmallWeighted>;

enum CanonicalFrame {
    Enter(SharedWeighted),
    Exit(SharedWeighted, u64, Vec<(Label, SharedWeighted)>),
}

// The caller has already paid the moved vector's complete allocated backing.
// Rc owns that vector without copying it; its counters and full enum header are
// separately acquired while all preceding owners remain fully charged.
fn own_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    state: Weighted,
) -> Result<SharedWeighted, Diagnostic> {
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            2 * std::mem::size_of::<SmallWeighted>()
                + std::mem::size_of::<Result<SharedWeighted, Diagnostic>>()
                + std::mem::size_of::<Result<(), Diagnostic>>(),
            "factored moved many weighted enum receiving",
        )?;
    }
    budget.reserve_bytes(
        function,
        physical_clone::rc_backing_bytes::<SmallWeighted>(),
        "factored weighted shared header",
    )?;
    let shared = SmallWeighted::Many(state);
    Ok(Rc::new(shared))
}

// Vec's FromIterator growth policy may round 1..3 items up to four slots.
// Build this producer through its exact precharged capacity instead.
fn weighted_successor(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    targets: BTreeMap<usize, u64>,
) -> Result<SharedWeighted, Diagnostic> {
    budget.reserve_array::<(usize, u64)>(
        function,
        targets.len(),
        "factored weighted successor vector",
    )?;
    let mut state = Vec::with_capacity(targets.len());
    state.extend(targets);
    own_weighted(function, budget, state)
}

fn share_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    state: &SharedWeighted,
) -> Result<SharedWeighted, Diagnostic> {
    let work = state
        .len()
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "factored weighted state work overflowed"))?;
    budget.charge(function, work, "factored weighted state copy")?;
    // Authentication remains sticky even though this clone creates no backing.
    // The receiving frame/map slot has its own full carrier reservation.
    budget.reserve_bytes(function, 0, "factored weighted state backing")?;
    Ok(Rc::clone(state))
}

/// Bottom-up hash-consing of the weighted subset construction. A determinized
/// state's id is its accepted weight together with each label and the id of
/// the state that label leads to, so two states share an id exactly when
/// they accept every observation sequence the same number of times.
pub(super) struct Canonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: empty_memo::Memo<Rc<ordered_weights::OrderedWeights>>,
    states: intern_arena::Arena<SharedWeighted>,
    ids: intern_arena::Arena<intern_arena::Signature>,
    child_ids: Vec<usize>,
    cursor: Vec<(usize, bool)>,
    frames: Vec<CanonicalFrame>,
    open: Vec<SharedWeighted>,
    scratch_paid: bool,
}

include!("before_selected_route_methods.rs");

impl Canonicalizer<'_> {
    pub(super) fn signatures(&self) -> &intern_arena::Arena<intern_arena::Signature> {
        &self.ids
    }
    pub(super) fn states(&self) -> &intern_arena::Arena<SharedWeighted> {
        &self.states
    }
}
#[path = "terminal_leaf.rs"]
mod terminal_leaf;
