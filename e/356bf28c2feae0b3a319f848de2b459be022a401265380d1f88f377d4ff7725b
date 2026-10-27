//! Invocation-owned receiving storage for the shared-history heapsort.
//! Only empty capacities survive a comparison; paths remain the sole root owners.
use super::super::*;
use super::{before_branch_frames, frame_event};
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
use std::rc::Rc;

// No frame borrows a path slot. Sorting may move every path between calls.
// Rc handles keep immutable contents alive; equality always reads those contents.
pub(super) enum Frame {
    RootItems {
        next: usize,
    },
    Node(Rc<ObservationDag>),
    SeqItems {
        node: Rc<ObservationDag>,
        next: usize,
    },
}
pub(super) struct Cursor {
    pub(super) pending: Vec<Frame>,
    pub(super) yielded: Option<Observation>,
    pub(super) current: Option<Frame>,
}
pub(super) struct Pair {
    pub(super) left: Cursor,
    pub(super) right: Cursor,
    pub(super) ordering: Ordering,
}
const PAYMENT: usize = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<Result<(), ByteFailure>>();
// Concrete retained owners and aggregate initializers. Receiving storage is
// represented by fields, not inferred from repeated call-stack addresses.
const TRIVIAL_GUARD: usize = 2 * size_of::<bool>();
const RECEIVERS: usize = 2 * size_of::<Pair>() + 4 * size_of::<Cursor>();
// Surviving invocation transports are still paid at each old frontier. Only
// pending/current/yielded and ordering themselves belong to the retained bank.
const PREPARE_CALL: usize = 2 * PAYMENT
    + 4 * size_of::<&mut Cursor>()
    + 4 * size_of::<&Observations>()
    + 2 * size_of::<Result<(), ByteFailure>>()
    + 4 * size_of::<Rc<ObservationDag>>()
    + 4 * size_of::<Option<&Rc<ObservationDag>>>()
    + 4 * size_of::<Frame>()
    + 4 * size_of::<bool>();
// Method receivers and completion/lent-refusal transports occur once per
// invocation. They cannot pay for the unbounded structural loop below.
const ADVANCE_CALL: usize = PAYMENT
    + 4 * size_of::<&mut Cursor>()
    + 4 * size_of::<&Observations>()
    + 2 * size_of::<Result<(), ByteFailure>>();
// The pop/take, moved match, scalar controls, and work-payment producer are
// common to every attempt, including exhaustion. The third PAYMENT funds the
// new branch reserve BEFORE its actual/formal arguments and Result can form.
// Match the already-owned frame: there is no extra selector helper or loan.
const FRAME_COMMON: usize = 3 * PAYMENT
    + 2 * size_of::<&mut Vec<Frame>>()
    + 2 * size_of::<&mut Option<Frame>>()
    + 2 * size_of::<Frame>()
    + 2 * size_of::<Option<Frame>>()
    + 4 * size_of::<Rc<ObservationDag>>() // moved node bindings precede branch payment
    + size_of::<[usize; 8]>()
    + 8 * size_of::<bool>();
// Complete old typed roles, partitioned by actual constructors. SeqItems has
// both node-kind/handle and item receivers; Node has prefix/child receivers.
// No node growth or Cat-child invoice is discounted here.
const ROOT_FRAME: usize = 4 * size_of::<Observation>()
    + 4 * size_of::<Option<&Observation>>()
    + 4 * size_of::<&[Observation]>();
const SEQ_FRAME: usize = ROOT_FRAME + 4 * size_of::<&ObservationDagKind>();
const NODE_FRAME: usize = 4 * size_of::<Option<Rc<ObservationDag>>>()
    + 2 * size_of::<std::iter::Rev<std::slice::Iter<'static, Rc<ObservationDag>>>>()
    + 4 * size_of::<Option<&Rc<ObservationDag>>>()
    + 4 * size_of::<&ObservationDagKind>()
    + 4 * size_of::<&Option<Rc<ObservationDag>>>();
#[cfg(test)]
const FRAME_STEP: usize = before_branch_frames::old_frame_step();
// Only a Node can invoke the backing-reservation producer. Pay its actual
// call plus this new renewal call at that branch, before either can form its
// transports. Item/resume/exhaustion frames never invoke the growth producer.
const NODE_STEP: usize = 2 * PAYMENT;
// A Cat can have arbitrarily many children even while the pending Vec is
// warm. Pay every iterator attempt, including its final None, and the actual
// clone, moved frame, push and handle teardown transports before forming them.
type Children<'a> = std::iter::Rev<std::slice::Iter<'a, Rc<ObservationDag>>>;
const CAT_CHILD_STEP: usize = PAYMENT
    + 2 * size_of::<&mut Children<'static>>()
    + 2 * size_of::<Option<&Rc<ObservationDag>>>()
    + 4 * size_of::<&Rc<ObservationDag>>()
    + 4 * size_of::<Rc<ObservationDag>>()
    + 2 * size_of::<Frame>()
    + 2 * size_of::<&mut Vec<Frame>>()
    + 4 * size_of::<usize>()
    + 4 * size_of::<bool>();
