//! Continuation frames for the iterative expression resolver.
//!
//! Hoisted out of `resolve_expr_iterative`: the frame shape, its result splice
//! helper, and the test-only capacity probe. Expression paths are four-byte
//! `PathId` coordinates into the call's private path arena; the census counts
//! that arena's backing once, beside the frame stack.

#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, Span, Statement, Type, UnaryOp};

#[cfg(test)]
use super::capacity_probe::{
    resolved_expr_owned_capacity, resolved_field_initializer_owned_capacity,
    resolved_match_arm_owned_capacity, resolved_statement_owned_capacity,
    resolved_type_owned_capacity,
};
use super::expr_nodes::{
    ResolvedExpr, ResolvedFieldInitializer, ResolvedMatchArm, ResolvedMatchPattern,
    ResolvedStatement,
};
use super::ids::{DeclarationId, FunctionInstanceId};
use super::nodes::{
    DeclarationKind, ResolvedBinding, ResolvedHostCommandOperation, ResolvedMatchMode, ResolvedType,
};
use super::resolve_expr::path_arena::{PathId, Seg, Step};
use super::Scope;

pub(super) enum Frame<'expr> {
    FinishVecConstructor {
        element: ResolvedType,
        count: u32,
    },
    EnterLoop(super::ControlTarget),
    LeaveLoop,
    EnterByteLoop(super::ControlTarget, Scope<'expr>),
    FinishLexicalExit {
        span: Span,
        path: PathId,
        kind: crate::ast::LexicalExitKind,
        target: super::ControlTarget,
        has_value: bool,
    },
    ForOwn(Box<super::resolve_for_own::ForOwnFrame<'expr>>),
    Enter {
        expr: &'expr Expr,
        bindings: Scope<'expr>,
        path: PathId,
    },
    FinishInvoke {
        span: Span,
        path: PathId,
        callable: ResolvedExpr,
        argument_count: usize,
    },
    FinishNativeCall {
        span: Span,
        path: PathId,
        import: DeclarationId,
        argument_count: usize,
    },
    FinishCall {
        span: Span,
        path: PathId,
        callee: DeclarationId,
        type_arguments: Vec<ResolvedType>,
        instance: Option<FunctionInstanceId>,
        return_source_type: Type,
        target_span: Span,
        argument_count: usize,
    },
    FinishStringOp {
        span: Span,
        path: PathId,
        op: crate::string_ops::StringOp,
        argument_count: usize,
    },
    FinishSortBy {
        span: Span,
        path: PathId,
        element: ResolvedType,
        callee: DeclarationId,
    },
    FinishVecField {
        span: Span,
        path: PathId,
        element: ResolvedType,
        field: DeclarationId,
    },
    FinishLiteralFormat {
        span: Span,
        path: PathId,
        template: String,
        argument_count: usize,
    },
    LiteralFormatArgNext {
        args: &'expr [Expr],
        index: usize,
        bindings: Scope<'expr>,
        path: PathId,
    },
    FinishStrOp {
        span: Span,
        path: PathId,
        op: crate::str_ops::StrOp,
        argument_count: usize,
    },
    FinishByteOp {
        span: Span,
        path: PathId,
        op: crate::byte_ops::ByteOp,
        argument_count: usize,
    },
    FinishOwnedGenericOp {
        span: Span,
        path: PathId,
        op: super::resolve_box_call::OwnedGenericCallSite,
        element: ResolvedType,
        argument_count: usize,
    },
    FinishHostIoOp {
        span: Span,
        path: PathId,
        op: crate::host_io_ops::HostIoOp,
        argument_count: usize,
    },
    FinishHostCommandOp {
        span: Span,
        path: PathId,
        op: ResolvedHostCommandOperation,
        argument_count: usize,
    },
    ChildNext {
        children: &'expr [Expr],
        index: usize,
        bindings: Scope<'expr>,
        path: PathId,
        segment: Seg,
    },
    MethodArgNext {
        args: &'expr [Expr],
        index: usize,
        bindings: Scope<'expr>,
        path: PathId,
    },
    FinishUnary {
        span: Span,
        path: PathId,
        op: UnaryOp,
    },
    FinishBinary {
        span: Span,
        path: PathId,
        op: BinaryOp,
    },
    AfterBinaryLeft {
        span: Span,
        path: PathId,
        op: BinaryOp,
        right: &'expr Expr,
        bindings: Scope<'expr>,
    },
    BlockNext {
        span: Span,
        path: PathId,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Scope<'expr>,
        resolved: Vec<ResolvedStatement>,
    },
    BlockAfterLet {
        span: Span,
        path: PathId,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Scope<'expr>,
        resolved: Vec<ResolvedStatement>,
    },
    BlockAfterAssign {
        span: Span,
        path: PathId,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Scope<'expr>,
        resolved: Vec<ResolvedStatement>,
        target: ResolvedBinding,
        /// Field Mutation v1: the resolved direct field and its
        /// substituted type when the target is `<binding>.<field>`.
        target_field: Option<(DeclarationId, ResolvedType)>,
    },
    BlockAfterUnsafe {
        span: Span,
        path: PathId,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Scope<'expr>,
        resolved: Vec<ResolvedStatement>,
    },
    BlockWhileCondition {
        span: Span,
        path: PathId,
        condition: &'expr Expr,
        body: &'expr Expr,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Scope<'expr>,
        resolved: Vec<ResolvedStatement>,
    },
    BlockWhileBody {
        span: Span,
        path: PathId,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Scope<'expr>,
        resolved: Vec<ResolvedStatement>,
        condition: Box<ResolvedExpr>,
        condition_span: Span,
    },
    BlockForBody {
        span: Span,
        path: PathId,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Scope<'expr>,
        resolved: Vec<ResolvedStatement>,
        source: ResolvedBinding,
        element: ResolvedType,
    },
    FinishBlock {
        span: Span,
        path: PathId,
        statements: Vec<ResolvedStatement>,
    },
    FinishIf {
        span: Span,
        path: PathId,
    },
    AfterIfCondition {
        span: Span,
        path: PathId,
        then_branch: &'expr Expr,
        else_branch: &'expr Expr,
        bindings: Scope<'expr>,
    },
    AfterIfThen {
        span: Span,
        path: PathId,
        else_branch: &'expr Expr,
        bindings: Scope<'expr>,
    },
    RecordNext {
        span: Span,
        path: PathId,
        type_name: &'expr str,
        record: DeclarationId,
        arguments: Vec<ResolvedType>,
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Scope<'expr>,
        resolved: Vec<ResolvedFieldInitializer>,
    },
    RecordAfterField {
        span: Span,
        path: PathId,
        type_name: &'expr str,
        record: DeclarationId,
        arguments: Vec<ResolvedType>,
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Scope<'expr>,
        resolved: Vec<ResolvedFieldInitializer>,
        field: DeclarationId,
    },
    VariantNext {
        span: Span,
        path: PathId,
        type_name: &'expr str,
        case_name: &'expr str,
        variant: DeclarationId,
        case: DeclarationId,
        type_arguments: &'expr [Type],
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Scope<'expr>,
        resolved: Vec<ResolvedFieldInitializer>,
    },
    VariantAfterField {
        span: Span,
        path: PathId,
        type_name: &'expr str,
        case_name: &'expr str,
        variant: DeclarationId,
        case: DeclarationId,
        type_arguments: &'expr [Type],
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Scope<'expr>,
        resolved: Vec<ResolvedFieldInitializer>,
        field: DeclarationId,
    },
    AfterMatchScrutinee {
        span: Span,
        path: PathId,
        mode: ResolvedMatchMode,
        arms: &'expr [crate::ast::MatchArm],
        bindings: Scope<'expr>,
    },
    MatchNext {
        span: Span,
        path: PathId,
        mode: ResolvedMatchMode,
        arms: &'expr [crate::ast::MatchArm],
        index: usize,
        bindings: Scope<'expr>,
        scrutinee: ResolvedExpr,
        matched_type: DeclarationId,
        instance_arguments: Vec<ResolvedType>,
        matched_kind: DeclarationKind,
        resolved: Vec<ResolvedMatchArm>,
    },
    MatchAfterArm {
        span: Span,
        path: PathId,
        mode: ResolvedMatchMode,
        arms: &'expr [crate::ast::MatchArm],
        index: usize,
        bindings: Scope<'expr>,
        scrutinee: ResolvedExpr,
        matched_type: DeclarationId,
        instance_arguments: Vec<ResolvedType>,
        matched_kind: DeclarationKind,
        resolved: Vec<ResolvedMatchArm>,
        pattern: ResolvedMatchPattern,
    },
    /// Refutable Match v1 decision chain over a Copy-scalar
    /// scrutinee. Arms resolve in order under the enclosing bindings;
    /// binding arms extend them for their own arm only.
    ScalarMatchNext {
        span: Span,
        path: PathId,
        mode: ResolvedMatchMode,
        arms: &'expr [crate::ast::MatchArm],
        index: usize,
        bindings: Scope<'expr>,
        scrutinee: ResolvedExpr,
        resolved: Vec<ResolvedMatchArm>,
    },
    ScalarMatchAfterArm {
        span: Span,
        path: PathId,
        mode: ResolvedMatchMode,
        arms: &'expr [crate::ast::MatchArm],
        index: usize,
        bindings: Scope<'expr>,
        scrutinee: ResolvedExpr,
        resolved: Vec<ResolvedMatchArm>,
        pattern: ResolvedMatchPattern,
    },
    FinishTry {
        span: Span,
        path: PathId,
    },
    /// Resumable Effects v1 (issue #204). See `hir::resolve_yield`.
    FinishYield {
        span: Span,
        path: PathId,
    },
    AfterUpdateBase {
        span: Span,
        path: PathId,
        fields: &'expr [crate::ast::FieldInitializer],
        bindings: Scope<'expr>,
    },
    UpdateNext {
        span: Span,
        path: PathId,
        base: ResolvedExpr,
        record: DeclarationId,
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Scope<'expr>,
        resolved: Vec<ResolvedFieldInitializer>,
    },
    UpdateAfterField {
        span: Span,
        path: PathId,
        base: ResolvedExpr,
        record: DeclarationId,
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Scope<'expr>,
        resolved: Vec<ResolvedFieldInitializer>,
        field: DeclarationId,
    },
    FinishProject {
        span: Span,
        path: PathId,
        field: &'expr str,
    },
    FinishMethodCall {
        span: Span,
        path: PathId,
        method: &'expr str,
        receiver: &'expr Expr,
        bindings: Scope<'expr>,
        type_arguments: Vec<ResolvedType>,
        args_len: usize,
    },
    FinishSuperMethod {
        span: Span,
        method_span: Span,
        path: PathId,
        method: &'expr str,
        holder: DeclarationId,
        callee: DeclarationId,
        args_len: usize,
    },
    /// The slot edge is created only when this frame resumes, after the
    /// interrupted continuation is re-pushed, preserving the slot bound.
    StartUpcast {
        source: &'expr Expr,
        bindings: Scope<'expr>,
        slot_parent: PathId,
        slot_edge: Step,
        holder: DeclarationId,
        span: Span,
        resume: Box<Frame<'expr>>,
    },
    FinishUpcast {
        slot_path: PathId,
        holder: DeclarationId,
        span: Span,
    },
}

mod fixed_results;
pub(super) use fixed_results::{take_pair, take_triple};

pub(super) fn take_results(results: &mut Vec<ResolvedExpr>, count: usize) -> Vec<ResolvedExpr> {
    let start = results
        .len()
        .checked_sub(count)
        .expect("expression continuation retains every child result");
    results.split_off(start)
}

#[cfg(test)]
pub(super) fn frame_owned_capacity(
    frame: &Frame<'_>,
    seen_scopes: &mut std::collections::HashSet<*const ()>,
) -> usize {
    frame_owned_capacity_with(frame, &mut |pointer| seen_scopes.insert(pointer))
}

#[cfg(test)]
fn frame_scope<'a, 'scope>(frame: &'a Frame<'scope>) -> Option<&'a Scope<'scope>> {
    match frame {
        Frame::Enter { bindings, .. }
        | Frame::ChildNext { bindings, .. }
        | Frame::LiteralFormatArgNext { bindings, .. }
        | Frame::MethodArgNext { bindings, .. }
        | Frame::StartUpcast { bindings, .. }
        | Frame::AfterBinaryLeft { bindings, .. }
        | Frame::AfterIfCondition { bindings, .. }
        | Frame::AfterIfThen { bindings, .. }
        | Frame::RecordNext { bindings, .. }
        | Frame::RecordAfterField { bindings, .. }
        | Frame::VariantNext { bindings, .. }
        | Frame::VariantAfterField { bindings, .. }
        | Frame::AfterMatchScrutinee { bindings, .. }
        | Frame::MatchNext { bindings, .. }
        | Frame::MatchAfterArm { bindings, .. }
        | Frame::ScalarMatchNext { bindings, .. }
        | Frame::ScalarMatchAfterArm { bindings, .. }
        | Frame::AfterUpdateBase { bindings, .. }
        | Frame::UpdateNext { bindings, .. }
        | Frame::UpdateAfterField { bindings, .. } => Some(bindings),
        Frame::BlockNext { scope, .. }
        | Frame::BlockAfterLet { scope, .. }
        | Frame::BlockAfterAssign { scope, .. }
        | Frame::BlockAfterUnsafe { scope, .. }
        | Frame::BlockWhileCondition { scope, .. }
        | Frame::BlockWhileBody { scope, .. } => Some(scope),
        Frame::BlockForBody { scope, .. } | Frame::EnterByteLoop(_, scope) => Some(scope),
        Frame::ForOwn(frame) => Some(&frame.scope),
        _ => None,
    }
}

/// Borrow every retained scope, including boxed upcast resumes, in the same
/// order as the owned-capacity receiver. No pointer is retained or dereferenced.
#[cfg(test)]
pub(super) fn visit_frame_scopes(mut frame: &Frame<'_>, mut visit: impl FnMut(*const ())) {
    loop {
        if let Some(scope) = frame_scope(frame) {
            visit(Rc::as_ptr(scope) as *const ());
        }
        match frame {
            Frame::StartUpcast { resume, .. } => frame = resume,
            _ => break,
        }
    }
}

#[cfg(test)]
pub(super) fn frame_owned_capacity_with(
    frame: &Frame<'_>,
    first_scope: &mut dyn FnMut(*const ()) -> bool,
) -> usize {
    if let Frame::ForOwn(frame) = frame {
        return super::resolve_for_own::capacity(frame, first_scope);
    }
    // Path coordinates are inline; their backing is counted once by the probe.
    let scope = frame_scope(frame).map_or(0, |scope| {
        if first_scope(Rc::as_ptr(scope) as *const ()) {
            super::resolver_scope::owned_capacity(scope)
        } else {
            0
        }
    });
    let retained = match frame {
        Frame::FinishVecConstructor { element, .. } => resolved_type_owned_capacity(element),
        Frame::EnterLoop(target)
        | Frame::EnterByteLoop(target, _)
        | Frame::FinishLexicalExit { target, .. } => {
            super::capacity_probe::lexical_target_capacity(target)
        }
        Frame::FinishInvoke { callable, .. } => resolved_expr_owned_capacity(callable),
        Frame::FinishLiteralFormat { template, .. } => template.capacity(),
        Frame::FinishVecField { element, field, .. }
        | Frame::FinishSortBy {
            element,
            callee: field,
            ..
        } => resolved_type_owned_capacity(element) + field.as_str().len(),
        Frame::FinishMethodCall { type_arguments, .. } => {
            type_arguments.capacity() * std::mem::size_of::<ResolvedType>()
                + type_arguments
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>()
        }
        Frame::FinishCall {
            type_arguments,
            return_source_type,
            ..
        } => {
            type_arguments.capacity() * std::mem::size_of::<ResolvedType>()
                + type_arguments
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>()
                + match return_source_type {
                    Type::Function { parameters, .. } => {
                        parameters.capacity() * std::mem::size_of::<Type>()
                            + std::mem::size_of::<Type>()
                    }
                    Type::Never
                    | Type::OnceFunction
                    | Type::OnceFunctionI64
                    | Type::OnceFunctionI64Pair
                    | Type::MutFunctionI64
                    | Type::I64
                    | Type::I32
                    | Type::Char
                    | Type::U8
                    | Type::Usize
                    | Type::ArrayU8(_)
                    | Type::F32
                    | Type::F64
                    | Type::Bool => 0,
                    Type::String | Type::Bytes | Type::Str | Type::SliceU8 | Type::StringMap => 0,
                    Type::Named { name, arguments } => {
                        name.capacity() + arguments.capacity() * std::mem::size_of::<Type>()
                    }
                }
        }
        Frame::BlockNext { resolved, .. }
        | Frame::BlockAfterLet { resolved, .. }
        | Frame::BlockAfterAssign { resolved, .. }
        | Frame::BlockAfterUnsafe { resolved, .. }
        | Frame::BlockWhileCondition { resolved, .. }
        | Frame::BlockWhileBody { resolved, .. }
        | Frame::BlockForBody { resolved, .. }
        | Frame::FinishBlock {
            statements: resolved,
            ..
        } => {
            resolved.capacity() * std::mem::size_of::<ResolvedStatement>()
                + resolved
                    .iter()
                    .map(resolved_statement_owned_capacity)
                    .sum::<usize>()
        }
        Frame::RecordNext {
            arguments,
            resolved,
            ..
        }
        | Frame::RecordAfterField {
            arguments,
            resolved,
            ..
        } => {
            arguments.capacity() * std::mem::size_of::<ResolvedType>()
                + arguments
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>()
                + resolved.capacity() * std::mem::size_of::<ResolvedFieldInitializer>()
                + resolved
                    .iter()
                    .map(resolved_field_initializer_owned_capacity)
                    .sum::<usize>()
        }
        Frame::VariantNext { resolved, .. } | Frame::VariantAfterField { resolved, .. } => {
            resolved.capacity() * std::mem::size_of::<ResolvedFieldInitializer>()
                + resolved
                    .iter()
                    .map(resolved_field_initializer_owned_capacity)
                    .sum::<usize>()
        }
        Frame::MatchNext {
            scrutinee,
            instance_arguments,
            resolved,
            ..
        }
        | Frame::MatchAfterArm {
            scrutinee,
            instance_arguments,
            resolved,
            ..
        } => {
            resolved_expr_owned_capacity(scrutinee)
                + instance_arguments.capacity() * std::mem::size_of::<ResolvedType>()
                + instance_arguments
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>()
                + resolved.capacity() * std::mem::size_of::<ResolvedMatchArm>()
                + resolved
                    .iter()
                    .map(resolved_match_arm_owned_capacity)
                    .sum::<usize>()
        }
        Frame::ScalarMatchNext {
            scrutinee,
            resolved,
            ..
        }
        | Frame::ScalarMatchAfterArm {
            scrutinee,
            resolved,
            ..
        } => {
            resolved_expr_owned_capacity(scrutinee)
                + resolved.capacity() * std::mem::size_of::<ResolvedMatchArm>()
                + resolved
                    .iter()
                    .map(resolved_match_arm_owned_capacity)
                    .sum::<usize>()
        }
        Frame::UpdateNext { base, resolved, .. }
        | Frame::UpdateAfterField { base, resolved, .. } => {
            resolved_expr_owned_capacity(base)
                + resolved.capacity() * std::mem::size_of::<ResolvedFieldInitializer>()
                + resolved
                    .iter()
                    .map(resolved_field_initializer_owned_capacity)
                    .sum::<usize>()
        }
        _ => 0,
    };
    // An interrupted continuation boxed beneath an upcast is retained too.
    let resumed = match frame {
        Frame::StartUpcast { resume, .. } => std::mem::size_of::<Frame<'_>>()
            .saturating_add(frame_owned_capacity_with(resume, first_scope)),
        _ => 0,
    };
    scope.saturating_add(retained).saturating_add(resumed)
}

#[cfg(test)]
pub(super) mod capacity_old;

#[cfg(test)]
mod literal_format_capacity_tests {
    use super::*;
    #[test]
    fn literal_format_frames_count_template_paths_and_shared_scope_once() {
        use super::super::resolve_expr::path_arena::PathArena;
        let bindings = crate::hir::resolver_scope::owned(BTreeMap::new());
        let template = "prefix {}".to_owned();
        let mut paths = PathArena::new("body");
        let path = paths.child_at(1, 1, PathId::ROOT, Seg::Arg.at(0));
        assert_eq!(paths.at(path).materialize_with(&[]), "body.arg.0");
        // The coordinate owns no backing; the arena's is counted once.
        let arena = paths.owned_capacity();
        assert_eq!(
            arena,
            super::super::resolve_expr::path_arena::PATH_NODE_BYTES
        );
        let frame = Frame::FinishLiteralFormat {
            span: Span::default(),
            path,
            template: template.clone(),
            argument_count: 1,
        };
        let mut seen = std::collections::HashSet::new();
        assert_eq!(frame_owned_capacity(&frame, &mut seen), template.capacity());
        let next = Frame::LiteralFormatArgNext {
            args: &[],
            index: 1,
            bindings: Rc::clone(&bindings),
            path,
        };
        assert_eq!(
            frame_owned_capacity(&next, &mut seen),
            super::super::resolver_scope::owned_capacity(&bindings)
        );
        assert_eq!(seen.len(), 1);
        assert_eq!(frame_owned_capacity(&next, &mut seen), 0);
        assert_eq!(paths.owned_capacity(), arena);
    }

    #[test]
    fn boxed_upcast_resume_frames_are_retained_capacity() {
        let resume = Frame::FinishLiteralFormat {
            span: Span::default(),
            path: PathId::ROOT,
            template: "resume {}".to_owned(),
            argument_count: 1,
        };
        let template = match &resume {
            Frame::FinishLiteralFormat { template, .. } => template.capacity(),
            _ => unreachable!(),
        };
        let expr = crate::ast::Expr {
            kind: crate::ast::ExprKind::Int(0),
            span: Span::default(),
        };
        let frame = Frame::FinishUpcast {
            slot_path: PathId::ROOT,
            holder: DeclarationId::new("holder"),
            span: Span::default(),
        };
        let mut seen = std::collections::HashSet::new();
        assert_eq!(frame_owned_capacity(&frame, &mut seen), 0);
        let start = Frame::StartUpcast {
            source: &expr,
            bindings: crate::hir::resolver_scope::owned(BTreeMap::new()),
            slot_parent: PathId::ROOT,
            slot_edge: Seg::ArgZero.into(),
            holder: DeclarationId::new("holder"),
            span: Span::default(),
            resume: Box::new(resume),
        };
        let scope = match &start {
            Frame::StartUpcast { bindings, .. } => {
                super::super::resolver_scope::owned_capacity(bindings)
            }
            _ => unreachable!(),
        };
        assert_eq!(
            frame_owned_capacity(&start, &mut seen),
            scope + std::mem::size_of::<Frame<'_>>() + template
        );
    }
}
