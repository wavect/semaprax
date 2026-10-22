//! Actual new singleton slots; old producer/Result tariffs are never credited.
use super::*;
use std::mem::size_of;

// One newly distinct signature-work site and one receiving-byte site. These
// narrow, sequential primitive frames belong to this exact id_inner activation.
// The existing direct-root request acquires them before either can form.
type Primitive<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, usize, &'a str);
type FrameGrowth<'a, 'hir> = (
    &'a mut ReplayBudget,
    &'a ResolvedFunction,
    &'a mut Vec<CanonicalFrame<'hir>>,
    usize,
    &'a str,
);
// Complete original owned-One frame shape remains a layout/invoice witness.
// It is not an executable old oracle: that is the unchanged parent owner.
#[cfg(test)]
pub(super) enum OwnedFrameBefore {
    Enter(SharedWeighted),
    Exit(SharedWeighted, u64, Vec<(Label, SharedWeighted)>),
    ExitOne(SharedWeighted, u64, (Label, SharedWeighted)),
}
#[cfg(test)]
type OwnedFrameGrowthBefore<'a> = (
    &'a mut ReplayBudget,
    &'a ResolvedFunction,
    &'a mut Vec<OwnedFrameBefore>,
    usize,
    &'a str,
);
// Historical owned-One candidate had eight additional growth frames. Keep its
// exact typed invoice independently, without treating it as live credit.
#[cfg(test)]
pub(super) const OWNED_SCHEDULE_PRIMITIVES: usize = 8
    * (2 * size_of::<OwnedFrameGrowthBefore<'static>>() + 2 * size_of::<Result<(), Diagnostic>>());
// Actual shared lexical continuation leaves only two scheduling expansions:
// original vector and new One, hence two additional growth primitive sites.
pub(super) const SCHEDULE_PRIMITIVES: usize =
    2 * (2 * size_of::<FrameGrowth<'static, 'static>>() + 2 * size_of::<Result<(), Diagnostic>>());
// Direct branch assignments reuse the exact old producer-output/caller
// receiving roles. They form no extra transport or payment call. The complete
// old Result/Option/Vec banks remain unchanged, without a fee credit.
pub(super) const PRIMITIVES: usize = 2
    * (2 * size_of::<Primitive<'static>>() + 2 * size_of::<Result<(), Diagnostic>>())
    + SCHEDULE_PRIMITIVES;

// Actual borrowed OnePassLabels selector, its constant-pattern predicate and
// the conditional reserve operand. PUBLICATION pays before the selector reads.
pub(super) const SELECT: usize = 2 * size_of::<&lexical_successors::OnePassLabels<'static>>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<usize>();

// Label references point into the immutable automaton held for 'hir. No
// temporary Label/Rc clone is constructed or destroyed on a signature hit.
// Both original state and child Rc owners retain their exact old Drop points.
// A miss clones into the final owned signature; all clone actual/formal
// reference and owned return/receiving fields are prepaid here, independently
// of the conservatively retained original publication-clone fees.
pub(super) const FIELDS: usize = 2 * size_of::<(&Label, SharedWeighted)>()
    + 2 * size_of::<&Label>()
    + 2 * size_of::<SharedWeighted>()
    + size_of::<&SharedWeighted>()
    + 2 * size_of::<&Label>()
    + 2 * size_of::<Label>();

// The exact real query type owns its actual fields. Fresh &Rc query receivers
// and the existing scalar key copy remain; iterator/remove alone disappeared.
pub(super) const RECEIVING: usize =
    lexical_successors::frame_loop::borrowed_single::INLINE_RECEIVING;

#[cfg(test)]
pub(super) mod census;
