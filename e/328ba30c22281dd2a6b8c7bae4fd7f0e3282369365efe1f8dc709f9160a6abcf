//! Positive synchronous weighted-subset proof. Inconclusive results retain the
//! complete canonical proof. No labels, multiplicities or source checks are elided.
use super::super::physical_budget::ByteFailure;
use super::*;
use std::cmp::Ordering;
use std::mem::size_of;

// An optimization boundary, not a new public allowance: larger inputs retain
// the complete existing comparator. Existing replay work is still debited.
const MAX: usize = 32_768;
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
enum Weighted {
    One((usize, u64)),
    Many(Rc<Vec<(usize, u64)>>),
}
impl Weighted {
    fn rows(&self) -> &[(usize, u64)] {
        match self {
            Self::One(row) => std::slice::from_ref(row),
            Self::Many(rows) => rows,
        }
    }
}
type Pair = (Weighted, Weighted);
type Edge<'a> = (&'a Label, usize, u64);
#[derive(Clone, Copy)]
struct Cursor {
    node: usize,
    next: usize,
}
struct Work<'f, 'b> {
    function: &'f ResolvedFunction,
    budget: &'b mut ReplayBudget,
    bank: FrameBank,
}
type Failure = ByteFailure;

// One nonrecursive invocation owns every primitive receiving place until Drop.
// The caller pays all frame families BEFORE constructing Work or calling any
// helper. No child can retain the bank, a function borrow or a donor ledger.
// Owned vectors/Rc payloads and per-state publication remain separately paid.
#[derive(Clone, Copy)]
enum Frame {
    Exact,
    Grow,
    Compare,
    Topology,
    HeapPush,
    HeapPop,
    EdgeCompare,
    Sift,
    Sort,
    Frontier,
    Weighted,
}
impl Frame {
    const fn controls(self) -> usize {
        match self {
            Self::Exact => {
                2 * size_of::<Vec<()>>()
                    + 2 * size_of::<(usize, u64)>()
                    + size_of::<Result<Vec<()>, Failure>>()
                    + 4 * size_of::<usize>()
                    + 2 * size_of::<Result<(), Failure>>()
            }
            Self::Grow => {
                4 * size_of::<&mut Vec<()>>()
                    + 4 * size_of::<usize>()
                    + 4 * size_of::<Result<(), Failure>>()
                    + 2 * size_of::<Option<usize>>()
            }
            Self::Compare => {
                4 * size_of::<&Label>()
                    + 2 * size_of::<Result<Ordering, Failure>>()
                    + 2 * size_of::<Ordering>()
                    + 2 * size_of::<Result<(), Failure>>()
            }
            Self::Topology => {
                2 * size_of::<Result<Option<Vec<usize>>, Failure>>()
                    + 2 * size_of::<Vec<Cursor>>()
                    + 2 * size_of::<Vec<u8>>()
                    + 2 * size_of::<Vec<usize>>()
                    + size_of::<[usize; 8]>()
                    + 4 * size_of::<&Automaton>()
            }
            Self::HeapPush => {
                4 * size_of::<usize>()
                    + 4 * size_of::<&mut Vec<usize>>()
                    + 2 * size_of::<Result<(), Failure>>()
            }
            Self::HeapPop => {
                6 * size_of::<usize>()
                    + 4 * size_of::<&mut Vec<usize>>()
                    + 2 * size_of::<Result<usize, Failure>>()
            }
            Self::EdgeCompare => {
                4 * size_of::<&Edge<'_>>()
                    + 2 * size_of::<Result<Ordering, Failure>>()
                    + 2 * size_of::<Ordering>()
            }
            Self::Sift => {
                4 * size_of::<&mut [Edge<'_>]>()
                    + size_of::<[usize; 8]>()
                    + 4 * size_of::<Result<(), Failure>>()
                    + 2 * size_of::<Result<Ordering, Failure>>()
            }
            Self::Sort => {
                4 * size_of::<&mut [Edge<'_>]>()
                    + 4 * size_of::<usize>()
                    + 4 * size_of::<Result<(), Failure>>()
            }
            Self::Frontier => {
                2 * size_of::<(
                    &Automaton,
                    &[(usize, u64)],
                    usize,
                    &[usize],
                    &[usize],
                    &mut [u64],
                    &mut Vec<usize>,
                    &mut Vec<Edge<'_>>,
                )>() + 2 * size_of::<Result<Option<u64>, Failure>>()
                    + size_of::<[usize; 8]>()
                    + 4 * size_of::<u64>()
                    + 4 * size_of::<Result<(), Failure>>()
            }
            Self::Weighted => {
                2 * size_of::<Weighted>()
                    + 2 * size_of::<Result<Weighted, Failure>>()
                    + 2 * size_of::<Vec<(usize, u64)>>()
                    + 4 * size_of::<usize>()
            }
        }
    }
}
struct FrameBank {
    function: *const ResolvedFunction,
    budget: *const ReplayBudget,
    ledger: crate::bounded_output::classifier_string::ClassifierLedger,
}
const BANK_CONTROLS: usize = 2*size_of::<FrameBank>()
    + 16*size_of::<Result<(),Failure>>()
    + 16*size_of::<&mut ReplayBudget>()
    + 16*size_of::<&ResolvedFunction>()
    + 16*size_of::<&Work<'static,'static>>()
    + 16*size_of::<&mut Work<'static,'static>>()
    + 22*size_of::<Frame>() + 22*size_of::<bool>()
    // Conservative initialized/formal Result receivers for the finite,
    // nonrecursive private call-site census (owned returns stay per-call).
    + 192*size_of::<Result<Weighted,Failure>>()
    + Frame::Exact.controls()
    + Frame::Grow.controls()
    + Frame::Compare.controls()
    + Frame::Topology.controls()
    + Frame::HeapPush.controls()
    + Frame::HeapPop.controls()
    + Frame::EdgeCompare.controls()
    + Frame::Sift.controls()
    + Frame::Sort.controls()
    + Frame::Frontier.controls()
    + Frame::Weighted.controls();

// Root caller owns the outward Diagnostic-bearing Result before entry, in
// addition to every new collection owner, construction and receiving place.
pub(super) fn entry_controls() -> usize {
    crate::bounded_output::classifier_string::ClassifierLedger::capture_into_controls()
        + crate::bounded_output::classifier_string::ClassifierLedger::current_receiving_controls()
        + crate::bounded_output::classifier_string::ClassifierLedger::insertion_terminal_controls()
        + BANK_CONTROLS
        + 2 * size_of::<Scratch<'static>>()
        + 2 * size_of::<Work<'static, 'static>>()
        + 2 * size_of::<Result<Option<bool>, Diagnostic>>()
        + 8 * size_of::<&ResolvedFunction>()
        + 8 * size_of::<&mut ReplayBudget>()
        + 8 * size_of::<&BuiltComparison>()
        + 16 * size_of::<usize>()
        + 8 * size_of::<bool>()
        + 8 * size_of::<Result<(), Failure>>()
}
struct Scratch<'a> {
    order: Vec<usize>,
    ranks: Vec<usize>,
    weights: Vec<u64>,
    heap: Vec<usize>,
    left: Vec<Edge<'a>>,
    right: Vec<Edge<'a>>,
    pending: Vec<Pair>,
    seen: Vec<Pair>,
}
impl Work<'_, '_> {
    fn authenticate(&mut self) -> Result<(), Failure> {
        if self.bank.function != std::ptr::from_ref(self.function)
            || self.bank.budget != std::ptr::from_ref(&*self.budget)
            || !self.bank.ledger.current()
        {
            self.budget.refusal_spent = true;
            return Err(Failure {
                message: std::borrow::Cow::Borrowed("paired proof receiving bank is detached"),
            });
        }
        self.budget
            .reserve_compact_bytes(self.function, 0, "paired receiving bank authentication")
    }
    fn frame(&mut self, _kind: Frame) -> Result<(), Failure> {
        self.authenticate()
    }

