//! Only the empty-signature route omits the borrowed query and child iterators.
//! These are actual constructor layouts, never an allowance or an old fee credit.
use super::{Diagnostic, Label};
use std::mem::size_of;
// New branch predicate and Vec receiver, plus its real payment Result.
pub(super) const BRANCH_CONTROLS: usize = 2 * size_of::<bool>()
    + size_of::<&Vec<(Label, super::SharedWeighted)>>()
    + size_of::<Result<(), Diagnostic>>();
// Complete initialized/received signature and Vec initializer, dictionary
// query argument/result and copied ID; the caller's unit destination receives
// every remaining nested Result. Node initialization remains separately paid.
pub(super) const EMPTY_CONTROLS: usize = 2 * size_of::<super::intern_arena::Signature>()
    + size_of::<Vec<(Label, usize)>>()
    + size_of::<&super::intern_arena::Signature>()
    + size_of::<Option<&usize>>()
    + 2 * size_of::<usize>()
    + size_of::<Result<(), Diagnostic>>();
