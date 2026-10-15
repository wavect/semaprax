//! Immutable borrowed `Slice<u8>` traversal over the existing byte reads.
//!
//! `for byte in view` keeps the Copy-scalar `for` paths exactly: one
//! `byte_len` snapshot at `.value.s0`, an ordinary `usize` counter at `.s1`,
//! and the bounded loop at `.s2`. Each item is one *receiver*: the existing
//! `byte_get` result matched once, whose only arm is `Some`. The counter is
//! below the snapshot of an immutable, frozen view, so `None` is inconsistent;
//! it selects no arm and reaches every backend's existing exhaustive-match
//! invariant refusal. No byte is forged, defaulted, skipped or loaded
//! unchecked, and no buffer or view is materialized. This spelling carries no
//! authority: `hir::validation::byte_traversal` replays it independently.
//!
//! The shared paths construct the snapshot and receiver in place; no Vec
//! carrier is built and discarded. Raw AST has no source type, so the
//! receiver is outside the workspace `for` prebound: its two further
//! expression identities debit their own carriers as every `ExpressionId`
//! does, and [`receiver_backing`] debits every other byte it retains on the
//! active builder ledger before anything is constructed.
use super::*;
use crate::hir::{
    CompilerDeclaration, ResolvedMatchArm, ResolvedMatchMode, ResolvedMatchPattern,
    ResolvedMatchPatternField, SourceForKind,
};

const WRAPPER: &str = "#for-bytes";
const LENGTH: &str = "#for-bytes-length";
const PAYLOAD: &str = "#for-byte";
/// Receiver identities beside its root at `{path}`.
const SCRUTINEE: &str = ".scrutinee";
const VIEW_ARGUMENT: &str = ".scrutinee.arg.0";
const INDEX_ARGUMENT: &str = ".scrutinee.arg.1";
const PAYLOAD_BINDING: &str = ".arm.0.binding.0";
const ARM_VALUE: &str = ".arm.0.value";

/// Only an immutable named view of the exact compiler-owned `Slice<u8>` type.
pub(in crate::hir) fn admits(source: &Binding) -> bool {
    source.ty == ResolvedType::SliceU8 && !source.mutable && source.traversal_row.is_none()
}

/// Locate this exact authored body without inheriting a mixed traversal or
/// closure's context. The source nesting bound is narrower than this existing
/// HIR/cache bound; no owning frontier or new admission allowance is acquired.
pub(in crate::hir) fn source_byte_ancestry(
    root: &crate::ast::Expr,
    target: &crate::ast::Expr,
    scope: &BTreeMap<String, Binding>,
) -> bool {
    use crate::ast::{Expr, ExprKind, Statement};
    #[inline(never)]
    fn walk(
        root: &Expr,
        target: &Expr,
        scope: &BTreeMap<String, Binding>,
        ancestry: Option<bool>,
        depth: usize,
    ) -> Option<bool> {
        if depth > crate::cache_codec::MAX_DEPTH {
            return None;
        }
        if std::ptr::eq(root, target) {
            return Some(ancestry == Some(true));
        }
        if let ExprKind::Block { statements, tail } = &root.kind {
            for statement in statements {
                if let Statement::For { values, body, .. } = statement {
                    let bytes = matches!(&values.kind, ExprKind::Var(name)
                        if scope.get(name).is_some_and(admits));
                    if let Some(found) = walk(
                        body,
                        target,
                        scope,
                        Some(ancestry.unwrap_or(true) && bytes),
                        depth + 1,
                    ) {
                        return Some(found);
                    }
                } else {
                    let ancestry = if matches!(
                        statement,
                        Statement::ForBorrow { .. } | Statement::ForOwn { .. }
                    ) {
                        Some(false)
                    } else {
                        ancestry
                    };
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            if let Some(found) = walk(child, target, scope, ancestry, depth + 1) {
                                return Some(found);
                            }
                        }
                    }
                }
            }
            return walk(tail, target, scope, ancestry, depth + 1);
        }
        let ancestry = if matches!(root.kind, ExprKind::Closure { .. }) {
            None
        } else {
            ancestry
        };
        let mut index = 0;
        while let Some(child) = root.child(index) {
            if let Some(found) = walk(child, target, scope, ancestry, depth + 1) {
                return Some(found);
            }
            index += 1;
        }
        None
    }
    walk(root, target, scope, None, 0).unwrap_or(false)
}

/// `None` is an unaddressable receiver spelling or a builder-ledger refusal;
/// a refusal is sticky on the active budget. Either way nothing of the
/// traversal was constructed.
pub(super) fn lower(
    function: &FunctionExecutionId,
    statement_path: &str,
    item_name: &str,
    item_span: Span,
    source: &ResolvedBinding,
    authored_body: ResolvedExpr,
    span: Span,
) -> Option<ResolvedStatement> {
    let path_len = statement_path
        .len()
        .checked_add(".value.s2.body.s0.value".len())?;
    if !crate::bounded_output::reserve_active_required(receiver_backing(function, path_len)?) {
        return None;
    }
    Some(lower_profile(
        function,
        statement_path,
        item_name,
        item_span,
        source,
        authored_body,
        span,
        Profile {
            wrapper: WRAPPER,
            length: LENGTH,
            length_callee: crate::byte_ops::LEN_ID,
            length_arguments: Vec::new(),
            item: ResolvedType::U8,
            kind: SourceForKind::BorrowBytes,
        },
        |path, counter| receiver(function, path, source, counter, item_span, span),
    ))
}

