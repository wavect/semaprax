//! Resolver-private expression path coordinates.
//!
//! A continuation retains a four-byte `PathId`. Each arena node retains only
//! its parent, its own closed edge spelling (a static segment and/or one
//! checked decimal index) and the checked accumulated UTF-8 length; no node
//! repeats an ancestor prefix. Final identities stream the unchanged scoped
//! spelling back to front into one exact buffer, so text exists only where a
//! receiver genuinely requires it.
//!
//! Handles are construction coordinates. They never authenticate source
//! membership, never intern or hash a public identity, and never replace any
//! source, HIR, loan or cleanup validation.
//!
//! Reclamation: a node records the frame-stack height when it was created.
//! Handles live only in resolver frames and in locals of the frame being
//! processed, and frames are pushed only after their nodes exist, so every
//! frame referencing a node sits at a position at least that height. After
//! each pop the nodes created above the new height are therefore unreferenced,
//! and creation heights are non-decreasing among live nodes, so they form a
//! suffix the arena truncates.
//!
//! Slot bound: every edge is created after its event's first push and is
//! pushed immediately, so live heights are distinct and lie in
//! `1..=frames.len()`. At most one node is live per frame-stack slot, and the
//! backing never exceeds the frame vector's capacity in nodes; see
//! `crate::hir::RESOLVER_FRAME_BYTES`.

use std::fmt;

use super::{ExpressionId, FunctionExecutionId, ValueId};
use crate::hir::ids::streamed;

/// The closed set of static edge spellings. Every path the resolver derives is
/// its root followed by these segments and checked decimal indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Seg {
    None,
    Value,
    Left,
    Right,
    Condition,
    Then,
    Else,
    Scrutinee,
    Operand,
    Request,
    Base,
    Tail,
    Source,
    Callable,
    Arg,
    NativeRustArg,
    Statement,
    Field,
    Arm,
    Body,
    Guard,
    Binding,
    BindingField,
    Record,
    ArgZero,
    ArgOne,
    BaseArgZero,
    BaseArgOne,
    ArgZeroSource,
    ForBody,
    ForCondition,
    ForItem,
    ForOwnSource,
    ForOwnItem,
    ForOwnBody,
    ForOwnCondition,
}

impl Seg {
    #[cfg(test)]
    pub(crate) const ALL: [Seg; 36] = [
        Seg::None,
        Seg::Value,
        Seg::Left,
        Seg::Right,
        Seg::Condition,
        Seg::Then,
        Seg::Else,
        Seg::Scrutinee,
        Seg::Operand,
        Seg::Request,
        Seg::Base,
        Seg::Tail,
        Seg::Source,
        Seg::Callable,
        Seg::Arg,
        Seg::NativeRustArg,
        Seg::Statement,
        Seg::Field,
        Seg::Arm,
        Seg::Body,
        Seg::Guard,
        Seg::Binding,
        Seg::BindingField,
        Seg::Record,
        Seg::ArgZero,
        Seg::ArgOne,
        Seg::BaseArgZero,
        Seg::BaseArgOne,
        Seg::ArgZeroSource,
        Seg::ForBody,
        Seg::ForCondition,
        Seg::ForItem,
        Seg::ForOwnSource,
        Seg::ForOwnItem,
        Seg::ForOwnBody,
        Seg::ForOwnCondition,
    ];

    /// This segment followed by one checked decimal index.
    pub(crate) const fn at(self, index: usize) -> Step {
        Step {
            index,
            lead: self,
            trail: Seg::None,
            indexed: true,
        }
    }

