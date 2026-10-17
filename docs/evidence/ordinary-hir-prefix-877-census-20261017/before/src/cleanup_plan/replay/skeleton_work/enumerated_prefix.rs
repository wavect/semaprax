//! Ordinary enumerated storage sharing; never an alternative/state union.
//! Sequence nodes use the existing prepaid iterative destruction frontier.
use super::*;
use std::cmp::Ordering;
use std::mem::size_of;

// Actual selector, moved node kind, result, payment and borrowed receivers.
// The existing first clone work debit precedes these new controls.
const NODE_CALL: usize = 2 * size_of::<ObservationDag>() // Rc::new actual/formal node aggregate
    + 2 * size_of::<ObservationDagKind>() // moved actual + owned formal
    + 2 * size_of::<&mut SkeletonWork<'static, 'static>>() // actual/formal work
    + size_of::<Result<std::rc::Rc<ObservationDag>, Diagnostic>>()
    + size_of::<std::rc::Rc<ObservationDag>>() // successful node receiver
    + 2 * size_of::<Option<usize>>() // checked release multiplication/addition
    + size_of::<Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>>()
    + size_of::<Result<Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>, Diagnostic>>()
    + 4 * size_of::<usize>() // edge count, release count, bound, backing byte price
    + 4 * size_of::<Result<(), Diagnostic>>() // work, scratch, header and failure controls
    + 8 * size_of::<&ReplayBudget>() + 8 * size_of::<&ResolvedFunction>()
    + 8 * size_of::<&str>() + 4 * size_of::<bool>();
pub(in crate::cleanup_plan::replay) const SHARE_CONTROLS: usize = NODE_CALL
    + size_of::<Observations>() // moved tail/header publication
    + 2 * size_of::<&mut SkeletonWork<'static, 'static>>() // renewal query actual/formal
    + size_of::<Result<bool, Diagnostic>>() + 12 * size_of::<bool>()
    + 4 * size_of::<&Observations>() + 2 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<Option<&std::rc::Rc<ObservationDag>>>() // actual/formal marker query
    + 2 * size_of::<&Option<std::rc::Rc<ObservationDag>>>() // borrowed Option view
    + size_of::<Option<usize>>() // marker comparison literal
    + size_of::<std::slice::Iter<'static, crate::cleanup_plan::CleanupBlock>>()
    + size_of::<std::slice::Iter<'static, crate::cleanup_plan::CleanupTransition>>();
pub(super) const DISPATCH_CONTROLS: usize = 2 * size_of::<Option<&std::rc::Rc<ObservationDag>>>()
    + 2 * size_of::<&Observations>()
    + 2 * size_of::<usize>()
    + 4 * size_of::<bool>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&ResolvedFunction>()
    + size_of::<Result<(), Diagnostic>>();
pub(super) const EXTEND_CONTROLS: usize = 2 * NODE_CALL // at most Seq head and Cat join
    + 2 * size_of::<Observations>() // moved actual + formal target
    + 2 * size_of::<&Observations>() + 2 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + size_of::<Result<Observations, Diagnostic>>()
    + size_of::<Option<std::rc::Rc<ObservationDag>>>() // optional head
    + 4 * size_of::<std::rc::Rc<ObservationDag>>() // head, prefix borrow clone, joined, publication
    + size_of::<Vec<std::rc::Rc<ObservationDag>>>() // Cat backing owner
    + size_of::<Vec<Observation>>() // copied suffix owner
    + 4 * size_of::<Result<(), Diagnostic>>() + 4 * size_of::<usize>();
pub(super) const PREPEND_CONTROLS: usize = 2 * NODE_CALL // leaf head and Cat join
    + 2 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + 2 * size_of::<&mut Observations>() + 2 * size_of::<Observation>()
    + size_of::<Vec<Observation>>() + size_of::<Vec<std::rc::Rc<ObservationDag>>>()
    + 3 * size_of::<std::rc::Rc<ObservationDag>>()
    + 4 * size_of::<Result<(), Diagnostic>>() + 4 * size_of::<usize>();