/// Exact requested backing one receiver at a `path_len`-byte path retains
/// beyond the shared carriers its five `ExpressionId` constructors debit
/// themselves: the five identity texts, the payload `ValueId`, its name, the
/// result type's argument vector, and the boxed scrutinee, argument, arm and
/// field vectors. The five declaration identities (`byte_get`, the
/// `Option<u8>` result type, and the matched variant, case and field) are
/// fixed compiler identities and retain no bytes. `None` means the spelling
/// is not addressable and nothing may be constructed.
pub(super) fn receiver_backing(function: &FunctionExecutionId, path_len: usize) -> Option<usize> {
    use crate::hir::ids::streamed::scoped_len;
    let expression =
        |suffix: &str| scoped_len(function, "expression", path_len.checked_add(suffix.len())?);
    let identities = [
        expression("")?,
        expression(SCRUTINEE)?,
        expression(VIEW_ARGUMENT)?,
        expression(INDEX_ARGUMENT)?,
        expression(ARM_VALUE)?,
    ];
    let payload_text = scoped_len(
        function,
        "value:local",
        path_len.checked_add(PAYLOAD_BINDING.len())?,
    )?;
    let payload = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::array::<u8>(payload_text).ok()?)
        .ok()?
        .0
        .pad_to_align()
        .size();
    let structure = [
        std::mem::size_of::<ResolvedType>(),
        std::mem::size_of::<ResolvedExpr>(),
        2 * std::mem::size_of::<ResolvedExpr>(),
        std::mem::size_of::<ResolvedMatchArm>(),
        std::mem::size_of::<ResolvedMatchPatternField>(),
    ];
    identities
        .into_iter()
        .chain([payload, PAYLOAD.len()])
        .chain(structure)
        .try_fold(0usize, usize::checked_add)
}

/// One bounded byte read: `match byte_get(view, counter) { Some { value } => value }`.
fn receiver(
    function: &FunctionExecutionId,
    path: &str,
    view: &ResolvedBinding,
    counter: &ResolvedBinding,
    item_span: Span,
    span: Span,
) -> ResolvedExpr {
    let id = receiver_id(function, path, "");
    let scrutinee_id = receiver_id(function, path, SCRUTINEE);
    let view = receiver_place(function, path, VIEW_ARGUMENT, view, span);
    let index = receiver_place(function, path, INDEX_ARGUMENT, counter, span);
    let payload = ResolvedBinding {
        id: ValueId::new(receiver_text(
            function,
            "value:local",
            path,
            PAYLOAD_BINDING,
        )),
        name: PAYLOAD.to_owned(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::U8,
        span: item_span,
    };
    let value = receiver_place(function, path, ARM_VALUE, &payload, span);
    let read = ResolvedExpr {
        id: scrutinee_id,
        ty: crate::byte_ops::ByteOp::Get.return_type(),
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::compiler(CompilerDeclaration::ByteGet),
            type_arguments: Vec::new(),
            instance: None,
            args: vec![view, index],
        },
        span,
    };
    ResolvedExpr {
        id,
        ty: ResolvedType::U8,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Match {
            mode: ResolvedMatchMode::Value,
            scrutinee: Box::new(read),
            arms: vec![ResolvedMatchArm {
                pattern: ResolvedMatchPattern::Variant {
                    variant: DeclarationId::compiler(CompilerDeclaration::Option),
                    case: DeclarationId::compiler(CompilerDeclaration::OptionSome),
                    fields: vec![ResolvedMatchPatternField {
                        field: DeclarationId::compiler(CompilerDeclaration::OptionSomeValue),
                        binding: payload,
                    }],
                },
                guard: None,
                value,
                span,
            }],
        },
        span,
    }
}

// Stream the exact old scoped identity without formatting a temporary path
// (or a generic execution key). emit prepays the complete requested text;
// receiver_backing separately covers retained payload/containers/declarations.
fn receiver_text(function: &FunctionExecutionId, kind: &str, path: &str, suffix: &str) -> String {
    let length = path
        .len()
        .checked_add(suffix.len())
        .expect("preflight receiver path");
    crate::hir::ids::streamed::emit(function, kind, length, |bytes| {
        bytes[..path.len()].copy_from_slice(path.as_bytes());
        bytes[path.len()..].copy_from_slice(suffix.as_bytes());
    })
}

fn receiver_id(function: &FunctionExecutionId, path: &str, suffix: &str) -> ExpressionId {
    ExpressionId::from_owned(receiver_text(function, "expression", path, suffix))
}

fn receiver_place(
    function: &FunctionExecutionId,
    path: &str,
    suffix: &str,
    binding: &ResolvedBinding,
    span: Span,
) -> ResolvedExpr {
    ResolvedExpr {
        id: receiver_id(function, path, suffix),
        ty: binding.ty.clone(),
        ownership: binding.ownership,
        kind: ResolvedExprKind::Place(Place {
            root: binding.id.clone(),
            projections: Vec::new(),
        }),
        span,
    }
}

#[cfg(test)]
mod tests;
