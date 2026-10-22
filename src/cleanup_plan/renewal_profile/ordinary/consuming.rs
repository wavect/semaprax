//! Exact owned-record consuming additions; no ordinary winner is replaced.
use super::{compare, iterator_shape, vector, Admission, Directory, Entry, Refusal, Scratch};
use crate::hir::lexical_exit::ChildCursor;
#[cfg(test)]
mod old;
mod stack;
#[cfg(test)]
mod tests;
use self::stack::Stack;
use crate::hir::{
    OwnershipMode as M, ResolvedBinding as B, ResolvedExpr as E, ResolvedExprKind as K,
    ResolvedFunction as F, ResolvedProgram as P, ResolvedStatement as S, ResolvedType as T,
};
fn visit<'a>(
    root: &'a E,
    admission: &mut Admission<'_>,
    mut visitor: impl FnMut(&'a E, &mut Admission<'_>) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    // Each visit owns its stack: the visitor re-enters visit via collect/declares.
    let mut stack = Stack::new(admission)?;
    stack.push(ChildCursor::new(root), admission)?;
    while !stack.is_empty() {
        admission.units(1)?;
        let frame = stack.top();
        let expression = frame.expression();
        if !frame.is_started() {
            visitor(expression, admission)?;
        }
        let child = if matches!(expression.kind, K::Closure { .. }) {
            None
        } else {
            admission.units(1)?;
            frame.next()
        };
        if let Some(child) = child {
            stack.push(ChildCursor::new(child), admission)?;
        } else {
            stack.pop();
        }
    }
    Ok(())
}
fn declares(
    root: &E,
    target: &B,
    mutable: bool,
    admission: &mut Admission<'_>,
) -> Result<bool, Refusal> {
    let mut found = false;
    visit(root, admission, |expression, admission| {
        if let K::Block { statements, .. } | K::VecConstructor { statements, .. } = &expression.kind
        {
            // Preserve any() within one statement list, even once a different
            // list has already found the binding in the old complete walk.
            for statement in statements {
                admission.units(1)?;
                if let S::Let {
                    binding,
                    mutable: actual,
                    ..
                } = statement
                {
                    if compare::values(&binding.id, &target.id, admission)?
                        && compare::types(&binding.ty, &target.ty, admission)?
                        && binding.ownership == target.ownership
                        && (!mutable || *actual)
                    {
                        found = true;
                        break;
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(found)
}
fn seed(step: &B, source: &E, item: &B, admission: &mut Admission<'_>) -> Result<bool, Refusal> {
    if !compare::types(&source.ty, &step.ty, admission)? || source.ownership != M::Own {
        return Ok(false);
    }
    let K::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &source.kind
    else {
        return Ok(false);
    };
    if !compare::text(callee.as_str(), crate::iterator_ops::NEXT_ID, admission)? {
        return Ok(false);
    }
    let [argument] = type_arguments.as_slice() else {
        return Ok(false);
    };
    if !compare::types(argument, &item.ty, admission)? {
        return Ok(false);
    }
    let [source] = args.as_slice() else {
        return Ok(false);
    };
    if source.ownership != M::Own {
        return Ok(false);
    }
    let T::Nominal {
        declaration,
        arguments,
    } = &source.ty
    else {
        return Ok(false);
    };
    if !compare::text(
        declaration.as_str(),
        crate::iterator_ops::ITER_ID,
        admission,
    )? {
        return Ok(false);
    }
    let [argument] = arguments.as_slice() else {
        return Ok(false);
    };
    compare::types(argument, &item.ty, admission)
}
pub(super) fn collect<'a>(
    program: &P,
    function: &'a F,
    output: &mut Scratch<Entry<'a>>,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    let mut candidate = false;
    for declaration in program.declarations.declarations() {
        admission.units(1)?;
        if declaration.kind == crate::hir::DeclarationKind::Record
            && declaration.identity_origin == crate::hir::IdentityOrigin::Explicit
        {
            candidate = true;
            break;
        }
    }
    if !candidate {
        return Ok(());
    }
    admission.reserve(NO_LOOP_SPINE_CONTROLS)?;
    if no_loop_spine(&function.body, admission)? {
        return Ok(());
    }
    visit(&function.body, admission, |expression, admission| {
        let K::Block { statements, tail } = &expression.kind else {
            return Ok(());
        };
        for statement in statements {
            admission.units(1)?;
            let (condition, body) = match statement {
                S::While {
                    condition, body, ..
                }
                | S::LexicalWhile {
                    condition, body, ..
                } => (condition.as_ref(), body.as_ref()),
                _ => continue,
            };
            let Some(protocol) = iterator_shape::recognize(condition, body, admission)? else {
                continue;
            };
            let Some(item) = protocol.owned_item else {
                continue;
            };
            let [S::Let {
                binding: step,
                mutable: true,
                value: source,
                ..
            }, S::While { .. } | S::LexicalWhile { .. }] = statements.as_slice()
            else {
                continue;
            };
            // ResolvedBinding equality includes all of id/name/type/mode.
            if !compare::values(&step.id, &protocol.step.id, admission)?
                || !compare::text(&step.name, &protocol.step.name, admission)?
                || !compare::types(&step.ty, &protocol.step.ty, admission)?
                || step.ownership != protocol.step.ownership
                || step.span != protocol.step.span
                || !(tail.ownership == M::Value && matches!(tail.kind, K::Int(0) | K::Usize(0)))
                || !seed(step, source, item, admission)?
                || item.ownership != M::Own
                || !matches!(&item.ty, T::Nominal {arguments, ..} if arguments.is_empty())
            {
                continue;
            }
            visit(protocol.authored_body, admission, |node, admission| {
                let K::Block { statements, .. } = &node.kind else {
                    return Ok(());
                };
                for statement in statements {
                    admission.units(1)?;
                    let S::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    else {
                        continue;
                    };
                    let T::Nominal { arguments, .. } = &binding.ty else {
                        continue;
                    };
                    let [element] = arguments.as_slice() else {
                        continue;
                    };
                    let K::Call { callee, .. } = &value.kind else {
                        continue;
                    };
                    let op = match crate::vec_ops::by_id_with_work(callee.as_str(), &mut |units| {
                        admission.units(units).is_ok()
                    }) {
                        Ok(Some(op)) => op,
                        Ok(None) => continue,
                        Err(()) => return admission.refuse(Refusal::Work),
                    };
                    if op != crate::vec_ops::VecOp::Push
                        || vector::mode(program, op, &item.ty, admission)? != Some(M::Own)
                        || vector::mode(program, op, element, admission)? != Some(M::Own)
                        || !vector::same_cell(program, binding, value, admission)?
                        || !declares(&function.body, binding, true, admission)?
                        || declares(protocol.authored_body, binding, false, admission)?
                    {
                        continue;
                    }
                    Directory::emit_consuming(output, &value.id, binding, admission)?;
                }
                Ok(())
            })?;
        }
        Ok(())
    })
}

pub(super) fn collect_with_presence<'a>(
    program: &P,
    function: &'a F,
    output: &mut Scratch<Entry<'a>>,
    presence: &super::RecordPresence<'_>,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    if !presence.get(&program.declarations, admission)? {
        return Ok(());
    }
    admission.reserve(NO_LOOP_SPINE_CONTROLS)?;
    if no_loop_spine(&function.body, admission)? {
        return Ok(());
    }
    visit(&function.body, admission, |expression, admission| {
        let K::Block { statements, tail } = &expression.kind else {
            return Ok(());
        };
        for statement in statements {
            admission.units(1)?;
            let (condition, body) = match statement {
                S::While {
                    condition, body, ..
                }
                | S::LexicalWhile {
                    condition, body, ..
                } => (condition.as_ref(), body.as_ref()),
                _ => continue,
            };
            let Some(protocol) = iterator_shape::recognize(condition, body, admission)? else {
                continue;
            };
            let Some(item) = protocol.owned_item else {
                continue;
            };
            let [S::Let {
                binding: step,
                mutable: true,
                value: source,
                ..
            }, S::While { .. } | S::LexicalWhile { .. }] = statements.as_slice()
            else {
                continue;
            };
            // ResolvedBinding equality includes all of id/name/type/mode.
            if !compare::values(&step.id, &protocol.step.id, admission)?
                || !compare::text(&step.name, &protocol.step.name, admission)?
                || !compare::types(&step.ty, &protocol.step.ty, admission)?
                || step.ownership != protocol.step.ownership
                || step.span != protocol.step.span
                || !(tail.ownership == M::Value && matches!(tail.kind, K::Int(0) | K::Usize(0)))
                || !seed(step, source, item, admission)?
                || item.ownership != M::Own
                || !matches!(&item.ty, T::Nominal {arguments, ..} if arguments.is_empty())
            {
                continue;
            }
            visit(protocol.authored_body, admission, |node, admission| {
                let K::Block { statements, .. } = &node.kind else {
                    return Ok(());
                };
                for statement in statements {
                    admission.units(1)?;
                    let S::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    else {
                        continue;
                    };
                    let T::Nominal { arguments, .. } = &binding.ty else {
                        continue;
                    };
                    let [element] = arguments.as_slice() else {
                        continue;
                    };
                    let K::Call { callee, .. } = &value.kind else {
                        continue;
                    };
                    let op = match crate::vec_ops::by_id_with_work(callee.as_str(), &mut |units| {
                        admission.units(units).is_ok()
                    }) {
                        Ok(Some(op)) => op,
                        Ok(None) => continue,
                        Err(()) => return admission.refuse(Refusal::Work),
                    };
                    if op != crate::vec_ops::VecOp::Push
                        || vector::mode(program, op, &item.ty, admission)? != Some(M::Own)
                        || vector::mode(program, op, element, admission)? != Some(M::Own)
                        || !vector::same_cell(program, binding, value, admission)?
                        || !declares(&function.body, binding, true, admission)?
                        || declares(protocol.authored_body, binding, false, admission)?
                    {
                        continue;
                    }
                    Directory::emit_consuming(output, &value.id, binding, admission)?;
                }
                Ok(())
            })?;
        }
        Ok(())
    })
}

// This is an absence proof over the live immutable expression, not an attached
// cleanup hint. A second nontrivial child conservatively keeps the full walk.
// Its constant live controls are paid once; every observed node/field uses the
// caller's existing work account. The original consuming stack cap is unchanged.
const NO_LOOP_SPINE_CONTROLS: usize = 6 * std::mem::size_of::<&E>()
    + 3 * std::mem::size_of::<Option<&E>>()
    + std::mem::size_of::<ChildCursor<'_>>()
    + std::mem::size_of::<std::slice::Iter<'_, S>>()
    + std::mem::size_of::<std::slice::Iter<'_, crate::hir::ResolvedFieldInitializer>>()
    + std::mem::size_of::<Option<&S>>()
    + std::mem::size_of::<Option<&crate::hir::ResolvedFieldInitializer>>()
    + 2 * std::mem::size_of::<&mut Admission<'_>>()
    + 3 * std::mem::size_of::<Result<(), Refusal>>()
    + 2 * std::mem::size_of::<Result<bool, Refusal>>()
    + std::mem::size_of::<[bool; 8]>()
    + std::mem::size_of::<&()>();

fn no_loop_spine(root: &E, admission: &mut Admission<'_>) -> Result<bool, Refusal> {
    let atomic = |value: &E| {
        matches!(
            value.kind,
            K::Int(_)
                | K::Int32(_)
                | K::Char(_)
                | K::Uint8(_)
                | K::Usize(_)
                | K::ArrayU8(_)
                | K::RepeatArrayU8 { .. }
                | K::Float32(_)
                | K::Float64(_)
                | K::Bool(_)
                | K::String(_)
                | K::BorrowedUtf8Literal(_)
                | K::Place(_)
                | K::BorrowPlace { .. }
                | K::FunctionReference { .. }
        )
    };
    let mut current = root;
    loop {
        admission.units(1)?;
        // The original consuming visitor does not enter closures.
        if matches!(current.kind, K::Closure { .. }) {
            return Ok(true);
        }
        if let K::Block { statements, .. } | K::VecConstructor { statements, .. } = &current.kind {
            for statement in statements {
                admission.units(1)?;
                if matches!(statement, S::While { .. } | S::LexicalWhile { .. }) {
                    return Ok(false);
                }
            }
        }
        let mut next = None;
        for child in ChildCursor::new(current) {
            admission.units(1)?;
            let mut leaf = atomic(child);
            if let K::ConstructRecord { fields, .. } | K::ConstructVariant { fields, .. } =
                &child.kind
            {
                leaf = true;
                for field in fields {
                    admission.units(1)?;
                    leaf &= atomic(&field.value);
                }
            }
            if !leaf && next.replace(child).is_some() {
                return Ok(false);
            }
        }
        match next {
            Some(child) => current = child,
            None => return Ok(true),
        }
    }
}

#[cfg(test)]
pub(super) fn collect_before_spine<'a>(
    program: &P,
    function: &'a F,
    output: &mut Scratch<Entry<'a>>,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    let mut candidate = false;
    for declaration in program.declarations.declarations() {
        admission.units(1)?;
        if declaration.kind == crate::hir::DeclarationKind::Record
            && declaration.identity_origin == crate::hir::IdentityOrigin::Explicit
        {
            candidate = true;
            break;
        }
    }
    if !candidate {
        return Ok(());
    }
    visit(&function.body, admission, |expression, admission| {
        let K::Block { statements, tail } = &expression.kind else {
            return Ok(());
        };
        for statement in statements {
            admission.units(1)?;
            let (condition, body) = match statement {
                S::While {
                    condition, body, ..
                }
                | S::LexicalWhile {
                    condition, body, ..
                } => (condition.as_ref(), body.as_ref()),
                _ => continue,
            };
            let Some(protocol) = iterator_shape::recognize(condition, body, admission)? else {
                continue;
            };
            let Some(item) = protocol.owned_item else {
                continue;
            };
            let [S::Let {
                binding: step,
                mutable: true,
                value: source,
                ..
            }, S::While { .. } | S::LexicalWhile { .. }] = statements.as_slice()
            else {
                continue;
            };
            // ResolvedBinding equality includes all of id/name/type/mode.
            if !compare::values(&step.id, &protocol.step.id, admission)?
                || !compare::text(&step.name, &protocol.step.name, admission)?
                || !compare::types(&step.ty, &protocol.step.ty, admission)?
                || step.ownership != protocol.step.ownership
                || step.span != protocol.step.span
                || !(tail.ownership == M::Value && matches!(tail.kind, K::Int(0) | K::Usize(0)))
                || !seed(step, source, item, admission)?
                || item.ownership != M::Own
                || !matches!(&item.ty, T::Nominal {arguments, ..} if arguments.is_empty())
            {
                continue;
            }
            visit(protocol.authored_body, admission, |node, admission| {
                let K::Block { statements, .. } = &node.kind else {
                    return Ok(());
                };
                for statement in statements {
                    admission.units(1)?;
                    let S::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    else {
                        continue;
                    };
                    let T::Nominal { arguments, .. } = &binding.ty else {
                        continue;
                    };
                    let [element] = arguments.as_slice() else {
                        continue;
                    };
                    let K::Call { callee, .. } = &value.kind else {
                        continue;
                    };
                    let op = match crate::vec_ops::by_id_with_work(callee.as_str(), &mut |units| {
                        admission.units(units).is_ok()
                    }) {
                        Ok(Some(op)) => op,
                        Ok(None) => continue,
                        Err(()) => return admission.refuse(Refusal::Work),
                    };
                    if op != crate::vec_ops::VecOp::Push
                        || vector::mode(program, op, &item.ty, admission)? != Some(M::Own)
                        || vector::mode(program, op, element, admission)? != Some(M::Own)
                        || !vector::same_cell(program, binding, value, admission)?
                        || !declares(&function.body, binding, true, admission)?
                        || declares(protocol.authored_body, binding, false, admission)?
                    {
                        continue;
                    }
                    Directory::emit_consuming(output, &value.id, binding, admission)?;
                }
                Ok(())
            })?;
        }
        Ok(())
    })
}
