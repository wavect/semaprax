//! Independent replay of immutable borrowed `Slice<u8>` traversal.
//!
//! The resolver spelling has no authority. Every fact of the lowering is
//! checked again from ordinary HIR: one `byte_len` snapshot, a zero seed, a
//! strictly ascending unit counter, one receiver over the same view, the
//! `BorrowBytes` latch when the loop has lexical exits, and a body that cannot
//! rebind the view, snapshot, counter, item or receiver payload. The receiver
//! is the existing `byte_get` result matched once with only `Some`; an
//! inconsistent `None` selects no arm and reaches each backend's existing
//! exhaustive-match invariant refusal. It is the only partial match HIR
//! admits, and only as the item read of one such complete region.
//!
//! The view's owner stays frozen for the whole region by two independent
//! proofs, neither of which reads the view's own liveness: the census below
//! refuses any assignment of the view's authenticated provenance root inside
//! the body, and loan planning starts an ordinary `TraversalFreeze` loan on
//! that root before the region and ends it only after the region, so a move
//! or transfer of an owned root anywhere in the traversal overlaps it.
use super::{
    DeclarationIndex, Diagnostic, LoopLatchKind, OwnershipMode, Place, ResolvedBinding,
    ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedMatchMode, ResolvedMatchPattern,
    ResolvedProgram, ResolvedStatement, ResolvedType, SourceForKind, ValueId,
};

fn fail(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

/// The independently authenticated parts of one receiver.
pub(in crate::hir) struct Receiver<'a> {
    pub(in crate::hir) view: &'a ResolvedExpr,
    pub(in crate::hir) index: &'a ResolvedExpr,
    pub(in crate::hir) payload: &'a ResolvedBinding,
}

fn bare_place(value: &ResolvedExpr) -> Option<&Place> {
    match &value.kind {
        ResolvedExprKind::Place(place) if place.projections.is_empty() => Some(place),
        _ => None,
    }
}

fn place_of(value: &ResolvedExpr, binding: &ResolvedBinding) -> bool {
    value.ty == binding.ty
        && value.ownership == binding.ownership
        && bare_place(value).is_some_and(|place| place.root == binding.id)
}

fn zero(value: &ResolvedExpr) -> bool {
    value.ty == ResolvedType::Usize
        && value.ownership == OwnershipMode::Value
        && matches!(value.kind, ResolvedExprKind::Usize(0))
}

/// Exact receiver spelling: `match byte_get(view, index) { Some { value } => value }`.
pub(in crate::hir) fn receiver_shape(expression: &ResolvedExpr) -> Option<Receiver<'_>> {
    let ResolvedExprKind::Match {
        mode: ResolvedMatchMode::Value,
        scrutinee,
        arms,
    } = &expression.kind
    else {
        return None;
    };
    let [arm] = arms.as_slice() else {
        return None;
    };
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &scrutinee.kind
    else {
        return None;
    };
    let [view, index] = args.as_slice() else {
        return None;
    };
    let ResolvedMatchPattern::Variant {
        variant,
        case,
        fields,
    } = &arm.pattern
    else {
        return None;
    };
    let [field] = fields.as_slice() else {
        return None;
    };
    let payload = &field.binding;
    let admitted = expression.ty == ResolvedType::U8
        && expression.ownership == OwnershipMode::Value
        && callee.as_str() == crate::byte_ops::GET_ID
        && type_arguments.is_empty()
        && scrutinee.ty.is_compiler_byte_option()
        && scrutinee.ownership == OwnershipMode::Value
        && view.ty == ResolvedType::SliceU8
        && bare_place(view).is_some()
        && index.ty == ResolvedType::Usize
        && index.ownership == OwnershipMode::Value
        && bare_place(index).is_some()
        && arm.guard.is_none()
        && variant.as_str() == crate::prelude::OPTION_ID
        && case.as_str() == crate::prelude::OPTION_SOME_ID
        && field.field.as_str() == crate::prelude::OPTION_SOME_VALUE_ID
        && payload.ty == ResolvedType::U8
        && payload.ownership == OwnershipMode::Value
        && place_of(&arm.value, payload);
    admitted.then_some(Receiver {
        view,
        index,
        payload,
    })
}

/// The exhaustiveness exemption the generic match validators may grant; the
/// per-function census below still binds it to one complete region.
pub(crate) fn partial_receiver(expression: &ResolvedExpr) -> bool {
    receiver_shape(expression).is_some()
}

/// One complete traversal region whose receiver was authenticated in place.
pub(crate) struct Region<'a> {
    pub(crate) receiver: &'a ResolvedExpr,
    pub(crate) authored_body: &'a ResolvedExpr,
    /// The snapshot's bare view place; the receiver reads this same place.
    pub(crate) view: &'a ResolvedExpr,
    pub(crate) payload: &'a ResolvedBinding,
}

