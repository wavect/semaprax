//! Per-variable ownership state carried by source verification: bindings,
//! invocation-local loans, borrow origins, the availability lattice, and the
//! checked value produced by every expression.

use crate::ast::{ParamMode, Type};
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Clone, Debug)]
pub(super) struct Binding {
    pub(super) ty: Type,
    pub(super) mode: ParamMode,
    pub(super) availability: Availability,
    pub(super) moved_places: HashMap<Vec<String>, Availability>,
    pub(super) definitely_partial: HashSet<Vec<String>>,
    pub(super) native_unit_discard: bool,
    /// Explicit Mutation v1: only local `let mut` bindings are mutable.
    pub(super) mutable: bool,
    /// Exact invocation-local shared loans currently protecting this place.
    /// A set, rather than a Boolean or counter, makes nested settlement
    /// remove only the loan it created.
    pub(super) active_loans: BTreeSet<SourceLoan>,
    /// Ultimate local owner protected by this borrowed slice binding. Slice
    /// aliases and ranges retain this origin instead of treating the parent
    /// descriptor as independently owned storage.
    pub(super) borrow_origin: Option<BorrowOrigin>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct SourceLoan {
    pub(super) id: SourceLoanId,
    pub(super) projections: Vec<String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct SourceLoanId {
    pub(super) borrower: String,
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BorrowOrigin {
    pub(super) root: String,
    pub(super) projections: Vec<String>,
    pub(super) loan: SourceLoanId,
    pub(super) parent: Option<SourceLoanId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Availability {
    Available,
    Moved,
    MaybeMoved,
}

impl Availability {
    pub(super) fn join(self, other: Self) -> Self {
        if self == other {
            self
        } else {
            Availability::MaybeMoved
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct CheckedValue {
    pub(super) ty: Type,
    pub(super) mode: ParamMode,
    pub(super) native_unit: bool,
}

impl CheckedValue {
    pub(super) fn value(ty: Type) -> Self {
        // A literal of a uniquely-owned type is one fresh owner, not a Copy
        // value. Keep source verification aligned with resolved HIR so the
        // value can cross exactly one ownership-taking call boundary.
        let mode = if ty.is_uniquely_owned() {
            ParamMode::Own
        } else {
            ParamMode::Value
        };
        Self {
            ty,
            mode,
            native_unit: false,
        }
    }

    pub(super) fn returned(ty: Type, contains_resource: bool) -> Self {
        let mode = if contains_resource || ty.is_uniquely_owned() {
            ParamMode::Own
        } else {
            ParamMode::Value
        };
        Self {
            ty,
            mode,
            native_unit: false,
        }
    }
}
