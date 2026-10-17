use super::{Frame, ResolvedExpr};
use crate::hir::borrowed_root_oracle::resolve_expr_frame::{
    frame_owned_capacity_with, visit_frame_scopes,
};
use crate::hir::capacity_probe::{note_iterative_phase_capacity, resolved_expr_owned_capacity};

pub(super) fn note(
    frames: &Vec<Frame<'_>>,
    results: &Vec<ResolvedExpr>,
    frame: &Frame<'_>,
    loops: &[Option<crate::hir::ControlTarget>],
    paths: &super::path_arena::PathArena<'_>,
) {
    let owned = owned_frames(frames, frame);
    note_iterative_phase_capacity(
        0,
        frames.capacity() * std::mem::size_of::<Frame<'_>>()
            + results.capacity() * std::mem::size_of::<ResolvedExpr>()
            + results
                .iter()
                .map(resolved_expr_owned_capacity)
                .sum::<usize>()
            + owned
            + paths.owned_capacity()
            + std::mem::size_of_val(loops)
            + loops
                .iter()
                .flatten()
                .map(crate::hir::capacity_probe::lexical_target_capacity)
                .sum::<usize>(),
    );
}

// A capacity observation is harness metadata, not resolver allocation. Scan
// the actual borrowed frame prefix instead of allocating a HashSet per event.
// This trades test-only pointer comparisons for zero measurement scratch;
// production frames/work and the complete payload census remain unchanged.
fn owned_frames(frames: &[Frame<'_>], current: &Frame<'_>) -> usize {
    frames
        .iter()
        .chain(std::iter::once(current))
        .enumerate()
        .fold(0usize, |total, (index, frame)| {
            let mut ordinal = 0usize;
            let owned = frame_owned_capacity_with(frame, &mut |pointer| {
                let mut seen = false;
                for previous in &frames[..index.min(frames.len())] {
                    visit_frame_scopes(previous, |prior| seen |= prior == pointer);
                }
                let mut position = 0usize;
                visit_frame_scopes(frame, |prior| {
                    seen |= position < ordinal && prior == pointer;
                    position += 1;
                });
                ordinal += 1;
                !seen
            });
            total.saturating_add(owned)
        })
}

#[cfg(any())]
mod tests;