/// `Ok(None)` means the region is not a byte traversal at all; a region that
/// starts like one but differs anywhere is refused rather than ignored.
pub(crate) fn region(region: &ResolvedExpr) -> Result<Option<Region<'_>>, Diagnostic> {
    let ResolvedExprKind::Block { statements, tail } = &region.kind else {
        return Ok(None);
    };
    let [ResolvedStatement::Let {
        binding: length,
        mutable: false,
        value: count,
        ..
    }, ResolvedStatement::Let {
        binding: counter,
        mutable: true,
        value: seed,
        ..
    }, looped] = statements.as_slice()
    else {
        return Ok(None);
    };
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &count.kind
    else {
        return Ok(None);
    };
    if callee.as_str() != crate::byte_ops::LEN_ID {
        return Ok(None);
    }
    let (condition, body, latch) = match looped {
        ResolvedStatement::While {
            condition, body, ..
        } => (condition, body, None),
        ResolvedStatement::LexicalWhile {
            condition,
            body,
            latch,
            ..
        } => (condition, body, Some(latch)),
        _ => return Ok(None),
    };
    let ResolvedExprKind::Block {
        statements: steps,
        tail: step_tail,
    } = &body.kind
    else {
        return Ok(None);
    };
    let [ResolvedStatement::Let {
        binding: item,
        mutable: false,
        value: read,
        ..
    }, ResolvedStatement::Let {
        binding: discard,
        mutable: false,
        value: authored_body,
        ..
    }, ResolvedStatement::Assign {
        binding: assigned,
        field: None,
        value: advance,
        ..
    }] = steps.as_slice()
    else {
        return Ok(None);
    };
    let Some(proof) = receiver_shape(read) else {
        return Ok(None);
    };
    let inconsistent = || fail("borrowed byte traversal differs from its authenticated lowering");
    let [view] = args.as_slice() else {
        return Err(inconsistent());
    };
    let ResolvedExprKind::Binary {
        op: crate::ast::BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        return Err(inconsistent());
    };
    let ResolvedExprKind::Binary {
        op: crate::ast::BinaryOp::Add,
        left: old,
        right: one,
    } = &advance.kind
    else {
        return Err(inconsistent());
    };
    let Some(view_place) = bare_place(view) else {
        return Err(inconsistent());
    };
    if !type_arguments.is_empty()
        || view.ty != ResolvedType::SliceU8
        || proof.view.ty != view.ty
        || proof.view.ownership != view.ownership
        || bare_place(proof.view) != Some(view_place)
        || count.ty != ResolvedType::Usize
        || count.ownership != OwnershipMode::Value
        || length.ty != ResolvedType::Usize
        || length.ownership != OwnershipMode::Value
        || counter.ty != ResolvedType::Usize
        || counter.ownership != OwnershipMode::Value
        || !zero(seed)
        || !zero(tail)
        || region.ty != ResolvedType::Usize
        || region.ownership != OwnershipMode::Value
        || condition.ty != ResolvedType::Bool
        || condition.ownership != OwnershipMode::Value
        || !place_of(left, counter)
        || !place_of(right, length)
        || !place_of(proof.index, counter)
        || item.ty != ResolvedType::U8
        || item.ownership != OwnershipMode::Value
        || discard.ty != authored_body.ty
        || discard.ownership != authored_body.ownership
        || assigned.id != counter.id
        || assigned.ty != counter.ty
        || advance.ty != ResolvedType::Usize
        || advance.ownership != OwnershipMode::Value
        || !place_of(old, counter)
        || !matches!(one.kind, ResolvedExprKind::Usize(1))
        || one.ty != ResolvedType::Usize
        || one.ownership != OwnershipMode::Value
        || !zero(step_tail)
    {
        return Err(inconsistent());
    }
    if let Some(latch) = latch {
        if !matches!(latch, LoopLatchKind::SourceForStep { kind: SourceForKind::BorrowBytes, step } if step == &advance.id)
        {
            return Err(inconsistent());
        }
    }
    frozen_body(
        authored_body,
        proof.payload,
        &[
            &length.id,
            &counter.id,
            &item.id,
            &discard.id,
            &proof.payload.id,
            &view_place.root,
        ],
    )?;
    Ok(Some(Region {
        receiver: read,
        authored_body,
        view,
        payload: proof.payload,
    }))
}

/// The authored body cannot rebind or assign the view, snapshot, counter,
/// item or receiver payload, cannot read the hidden payload directly, and
/// cannot contain another receiver: source refuses nested traversal, so every
/// HIR node lies in at most one region body and each replay stays linear.
fn frozen_body(
    body: &ResolvedExpr,
    payload: &ResolvedBinding,
    frozen: &[&ValueId],
) -> Result<(), Diagnostic> {
    walk(body, |expression, authorized| {
        if matches!(&expression.kind, ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } if place.root == payload.id)
        {
            return Err(fail("borrowed byte receiver payload escapes its receiver"));
        }
        if receiver_shape(expression).is_some() {
            return Err(fail(
                "borrowed byte traversal is nested in another traversal",
            ));
        }
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Let { binding, .. }
                | ResolvedStatement::Assign { binding, .. } = statement
                {
                    if frozen.contains(&&binding.id) {
                        return Err(fail(
                            "borrowed byte traversal view, owner, counter or item is mutated or rebound",
                        ));
                    }
                }
            }
        }
        if let ResolvedExprKind::Match { arms, .. } = &expression.kind {
            for arm in arms {
                if let ResolvedMatchPattern::Variant { fields, .. } = &arm.pattern {
                    if fields
                        .iter()
                        .any(|field| frozen.contains(&&field.binding.id))
                    {
                        return Err(fail("borrowed byte traversal binding is shadowed"));
                    }
                }
            }
        }
        Ok(authorized)
    })?;
    Ok(())
}