macro_rules! share {
    ($work:ident, $prefix:ident) => {{
        // Two scalar results reuse the original paid split's finite probe
        // storage before prefix_fusion acquires that same lexical frame.
        // prefix_fusion owns the compile-time whole-layout inequality.
        if matches!(&$work.budget.payer, &AllocationPayer::ActiveBuilder)
            && $work.budget.admission == AdmissionMode::Ordinary
            && !$work.budget.selected() && !$work.budget.merge_paths
            && !$work.budget.lent_skeleton
            && $work.function.cleanup_plan.lexical_control.is_none()
            && $prefix.observations.items.len() >= 4096
        {
            $work.budget.reserve_bytes(
                $work.function,
                $crate::cleanup_plan::replay::skeleton_work::enumerated_prefix::SHARE_CONTROLS,
                "enumerated HIR prefix selector and publication receivers",
            )?;
            if $prefix.observations.prefix.as_ref().is_none_or(|node|
                    node.ordinal == Some($crate::cleanup_plan::replay::skeleton_work::enumerated_prefix::SEQUENCE_MARKER))
                && $crate::cleanup_plan::replay::skeleton_work::enumerated_prefix::without_renewal(
                    $work,
                )?
            {
                let kind = $crate::cleanup_plan::replay::skeleton_work::ObservationDagKind::Seq(
                    $prefix.observations.prefix.take(),
                    std::mem::take(&mut $prefix.observations.items),
                );
                $prefix.observations.prefix = Some(
                    $crate::cleanup_plan::replay::skeleton_work::enumerated_prefix::node(
                        $work, kind,
                    )?,
                );
            }
        }
    }};
}
pub(in crate::cleanup_plan::replay) use share;

