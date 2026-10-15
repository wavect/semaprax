//! One real paid fixed cursor, reused after each complete source-expression walk.
//! Synthetic owning parameter providers retain their explicit legacy fallback.
use super::*;
use crate::cleanup_plan::replay::nested_shape::type_needs_drop_with_budget;
use crate::cleanup_plan::replay::ordinary_inventory::storage::array;

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
        Ok(Self {
            frames: array(function, 1028, budget, "supplemental fixed cursor")?,
        })
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
                        if frames.len() + 3 > frames.capacity() {
                            return Err(replay_error(
                                function,
                                "supplemental-slot traversal exceeds the admitted depth",
                            ));
                        }
                        frames.push(Frame::Expr(expression, next + 1));
                        frames.push(Frame::CallArgument(expression, next));
                        frames.push(Frame::Expr(argument, 0));
                    }
                } else if let Some(child) = replay_expression_child(expression, next) {
                    if frames.len() + 2 > frames.capacity() {
                        return Err(replay_error(
                            function,
                            "supplemental-slot traversal exceeds the admitted depth",
                        ));
                    }
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