    fn bytes(&mut self, bytes: usize) -> Result<(), Failure> {
        self.authenticate()?;
        self.budget
            .reserve_compact_bytes(self.function, bytes, "paired weighted proof storage")
    }
    fn units(&mut self, units: usize) -> Result<(), Failure> {
        self.authenticate()?;
        self.budget
            .charge_compact(self.function, units, "paired weighted proof traversal")
    }
    fn arithmetic(&mut self) -> Failure {
        self.budget
            .compact_refusal(self.function, "paired weighted proof layout overflowed", "")
    }
    fn exact<T: Copy>(&mut self, n: usize, value: T) -> Result<Vec<T>, Failure> {
        const {
            assert!(size_of::<T>() <= size_of::<(usize, u64)>());
        }
        self.bytes(Frame::Exact.controls())?;
        self.budget
            .reserve_compact_array::<T>(self.function, n, "paired exact scratch backing")?;
        self.units(n)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(n).map_err(|_| self.arithmetic())?;
        if rows.capacity() != n {
            return Err(self.arithmetic());
        }
        rows.resize(n, value);
        Ok(rows)
    }
    fn grow<T>(&mut self, rows: &mut Vec<T>, additional: usize) -> Result<(), Failure> {
        const {
            assert!(size_of::<Vec<T>>() == size_of::<Vec<()>>());
        }
        // The generic receiver, arguments, actual growth Result and its nested
        // reservation Result are not inferred from the vector's backing.
        self.frame(Frame::Grow)?;
        self.budget.reserve_compact_vec(
            self.function,
            rows,
            additional,
            "paired growing frontier backing",
        )
    }
    fn compare(&mut self, a: &Label, b: &Label) -> Result<Ordering, Failure> {
        self.frame(Frame::Compare)?;
        self.units(1)?;
        Ok(a.cmp(b))
    }
}
fn topology(
    a: &Automaton,
    roots: [usize; 2],
    w: &mut Work<'_, '_>,
) -> Result<Option<Vec<usize>>, Failure> {
    w.frame(Frame::Topology)?;
    let n = a.nodes.len();
    if n > MAX || roots.iter().any(|r| *r >= n) {
        return Ok(None);
    }
    let mut colors = w.exact(n, 0u8)?;
    let mut frames = w.exact(n, Cursor { node: 0, next: 0 })?;
    let mut order = w.exact(n, 0usize)?;
    let mut count = 0;
    for root in roots {
        if colors[root] == 2 {
            continue;
        }
        frames[0].node = root;
        frames[0].next = 0;
        colors[root] = 1;
        let mut depth = 1;
        while depth > 0 {
            w.units(1)?;
            let frame = &mut frames[depth - 1];
            let node = &a.nodes[frame.node];
            let Some(total) = node.epsilon().len().checked_add(node.moves().len()) else {
                return Ok(None);
            };
            if frame.next == total {
                colors[frame.node] = 2;
                order[count] = frame.node;
                count += 1;
                depth -= 1;
                continue;
            }
            let child = if frame.next < node.epsilon().len() {
                node.epsilon()[frame.next]
            } else {
                node.moves()[frame.next - node.epsilon().len()].1
            };
            frame.next += 1;
            if child >= n {
                return Ok(None);
            }
            match colors[child] {
                2 => {}
                1 => return Ok(None),
                _ => {
                    if depth == n {
                        return Ok(None);
                    }
                    frames[depth].node = child;
                    frames[depth].next = 0;
                    colors[child] = 1;
                    depth += 1;
                }
            }
        }
    }
    order.truncate(count);
    Ok(Some(order))
}
// Heap slots are acquired once for all automaton nodes. Positive roots and
// postorder priority ensure each node enters the pending heap at most once
// per closure; a node is visited only after every possible predecessor.
fn heap_push(heap: &mut Vec<usize>, rank: usize, w: &mut Work<'_, '_>) -> Result<(), Failure> {
    w.frame(Frame::HeapPush)?;
    w.units(1)?;
    if heap.len() == heap.capacity() {
        return Err(w.arithmetic());
    }
    heap.push(rank);
    let mut at = heap.len() - 1;
    while at > 0 {
        w.units(1)?;
        let parent = (at - 1) / 2;
        if heap[parent] >= heap[at] {
            break;
        }
        heap.swap(parent, at);
        at = parent;
    }
    Ok(())
}
fn heap_pop(heap: &mut Vec<usize>, w: &mut Work<'_, '_>) -> Result<usize, Failure> {
    w.frame(Frame::HeapPop)?;
    w.units(1)?;
    let out = heap[0];
    let last = heap.pop().expect("nonempty pending heap");
    if heap.is_empty() {
        return Ok(out);
    }
    heap[0] = last;
    let mut at = 0usize;
    loop {
        w.units(1)?;
        let child = at * 2 + 1;
        if child >= heap.len() {
            break;
        }
        let greatest = if child + 1 < heap.len() && heap[child + 1] > heap[child] {
            child + 1
        } else {
            child
        };
        if heap[at] >= heap[greatest] {
            break;
        }
        heap.swap(at, greatest);
        at = greatest;
    }
    Ok(out)
}
fn edge_cmp(a: &Edge<'_>, b: &Edge<'_>, w: &mut Work<'_, '_>) -> Result<Ordering, Failure> {
    w.frame(Frame::EdgeCompare)?;
    Ok(w.compare(a.0, b.0)?.then(a.1.cmp(&b.1)))
}
fn sift(edges: &mut [Edge<'_>], mut root: usize, w: &mut Work<'_, '_>) -> Result<(), Failure> {
    w.frame(Frame::Sift)?;
    loop {
        w.units(1)?;
        let child = root * 2 + 1;
        if child >= edges.len() {
            return Ok(());
        }
        let mut greatest = child;
        if child + 1 < edges.len() && edge_cmp(&edges[child], &edges[child + 1], w)?.is_lt() {
            greatest += 1;
        }
        if !edge_cmp(&edges[root], &edges[greatest], w)?.is_lt() {
            return Ok(());
        }
        w.units(1)?;
        edges.swap(root, greatest);
        root = greatest;
    }
}
fn sort(edges: &mut [Edge<'_>], w: &mut Work<'_, '_>) -> Result<(), Failure> {
    w.frame(Frame::Sort)?;
    for root in (0..edges.len() / 2).rev() {
        sift(edges, root, w)?;
    }
    for end in (1..edges.len()).rev() {
        w.units(1)?;
        edges.swap(0, end);
        sift(&mut edges[..end], 0, w)?;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn frontier<'a>(
    a: &'a Automaton,
    roots: &[(usize, u64)],
    sink: usize,
    ranks: &[usize],
    order: &[usize],
    weights: &mut [u64],
    heap: &mut Vec<usize>,
    edges: &mut Vec<Edge<'a>>,
    w: &mut Work<'_, '_>,
) -> Result<Option<u64>, Failure> {
    w.frame(Frame::Frontier)?;
    if !heap.is_empty() {
        return Ok(None);
    }
    edges.clear();
    for (node, weight) in roots {
        w.units(1)?;
        if *node >= weights.len() || *weight == 0 {
            return Ok(None);
        }
        if weights[*node] == 0 {
            heap_push(heap, ranks[*node], w)?;
        }
        let Some(sum) = weights[*node].checked_add(*weight) else {
            return Ok(None);
        };
        weights[*node] = sum;
    }
    let mut accepted = 0u64;
    while !heap.is_empty() {
        let rank = heap_pop(heap, w)?;
        let node = order[rank];
        let weight = std::mem::take(&mut weights[node]);
        if node == sink {
            let Some(sum) = accepted.checked_add(weight) else {
                return Ok(None);
            };
            accepted = sum;
        }
        let row = &a.nodes[node];
        for child in row.epsilon() {
            w.units(1)?;
            if weights[*child] == 0 {
                heap_push(heap, ranks[*child], w)?;
            }
            let Some(sum) = weights[*child].checked_add(weight) else {
                return Ok(None);
            };
            weights[*child] = sum;
        }
        for (label, child) in row.moves() {
            w.units(1)?;
            w.bytes(2 * size_of::<Edge<'_>>())?;
            w.grow(edges, 1)?;
            edges.push((label, *child, weight));
        }
    }
    sort(edges, w)?;
    let mut output = 0;
    for input in 0..edges.len() {
        w.units(1)?;
        let row = edges[input];
        if output > 0
            && edges[output - 1].1 == row.1
            && w.compare(edges[output - 1].0, row.0)?.is_eq()
        {
            let Some(sum) = edges[output - 1].2.checked_add(row.2) else {
                return Ok(None);
            };
            edges[output - 1].2 = sum;
        } else {
            edges[output] = row;
            output += 1;
        }
    }
    edges.truncate(output);
    Ok(Some(accepted))
}
fn weighted(edges: &[Edge<'_>], w: &mut Work<'_, '_>) -> Result<Weighted, Failure> {
    w.bytes(Frame::Weighted.controls())?;
    if edges.len() == 1 {
        return Ok(Weighted::One((edges[0].1, edges[0].2)));
    }
    let mut rows = w.exact(edges.len(), (0usize, 0u64))?;
    w.units(edges.len())?;
    for (out, row) in rows.iter_mut().zip(edges) {
        out.0 = row.1;
        out.1 = row.2;
    }
    w.bytes(physical_clone::rc_backing_bytes::<Vec<(usize, u64)>>())?;
    Ok(Weighted::Many(Rc::new(rows)))
}
fn run(b: &BuiltComparison, w: &mut Work<'_, '_>) -> Result<Option<bool>, Failure> {
    let a = &b.automaton;
    if b.sink >= a.nodes.len()
        || !a.nodes[b.sink].moves().is_empty()
        || !a.nodes[b.sink].epsilon().is_empty()
    {
        return Ok(None);
    }
    let Some(order) = topology(a, [b.hir_root, b.plan_root], w)? else {
        return Ok(None);
    };
    let mut s = Scratch {
        order,
        ranks: w.exact(a.nodes.len(), 0usize)?,
        weights: w.exact(a.nodes.len(), 0u64)?,
        heap: Vec::new(),
        left: Vec::new(),
        right: Vec::new(),
        pending: Vec::new(),
        seen: Vec::new(),
    };
    for (rank, node) in s.order.iter().enumerate() {
        w.units(1)?;
        s.ranks[*node] = rank;
    }
    w.budget
        .reserve_compact_array::<usize>(w.function, a.nodes.len(), "paired heap backing")?;
    s.heap
        .try_reserve_exact(a.nodes.len())
        .map_err(|_| w.arithmetic())?;
    if s.heap.capacity() != a.nodes.len() {
        return Err(w.arithmetic());
    }
    w.grow(&mut s.pending, 1)?;
    s.pending.push((
        Weighted::One((b.hir_root, 1)),
        Weighted::One((b.plan_root, 1)),
    ));
    while let Some(pair) = s.pending.pop() {
        w.units(1)?;
        // Equality of stored numeric weighted states is never pointer identity.
        let (mut low, mut high) = (0, s.seen.len());
        let mut found = false;
        while low < high {
            w.bytes(4 * size_of::<usize>() + 2 * size_of::<Ordering>())?;
            let at = low + (high - low) / 2;
            let units = pair
                .0
                .rows()
                .len()
                .checked_add(pair.1.rows().len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| w.arithmetic())?;
            w.units(units)?;
            match pair.cmp(&s.seen[at]) {
                Ordering::Equal => {
                    found = true;
                    break;
                }
                Ordering::Less => high = at,
                Ordering::Greater => low = at + 1,
            }
        }
        if found {
            continue;
        }
        if s.seen.len() == MAX {
            return Ok(None);
        }
        w.units(s.seen.len() - low + pair.0.rows().len() + pair.1.rows().len() + 1)?;
        // Reserve the same visited directory before either frontier. The
        // current pair already lives through this iteration; transfer it only
        // after its last borrow instead of cloning both full Weighted values.
        w.bytes(2 * size_of::<Pair>())?;
        w.grow(&mut s.seen, 1)?;
        let Some(left) = frontier(
            a,
            pair.0.rows(),
            b.sink,
            &s.ranks,
            &s.order,
            &mut s.weights,
            &mut s.heap,
            &mut s.left,
            w,
        )?
        else {
            return Ok(None);
        };
        let Some(right) = frontier(
            a,
            pair.1.rows(),
            b.sink,
            &s.ranks,
            &s.order,
            &mut s.weights,
            &mut s.heap,
            &mut s.right,
            w,
        )?
        else {
            return Ok(None);
        };
        if left != right {
            return Ok(None);
        }
        let (mut left, mut right) = (0, 0);
        while left < s.left.len() && right < s.right.len() {
            w.units(1)?;
            if !w.compare(s.left[left].0, s.right[right].0)?.is_eq() {
                return Ok(None);
            }
            let (mut end_l, mut end_r) = (left + 1, right + 1);
            while end_l < s.left.len() {
                if !w.compare(s.left[left].0, s.left[end_l].0)?.is_eq() {
                    break;
                }
                end_l += 1;
            }
            while end_r < s.right.len() {
                if !w.compare(s.right[right].0, s.right[end_r].0)?.is_eq() {
                    break;
                }
                end_r += 1;
            }
            if s.pending.len() == MAX {
                return Ok(None);
            }
            let lhs = weighted(&s.left[left..end_l], w)?;
            let rhs = weighted(&s.right[right..end_r], w)?;
            w.bytes(2 * size_of::<Pair>())?;
            w.grow(&mut s.pending, 1)?;
            s.pending.push((lhs, rhs));
            left = end_l;
            right = end_r;
        }
        if left != s.left.len() || right != s.right.len() {
            return Ok(None);
        }
        // Neither frontier nor successor publication reads or mutates seen.
        // Its sorted insertion position and pre-reserved capacity are intact.
        s.seen.insert(low, pair);
    }
    Ok(Some(true))
}
pub(super) fn positive(
    function: &ResolvedFunction,
    built: &BuiltComparison,
    budget: &mut ReplayBudget,
) -> Result<Option<bool>, Diagnostic> {
    // The caller pays ENTRY_CONTROLS before constructing this outward receiver.
    // No owned evidence survives this exclusive immutable-input/budget borrow.
    let mut bank = FrameBank {
        function: std::ptr::from_ref(function),
        budget: std::ptr::from_ref(&*budget),
        ledger: crate::bounded_output::classifier_string::ClassifierLedger::empty_receiving(),
    };
    bank.ledger.capture_current_into();
    let result = run(
        built,
        &mut Work {
            function,
            budget,
            bank,
        },
    );
    result.map_err(ByteFailure::into_diagnostic)
}

#[cfg(test)]
mod tests;