const COMPARE_CALL: usize = 2 * size_of::<(
    &mut Pair,
    &SkeletonPath,
    &SkeletonPath,
    &ResolvedFunction,
    &mut ReplayBudget,
)>() + 4 * size_of::<Result<(), ByteFailure>>()
    + 4 * size_of::<&Option<Observation>>()
    + 4 * size_of::<&Observation>()
    + 4 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<Ordering>()
    + 4 * size_of::<bool>();

impl Cursor {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
            yielded: None,
            current: None,
        }
    }
    fn clear(&mut self) {
        self.yielded = None;
        self.current = None;
        self.pending.clear();
    }
    fn prepare(
        &mut self,
        observations: &Observations,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), ByteFailure> {
        self.clear();
        // Keep the old authentication and growth frontier, including no-growth
        // calls. A reused backing is the actual backing acquired on an earlier
        // comparison, never a credit or a guessed maximum.
        budget.reserve_compact_vec(
            function,
            &mut self.pending,
            2,
            "enumerated HIR initial cursor backing",
        )?;
        self.pending.push(Frame::RootItems { next: 0 });
        if let Some(prefix) = &observations.prefix {
            self.pending.push(Frame::Node(Rc::clone(prefix)));
        }
        Ok(())
    }
    // Return only completion. The actual observation receiver lives in self;
    // neither a returned reference nor a slice iterator crosses a path swap.
    fn next(
        &mut self,
        observations: &Observations,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), ByteFailure> {
        self.yielded = None;
        if budget.lent_skeleton {
            return Err(budget.compact_refusal(
                function,
                "enumerated HIR cursor cannot spend a lent skeleton allowance",
                "",
            ));
        }
        loop {
            budget.reserve_compact_bytes(
                function,
                FRAME_COMMON,
                "enumerated HIR cursor common frame receiving renewal",
            )?;
            #[cfg(test)]
            frame_event(0);
            self.current = self.pending.pop();
            if self.current.is_none() {
                break;
            }
            budget.charge_compact(function, 1, "enumerated HIR cursor traversal")?;
            match self.current.take().expect("the popped frame was present") {
                Frame::RootItems { next } => {
                    #[cfg(test)]
                    frame_event(1);
                    budget.reserve_compact_bytes(
                        function,
                        ROOT_FRAME,
                        "enumerated HIR root-items frame receivers",
                    )?;
                    if let Some(item) = observations.items.get(next) {
                        self.yielded = Some(Rc::clone(item));
                        self.pending.push(Frame::RootItems { next: next + 1 });
                        return Ok(());
                    }
                }
                Frame::SeqItems { node, next } => {
                    #[cfg(test)]
                    frame_event(2);
                    budget.reserve_compact_bytes(
                        function,
                        SEQ_FRAME,
                        "enumerated HIR sequence-items frame receivers",
                    )?;
                    let ObservationDagKind::Seq(_, items) = &node.kind else {
                        unreachable!("sequence cursor frame retains its immutable node")
                    };
                    if let Some(item) = items.get(next) {
                        self.yielded = Some(Rc::clone(item));
                        self.pending.push(Frame::SeqItems {
                            node,
                            next: next + 1,
                        });
                        return Ok(());
                    }
                }
                Frame::Node(node) => {
                    #[cfg(test)]
                    frame_event(3);
                    budget.reserve_compact_bytes(
                        function,
                        NODE_FRAME,
                        "enumerated HIR node frame receivers",
                    )?;
                    budget.reserve_compact_bytes(
                        function,
                        NODE_STEP,
                        "enumerated HIR node growth receiving renewal",
                    )?;
                    match &node.kind {
                        ObservationDagKind::Seq(prefix, _) => {
                            budget.reserve_compact_vec(
                                function,
                                &mut self.pending,
                                2,
                                "enumerated HIR sequence cursor backing",
                            )?;
                            let prefix = prefix.as_ref().map(Rc::clone);
                            // Move this handle into the resume frame; do not clone
                            // the Seq node merely to remember its items.
                            self.pending.push(Frame::SeqItems { node, next: 0 });
                            if let Some(prefix) = prefix {
                                self.pending.push(Frame::Node(prefix));
                            }
                        }
                        ObservationDagKind::Cat(children) => {
                            budget.reserve_compact_vec(
                                function,
                                &mut self.pending,
                                children.len(),
                                "enumerated HIR concatenation cursor backing",
                            )?;
                            let mut children = children.iter().rev();
                            loop {
                                budget.reserve_compact_bytes(
                                    function,
                                    CAT_CHILD_STEP,
                                    "enumerated HIR concatenation child receiving renewal",
                                )?;
                                let Some(child) = children.next() else {
                                    break;
                                };
                                self.pending.push(Frame::Node(Rc::clone(child)));
                            }
                        }
                        ObservationDagKind::Alt(_) => {
                            return Err(budget.compact_refusal(
                                function,
                                "enumerated HIR history cannot contain merged alternatives",
                                "",
                            ))
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
impl Pair {
    pub(super) fn new() -> Self {
        Self {
            left: Cursor::new(),
            right: Cursor::new(),
            ordering: Ordering::Equal,
        }
    }
    pub(super) fn order(
        &mut self,
        a: &SkeletonPath,
        b: &SkeletonPath,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), ByteFailure> {
        let result = self.compare(a, b, function, budget);
        // Clear on BOTH outcomes before the caller can swap. On failure this
        // is infallible handle teardown: it cannot replace the chosen status.
        // Every node still has an original path root, so no DAG is destroyed.
        self.left.clear();
        self.right.clear();
        result
    }
    fn compare(
        &mut self,
        a: &SkeletonPath,
        b: &SkeletonPath,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), ByteFailure> {
        budget.reserve_compact_bytes(
            function,
            2 * PREPARE_CALL + COMPARE_CALL,
            "enumerated HIR paired cursor receivers",
        )?;
        self.left.prepare(&a.observations, function, budget)?;
        self.right.prepare(&b.observations, function, budget)?;
        loop {
            // Retained fields survive. Pay invocation transports here; next
            // separately renews every dynamic frame and Cat-child attempt.
            budget.reserve_compact_bytes(
                function,
                2 * ADVANCE_CALL,
                "enumerated HIR paired cursor next receivers",
            )?;
            #[cfg(test)]
            super::advance_pair_event();
            self.left.next(&a.observations, function, budget)?;
            // Right still advances when left is exhausted.
            self.right.next(&b.observations, function, budget)?;
            match (&self.left.yielded, &self.right.yielded) {
                (Some(a), Some(b)) => {
                    self.ordering = a.as_ref().cmp(b.as_ref());
                    if self.ordering != Ordering::Equal {
                        return Ok(());
                    }
                }
                (None, None) => {
                    self.ordering = a.terminal.cmp(&b.terminal);
                    return Ok(());
                }
                (None, Some(_)) => {
                    self.ordering = Ordering::Less;
                    return Ok(());
                }
                (Some(_), None) => {
                    self.ordering = Ordering::Greater;
                    return Ok(());
                }
            }
        }
    }
}
fn sift(
    paths: &mut [SkeletonPath],
    mut root: usize,
    end: usize,
    pair: &mut Pair,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), ByteFailure> {
    loop {
        budget.charge_compact(function, 1, "enumerated HIR content sort traversal")?;
        if end < 2 || root > (end - 2) / 2 {
            return Ok(());
        }
        let mut child = root * 2 + 1;
        if child + 1 < end {
            budget.reserve_compact_bytes(
                function,
                ORDER_CALL,
                "enumerated HIR sibling order call receivers",
            )?;
            pair.order(&paths[child], &paths[child + 1], function, budget)?;
            if pair.ordering == Ordering::Less {
                child += 1;
            }
        }
        budget.reserve_compact_bytes(
            function,
            ORDER_CALL,
            "enumerated HIR parent order call receivers",
        )?;
        pair.order(&paths[root], &paths[child], function, budget)?;
        if pair.ordering != Ordering::Less {
            return Ok(());
        }
        paths.swap(root, child);
        root = child;
    }
}
pub(in crate::cleanup_plan::replay::skeleton_work::enumerated_prefix) fn sort(
    paths: &mut [SkeletonPath],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), ByteFailure> {
    budget.reserve_compact_bytes(function, SORT_CALL, "enumerated HIR content sort receivers")?;
    // The original len receiver is part of SORT_CALL. Pay the new branch
    // before querying it; a singleton performs no comparison and therefore
    // must not acquire either cursor or its receiving bank.
    budget.reserve_compact_bytes(
        function,
        TRIVIAL_GUARD,
        "enumerated HIR trivial content sort guard",
    )?;
    let len = paths.len();
    if len < 2 {
        return Ok(());
    }
    budget.reserve_compact_bytes(
        function,
        RECEIVERS,
        "enumerated HIR reusable content sort receiving bank",
    )?;
    let mut pair = Pair::new();
    for root in (0..len / 2).rev() {
        budget.reserve_compact_bytes(function, SORT_CALL, "enumerated HIR sift call receivers")?;
        sift(paths, root, len, &mut pair, function, budget)?;
    }
    for end in (1..len).rev() {
        paths.swap(0, end);
        budget.reserve_compact_bytes(function, SORT_CALL, "enumerated HIR sift call receivers")?;
        sift(paths, 0, end, &mut pair, function, budget)?;
    }
    Ok(())
}

// TEST_ONLY_948_EXACT_BANK_ACCESS
pub(super) fn banks() -> [usize; 9] {
    [PAYMENT, ADVANCE_CALL, FRAME_COMMON, ROOT_FRAME, SEQ_FRAME, NODE_FRAME,
        NODE_STEP, CAT_CHILD_STEP, COMPARE_CALL]
}
