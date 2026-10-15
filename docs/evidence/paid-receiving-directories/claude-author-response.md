# claude author response (paid-receiving-directories)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

I haven't compiled or run any of this, and I haven't seen several helpers, so I flag where I inferred rather than read them.

## New file: `directory.rs`

It goes beside the receiving file and is declared with `mod directory;` (hunk 1 below). If the receiving file isn't a directory-style module, adjust the path. It's about 300 lines.

```rust
//! Complete paid ordered key directory for receiving replay. Indices are private
//! construction coordinates; keys are compared in full and never hashed.
use super::*;
use std::cmp::Ordering;

const NIL: usize = usize::MAX;
// Per-step charges mirror the expression-fact arena.
const VISIT_UNITS: usize = 3;
const STEP_UNITS: usize = 2;
const REBALANCE_UNITS: usize = 16;
const ROTATION_UNITS: usize = 20;

/// Work for one full-key comparison: full string `min(len) + 1` plus one unit per
/// structural numeric field. `None` on arithmetic overflow.
pub(super) trait KeyWork: Ord {
    fn comparison_units(&self, other: &Self) -> Option<usize>;
}

impl KeyWork for u32 {
    fn comparison_units(&self, _: &Self) -> Option<usize> {
        Some(1)
    }
}

impl KeyWork for ExpressionId {
    fn comparison_units(&self, other: &Self) -> Option<usize> {
        self.as_str()
            .len()
            .min(other.as_str().len())
            .checked_add(1)
    }
}

impl KeyWork for StatusSourceId {
    fn comparison_units(&self, other: &Self) -> Option<usize> {
        // The lane is one structural numeric field.
        self.expression
            .comparison_units(&other.expression)?
            .checked_add(1)
    }
}

impl<T: KeyWork> KeyWork for &T {
    fn comparison_units(&self, other: &Self) -> Option<usize> {
        (**self).comparison_units(*other)
    }
}

struct Node<K> {
    key: K,
    left: usize,
    right: usize,
    parent: usize,
    next: usize,
    height: u8,
}

struct Probe {
    found: bool,
    parent: usize,
    before: usize,
    after: usize,
    side: Ordering,
}

pub(super) struct Directory<K> {
    nodes: Vec<Node<K>>,
    root: usize,
    first: usize,
    phase: &'static str,
}

struct Iter<'a, K> {
    nodes: &'a [Node<K>],
    next: usize,
}

impl<'a, K> Iterator for Iter<'a, K> {
    type Item = &'a K;
    fn next(&mut self) -> Option<Self::Item> {
        let node = self.nodes.get(self.next)?;
        self.next = node.next;
        Some(&node.key)
    }
}

fn overflow(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    detail: &'static str,
) -> Diagnostic {
    budget.budget_refusal(function, detail, "")
}

impl<K: KeyWork> Directory<K> {
    pub(super) fn new(phase: &'static str) -> Self {
        Self {
            nodes: Vec::new(),
            root: NIL,
            first: NIL,
            phase,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.nodes.len()
    }

    fn authenticate(
        &self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        budget.reserve_bytes(function, 0, self.phase)?;
        budget.charge(function, 0, self.phase)
    }

    fn iter(&self) -> Iter<'_, K> {
        Iter {
            nodes: &self.nodes,
            next: self.first,
        }
    }

    fn probe(
        &self,
        function: &ResolvedFunction,
        key: &K,
        budget: &mut ReplayBudget,
    ) -> Result<Probe, Diagnostic> {
        let mut probe = Probe {
            found: false,
            parent: NIL,
            before: NIL,
            after: NIL,
            side: Ordering::Equal,
        };
        let mut at = self.root;
        while at != NIL {
            let node = &self.nodes[at];
            let units = key
                .comparison_units(&node.key)
                .and_then(|units| units.checked_add(VISIT_UNITS))
                .ok_or_else(|| overflow(function, budget, "directory key comparison work overflow"))?;
            budget.charge(function, units, self.phase)?;
            probe.side = Ord::cmp(key, &node.key);
            if probe.side == Ordering::Equal {
                probe.found = true;
                return Ok(probe);
            }
            probe.parent = at;
            if probe.side == Ordering::Less {
                probe.after = at;
                at = node.left;
            } else {
                probe.before = at;
                at = node.right;
            }
        }
        Ok(probe)
    }

    pub(super) fn contains(
        &self,
        function: &ResolvedFunction,
        key: &K,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.authenticate(function, budget)?;
        Ok(self.probe(function, key, budget)?.found)
    }

    /// Returns `false` for a duplicate, like `BTreeSet::insert`.
    pub(super) fn insert(
        &mut self,
        function: &ResolvedFunction,
        key: K,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.authenticate(function, budget)?;
        let probe = self.probe(function, &key, budget)?;
        if probe.found {
            return Ok(false);
        }
        self.grow(function, budget)?;
        budget.charge(function, VISIT_UNITS, self.phase)?;
        let index = self.nodes.len();
        self.nodes.push(Node {
            key,
            left: NIL,
            right: NIL,
            parent: probe.parent,
            next: probe.after,
            height: 1,
        });
        if probe.before == NIL {
            self.first = index;
        } else {
            self.nodes[probe.before].next = index;
        }
        if probe.parent == NIL {
            self.root = index;
        } else if probe.side == Ordering::Less {
            self.nodes[probe.parent].left = index;
        } else {
            self.nodes[probe.parent].right = index;
        }
        self.rebalance(probe.parent, function, budget)?;
        Ok(true)
    }

    // Relocation is metered before any work; `reserve_vec` pays the complete
    // replacement array while the old allocation is still live.
    fn grow(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if self.nodes.len() == self.nodes.capacity() {
            let units = self
                .nodes
                .len()
                .checked_add(1)
                .ok_or_else(|| overflow(function, budget, "directory relocation work overflow"))?;
            budget.charge(function, units, self.phase)?;
        }
        budget.reserve_vec(function, &mut self.nodes, 1, self.phase)
    }

    /// Full-key set equality by lockstep ordered walk.
    pub(super) fn same_keys(
        &self,
        other: &Self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.authenticate(function, budget)?;
        if self.len() != other.len() {
            return Ok(false);
        }
        let (mut left, mut right) = (self.iter(), other.iter());
        loop {
            match (left.next(), right.next()) {
                (Some(a), Some(b)) => {
                    Self::charge_pair(a, b, function, budget, self.phase)?;
                    if Ord::cmp(a, b) != Ordering::Equal {
                        return Ok(false);
                    }
                }
                (None, None) => return Ok(true),
                _ => return Ok(false),
            }
        }
    }

    /// Ordered merge walk; `true` when no full key is shared.
    pub(super) fn is_disjoint(
        &self,
        other: &Self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.authenticate(function, budget)?;
        let (mut left, mut right) = (self.iter(), other.iter());
        let (mut a, mut b) = (left.next(), right.next());
        while let (Some(x), Some(y)) = (a, b) {
            Self::charge_pair(x, y, function, budget, self.phase)?;
            match Ord::cmp(x, y) {
                Ordering::Equal => return Ok(false),
                Ordering::Less => a = left.next(),
                Ordering::Greater => b = right.next(),
            }
        }
        Ok(true)
    }

    fn charge_pair(
        a: &K,
        b: &K,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        phase: &'static str,
    ) -> Result<(), Diagnostic> {
        let units = a
            .comparison_units(b)
            .and_then(|units| units.checked_add(STEP_UNITS))
            .ok_or_else(|| overflow(function, budget, "directory key comparison work overflow"))?;
        budget.charge(function, units, phase)
    }

    fn height(&self, at: usize) -> u8 {
        if at == NIL {
            0
        } else {
            self.nodes[at].height
        }
    }

    // Callers meter the enclosing rebalance/rotation step before calling.
    fn update(
        &mut self,
        at: usize,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        let height = self
            .height(self.nodes[at].left)
            .max(self.height(self.nodes[at].right))
            .checked_add(1)
            .ok_or_else(|| overflow(function, budget, "directory AVL height overflow"))?;
        self.nodes[at].height = height;
        Ok(())
    }

    fn attach(&mut self, parent: usize, old: usize, next: usize) {
        if parent == NIL {
            self.root = next;
        } else if self.nodes[parent].left == old {
            self.nodes[parent].left = next;
        } else {
            self.nodes[parent].right = next;
        }
        self.nodes[next].parent = parent;
    }

    fn rotate_left(
        &mut self,
        at: usize,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        budget.charge(function, ROTATION_UNITS, self.phase)?;
        let right = self.nodes[at].right;
        let middle = self.nodes[right].left;
        let parent = self.nodes[at].parent;
        self.attach(parent, at, right);
        self.nodes[right].left = at;
        self.nodes[at].parent = right;
        self.nodes[at].right = middle;
        if middle != NIL {
            self.nodes[middle].parent = at;
        }
        self.update(at, function, budget)?;
        self.update(right, function, budget)?;
        Ok(right)
    }

    fn rotate_right(
        &mut self,
        at: usize,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        budget.charge(function, ROTATION_UNITS, self.phase)?;
        let left = self.nodes[at].left;
        let middle = self.nodes[left].right;
        let parent = self.nodes[at].parent;
        self.attach(parent, at, left);
        self.nodes[left].right = at;
        self.nodes[at].parent = left;
        self.nodes[at].left = middle;
        if middle != NIL {
            self.nodes[middle].parent = at;
        }
        self.update(at, function, budget)?;
        self.update(left, function, budget)?;
        Ok(left)
    }

    fn rebalance(
        &mut self,
        mut at: usize,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        while at != NIL {
            budget.charge(function, REBALANCE_UNITS, self.phase)?;
            self.update(at, function, budget)?;
            let (left, right) = (self.nodes[at].left, self.nodes[at].right);
            let top = if i16::from(self.height(left)) - i16::from(self.height(right)) > 1 {
                if self.height(self.nodes[left].left) < self.height(self.nodes[left].right) {
                    self.rotate_left(left, function, budget)?;
                }
                self.rotate_right(at, function, budget)?
            } else if i16::from(self.height(right)) - i16::from(self.height(left)) > 1 {
                if self.height(self.nodes[right].right) < self.height(self.nodes[right].left) {
                    self.rotate_right(right, function, budget)?;
                }
                self.rotate_left(at, function, budget)?
            } else {
                at
            };
            at = self.nodes[top].parent;
        }
        Ok(())
    }
}
```

