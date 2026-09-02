//! Continuation frames for the iterative expression resolver.
//!
//! Hoisted out of `resolve_expr_iterative` unchanged: the frame shape,
//! its result splice helper, and the test-only capacity probe.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, Span, Statement, Type, UnaryOp};

#[cfg(test)]
use super::capacity_probe::{
    resolved_expr_owned_capacity, resolved_field_initializer_owned_capacity,
    resolved_match_arm_owned_capacity, resolved_statement_owned_capacity,
    resolved_type_owned_capacity, resolver_scope_owned_capacity,
};
use super::expr_nodes::{
    ResolvedExpr, ResolvedFieldInitializer, ResolvedMatchArm, ResolvedMatchPattern,
    ResolvedStatement,
};
use super::ids::{DeclarationId, FunctionInstanceId};
use super::nodes::{
    DeclarationKind, ResolvedBinding, ResolvedHostCommandOperation, ResolvedMatchMode, ResolvedType,
};
use super::Binding;

pub(super) enum Frame<'expr> {
    Enter {
        expr: &'expr Expr,
        bindings: Rc<BTreeMap<String, Binding>>,
        path: String,
    },
    FinishNativeCall {
        span: Span,
        path: String,
        import: DeclarationId,
        argument_count: usize,
    },
    FinishCall {
        span: Span,
        path: String,
        callee: DeclarationId,
        type_arguments: Vec<ResolvedType>,
        instance: Option<FunctionInstanceId>,
        return_source_type: Type,
        target_span: Span,
        argument_count: usize,
    },
    FinishStringOp {
        span: Span,
        path: String,
        op: crate::string_ops::StringOp,
        argument_count: usize,
    },
    FinishStrOp {
        span: Span,
        path: String,
        op: crate::str_ops::StrOp,
        argument_count: usize,
    },
    FinishByteOp {
        span: Span,
        path: String,
        op: crate::byte_ops::ByteOp,
        argument_count: usize,
    },
    FinishHostIoOp {
        span: Span,
        path: String,
        op: crate::host_io_ops::HostIoOp,
        argument_count: usize,
    },
    FinishHostCommandOp {
        span: Span,
        path: String,
        op: ResolvedHostCommandOperation,
        argument_count: usize,
    },
    ChildNext {
        children: &'expr [Expr],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        path: String,
        segment: &'static str,
    },
    MethodArgNext {
        args: &'expr [Expr],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        path: String,
    },
    FinishUnary {
        span: Span,
        path: String,
        op: UnaryOp,
    },
    FinishBinary {
        span: Span,
        path: String,
        op: BinaryOp,
    },
    AfterBinaryLeft {
        span: Span,
        path: String,
        op: BinaryOp,
        right: &'expr Expr,
        bindings: Rc<BTreeMap<String, Binding>>,
    },
    BlockNext {
        span: Span,
        path: String,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedStatement>,
    },
    BlockAfterLet {
        span: Span,
        path: String,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedStatement>,
    },
    BlockAfterAssign {
        span: Span,
        path: String,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedStatement>,
        target: ResolvedBinding,
        /// Field Mutation v1: the resolved direct field and its
        /// substituted type when the target is `<binding>.<field>`.
        target_field: Option<(DeclarationId, ResolvedType)>,
    },
    BlockAfterUnsafe {
        span: Span,
        path: String,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedStatement>,
    },
    BlockWhileCondition {
        span: Span,
        path: String,
        condition: &'expr Expr,
        body: &'expr Expr,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedStatement>,
    },
    BlockWhileBody {
        span: Span,
        path: String,
        statements: &'expr [Statement],
        tail: &'expr Expr,
        index: usize,
        scope: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedStatement>,
        condition: Box<ResolvedExpr>,
        condition_span: Span,
    },
    FinishBlock {
        span: Span,
        path: String,
        statements: Vec<ResolvedStatement>,
    },
    FinishIf {
        span: Span,
        path: String,
    },
    AfterIfCondition {
        span: Span,
        path: String,
        then_branch: &'expr Expr,
        else_branch: &'expr Expr,
        bindings: Rc<BTreeMap<String, Binding>>,
    },
    AfterIfThen {
        span: Span,
        path: String,
        else_branch: &'expr Expr,
        bindings: Rc<BTreeMap<String, Binding>>,
    },
    RecordNext {
        span: Span,
        path: String,
        type_name: &'expr str,
        record: DeclarationId,
        arguments: Vec<ResolvedType>,
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedFieldInitializer>,
    },
    RecordAfterField {
        span: Span,
        path: String,
        type_name: &'expr str,
        record: DeclarationId,
        arguments: Vec<ResolvedType>,
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedFieldInitializer>,
        field: DeclarationId,
    },
    VariantNext {
        span: Span,
        path: String,
        type_name: &'expr str,
        case_name: &'expr str,
        variant: DeclarationId,
        case: DeclarationId,
        type_arguments: &'expr [Type],
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedFieldInitializer>,
    },
    VariantAfterField {
        span: Span,
        path: String,
        type_name: &'expr str,
        case_name: &'expr str,
        variant: DeclarationId,
        case: DeclarationId,
        type_arguments: &'expr [Type],
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedFieldInitializer>,
        field: DeclarationId,
    },
    AfterMatchScrutinee {
        span: Span,
        path: String,
        mode: ResolvedMatchMode,
        arms: &'expr [crate::ast::MatchArm],
        bindings: Rc<BTreeMap<String, Binding>>,
    },
    MatchNext {
        span: Span,
        path: String,
        mode: ResolvedMatchMode,
        arms: &'expr [crate::ast::MatchArm],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        scrutinee: ResolvedExpr,
        matched_type: DeclarationId,
        instance_arguments: Vec<ResolvedType>,
        matched_kind: DeclarationKind,
        resolved: Vec<ResolvedMatchArm>,
    },
    MatchAfterArm {
        span: Span,
        path: String,
        mode: ResolvedMatchMode,
        arms: &'expr [crate::ast::MatchArm],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
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
        path: String,
        mode: ResolvedMatchMode,
        arms: &'expr [crate::ast::MatchArm],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        scrutinee: ResolvedExpr,
        resolved: Vec<ResolvedMatchArm>,
    },
    ScalarMatchAfterArm {
        span: Span,
        path: String,
        mode: ResolvedMatchMode,
        arms: &'expr [crate::ast::MatchArm],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        scrutinee: ResolvedExpr,
        resolved: Vec<ResolvedMatchArm>,
        pattern: ResolvedMatchPattern,
    },
    FinishTry {
        span: Span,
        path: String,
    },
    AfterUpdateBase {
        span: Span,
        path: String,
        fields: &'expr [crate::ast::FieldInitializer],
        bindings: Rc<BTreeMap<String, Binding>>,
    },
    UpdateNext {
        span: Span,
        path: String,
        base: ResolvedExpr,
        record: DeclarationId,
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedFieldInitializer>,
    },
    UpdateAfterField {
        span: Span,
        path: String,
        base: ResolvedExpr,
        record: DeclarationId,
        fields: &'expr [crate::ast::FieldInitializer],
        index: usize,
        bindings: Rc<BTreeMap<String, Binding>>,
        resolved: Vec<ResolvedFieldInitializer>,
        field: DeclarationId,
    },
    FinishProject {
        span: Span,
        path: String,
        field: &'expr str,
    },
    FinishMethodCall {
        span: Span,
        path: String,
        method: &'expr str,
        receiver: &'expr Expr,
        bindings: Rc<BTreeMap<String, Binding>>,
        type_arguments: Vec<ResolvedType>,
        args_len: usize,
    },
    FinishSuperMethod {
        span: Span,
        method_span: Span,
        path: String,
        method: &'expr str,
        holder: DeclarationId,
        callee: DeclarationId,
        args_len: usize,
    },
    StartUpcast {
        source: &'expr Expr,
        bindings: Rc<BTreeMap<String, Binding>>,
        slot_path: String,
        holder: DeclarationId,
        span: Span,
        resume: Box<Frame<'expr>>,
    },
    FinishUpcast {
        slot_path: String,
        holder: DeclarationId,
        span: Span,
    },
}

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
    seen_scopes: &mut std::collections::HashSet<*const BTreeMap<String, Binding>>,
) -> usize {
    let path = match frame {
        Frame::Enter { path, .. }
        | Frame::FinishNativeCall { path, .. }
        | Frame::FinishCall { path, .. }
        | Frame::FinishStringOp { path, .. }
        | Frame::FinishStrOp { path, .. }
        | Frame::FinishByteOp { path, .. }
        | Frame::FinishHostIoOp { path, .. }
        | Frame::FinishHostCommandOp { path, .. }
        | Frame::ChildNext { path, .. }
        | Frame::MethodArgNext { path, .. }
        | Frame::FinishUnary { path, .. }
        | Frame::FinishBinary { path, .. }
        | Frame::AfterBinaryLeft { path, .. }
        | Frame::BlockNext { path, .. }
        | Frame::BlockAfterLet { path, .. }
        | Frame::BlockAfterAssign { path, .. }
        | Frame::BlockAfterUnsafe { path, .. }
        | Frame::BlockWhileCondition { path, .. }
        | Frame::BlockWhileBody { path, .. }
        | Frame::FinishBlock { path, .. }
        | Frame::FinishIf { path, .. }
        | Frame::AfterIfCondition { path, .. }
        | Frame::AfterIfThen { path, .. }
        | Frame::RecordNext { path, .. }
        | Frame::RecordAfterField { path, .. }
        | Frame::VariantNext { path, .. }
        | Frame::VariantAfterField { path, .. }
        | Frame::AfterMatchScrutinee { path, .. }
        | Frame::MatchNext { path, .. }
        | Frame::MatchAfterArm { path, .. }
        | Frame::ScalarMatchNext { path, .. }
        | Frame::ScalarMatchAfterArm { path, .. }
        | Frame::FinishTry { path, .. }
        | Frame::AfterUpdateBase { path, .. }
        | Frame::UpdateNext { path, .. }
        | Frame::UpdateAfterField { path, .. }
        | Frame::FinishProject { path, .. }
        | Frame::FinishMethodCall { path, .. }
        | Frame::FinishSuperMethod { path, .. }
        | Frame::StartUpcast {
            slot_path: path, ..
        }
        | Frame::FinishUpcast {
            slot_path: path, ..
        } => path.capacity(),
    };
    // Continuations share immutable binding maps through `Rc`. Count
    // the owned map allocation once, not once per retaining frame.
    let mut unique_scope_capacity = |scope: &Rc<BTreeMap<String, Binding>>| {
        if seen_scopes.insert(Rc::as_ptr(scope)) {
            resolver_scope_owned_capacity(scope)
        } else {
            0
        }
    };
    let scope = match frame {
        Frame::Enter { bindings, .. }
        | Frame::ChildNext { bindings, .. }
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
        | Frame::UpdateAfterField { bindings, .. } => unique_scope_capacity(bindings),
        Frame::BlockNext { scope, .. }
        | Frame::BlockAfterLet { scope, .. }
        | Frame::BlockAfterAssign { scope, .. }
        | Frame::BlockAfterUnsafe { scope, .. }
        | Frame::BlockWhileCondition { scope, .. }
        | Frame::BlockWhileBody { scope, .. } => unique_scope_capacity(scope),
        _ => 0,
    };
    let retained = match frame {
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
                    Type::I64
                    | Type::I32
                    | Type::Char
                    | Type::U8
                    | Type::Usize
                    | Type::ArrayU8(_)
                    | Type::F32
                    | Type::F64
                    | Type::Bool => 0,
                    Type::String | Type::Bytes | Type::Str | Type::SliceU8 => 0,
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
    path.saturating_add(scope).saturating_add(retained)
}