pub(in crate::cleanup_plan::replay) fn without_renewal(
    work: &mut SkeletonWork<'_, '_>,
) -> Result<bool, Diagnostic> {
    for block in &work.function.cleanup_plan.blocks {
        work.budget
            .charge(work.function, 1, "enumerated HIR renewal exclusion block")?;
        for transition in &block.transitions {
            work.budget.charge(
                work.function,
                1,
                "enumerated HIR renewal exclusion transition",
            )?;
            if matches!(
                transition,
                CleanupTransition::ReserveRenewal { .. } | CleanupTransition::Renew { .. }
            ) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

// Private representation marker only; content cursors still reconstruct every
// observation. It grants no equality, ordering, admission or work authority.
pub(in crate::cleanup_plan::replay) const SEQUENCE_MARKER: usize = usize::MAX;

// Dedicated sequence storage: original clone/extension work uses the held
// skeleton loan; all additional bound/destruction traversal uses ordinary free
// work. Existing node layout is reused without reusing its ordinal constructor.
pub(in crate::cleanup_plan::replay) fn node(
    work: &mut SkeletonWork<'_, '_>,
    kind: ObservationDagKind,
) -> Result<std::rc::Rc<ObservationDag>, Diagnostic> {
    // Caller prepays SHARE_CONTROLS before constructing the moved kind or
    // forwarding its Result. No carrier is paid retrospectively here.
    if work.budget.lent_skeleton || work.budget.merge_paths || work.budget.selected() {
        return Err(work.budget.budget_refusal(
            work.function,
            "enumerated HIR sequence storage requires ordinary free-work ownership",
            "",
        ));
    }
    let edges = kind.edges();
    let release = edges
        .checked_mul(2)
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| {
            work.budget.budget_refusal(
                work.function,
                "enumerated HIR immutable release work overflowed",
                "",
            )
        })?;
    work.budget.charge(
        work.function,
        release,
        "enumerated HIR immutable bound and release",
    )?;
    if matches!(&kind, ObservationDagKind::Alt(_)) {
        return Err(work.budget.budget_refusal(
            work.function,
            "enumerated HIR cannot construct merged alternatives",
            "",
        ));
    }
    let bound = kind.pending_bound().ok_or_else(|| {
        work.budget.budget_refusal(
            work.function,
            "enumerated HIR destruction bound overflowed",
            "",
        )
    })?;
    let scratch = work.ordinary_frontier(bound)?;
    work.budget.reserve_bytes(
        work.function,
        physical_clone::rc_backing_bytes::<ObservationDag>(),
        "enumerated HIR immutable sequence node",
    )?;
    Ok(std::rc::Rc::new(ObservationDag {
        ordinal: Some(SEQUENCE_MARKER),
        kind,
        pending_bound: Some(bound),
        scratch,
    }))
}

// Cursor frames borrow stable immutable node/slice storage; no Rc clone and no
// pointer identity participates in semantic comparison. Alt is always refused.
enum Frame<'a> {
    Node(&'a ObservationDag),
    Items(std::slice::Iter<'a, Observation>),
}
pub(in crate::cleanup_plan::replay) struct Cursor<'a> {
    pending: Vec<Frame<'a>>,
}
pub(in crate::cleanup_plan::replay) const CURSOR_STORAGE: usize = 2 * size_of::<Cursor<'static>>();
pub(in crate::cleanup_plan::replay) const CURSOR_CALL: usize =
    4 * size_of::<&mut Cursor<'static>>()
        + 4 * size_of::<Frame<'static>>()
        + 4 * size_of::<Option<Frame<'static>>>()
        + 4 * size_of::<Option<&SkeletonObservation>>()
        + 4 * size_of::<Result<(), Diagnostic>>()
        + size_of::<Result<Option<&SkeletonObservation>, Diagnostic>>()
        + 10 * size_of::<usize>()
        + 8 * size_of::<bool>()
        + 8 * size_of::<&ResolvedFunction>()
        + 8 * size_of::<&mut ReplayBudget>()
        + 4 * size_of::<&Observations>()
        + 4 * size_of::<&str>();

impl<'a> Cursor<'a> {
    pub(in crate::cleanup_plan::replay) fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }
    pub(in crate::cleanup_plan::replay) fn prepare(
        &mut self,
        observations: &'a Observations,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        self.pending.clear();
        budget.reserve_vec(
            function,
            &mut self.pending,
            2,
            "enumerated HIR initial cursor backing",
        )?;
        self.pending.push(Frame::Items(observations.items.iter()));
        if let Some(prefix) = &observations.prefix {
            self.pending.push(Frame::Node(prefix));
        }
        Ok(())
    }
    pub(in crate::cleanup_plan::replay) fn next(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&'a SkeletonObservation>, Diagnostic> {
        if budget.lent_skeleton {
            return Err(budget.budget_refusal(
                function,
                "enumerated HIR cursor cannot spend a lent skeleton allowance",
                "",
            ));
        }
        while let Some(frame) = self.pending.pop() {
            budget.charge(function, 1, "enumerated HIR cursor traversal")?;
            match frame {
                Frame::Items(mut items) => {
                    if let Some(item) = items.next() {
                        // The popped slot is still within paid capacity.
                        self.pending.push(Frame::Items(items));
                        return Ok(Some(item.as_ref()));
                    }
                }
                Frame::Node(node) => match &node.kind {
                    ObservationDagKind::Seq(prefix, items) => {
                        budget.reserve_vec(
                            function,
                            &mut self.pending,
                            2,
                            "enumerated HIR sequence cursor backing",
                        )?;
                        self.pending.push(Frame::Items(items.iter()));
                        if let Some(prefix) = prefix {
                            self.pending.push(Frame::Node(prefix));
                        }
                    }
                    ObservationDagKind::Cat(children) => {
                        budget.reserve_vec(
                            function,
                            &mut self.pending,
                            children.len(),
                            "enumerated HIR concatenation cursor backing",
                        )?;
                        for child in children.iter().rev() {
                            self.pending.push(Frame::Node(child));
                        }
                    }
                    ObservationDagKind::Alt(_) => {
                        return Err(budget.budget_refusal(
                            function,
                            "enumerated HIR history cannot contain merged alternatives",
                            "",
                        ))
                    }
                },
            }
        }
        Ok(None)
    }
}

const ORDER_CALL: usize = 4 * size_of::<&SkeletonPath>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + size_of::<Result<Ordering, Diagnostic>>()
    + size_of::<Ordering>();
fn order(
    a: &SkeletonPath,
    b: &SkeletonPath,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    budget.reserve_bytes(
        function,
        2 * CURSOR_STORAGE + 2 * CURSOR_CALL,
        "enumerated HIR paired cursor receivers",
    )?;
    let mut left = Cursor::new();
    let mut right = Cursor::new();
    left.prepare(&a.observations, function, budget)?;
    right.prepare(&b.observations, function, budget)?;
    loop {
        budget.reserve_bytes(
            function,
            2 * CURSOR_CALL,
            "enumerated HIR paired cursor next receivers",
        )?;
        let x = left.next(function, budget)?;
        let y = right.next(function, budget)?;
        match (x, y) {
            (Some(a), Some(b)) => {
                let order = a.cmp(b);
                if order != Ordering::Equal {
                    return Ok(order);
                }
            }
            (None, None) => return Ok(a.terminal.cmp(&b.terminal)),
            (None, Some(_)) => return Ok(Ordering::Less),
            (Some(_), None) => return Ok(Ordering::Greater),
        }
    }
}

const SORT_CALL: usize = 2 * size_of::<&mut [SkeletonPath]>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 8 * size_of::<usize>()
    + 4 * size_of::<bool>()
    + 2 * size_of::<std::iter::Rev<std::ops::Range<usize>>>()
    + size_of::<Result<(), Diagnostic>>();
fn sift(
    paths: &mut [SkeletonPath],
    mut root: usize,
    end: usize,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    loop {
        budget.charge(function, 1, "enumerated HIR content sort traversal")?;
        if end < 2 || root > (end - 2) / 2 {
            return Ok(());
        }
        let mut child = root * 2 + 1;
        if child + 1 < end {
            budget.reserve_bytes(
                function,
                ORDER_CALL,
                "enumerated HIR sibling order call receivers",
            )?;
            if order(&paths[child], &paths[child + 1], function, budget)? == Ordering::Less {
                child += 1;
            }
        }
        budget.reserve_bytes(
            function,
            ORDER_CALL,
            "enumerated HIR parent order call receivers",
        )?;
        if order(&paths[root], &paths[child], function, budget)? != Ordering::Less {
            return Ok(());
        }
        paths.swap(root, child);
        root = child;
    }
}
pub(in crate::cleanup_plan::replay) const ENTRY_CALL: usize = 4 * size_of::<&[SkeletonPath]>()
    + 2 * size_of::<&mut [SkeletonPath]>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<std::slice::Iter<'static, SkeletonPath>>()
    + size_of::<
        std::iter::Chain<
            std::slice::Iter<'static, SkeletonPath>,
            std::slice::Iter<'static, SkeletonPath>,
        >,
    >()
    + size_of::<Result<bool, Diagnostic>>()
    + size_of::<Result<(), Diagnostic>>()
    + 4 * size_of::<bool>();
pub(in crate::cleanup_plan::replay) const HIR_QUERY_CONTROLS: usize = 2 * size_of::<
    &Option<std::rc::Rc<ObservationDag>>,
>() + 2 * size_of::<bool>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&ResolvedFunction>()
    + size_of::<Result<(), Diagnostic>>();
pub(in crate::cleanup_plan::replay) fn sort(
    paths: &mut [SkeletonPath],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    // Preserve the former ordering algorithm on flat inputs. Ordinary entry
    // query receivers are charged separately; Selected/Oracle never call here.
    if paths.iter().all(|path| path.observations.prefix.is_none()) {
        paths.sort();
        return Ok(());
    }
    budget.reserve_bytes(function, SORT_CALL, "enumerated HIR content sort receivers")?;
    let len = paths.len();
    for root in (0..len / 2).rev() {
        budget.reserve_bytes(function, SORT_CALL, "enumerated HIR sift call receivers")?;
        sift(paths, root, len, function, budget)?;
    }
    for end in (1..len).rev() {
        paths.swap(0, end);
        budget.reserve_bytes(function, SORT_CALL, "enumerated HIR sift call receivers")?;
        sift(paths, 0, end, function, budget)?;
    }
    Ok(())
}
pub(in crate::cleanup_plan::replay) fn equal(
    left: &[SkeletonPath],
    right: &[SkeletonPath],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if left
        .iter()
        .chain(right)
        .all(|path| path.observations.prefix.is_none())
    {
        return Ok(left == right);
    }
    budget.reserve_bytes(
        function,
        ORDER_CALL
            + 4 * size_of::<&[SkeletonPath]>()
            + 2 * size_of::<Result<bool, Diagnostic>>()
            + 2 * size_of::<bool>(),
        "enumerated HIR equality receivers",
    )?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        budget.reserve_bytes(
            function,
            ORDER_CALL,
            "enumerated HIR equality order call receivers",
        )?;
        if order(left, right, function, budget)? != Ordering::Equal {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod before;
#[cfg(test)]
mod source_custody;
#[cfg(test)]
mod tests;

pub(super) fn extend(
    work: &mut SkeletonWork<'_, '_>,
    target: Observations,
    suffix: &Observations,
) -> Result<Observations, Diagnostic> {
    let prefix = suffix
        .prefix
        .as_ref()
        .expect("shared suffix observed by caller");
    let head = if target.items.is_empty() {
        target.prefix
    } else {
        Some(node(
            work,
            ObservationDagKind::Seq(target.prefix, target.items),
        )?)
    };
    let joined = match head {
        None => prefix.clone(),
        Some(head) => {
            work.budget.reserve_array::<std::rc::Rc<ObservationDag>>(
                work.function,
                2,
                "enumerated HIR concatenation backing",
            )?;
            node(work, ObservationDagKind::Cat(vec![head, prefix.clone()]))?
        }
    };
    work.budget
        .reserve_clone(work.function, &suffix.items, "ordinary DAG suffix tail")?;
    Ok(Observations {
        prefix: Some(joined),
        items: suffix.items.clone(),
    })
}
pub(super) fn prepend(
    work: &mut SkeletonWork<'_, '_>,
    target: &mut Observations,
    observation: Observation,
) -> Result<(), Diagnostic> {
    work.budget
        .reserve_array::<Observation>(work.function, 1, "enumerated HIR prepend head")?;
    let head = node(work, ObservationDagKind::Seq(None, vec![observation]))?;
    work.budget.reserve_array::<std::rc::Rc<ObservationDag>>(
        work.function,
        2,
        "enumerated HIR prepend concatenation",
    )?;
    let prefix = target
        .prefix
        .as_ref()
        .expect("shared prepend caller")
        .clone();
    target.prefix = Some(node(work, ObservationDagKind::Cat(vec![head, prefix]))?);
    Ok(())
}

#[cfg(test)]
thread_local! {
    static ORIGINAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn original_selected() -> bool {
    ORIGINAL.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn with_original<T>(operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORIGINAL.with(|flag| flag.set(self.0));
        }
    }
    let previous = ORIGINAL.with(|flag| flag.replace(true));
    let _restore = Restore(previous);
    operation()
}
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn old_sequence(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    before::sequence_skeleton_paths(prefixes, suffixes, work)
}