## Receiving diff

```diff
 use super::*;
+mod directory;
+use directory::Directory;
```
```diff
 // validate_status_sources_core
-    let mut ids = BTreeSet::new();
+    let mut ids = Directory::new("cleanup status-source identity directory");
     for source in &function.cleanup_plan.status_sources {
-        if !ids.insert(source.id.clone()) {
+        if !ids.insert(function, &source.id, budget)? {
```
```diff
 // validate_blocks_and_edges
-    let statuses = plan
-        .status_sources
-        .iter()
-        .map(|source| source.id.clone())
-        .collect::<BTreeSet<_>>();
+    let mut statuses = Directory::new("cleanup status-source directory");
+    for source in &plan.status_sources {
+        statuses.insert(function, &source.id, budget)?;
+    }
```
```diff
-    let mut referenced_edges = BTreeSet::new();
-    let mut referenced_exits = BTreeSet::new();
-    let mut committed_calls = BTreeSet::new();
+    let mut referenced_edges = Directory::new("cleanup edge ownership directory");
+    let mut referenced_exits = Directory::new("cleanup exit ownership directory");
+    let mut committed_calls = Directory::new("cleanup committed-call directory");
```
```diff
-                    if !committed_calls.insert(call.clone()) && plan.lexical_control.is_none() {
+                    if !committed_calls.insert(function, call, budget)?
+                        && plan.lexical_control.is_none()
+                    {
```
```diff
-                    require_status(function, &statuses, source)?;
+                    require_status_paid(function, &statuses, source, budget)?;
```
```diff
-                validate_owned_edge(function, block.id, *edge, &mut referenced_edges)?;
+                validate_owned_edge_paid(function, block.id.0, edge.0, &mut referenced_edges, budget)?;
```
The same replacement applies inside the `Branch` loop.
```diff
-                    .is_none_or(|value| value >= plan.exits.len())
-                    || !referenced_exits.insert(*exit)
+                    .is_none_or(|value| value >= plan.exits.len())
+                    || !referenced_exits.insert(function, exit.0, budget)?
```
```diff
-    let expected_calls = expressions
-        .entries_paid(function, budget)?
-        .filter_map(|(expression, fact)| fact.as_ref().map(|_| expression.clone()))
-        .collect::<BTreeSet<_>>();
+    let mut expected_calls = Directory::new("cleanup expected-call directory");
+    for (expression, fact) in expressions.entries_paid(function, budget)? {
+        if fact.is_some() {
+            expected_calls.insert(function, expression, budget)?;
+        }
+    }
 ...
-    if plan.lexical_control.is_none() && committed_calls != expected_calls {
+    if plan.lexical_control.is_none()
+        && !committed_calls.same_keys(&expected_calls, function, budget)?
+    {
```
Helper changes (the `validate_edge_condition` signature changes to `statuses: &Directory<&StatusSourceId>` and `budget` is passed through):
```diff
-        EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
-            require_status(function, statuses, source)
-        }
+        EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
+            require_status_paid(function, statuses, source, budget)
+        }
```
```rust
fn require_status_paid(
    function: &ResolvedFunction,
    statuses: &Directory<&StatusSourceId>,
    source: &StatusSourceId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if statuses.contains(function, &source, budget)? {
        Ok(())
    } else {
        Err(replay_error(
            function,
            format_args!(
                "cleanup plan references unknown status source for expression `{}`",
                source.expression
            ),
        ))
    }
}

fn validate_owned_edge_paid(
    function: &ResolvedFunction,
    block: u32,
    edge: u32,
    referenced: &mut Directory<u32>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let Some(record) = usize::try_from(edge)
        .ok()
        .and_then(|index| function.cleanup_plan.edges.get(index))
    else {
        return Err(replay_error(function, "cleanup block references an unknown edge"));
    };
    if record.from.0 != block || !referenced.insert(function, edge, budget)? {
        return Err(replay_error(
            function,
            "cleanup edge is owned by the wrong block or referenced twice",
        ));
    }
    Ok(())
}
```

