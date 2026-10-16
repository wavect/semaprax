//! One paid depth-bounded cursor, reused after each complete expression walk.
//! Synthetic owning parameter providers retain their explicit legacy fallback.
use super::*;
use crate::cleanup_plan::replay::nested_shape::type_needs_drop_with_budget;
const FRAME_CAPACITY: usize = 1028;

enum Frame<'a> {
    Expr(&'a ResolvedExpr, usize),
    CallArgument(&'a ResolvedExpr, usize),
}
pub(in crate::cleanup_plan::replay) struct Scratch<'a> {
    frames: Vec<Frame<'a>>,
}
impl Scratch<'_> {
    pub(in crate::cleanup_plan::replay) fn new(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        budget.reserve_array::<Self>(function, 1, "supplemental cursor owner")?;
        let mut frames = Vec::new();
        reserve_frames(&mut frames, 1, function, budget)?;
        Ok(Self { frames })
    }
}

pub(in crate::cleanup_plan::replay) fn walk<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &'a ResolvedExpr,
    next_flag: &mut u32,
    scratch: &mut Scratch<'a>,
    budget: &mut ReplayBudget,
    mut receive: impl FnMut(StorageId, &ResolvedType, FieldLivenessShape) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    budget.reserve_bytes(function, 0, "supplemental receiving cursor")?;
    let frames = &mut scratch.frames;
    debug_assert!(frames.is_empty());
    reserve_frames(frames, 1, function, budget)?;
    frames.push(Frame::Expr(expression, 0));
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Expr(expression, next) => {
                if let Some(crate::cleanup_plan::native_rust::CallParts {
                    callee,
                    instance,
                    args,
                    type_arguments,
                }) = parts(expression, function)?
                {
                    let params = resolved_call::resolved_call_param_view(
                        program,
                        function,
                        expression,
                        callee,
                        instance,
                        type_arguments,
                    )?;
                    if params.len() != args.len() {
                        return Err(replay_error(
                            function,
                            format_args!("cleanup call `{}` has inconsistent arity", expression.id),
                        ));
                    }
                    if let Some(argument) = args.get(next) {
                        if frames.len() + 3 > FRAME_CAPACITY {
                            return Err(replay_error(
                                function,
                                "supplemental-slot traversal exceeds the admitted depth",
                            ));
                        }
                        reserve_frames(frames, 3, function, budget)?;
                        frames.push(Frame::Expr(expression, next + 1));
                        frames.push(Frame::CallArgument(expression, next));
                        frames.push(Frame::Expr(argument, 0));
                    }
                } else if let Some(child) = replay_expression_child(expression, next) {
                    if frames.len() + 2 > FRAME_CAPACITY {
                        return Err(replay_error(
                            function,
                            "supplemental-slot traversal exceeds the admitted depth",
                        ));
                    }
                    reserve_frames(frames, 2, function, budget)?;
                    frames.push(Frame::Expr(expression, next + 1));
                    frames.push(Frame::Expr(child, 0));
                }
            }
            Frame::CallArgument(expression, index) => {
                let Some(crate::cleanup_plan::native_rust::CallParts {
                    callee,
                    instance,
                    args,
                    type_arguments,
                }) = parts(expression, function)?
                else {
                    unreachable!("call-argument continuation retains a call");
                };
                let params = resolved_call::resolved_call_param_view(
                    program,
                    function,
                    expression,
                    callee,
                    instance,
                    type_arguments,
                )?;
                let argument = &args[index];
                let parameter = params
                    .get(index)
                    .expect("call parameter arity checked at entry");
                if parameter.ownership == OwnershipMode::Own
                    && type_needs_drop_with_budget(program, function, parameter.ty, budget)?
                {
                    let parameter_index = u32::try_from(index)
                        .map_err(|_| replay_error(function, "too many call parameters"))?;
                    let storage = StorageId::CallArgument {
                        call: expression.id.clone(),
                        parameter_index,
                        value_expression: argument.id.clone(),
                    };
                    let shape = nested_shape::expected_shape_for_type_paid(
                        program,
                        function,
                        &argument.ty,
                        next_flag,
                        budget,
                    )?;
                    receive(storage, &argument.ty, shape)?;
                }
            }
        }
    }
    Ok(())
}

// Capacity never supplies traversal authority. Replacements retain every live
// continuation and pay their full backing while the old buffer is still held.
fn reserve_frames<'a>(
    frames: &mut Vec<Frame<'a>>,
    additional: usize,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    budget.reserve_bytes(function, 0, "supplemental fixed cursor")?;
    let required = frames
        .len()
        .checked_add(additional)
        .filter(|n| *n <= FRAME_CAPACITY)
        .ok_or_else(|| {
            replay_error(
                function,
                "supplemental-slot traversal exceeds the admitted depth",
            )
        })?;
    if required <= frames.capacity() {
        return Ok(());
    }
    // Unobserved replay keeps the original fixed cursor and performs no
    // relocation. Growing storage belongs to allocation-paying invocations;
    // applying it to legacy callers would add work absent from their receiver.
    let target = if budget.pays_allocations() {
        frames
            .capacity()
            .saturating_mul(2)
            .clamp(required.max(2), FRAME_CAPACITY)
    } else {
        FRAME_CAPACITY
    };
    if !frames.is_empty() {
        budget.charge(function, frames.len(), "supplemental cursor relocation")?;
    }
    budget.reserve_bytes(
        function,
        2 * std::mem::size_of::<Vec<Frame<'a>>>() + std::mem::size_of::<Frame<'a>>(),
        "supplemental cursor controls",
    )?;
    budget.reserve_array::<Frame<'a>>(function, target, "supplemental fixed cursor")?;
    let mut replacement = Vec::new();
    replacement.try_reserve_exact(target).map_err(|_| {
        budget.budget_refusal(
            function,
            "cleanup replay physical vector allocation failed",
            "",
        )
    })?;
    if replacement.capacity() != target {
        return Err(budget.budget_refusal(
            function,
            "cleanup replay physical vector capacity disagrees with its reservation",
            "",
        ));
    }
    replacement.append(frames);
    *frames = replacement;
    Ok(())
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "complete independently compiled original receiving walk"
)]
#[path = "../../../../docs/evidence/supplemental-cursor-growth-v1/receiving.before.rs.txt"]
mod before;
#[cfg(test)]
mod tests;