    pub(crate) const fn text(self) -> &'static str {
        match self {
            Seg::None => "",
            Seg::Value => ".value",
            Seg::Left => ".left",
            Seg::Right => ".right",
            Seg::Condition => ".condition",
            Seg::Then => ".then",
            Seg::Else => ".else",
            Seg::Scrutinee => ".scrutinee",
            Seg::Operand => ".operand",
            Seg::Request => ".request",
            Seg::Base => ".base",
            Seg::Tail => ".tail",
            Seg::Source => ".source",
            Seg::Callable => ".callable",
            Seg::Arg => ".arg.",
            Seg::NativeRustArg => ".native-rust-arg.",
            Seg::Statement => ".s",
            Seg::Field => ".field.",
            Seg::Arm => ".arm.",
            Seg::Body => ".body",
            Seg::Guard => ".guard",
            Seg::Binding => ".binding",
            Seg::BindingField => ".binding.",
            Seg::Record => ".record",
            Seg::ArgZero => ".arg.0",
            Seg::ArgOne => ".arg.1",
            Seg::BaseArgZero => ".base.arg.0",
            Seg::BaseArgOne => ".base.arg.1",
            Seg::ArgZeroSource => ".arg.0.source",
            Seg::ForBody => ".value.s2.body.s1.value",
            Seg::ForCondition => ".value.s2.condition",
            Seg::ForItem => ".value.s2.body.s0",
            Seg::ForOwnSource => ".value.s0.value.arg.0",
            Seg::ForOwnItem => ".value.s1.body.s0.value.arm.1.binding.0",
            Seg::ForOwnBody => ".value.s1.body.s0.value.arm.1.value.s0.value",
            Seg::ForOwnCondition => ".value.s1.condition",
        }
    }
}

/// One edge: `lead`, then the decimal `index` when `indexed`, then `trail`.
/// Every edge spells at least one byte, so a parent walk takes at most as
/// many steps as the path has bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Step {
    index: usize,
    lead: Seg,
    trail: Seg,
    indexed: bool,
}

impl From<Seg> for Step {
    fn from(lead: Seg) -> Self {
        assert!(lead != Seg::None, "an unindexed edge spells a segment");
        Self {
            index: 0,
            lead,
            trail: Seg::None,
            indexed: false,
        }
    }
}

impl Step {
    /// A bare checked decimal index edge.
    pub(crate) const fn index(index: usize) -> Self {
        Seg::None.at(index)
    }

    /// This indexed edge followed by a static trailing segment.
    pub(crate) const fn then(self, trail: Seg) -> Self {
        Self { trail, ..self }
    }

    fn len(self) -> Option<usize> {
        let digits = if self.indexed {
            streamed::decimal_len(self.index)
        } else {
            0
        };
        let edge = self.lead.text().len().checked_add(digits)?;
        edge.checked_add(self.trail.text().len())
    }

    /// Write this edge so that it ends at `end`; returns its start.
    fn write_back(self, bytes: &mut [u8], end: usize) -> usize {
        let trail = self.trail.text();
        let mut at = end - trail.len();
        bytes[at..end].copy_from_slice(trail.as_bytes());
        if self.indexed {
            at = streamed::put_decimal_back(bytes, at, self.index);
        }
        let lead = self.lead.text();
        let start = at - lead.len();
        bytes[start..at].copy_from_slice(lead.as_bytes());
        start
    }
}

impl fmt::Display for Step {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.lead.text())?;
        if self.indexed {
            write!(formatter, "{}", self.index)?;
        }
        formatter.write_str(self.trail.text())
    }
}

/// A suffix of edges spelled after a path without creating arena nodes.
struct Suffix<'a>(&'a [Step]);

impl fmt::Display for Suffix<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .try_for_each(|step| fmt::Display::fmt(step, formatter))
    }
}

/// Handle to one live arena node. Copy, four bytes, no backing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PathId(u32);

impl PathId {
    pub(crate) const ROOT: Self = Self(0);
    /// Spelled empty, as the formatting route spelled a refused path.
    const REFUSED: Self = Self(u32::MAX);
}

/// Compact node: checked `u32` coordinates. Every bound is far beyond any
/// admitted budget (a 4 GiB path or four billion siblings); exceeding one is
/// a sticky refusal, never a truncation.
#[derive(Clone, Copy, Debug)]
struct PathNode {
    parent: u32,
    height: u32,
    len: u32,
    index: u32,
    lead: Seg,
    trail: Seg,
    indexed: bool,
}