## Things I did not see and did not assume

- **`require_status` and `validate_owned_edge`:** I wrote the paid versions above from their call sites only. Compare their checks, check order, and error text against the parent originals. Any test that matches those messages may need the text aligned.
- **Budget API:** the file calls `reserve_bytes(function, 0, &str)`, `charge(function, usize, &str)`, `budget_refusal(function, &str, "")`, and `reserve_vec(function, &mut Vec<T>, usize, &str)`. I used the signatures shown and assume they return `Result<_, Diagnostic>`. `reserve_vec` is `pub(super)`, so it must be visible from `replay::…::directory`, which holds only if the budget module sits directly under `replay`.
- **Names via `super::*`:** `ResolvedFunction`, `ReplayBudget`, `Diagnostic`, `ExpressionId`, `StatusSourceId`, and `replay_error` must resolve through the receiving file's glob import. `ExpressionId` must be `Ord` with `as_str()`.
- **Numeric ids:** `EdgeId.0`, `ExitTargetId.0`, and `BlockId.0` are used as public `u32` fields, as the shown code does.
- **`reserve_vec` when not paying:** it returns early if `pays_allocations()` is false, so growth in that mode is the plain `push`. That matches existing unpaid semantics.

## Behavior notes

- **Duplicate and growth order:** a duplicate insert returns `false` after the metered search and never reaches growth. The arena grows before dispatch, so this is a deliberate boundary shift.
- **Per-call charges:** every insert and lookup does `reserve_bytes0` and `charge0` first. Comparison, relocation, rebalance, and rotation work is charged before it runs, with the arena's constants (3, 16, 20) and no upfront forecast.
- **Failure after link:** a failure after the node is linked leaves the directory ordered but possibly unbalanced. The budget refusal is sticky, so the function fails anyway.
- **Dead code:** `is_disjoint` has no caller yet, because the transfer flag sets aren't converted. That may produce a `dead_code` warning, and I added no waiver. Land it with the flag follow-up or delete it.
- **Dropped helpers:** I omitted a paid `iter` helper, since ordered walking is already exercised through `same_keys` and `is_disjoint`. I also omitted a `balanced` test hook, since nothing was to be run.

## Still unpriced and needing a scoped follow-up

- **Arithmetic cases:** the `normalized_cases` `BTreeSet` and its `is_empty`/length comparison in `validate_status_sources_core`.
- **Transfer and consumed flag sets:** the `source_flags`/`destination_flags` sets with `is_disjoint`, and the `consumed` set in `CallCommit`. Their key types weren't shown. Each needs a `KeyWork` impl for the concrete numeric or enum key, plus `Directory<Key>` at those sites.

So the function is not wholly funded. Only the status, edge, exit, and call directories are paid.

Funding these directories can move resource-boundary refusals relative to the old thresholds, so I make no claim of physical or error equality there. With enough budget, the accept/reject result and semantic error priority are intended to be unchanged. `Facts::Tree` still routes to the original old-oracle functions untouched.