/// Census: every partial receiver is the item read of one complete region
/// whose view carries authenticated byte-slice provenance, and that
/// provenance root is frozen for the whole region. A receiver anywhere else is
/// hostile HIR and is refused before any backend.
///
/// Provenance is derived for monomorphic functions only, so a traversal
/// lowered inside a generic instance has no authenticated root and is refused.
/// The stored provenance is itself replayed against a fresh derivation before
/// validation succeeds, so a forged root cannot relax this freeze.
pub(in crate::hir) fn validate_function(
    declarations: &DeclarationIndex,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut unauthorized = false;
    for root in function
        .ensures
        .iter()
        .rev()
        .chain(std::iter::once(&function.body))
        .chain(function.requires.iter().rev())
    {
        walk(root, |expression, mut authorized| {
            if let Some(region) = region(expression)? {
                let view = bare_place(region.view)
                    .ok_or_else(|| fail("borrowed byte traversal view is not a bare place"))?;
                let provenance = declarations.byte_slice_provenance(&view.root)
                    .ok_or_else(|| fail("borrowed byte traversal view lacks authenticated byte-slice provenance"))?;
                if provenance.root != view.root {
                    frozen_body(region.authored_body, region.payload, &[&provenance.root])?;
                }
                authorized = Some(region.receiver);
            }
            if receiver_shape(expression).is_some()
                && !authorized.is_some_and(|receiver| std::ptr::eq(receiver, expression))
            {
                unauthorized = true;
            }
            Ok(authorized)
        })?;
    }
    // Region/provenance errors retain their original precedence over the
    // final whole-function detached-receiver census refusal.
    if unauthorized {
        return Err(fail(
            "borrowed byte receiver is outside an authenticated Slice<u8> traversal",
        ));
    }
    Ok(())
}

/// Receiver presence selects Graph v83. It grants nothing: validation still
/// binds every receiver to one complete region before any consumer runs.
pub(crate) fn function_uses(function: &ResolvedFunction) -> bool {
    roots_use(
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures),
    )
}

pub(crate) fn program_uses(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(function_uses)
        || roots_use(program.function_templates.iter().flat_map(|f| {
            f.requires
                .iter()
                .chain(std::iter::once(&f.body))
                .chain(&f.ensures)
        }))
}


// Like the saved Vec frontier, closures expose their evaluated captures only;
// their separately checked bodies cannot borrow authorization from the caller.
#[derive(Clone, Copy)]
struct Frame<'a> {
    children: crate::hir::lexical_exit::ChildCursor<'a>,
    authorized: Option<&'a ResolvedExpr>,
    capture: usize,
    entered: bool,
}

fn walk<'a>(
    root: &'a ResolvedExpr,
    mut visit: impl FnMut(
        &'a ResolvedExpr,
        Option<&'a ResolvedExpr>,
    ) -> Result<Option<&'a ResolvedExpr>, Diagnostic>,
) -> Result<(), Diagnostic> {
    // Same fixed depth as the shared lexical cursor. This is stack storage,
    // not a heap allocation or permission to obtain physical authority.
    let mut frames = [None; crate::cache_codec::MAX_DEPTH + 1];
    let frame = |expression, authorized| Frame {
        children: crate::hir::lexical_exit::ChildCursor::new(expression),
        authorized,
        capture: 0,
        entered: false,
    };
    frames[0] = Some(frame(root, None));
    let mut len = 1;
    while len != 0 {
        let current = frames[len - 1]
            .as_mut()
            .expect("occupied byte traversal frame");
        let expression = current.children.expression();
        if !current.entered {
            current.authorized = visit(expression, current.authorized)?;
            current.entered = true;
        }
        let next = if let ResolvedExprKind::Closure { captures, .. } = &expression.kind {
            let next = captures.get(current.capture).map(|capture| &capture.value);
            current.capture += usize::from(next.is_some());
            next
        } else {
            current.children.next()
        };
        let authorized = current.authorized;
        if let Some(next) = next {
            if len == frames.len() {
                return Err(fail(
                    "borrowed byte traversal scan exceeds admitted expression depth",
                ));
            }
            frames[len] = Some(frame(next, authorized));
            len += 1;
        } else {
            len -= 1;
            frames[len] = None;
        }
    }
    Ok(())
}

fn roots_use<'a>(roots: impl DoubleEndedIterator<Item = &'a ResolvedExpr>) -> bool {
    for root in roots.rev() {
        let mut found = false;
        if walk(root, |expression, authorized| {
            found |= receiver_shape(expression).is_some();
            Ok(authorized)
        })
        .is_err()
            || found
        {
            return true;
        }
    }
    false
}