impl PathNode {
    fn step(self) -> Step {
        Step {
            index: self.index as usize,
            lead: self.lead,
            trail: self.trail,
            indexed: self.indexed,
        }
    }
}

/// Backing bytes per frame-stack slot; part of `RESOLVER_FRAME_BYTES`.
pub(crate) const PATH_NODE_BYTES: usize = std::mem::size_of::<PathNode>();

/// Test-only literal work ledger for the analytical controls.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PathWork {
    pub(crate) nodes_created: usize,
    pub(crate) nodes_released: usize,
    pub(crate) node_bytes_charged: usize,
    pub(crate) parent_steps: usize,
    pub(crate) path_bytes_written: usize,
    pub(crate) high_water_nodes: usize,
}

pub(crate) struct PathArena<'r> {
    root: &'r str,
    nodes: Vec<PathNode>,
    refused: bool,
    #[cfg(test)]
    work: std::cell::Cell<PathWork>,
}

impl<'r> PathArena<'r> {
    /// Borrows the root spelling; allocates nothing until the first edge.
    pub(crate) fn new(root: &'r str) -> Self {
        Self {
            root,
            nodes: Vec::new(),
            refused: false,
            #[cfg(test)]
            work: std::cell::Cell::new(PathWork::default()),
        }
    }

    /// Append one edge under `parent` for a frame pushed immediately after.
    /// Create it only after the event's first push (a continuation that does
    /// not hold it), so sibling edges are released with that continuation.
    pub(crate) fn child<T>(
        &mut self,
        frames: &Vec<T>,
        parent: PathId,
        step: impl Into<Step>,
    ) -> PathId {
        self.child_at(frames.len(), frames.capacity(), parent, step.into())
    }

    /// `height` is the frame-stack length at creation; see the module note for
    /// why that bounds every reference. `slots` is the frame vector capacity.
    ///
    /// The backing grows geometrically but never beyond the frame capacity,
    /// and each new allocation is charged as required work before it is made,
    /// so the cumulative debit covers the old/new overlap. A refusal is sticky
    /// and every later handle spells empty text.
    pub(crate) fn child_at(
        &mut self,
        height: usize,
        slots: usize,
        parent: PathId,
        step: Step,
    ) -> PathId {
        if self.refused || parent == PathId::REFUSED {
            return PathId::REFUSED;
        }
        debug_assert!(self.nodes.len() < height, "one live edge per frame slot");
        let len = step
            .len()
            .and_then(|edge| self.len(parent).checked_add(edge))
            .and_then(|len| u32::try_from(len).ok());
        let next = self
            .nodes
            .len()
            .checked_add(1)
            .and_then(|next| u32::try_from(next).ok())
            .filter(|next| *next != PathId::REFUSED.0);
        let index = u32::try_from(step.index).ok();
        let (Some(len), Some(next), Some(index), Ok(height)) =
            (len, next, index, u32::try_from(height))
        else {
            return self.refuse();
        };
        if self.nodes.len() == self.nodes.capacity() && !self.grow(slots) {
            return PathId::REFUSED;
        }
        self.nodes.push(PathNode {
            parent: parent.0,
            height,
            len,
            index,
            lead: step.lead,
            trail: step.trail,
            indexed: step.indexed,
        });
        #[cfg(test)]
        self.note(|work| {
            work.nodes_created += 1;
            work.high_water_nodes = work.high_water_nodes.max(self.nodes.len());
        });
        PathId(next)
    }

