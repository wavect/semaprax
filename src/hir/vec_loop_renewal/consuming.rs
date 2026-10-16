//! Additive consuming owned-record renewal. Frozen ordinary collectors stay closed.
use super::{
    ElementAdmission, ExpressionId, OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedStatement, ResolvedType,
};
use std::collections::BTreeMap;
pub(crate) mod empty_clone;
mod funded;
pub(crate) use funded::append_bindings_funded;

#[derive(Clone, Copy)]
pub(crate) struct Site<'a> {
    pub(crate) condition: &'a ResolvedExpr,
    pub(crate) body: &'a ResolvedExpr,
    pub(crate) item: &'a ResolvedBinding,
    pub(crate) at: &'a ResolvedExpr,
    pub(crate) binding: &'a ResolvedBinding,
}

type Frames<'a> =
    [Option<crate::hir::lexical_exit::ChildCursor<'a>>; crate::cache_codec::MAX_DEPTH + 1];
fn frames<'a>() -> Option<Frames<'a>> {
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Frames<'_>>()) {
        return None;
    }
    Some([None; crate::cache_codec::MAX_DEPTH + 1])
}
/// Depth-first authored-order traversal with no heap frontier or ID clones.
/// Overflow refuses this additive profile, without changing legacy collectors.
fn visit<'a>(root: &'a ResolvedExpr, visitor: impl FnMut(&'a ResolvedExpr)) -> bool {
    let Some(mut frames) = frames() else {
        return false;
    };
    visit_with(root, visitor, &mut frames)
}
fn visit_with<'a>(
    root: &'a ResolvedExpr,
    mut visitor: impl FnMut(&'a ResolvedExpr),
    stack: &mut Frames<'a>,
) -> bool {
    use crate::hir::lexical_exit::ChildCursor;
    stack[0] = Some(ChildCursor::new(root));
    let mut len = 1;
    while len != 0 {
        let frame = stack[len - 1]
            .as_mut()
            .expect("occupied consuming renewal frame");
        let expr = frame.expression();
        if !frame.is_started() {
            visitor(expr);
        }
        let child = if matches!(expr.kind, ResolvedExprKind::Closure { .. }) {
            None
        } else {
            frame.next()
        };
        if let Some(child) = child {
            if len == stack.len() {
                stack.fill(None);
                return false;
            }
            stack[len] = Some(ChildCursor::new(child));
            len += 1;
        } else {
            stack[len - 1] = None;
            len -= 1;
        }
    }
    true
}
fn declares(root: &ResolvedExpr, target: &ResolvedBinding, mutable: bool) -> bool {
    let mut found = false;
    visit(root, |expr| {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expr.kind
        {
            found |= statements.iter().any(|statement| {
                matches!(statement,
                ResolvedStatement::Let { binding, mutable: actual, .. }
                    if binding.id == target.id && binding.ty == target.ty
                        && binding.ownership == target.ownership && (!mutable || *actual))
            });
        }
    }) && found
}

fn scan<'a>(
    function: &'a ResolvedFunction,
    admission: &ElementAdmission,
    emit: impl FnMut(Site<'a>),
) -> bool {
    let Some(mut frames) = frames() else {
        return false;
    };
    scan_with(function, admission, emit, &mut frames)
}
fn scan_with<'a>(
    function: &'a ResolvedFunction,
    admission: &ElementAdmission,
    mut emit: impl FnMut(Site<'a>),
    frames: &mut Frames<'a>,
) -> bool {
    let mut complete_all = true;
    let complete = visit_with(
        &function.body,
        |expr| {
            let ResolvedExprKind::Block { statements, tail } = &expr.kind else {
                return;
            };
            for statement in statements {
                let (condition, body) = match statement {
                    ResolvedStatement::While {
                        condition, body, ..
                    }
                    | ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => (condition.as_ref(), body.as_ref()),
                    _ => continue,
                };
                let Some(protocol) = super::super::iterator_loop::recognize(condition, body) else {
                    continue;
                };
                let Some(item) = protocol.owned_item else {
                    continue;
                };
                let [ResolvedStatement::Let {
                    binding: step,
                    mutable: true,
                    value: seed,
                    ..
                }, ResolvedStatement::While { .. } | ResolvedStatement::LexicalWhile { .. }] =
                    statements.as_slice()
                else {
                    continue;
                };
                if step != protocol.step
                    || !(tail.ownership == OwnershipMode::Value
                        && matches!(
                            tail.kind,
                            ResolvedExprKind::Int(0) | ResolvedExprKind::Usize(0)
                        ))
                    || seed.ty != step.ty
                    || seed.ownership != OwnershipMode::Own
                    || !matches!(&seed.kind, ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
                    if callee.as_str() == crate::iterator_ops::NEXT_ID
                    && type_arguments.as_slice() == std::slice::from_ref(&item.ty)
                    && matches!(args.as_slice(), [source] if source.ownership == OwnershipMode::Own
                        && matches!(&source.ty, ResolvedType::Nominal { declaration, arguments }
                            if declaration.as_str() == crate::iterator_ops::ITER_ID && arguments.as_slice() == std::slice::from_ref(&item.ty))))
                {
                    continue;
                }

                // Exact monomorphic owned rows; older scalar/generic/primitive routes
                // remain owned by their frozen classifiers.
                if item.ownership != OwnershipMode::Own
                    || !matches!(&item.ty,
                ResolvedType::Nominal { arguments, .. } if arguments.is_empty())
                {
                    continue;
                }
                complete_all &= visit(protocol.authored_body, |node| {
                    let ResolvedExprKind::Block { statements, .. } = &node.kind else {
                        return;
                    };
                    for statement in statements {
                        let ResolvedStatement::Assign {
                            binding,
                            field: None,
                            value,
                            ..
                        } = statement
                        else {
                            continue;
                        };
                        let ResolvedType::Nominal { arguments, .. } = &binding.ty else {
                            continue;
                        };
                        let [element] = arguments.as_slice() else {
                            continue;
                        };
                        let ResolvedExprKind::Call { callee, .. } = &value.kind else {
                            continue;
                        };
                        let Some(op) = crate::vec_ops::by_id(callee.as_str()) else {
                            continue;
                        };
                        // The exact row family and ordinary staged argument checks
                        // apply to the yielded item and to an independently admitted
                        // fallible row argument (e.g. clone_at). No argument exemption
                        // or allocation permission is supplied by this classifier.
                        if op != crate::vec_ops::VecOp::Push
                            || admission(op, &item.ty) != Some(OwnershipMode::Own)
                            || admission(op, element) != Some(OwnershipMode::Own)
                            || !super::same_cell(binding, value, Some(admission))
                            || !declares(&function.body, binding, true)
                            || declares(protocol.authored_body, binding, false)
                        {
                            continue;
                        }
                        emit(Site {
                            condition,
                            body,
                            item,
                            at: value,
                            binding,
                        });
                    }
                });
            }
        },
        frames,
    );
    complete && complete_all
}

