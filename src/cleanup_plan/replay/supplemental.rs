//! Supplemental call-argument storage replay.
use super::*;

pub(super) fn collect_supplemental_slots(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    next_flag: &mut u32,
    slots: &mut Vec<ExpectedSupplementalSlot>,
) -> Result<(), Diagnostic> {
    enum Frame<'a> {
        Expr(&'a ResolvedExpr, usize),
        CallArgument(&'a ResolvedExpr, usize),
    }
    let mut frames = Vec::with_capacity(1028);
    frames.push(Frame::Expr(expression, 0));
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Expr(expression, next) => {
                if let Some(super::super::native_rust::CallParts {
                    callee,
                    instance,
                    args,
                    type_arguments,
                }) = parts(expression)
                {
                    let params =
                        resolved_call_params(program, function, callee, instance, type_arguments)?;
                    if params.len() != args.len() {
                        return Err(replay_error(
                            function,
                            format!("cleanup call `{}` has inconsistent arity", expression.id),
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
                let Some(super::super::native_rust::CallParts {
                    callee,
                    instance,
                    args,
                    type_arguments,
                }) = parts(expression)
                else {
                    unreachable!("call-argument continuation retains a call");
                };
                let params =
                    resolved_call_params(program, function, callee, instance, type_arguments)?;
                let argument = &args[index];
                let parameter = &params[index];
                if parameter.ownership == OwnershipMode::Own
                    && type_needs_drop(program, function, &parameter.ty)?
                {
                    let parameter_index = u32::try_from(index)
                        .map_err(|_| replay_error(function, "too many call parameters"))?;
                    let storage = StorageId::CallArgument {
                        call: expression.id.clone(),
                        parameter_index,
                        value_expression: argument.id.clone(),
                    };
                    let shape =
                        expected_shape_for_type(program, function, &argument.ty, next_flag)?;
                    slots.push(ExpectedSupplementalSlot {
                        storage,
                        ty: argument.ty.clone(),
                        shape,
                    });
                }
            }
        }
    }
    Ok(())
}

// Affine creation and invocation have ordinary owned argument epochs even
// though their checked syntax is not an ordinary named Call node.
fn parts(expression: &ResolvedExpr) -> Option<super::super::native_rust::CallParts<'_>> {
    if let Some((callee, args)) = crate::hir::closure::once::call(expression) {
        Some(super::super::native_rust::CallParts {
            callee,
            args,
            instance: None,
            type_arguments: &[],
        })
    } else {
        super::super::native_rust::parts(expression)
    }
}