    /// Geometric growth capped by the frame capacity `slots`.
    fn grow(&mut self, slots: usize) -> bool {
        let doubled = self.nodes.capacity().saturating_mul(2).max(4).min(slots);
        let capacity = self
            .nodes
            .len()
            .checked_add(1)
            .map(|needed| needed.max(doubled));
        let bytes = capacity.and_then(|capacity| capacity.checked_mul(PATH_NODE_BYTES));
        let (Some(capacity), Some(bytes)) = (capacity, bytes) else {
            self.refuse();
            return false;
        };
        if capacity > slots {
            self.refuse();
            return false;
        }
        if !crate::bounded_output::reserve_active_required(bytes) {
            self.refused = true;
            return false;
        }
        self.nodes.reserve_exact(capacity - self.nodes.len());
        #[cfg(test)]
        self.note(|work| work.node_bytes_charged += bytes);
        self.admit_growth(capacity, slots)
    }

    /// An allocator may grant more capacity than requested. Reconcile that
    /// exceptional backing with the same ledger, then refuse before publishing
    /// a node: only exact capacity preserves the frame-slot forecast. If the
    /// delta refuses, the excess allocation has already happened; it is not a
    /// claim of prepaid exceptional allocation. Drop retains normal ownership.
    fn admit_growth(&mut self, requested: usize, slots: usize) -> bool {
        let actual = self.nodes.capacity();
        if actual > requested {
            let Some(extra) = (actual - requested).checked_mul(PATH_NODE_BYTES) else {
                self.refuse();
                return false;
            };
            if !crate::bounded_output::reserve_active_required(extra) {
                self.refused = true;
                return false;
            }
            #[cfg(test)]
            self.note(|work| work.node_bytes_charged += extra);
        }
        if actual != requested || actual > slots {
            self.refuse();
            return false;
        }
        true
    }

    fn refuse(&mut self) -> PathId {
        self.refused = true;
        streamed::refuse_unaddressable();
        PathId::REFUSED
    }

    /// Drop the nodes created above the current frame-stack height.
    pub(crate) fn release_above(&mut self, height: usize) {
        while self
            .nodes
            .last()
            .is_some_and(|node| node.height as usize > height)
        {
            self.nodes.pop();
            #[cfg(test)]
            self.note(|work| work.nodes_released += 1);
        }
    }

    pub(crate) fn at(&self, id: PathId) -> ExprPath<'_> {
        ExprPath(Repr::Arena(self, id))
    }

    fn len(&self, id: PathId) -> usize {
        match id {
            PathId::ROOT => self.root.len(),
            PathId::REFUSED => 0,
            PathId(index) => self.nodes[index as usize - 1].len as usize,
        }
    }

    /// Fill `bytes` (exactly the path plus suffix length) back to front.
    fn fill(&self, id: PathId, suffix: &[Step], bytes: &mut [u8]) {
        let mut end = bytes.len();
        for step in suffix.iter().rev() {
            end = step.write_back(bytes, end);
        }
        let mut at = id;
        loop {
            match at {
                PathId::ROOT => {
                    bytes[..end].copy_from_slice(self.root.as_bytes());
                    break;
                }
                PathId::REFUSED => {
                    assert_eq!(end, 0, "a refused path spells no prefix");
                    break;
                }
                PathId(index) => {
                    let node = self.nodes[index as usize - 1];
                    end = node.step().write_back(bytes, end);
                    at = PathId(node.parent);
                    #[cfg(test)]
                    self.note(|work| work.parent_steps += 1);
                }
            }
        }
        #[cfg(test)]
        self.note(|work| work.path_bytes_written += bytes.len());
    }

    #[cfg(test)]
    fn note(&self, update: impl FnOnce(&mut PathWork)) {
        let mut work = self.work.get();
        update(&mut work);
        self.work.set(work);
    }

    #[cfg(test)]
    pub(crate) fn work(&self) -> PathWork {
        self.work.get()
    }

    /// Owned backing for the resolver's test-only retained-capacity census.
    #[cfg(test)]
    pub(crate) fn owned_capacity(&self) -> usize {
        self.nodes.capacity() * PATH_NODE_BYTES
    }

    #[cfg(test)]
    pub(crate) fn live_nodes(&self) -> usize {
        self.nodes.len()
    }
}

