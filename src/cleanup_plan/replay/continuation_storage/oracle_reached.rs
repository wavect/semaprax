//! Oracle-local reached backing. No production or Legacy sharing/credit.
use super::*;
use std::mem::size_of;
// New dispatch/helper actual and formal loans, returned and received Vec sum,
// payment/backing calls and sums, allocation/release layout and pointer roles.
pub(super) const FRESH_CALL: usize = 2 * size_of::<(&ResolvedFunction, &mut ReplayBudget)>()
    + 2 * size_of::<Result<Vec<Frame<'static>>, Diagnostic>>()
    + 4 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 4 * size_of::<Result<(), Diagnostic>>()
    + 3 * size_of::<Vec<Frame<'static>>>()
    + 4 * size_of::<usize>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<std::alloc::Layout>()
    + 4 * size_of::<std::ptr::NonNull<Frame<'static>>>();
// Replacement exists beside old backing; pay complete replacement, never a
// capacity delta. Include retained/moved frame and append/release receivers.
pub(super) const GROW_CALL: usize = 2 * size_of::<(
    &mut Vec<Frame<'static>>,
    &ResolvedFunction,
    &mut ReplayBudget,
)>() + 2 * size_of::<Result<(), Diagnostic>>()
    + 8 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 10 * size_of::<Result<(), Diagnostic>>()
    + 5 * size_of::<Vec<Frame<'static>>>()
    + 8 * size_of::<&mut Vec<Frame<'static>>>()
    + 4 * size_of::<&Vec<Frame<'static>>>()
    + size_of::<Frame<'static>>()
    + 32 * size_of::<usize>()
    + 8 * size_of::<bool>()
    + 4 * size_of::<std::alloc::Layout>()
    + 8 * size_of::<std::ptr::NonNull<Frame<'static>>>();

pub(super) fn fresh<'a>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<Frame<'a>>, Diagnostic> {
    budget.charge(
        function,
        2,
        "oracle continuation initial allocation and release",
    )?;
    budget.reserve_array::<Frame<'a>>(function, 2, "oracle continuation initial backing")?;
    Ok(Vec::with_capacity(2))
}

pub(super) fn grow(
    frames: &mut Vec<Frame<'_>>,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    // Eight setup steps, retained read/move pairs, and old backing release.
    // At deep reaches the cumulative backing/work can exceed the old fixed fee.
    budget.charge(
        function,
        9 + 2 * frames.len(),
        "oracle continuation relocation and release",
    )?;
    let target = frames
        .capacity()
        .saturating_mul(2)
        .clamp((frames.len() + 1).max(2), FRAME_CAPACITY);
    budget.reserve_array::<Frame<'_>>(
        function,
        target,
        "oracle continuation replacement backing",
    )?;
    let mut replacement = Vec::with_capacity(target);
    replacement.append(frames);
    *frames = replacement;
    Ok(())
}

// Independently executable exact predecessor constructor/growth. Full original
// module bytes and digest are held in ../oracle_closure_before/manifest.json.
#[allow(dead_code)]
pub(super) fn old_fresh<'hir>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<Frame<'hir>>, Diagnostic> {
    budget.reserve_array::<Frame<'hir>>(
        function,
        FRAME_CAPACITY,
        "typed HIR continuation stack",
    )?;
    match budget {
        ReplayBudget {
            payer: AllocationPayer::LegacyUnobserved,
            lent_skeleton: true,
            ..
        } => Ok(Vec::with_capacity(2)),
        _ => Ok(Vec::with_capacity(FRAME_CAPACITY)),
    }
}

#[allow(dead_code)]
pub(super) fn old_reserve_push(
    frames: &mut Vec<Frame<'_>>,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if frames.len() >= FRAME_CAPACITY {
        return Err(replay_error(
            function,
            "typed-HIR skeleton traversal exceeds the admitted depth",
        ));
    }
    budget.reserve_bytes(function, 0, "held continuation growth")?;
    if frames.len() < frames.capacity() {
        return Ok(());
    }
    if let ReplayBudget {
        payer: AllocationPayer::LegacyUnobserved,
        lent_skeleton: true,
        ..
    } = budget
    {
        budget.reserve_bytes(
            function,
            reached::GROW_CALL,
            "reached continuation growth receiving",
        )?;
        return reached::grow(frames, function, budget);
    }
    // Every other payer retains its complete original paying/fallback path.
    // This branch never borrows the ordinary receiver's new growth strategy.
    if !budget.pays_allocations() {
        return Err(replay_error(
            function,
            "typed-HIR skeleton traversal exceeds the admitted depth",
        ));
    }
    // Actual old/new Vec controls, the result, target and retained/moved frame.
    budget.reserve_bytes(
        function,
        2 * std::mem::size_of::<Vec<Frame<'_>>>()
            + std::mem::size_of::<Result<(), Diagnostic>>()
            + std::mem::size_of::<usize>()
            + std::mem::size_of::<Frame<'_>>(),
        "held continuation replacement controls",
    )?;
    // Eight setup steps and a read/move for every retained live frame.
    budget.charge(
        function,
        8 + 2 * frames.len(),
        "held continuation relocation",
    )?;
    let target = frames
        .capacity()
        .saturating_mul(2)
        .clamp((frames.len() + 1).max(2), FRAME_CAPACITY);
    budget.reserve_array::<Frame<'_>>(function, target, "held continuation replacement backing")?;
    let mut replacement = Vec::with_capacity(target);
    replacement.append(frames);
    *frames = replacement;
    Ok(())
}