// A one-entry-per-node upper bound includes all eleven key/value slots and
// twelve child pointers of a Rust BTree node. Charge before cloning an ID or
// publishing the entry; duplicate keys need no second allocation.
fn reserve_entry<T>(id: &ExpressionId) -> bool {
    let bytes = (std::mem::size_of::<ExpressionId>() + std::mem::size_of::<T>())
        .saturating_mul(11)
        .saturating_add(16 * std::mem::size_of::<usize>())
        .saturating_add(id.as_str().len());
    crate::bounded_output::reserve_active_required(bytes)
}

// Both owned_leaf_collection and owned_record_collection require the yielded
// item's current declaration to be Record + Explicit. Absence refuses discovery
// without reserving expression scratch; presence grants nothing and retains the
// complete independent body/protocol/element proof below.
fn has_record_candidate(program: &dyn crate::cleanup::ReplayContext) -> bool {
    program.declarations().declarations().any(|declaration| {
        declaration.kind == crate::hir::DeclarationKind::Record
            && declaration.identity_origin == crate::hir::IdentityOrigin::Explicit
    })
}

pub(crate) fn sites<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
) -> BTreeMap<ExpressionId, Site<'a>> {
    let mut found = BTreeMap::new();
    if !has_record_candidate(program) {
        return found;
    }
    let complete = scan(
        function,
        &|op, element| super::element_mode(program.declarations(), op, element),
        |site| {
            if !found.contains_key(&site.at.id) && reserve_entry::<Site<'_>>(&site.at.id) {
                found.insert(site.at.id.clone(), site);
            }
        },
    );
    if !complete {
        found.clear();
    }
    found
}

pub(super) fn append_bindings<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    found: &mut BTreeMap<ExpressionId, &'a ResolvedBinding>,
) {
    if !has_record_candidate(program) {
        return;
    }
    // Both independent traversals borrow one actually held fixed cursor. Nested
    // candidate/body/declaration walks still reserve their overlapping scratch.
    let Some(mut frames) = frames() else {
        return;
    };
    // Scan once before publishing: an incomplete new-profile walk cannot leave
    // partially authorized renewal entries in the held cleanup index.
    if !scan_with(
        function,
        &|op, element| super::element_mode(program.declarations(), op, element),
        |_| {},
        &mut frames,
    ) {
        return;
    }
    scan_with(
        function,
        &|op, element| super::element_mode(program.declarations(), op, element),
        |site| {
            if !found.contains_key(&site.at.id) && reserve_entry::<&ResolvedBinding>(&site.at.id) {
                found.insert(site.at.id.clone(), site.binding);
            }
        },
        &mut frames,
    );
}

pub(crate) fn requires_with_admission(
    function: &ResolvedFunction,
    admission: impl Fn(crate::vec_ops::VecOp, &ResolvedType) -> Option<OwnershipMode>,
) -> bool {
    let mut found = false;
    scan(function, &admission, |_| found = true) && found
}
pub(crate) fn requires(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> bool {
    requires_with_admission(function, |op, element| {
        super::element_mode(program.declarations(), op, element)
    })
}
pub(crate) fn candidate(function: &ResolvedFunction) -> bool {
    requires_with_admission(function, |op, element| {
        (op == crate::vec_ops::VecOp::Push
            && matches!(element, ResolvedType::Nominal { arguments, .. } if arguments.is_empty()))
        .then_some(OwnershipMode::Own)
    })
}
#[cfg(test)]
mod tests;