/// Borrowed coordinate accepted by helpers shared with the recursive
/// reference resolver. `Text` keeps the formatting encoder (`ExpressionId::new`
/// and `ValueId::local`) byte for byte; `Arena` streams the same spelling.
#[derive(Clone, Copy)]
pub(crate) struct ExprPath<'a>(Repr<'a>);

#[derive(Clone, Copy)]
enum Repr<'a> {
    Text(&'a str),
    Arena(&'a PathArena<'a>, PathId),
}

impl<'a> From<&'a str> for ExprPath<'a> {
    fn from(text: &'a str) -> Self {
        Self(Repr::Text(text))
    }
}

impl<'a> From<&'a String> for ExprPath<'a> {
    fn from(text: &'a String) -> Self {
        Self(Repr::Text(text))
    }
}

impl ExprPath<'_> {
    pub(crate) fn expression_id(self, function: &FunctionExecutionId) -> ExpressionId {
        self.expression_id_with(function, &[])
    }

    pub(crate) fn expression_id_with(
        self,
        function: &FunctionExecutionId,
        suffix: &[Step],
    ) -> ExpressionId {
        match self.0 {
            Repr::Text(text) if suffix.is_empty() => ExpressionId::new(function, text),
            Repr::Text(text) => ExpressionId::new(function, &format!("{text}{}", Suffix(suffix))),
            Repr::Arena(..) => {
                ExpressionId::from_owned(self.stream(function, "expression", suffix))
            }
        }
    }

    pub(crate) fn value_local_with(
        self,
        function: &FunctionExecutionId,
        suffix: &[Step],
    ) -> ValueId {
        match self.0 {
            Repr::Text(text) if suffix.is_empty() => ValueId::local(function, text),
            Repr::Text(text) => ValueId::local(function, &format!("{text}{}", Suffix(suffix))),
            Repr::Arena(..) => ValueId::new(self.stream(function, "value:local", suffix)),
        }
    }

    /// Borrow an existing textual path, or hold one exactly paid arena spelling
    /// only while a receiver that requires source path text uses it.
    pub(crate) fn with_text<T>(self, receive: impl FnOnce(&str) -> T) -> T {
        match self.0 {
            Repr::Text(text) => receive(text),
            Repr::Arena(..) => receive(&self.materialize()),
        }
    }

    pub(crate) fn materialize(self) -> String {
        self.materialize_with(&[])
    }

    /// Owned text for a receiver that genuinely requires it. The exact length
    /// is charged with the formatting route's reservation before allocation.
    pub(crate) fn materialize_with(self, suffix: &[Step]) -> String {
        match self.0 {
            Repr::Text(text) => format!("{text}{}", Suffix(suffix)),
            Repr::Arena(arena, id) => {
                let Some(len) = suffix_len(arena.len(id), suffix) else {
                    return streamed::refuse_unaddressable();
                };
                if !crate::bounded_output::reserve_active(len) {
                    return String::new();
                }
                let mut bytes = vec![0u8; len];
                arena.fill(id, suffix, &mut bytes);
                String::from_utf8(bytes).expect("path arena writes complete UTF-8 components")
            }
        }
    }

    fn stream(self, function: &FunctionExecutionId, kind: &str, suffix: &[Step]) -> String {
        let Repr::Arena(arena, id) = self.0 else {
            unreachable!("text coordinates use the formatting encoder")
        };
        let Some(len) = suffix_len(arena.len(id), suffix) else {
            return streamed::refuse_unaddressable();
        };
        streamed::emit(function, kind, len, |bytes| arena.fill(id, suffix, bytes))
    }
}

fn suffix_len(base: usize, suffix: &[Step]) -> Option<usize> {
    suffix
        .iter()
        .try_fold(base, |len, step| len.checked_add(step.len()?))
}

#[cfg(any())]
#[path = "path_arena_tests.rs"]
mod path_arena_tests;

#[cfg(any())]
#[path = "path_arena_regression_tests.rs"]
mod path_arena_regression_tests;
