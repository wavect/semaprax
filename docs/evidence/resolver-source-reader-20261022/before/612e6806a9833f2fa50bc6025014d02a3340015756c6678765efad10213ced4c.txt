//! Saved pre-change owned HashSet capacity receiver; test-only oracle.
use super::*;

pub(in crate::hir) fn frame_owned_capacity(
    frame: &Frame<'_>,
    seen_scopes: &mut std::collections::HashSet<*const BTreeMap<String, Binding>>,
) -> usize {
    if let Frame::ForOwn(frame) = frame {
        return for_own_capacity(frame, seen_scopes);
    }
    // Path fields are inline `PathId` coordinates. Their arena backing is
    // shared by every frame and counted once by `resolve_expr::capacity`.
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
        | Frame::UpdateAfterField { bindings, .. } => unique_scope_capacity(bindings),
        Frame::BlockNext { scope, .. }
        | Frame::BlockAfterLet { scope, .. }
        | Frame::BlockAfterAssign { scope, .. }
        | Frame::BlockAfterUnsafe { scope, .. }
        | Frame::BlockWhileCondition { scope, .. }
        | Frame::BlockWhileBody { scope, .. } => unique_scope_capacity(scope),
        Frame::BlockForBody { scope, .. } => unique_scope_capacity(scope),
        _ => 0,
    };
    let retained = match frame {
        Frame::FinishVecConstructor { element, .. } => resolved_type_owned_capacity(element),
        Frame::EnterLoop(target) | Frame::FinishLexicalExit { target, .. } => {
            crate::hir::capacity_probe::lexical_target_capacity(target)
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
            .saturating_add(frame_owned_capacity(resume, seen_scopes)),
        _ => 0,
    };
    scope.saturating_add(retained).saturating_add(resumed)
}

fn for_own_capacity(
    frame: &crate::hir::borrowed_root_oracle::resolve_for_own::ForOwnFrame<'_>,
    seen: &mut std::collections::HashSet<*const BTreeMap<String, Binding>>,
) -> usize {
    std::mem::size_of::<crate::hir::borrowed_root_oracle::resolve_for_own::ForOwnFrame<'_>>()
        + frame.resolved.capacity() * std::mem::size_of::<ResolvedStatement>()
        + frame
            .resolved
            .iter()
            .map(resolved_statement_owned_capacity)
            .sum::<usize>()
        + frame
            .source
            .as_ref()
            .map_or(0, resolved_expr_owned_capacity)
        + if seen.insert(Rc::as_ptr(&frame.scope)) {
            resolver_scope_owned_capacity(&frame.scope)
        } else {
            0
        }
}
