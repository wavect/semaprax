//! Test-only recursive expression resolver.
//!
//! Kept as the differential reference the iterative resolver is
//! checked against; never used by a production build.

use std::collections::BTreeMap;

use crate::ast::{BinaryOp, Expr, ExprKind, MatchPattern, Statement, TypeDeclarationKind, UnaryOp};
use crate::diagnostic::Diagnostic;

use super::expr_nodes::{
    PatternValue, ResolvedExpr, ResolvedExprKind, ResolvedFieldInitializer, ResolvedMatchArm,
    ResolvedMatchPattern, ResolvedMatchPatternField, ResolvedStatement,
};
use super::ids::{DeclarationId, ExpressionId, FunctionExecutionId, FunctionInstanceId, ValueId};
use super::monomorphize::{substitute_source_function_type, substitute_type};
use super::nodes::{
    is_scalar_resolved_type, resolver_admits_flat_owned_byte_variant, DeclarationKind,
    OwnershipMode, ResolvedBinding, ResolvedHostCommandCall, ResolvedImportResultKind,
    ResolvedMatchMode, ResolvedNativeRustImportCall, ResolvedType,
};
use super::{Binding, Place, PlaceProjection, Resolver};

impl Resolver<'_> {
    #[cfg(test)]
    #[allow(dead_code)]
    pub(super) fn resolve_expr_recursive_reference(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let id = ExpressionId::new(function, path);
        let (kind, ty, ownership) = match &expr.kind {
            ExprKind::Int(value) => (
                ResolvedExprKind::Int(*value),
                ResolvedType::I64,
                OwnershipMode::Value,
            ),
            ExprKind::Int32(value) => (
                ResolvedExprKind::Int32(*value),
                ResolvedType::I32,
                OwnershipMode::Value,
            ),
            ExprKind::Char(value) => (
                ResolvedExprKind::Char(*value),
                ResolvedType::Char,
                OwnershipMode::Value,
            ),
            ExprKind::Uint8(value) => (
                ResolvedExprKind::Uint8(*value),
                ResolvedType::U8,
                OwnershipMode::Value,
            ),
            ExprKind::Usize(value) => (
                ResolvedExprKind::Usize(*value),
                ResolvedType::Usize,
                OwnershipMode::Value,
            ),
            ExprKind::ArrayU8(values) => (
                ResolvedExprKind::ArrayU8(values.clone()),
                ResolvedType::ArrayU8(values.len() as u32),
                OwnershipMode::Value,
            ),
            ExprKind::RepeatArrayU8 { value, count } => (
                ResolvedExprKind::RepeatArrayU8 {
                    value: *value,
                    count: *count,
                },
                ResolvedType::ArrayU8(*count),
                OwnershipMode::Value,
            ),
            ExprKind::Float32(bits) => (
                ResolvedExprKind::Float32(*bits),
                ResolvedType::F32,
                OwnershipMode::Value,
            ),
            ExprKind::Float64(bits) => (
                ResolvedExprKind::Float64(*bits),
                ResolvedType::F64,
                OwnershipMode::Value,
            ),
            ExprKind::Bool(value) => (
                ResolvedExprKind::Bool(*value),
                ResolvedType::Bool,
                OwnershipMode::Value,
            ),
            ExprKind::String(value) => (
                ResolvedExprKind::String(value.clone()),
                ResolvedType::String,
                OwnershipMode::Own,
            ),
            ExprKind::Var(name) => {
                let binding = bindings.get(name).ok_or_else(|| {
                    self.error("SPX-H002", format!("unresolved value `{name}`"), expr.span)
                })?;
                (
                    ResolvedExprKind::Place(Place {
                        root: binding.id.clone(),
                        projections: Vec::new(),
                    }),
                    binding.ty.clone(),
                    binding.ownership,
                )
            }
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } => {
                if let Some(import_id) = self.declarations.native_rust_import_id(name).cloned() {
                    let import = self
                        .program
                        .interfaces
                        .iter()
                        .flat_map(|interface| &interface.imports)
                        .find(|import| import.stable_id == import_id.as_str())
                        .expect("native Rust import index is built from source imports");
                    if !type_arguments.is_empty() || args.len() != import.params.len() {
                        return Err(self.error(
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: scalar value signature required",
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.native-rust-arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (argument, parameter) in args.iter().zip(&import.params) {
                        if argument.ty != self.resolve_type(&parameter.ty, parameter.span)? {
                            return Err(self.error(
                                "SPX-B107",
                                "Native Rust Interop declaration set is unsupported: scalar value signature required",
                                argument.span,
                            ));
                        }
                    }
                    let result = match import.result {
                        crate::ast::ImportResult::Unit => ResolvedImportResultKind::Unit,
                        crate::ast::ImportResult::I64 => ResolvedImportResultKind::I64,
                        crate::ast::ImportResult::Bool => ResolvedImportResultKind::Bool,
                    };
                    let ty = match result {
                        ResolvedImportResultKind::Unit => ResolvedType::Unit,
                        ResolvedImportResultKind::I64 => ResolvedType::I64,
                        ResolvedImportResultKind::Bool => ResolvedType::Bool,
                    };
                    return Ok(ResolvedExpr {
                        id,
                        ty,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::NativeRustImportCall(
                            ResolvedNativeRustImportCall {
                                expression: ExpressionId::new(function, path),
                                import: import_id,
                                args,
                                result,
                            },
                        ),
                        span: expr.span,
                    });
                }
                if let Some(op) = crate::string_ops::by_name(name) {
                    // Oracle parity: the recursive-reference resolver admits
                    // string operations exactly like the iterative resolver.
                    if !type_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-H006",
                            format!("string operation `{name}` has type arguments"),
                            expr.span,
                        ));
                    }
                    if args.len() != op.arity() {
                        return Err(self.error(
                            "SPX-H006",
                            format!(
                                "string operation `{name}` expects {} arguments, received {}",
                                op.arity(),
                                args.len()
                            ),
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (index, argument) in args.iter().enumerate() {
                        if argument.ty != op.param_types()[index] {
                            let expected = &op.param_types()[index];
                            return Err(self.error(
                                "SPX-H006",
                                format!(
                                    "string operation `{}` argument {} expects `{}`, received `{}`",
                                    op.name(),
                                    index,
                                    expected.identity_key(),
                                    argument.ty.identity_key()
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let ty = op.return_type();
                    let ownership =
                        self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                    return Ok(ResolvedExpr {
                        id,
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span: expr.span,
                    });
                }
                if let Some(op) = crate::str_ops::by_name(name) {
                    if !type_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-H006",
                            format!("borrowed string operation `{name}` has type arguments"),
                            expr.span,
                        ));
                    }
                    if args.len() != op.arity() {
                        return Err(self.error(
                            "SPX-H006",
                            format!(
                                "borrowed string operation `{name}` expects {} arguments, received {}",
                                op.arity(),
                                args.len()
                            ),
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (index, argument) in args.iter().enumerate() {
                        if argument.ty != op.param_types()[index] {
                            let expected = &op.param_types()[index];
                            return Err(self.error(
                                "SPX-H006",
                                format!(
                                    "borrowed string operation `{}` argument {} expects `{}`, received `{}`",
                                    op.name(),
                                    index,
                                    expected.identity_key(),
                                    argument.ty.identity_key()
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let ty = op.return_type();
                    let ownership =
                        self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                    return Ok(ResolvedExpr {
                        id,
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span: expr.span,
                    });
                }
                if let Some(op) = crate::byte_ops::by_name(name) {
                    if !type_arguments.is_empty() || args.len() != op.arity() {
                        return Err(self.error(
                            "SPX-H006",
                            format!("invalid byte operation `{name}` call shape"),
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (index, argument) in args.iter().enumerate() {
                        if !op.accepts_resolved(index, &argument.ty) {
                            return Err(self.error(
                                "SPX-H006",
                                format!(
                                    "byte operation `{}` argument {} has the wrong type",
                                    op.name(),
                                    index
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let ty = op.return_type();
                    let ownership =
                        self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                    if op == crate::byte_ops::ByteOp::Range {
                        let mut args = args.into_iter();
                        let source = args.next().expect("range has a source");
                        if !matches!(source.kind, ResolvedExprKind::Place(ref place) if place.projections.is_empty())
                        {
                            return Err(self.error(
                                "SPX-T266",
                                "byte_range requires an exact named Slice<u8> source",
                                source.span,
                            ));
                        }
                        let start = args.next().expect("range has a start");
                        let end = args.next().expect("range has an end");
                        debug_assert!(args.next().is_none());
                        return Ok(ResolvedExpr {
                            id,
                            ty,
                            ownership: OwnershipMode::Borrow,
                            kind: ResolvedExprKind::ByteRange {
                                operation: DeclarationId::new(crate::byte_ops::RANGE_ID),
                                source: Box::new(source),
                                start: Box::new(start),
                                end: Box::new(end),
                            },
                            span: expr.span,
                        });
                    }
                    if op.is_view() {
                        let ResolvedExprKind::Place(place) = &args[0].kind else {
                            return Err(self.error(
                                "SPX-T266",
                                format!(
                                    "borrowed view `{name}` requires an exact named storage root"
                                ),
                                args[0].span,
                            ));
                        };
                        return Ok(ResolvedExpr {
                            id,
                            ty,
                            ownership: OwnershipMode::Borrow,
                            kind: ResolvedExprKind::BorrowPlace {
                                operation: DeclarationId::new(op.id()),
                                place: place.clone(),
                            },
                            span: expr.span,
                        });
                    }
                    return Ok(ResolvedExpr {
                        id,
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span: expr.span,
                    });
                }
                if let Some(op) = crate::host_io_ops::by_name(name) {
                    if !type_arguments.is_empty() || args.len() != op.arity() {
                        return Err(self.error(
                            "SPX-T269",
                            format!("invalid host I/O operation `{name}` call shape"),
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (index, argument) in args.iter().enumerate() {
                        if !op.accepts_resolved(index, &argument.ty) {
                            return Err(self.error(
                                "SPX-T269",
                                format!(
                                    "host I/O operation `{}` argument {} has the wrong type",
                                    op.name(),
                                    index
                                ),
                                argument.span,
                            ));
                        }
                    }
                    return Ok(ResolvedExpr {
                        id,
                        ty: op.return_type(),
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span: expr.span,
                    });
                }
                if let Some(op) = crate::command_io_ops::by_name(name) {
                    if !type_arguments.is_empty() || args.len() != crate::command_io_ops::arity(op)
                    {
                        return Err(self.error(
                            "SPX-T270",
                            format!("invalid command I/O operation `{name}` call shape"),
                            expr.span,
                        ));
                    }
                    let args = args
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            self.resolve_expr_recursive_reference(
                                function,
                                argument,
                                bindings,
                                &format!("{path}.arg.{index}"),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    for (index, argument) in args.iter().enumerate() {
                        if !crate::command_io_ops::accepts_resolved(op, index, &argument.ty) {
                            return Err(self.error(
                                "SPX-T270",
                                format!("command I/O operation `{name}` argument {index} has the wrong type"),
                                argument.span,
                            ));
                        }
                    }
                    return Ok(ResolvedExpr {
                        id: id.clone(),
                        ty: crate::command_io_ops::return_type(op),
                        ownership: crate::command_io_ops::result_ownership(op),
                        kind: ResolvedExprKind::HostCommandCall(ResolvedHostCommandCall {
                            expression: id,
                            operation: op,
                            args,
                        }),
                        span: expr.span,
                    });
                }
                let template = self
                    .declarations
                    .function_id(name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H003",
                            format!("unresolved function `{name}`"),
                            expr.span,
                        )
                    })?;
                let target = self
                    .program
                    .functions
                    .iter()
                    .find(|function| function.stable_id == template.as_str())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H003",
                            format!("function identity `{template}` has no declaration"),
                            expr.span,
                        )
                    })?;
                let resolved_arguments = type_arguments
                    .iter()
                    .map(|argument| self.resolve_type(argument, expr.span))
                    .collect::<Result<Vec<_>, _>>()?;
                let (callee, instance, return_source_type) = if target.type_parameters.is_empty() {
                    if !resolved_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-H006",
                            format!("monomorphic function `{template}` has type arguments"),
                            expr.span,
                        ));
                    }
                    (template.clone(), None, target.return_type.clone())
                } else {
                    if resolved_arguments.len() != target.type_parameters.len()
                        || resolved_arguments.iter().any(|argument| {
                            !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                        })
                    {
                        return Err(self.error(
                            "SPX-H006",
                            format!("generic function `{template}` has invalid type arguments"),
                            expr.span,
                        ));
                    }
                    let instance = FunctionInstanceId::derive(&template, &resolved_arguments);
                    let return_type = substitute_source_function_type(
                        target,
                        type_arguments,
                        &target.return_type,
                    )
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H006",
                            format!("generic function `{template}` return substitution failed"),
                            expr.span,
                        )
                    })?;
                    (template.clone(), Some(instance), return_type)
                };
                let args = args
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        self.resolve_expr_recursive_reference(
                            function,
                            argument,
                            bindings,
                            &format!("{path}.arg.{index}"),
                        )
                    })
                    .collect::<Result<_, _>>()?;
                let ty = self.resolve_type(&return_source_type, target.span)?;
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, target.span)?;
                (
                    ResolvedExprKind::Call {
                        callee,
                        type_arguments: resolved_arguments,
                        instance,
                        args,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::MethodCall {
                receiver,
                method,
                type_arguments,
                args,
                ..
            } => {
                if !type_arguments.is_empty() {
                    return Err(self.error(
                        "SPX-P106",
                        "method call generic arguments are not supported in this slice",
                        expr.span,
                    ));
                }
                let receiver = self.resolve_expr_recursive_reference(
                    function,
                    receiver,
                    bindings,
                    &format!("{path}.arg.0"),
                )?;
                let ResolvedType::Nominal {
                    declaration: class_id,
                    arguments: class_args,
                } = &receiver.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        format!("cannot resolve method `{method}` on a non-class value"),
                        expr.span,
                    ));
                };
                let class_decl = self.declarations.declaration(class_id).ok_or_else(|| {
                    self.error("SPX-H001", format!("unknown class `{class_id}`"), expr.span)
                })?;
                if class_decl.kind != DeclarationKind::Class {
                    return Err(self.error(
                        "SPX-H001",
                        format!("method `{method}` requires a class receiver, found `{class_id}`"),
                        expr.span,
                    ));
                }
                let method_id = self
                    .declarations
                    .declarations()
                    .find(|decl| {
                        decl.kind == DeclarationKind::Function
                            && decl.name == *method
                            && decl.owner.as_ref() == Some(class_id)
                    })
                    .map(|decl| decl.id.clone())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved method `{method}` on class `{class_id}`"),
                            expr.span,
                        )
                    })?;
                let class_ast = self
                    .program
                    .types
                    .iter()
                    .find(|t| t.stable_id == class_id.as_str())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H006",
                            format!("class `{class_id}` has no AST"),
                            expr.span,
                        )
                    })?;
                let TypeDeclarationKind::Class { methods, .. } = &class_ast.kind else {
                    return Err(self.error(
                        "SPX-H006",
                        format!("`{class_id}` is not a class"),
                        expr.span,
                    ));
                };
                let method_ast = methods.iter().find(|m| m.name == *method).ok_or_else(|| {
                    self.error(
                        "SPX-H001",
                        format!("unresolved method `{method}` on class `{class_id}`"),
                        expr.span,
                    )
                })?;
                let Some(self_param) = method_ast.params.first() else {
                    return Err(self.error(
                        "SPX-H001",
                        format!("method `{method}` has no self parameter"),
                        method_ast.span,
                    ));
                };
                let self_ty = self.resolve_type(&self_param.ty, self_param.span)?;
                let expected_self = ResolvedType::Nominal {
                    declaration: class_id.clone(),
                    arguments: class_args.clone(),
                };
                if self_ty != expected_self {
                    return Err(self.error(
                        "SPX-H001",
                        format!(
                            "method `{method}` self parameter type does not match class `{class_id}`"
                        ),
                        self_param.span,
                    ));
                }
                if method_ast.params.len() - 1 != args.len() {
                    return Err(self.error(
                        "SPX-H001",
                        format!(
                            "method `{method}` expects {} arguments, found {}",
                            method_ast.params.len() - 1,
                            args.len()
                        ),
                        expr.span,
                    ));
                }
                let mut call_args = Vec::with_capacity(1 + args.len());
                call_args.push(receiver);
                for (index, (argument, param)) in args
                    .iter()
                    .zip(method_ast.params.iter().skip(1))
                    .enumerate()
                {
                    let resolved = self.resolve_expr_recursive_reference(
                        function,
                        argument,
                        bindings,
                        &format!("{path}.arg.{}", index + 1),
                    )?;
                    let param_ty = self.resolve_type(&param.ty, param.span)?;
                    if resolved.ty != param_ty {
                        return Err(self.error(
                            "SPX-H001",
                            format!(
                                "method `{method}` argument `{}` expects type mismatch",
                                param.name
                            ),
                            argument.span,
                        ));
                    }
                    call_args.push(resolved);
                }
                let ty = self.resolve_type(&method_ast.return_type, method_ast.span)?;
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                (
                    ResolvedExprKind::Call {
                        callee: method_id,
                        type_arguments: Vec::new(),
                        instance: None,
                        args: call_args,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::Unary { op, value } => {
                // Peel this linear family without consuming resolver frames.
                // The general expression-frame conversion handles the other
                // recursive families separately; this fast path preserves the
                // exact canonical `.value` identity chain.
                let mut unary = Vec::new();
                unary.push((*op, expr.span, path.to_owned()));
                let mut leaf = value.as_ref();
                let mut leaf_path = format!("{path}.value");
                while let ExprKind::Unary { op, value } = &leaf.kind {
                    unary.push((*op, leaf.span, leaf_path.clone()));
                    leaf = value;
                    leaf_path.push_str(".value");
                }
                let mut resolved =
                    self.resolve_expr_recursive_reference(function, leaf, bindings, &leaf_path)?;
                for (op, span, unary_path) in unary.into_iter().rev() {
                    let ty = match (&op, &resolved.ty) {
                        (UnaryOp::Neg, ResolvedType::F32) => ResolvedType::F32,
                        (UnaryOp::Neg, ResolvedType::F64) => ResolvedType::F64,
                        (UnaryOp::Neg, ResolvedType::I32) => ResolvedType::I32,
                        (UnaryOp::Neg, _) => ResolvedType::I64,
                        (UnaryOp::Not, _) => ResolvedType::Bool,
                    };
                    resolved = ResolvedExpr {
                        id: ExpressionId::new(function, &unary_path),
                        ty,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Unary {
                            op,
                            value: Box::new(resolved),
                        },
                        span,
                    };
                }
                return Ok(resolved);
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.resolve_expr_recursive_reference(
                    function,
                    left,
                    bindings,
                    &format!("{path}.left"),
                )?;
                let right = self.resolve_expr_recursive_reference(
                    function,
                    right,
                    bindings,
                    &format!("{path}.right"),
                )?;
                let ty = match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem => ResolvedType::I64,
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
                    | BinaryOp::And
                    | BinaryOp::Or => ResolvedType::Bool,
                };
                (
                    ResolvedExprKind::Binary {
                        op: *op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    ty,
                    OwnershipMode::Value,
                )
            }
            ExprKind::Block { statements, tail } => {
                let mut scope = bindings.clone();
                let mut resolved_statements = Vec::with_capacity(statements.len());
                for (index, statement) in statements.iter().enumerate() {
                    let statement_path = format!("{path}.s{index}");
                    match statement {
                        Statement::Let {
                            name,
                            name_span,
                            mutable,
                            declared: _,
                            value,
                            span,
                        } => {
                            let value = self.resolve_expr_recursive_reference(
                                function,
                                value,
                                &scope,
                                &format!("{statement_path}.value"),
                            )?;
                            let binding = ResolvedBinding {
                                id: ValueId::local(function, &statement_path),
                                name: name.clone(),
                                ownership: value.ownership,
                                ty: value.ty.clone(),
                                span: *name_span,
                            };
                            scope.insert(
                                name.clone(),
                                Binding {
                                    id: binding.id.clone(),
                                    ty: binding.ty.clone(),
                                    ownership: binding.ownership,
                                    mutable: *mutable,
                                },
                            );
                            resolved_statements.push(ResolvedStatement::Let {
                                binding,
                                mutable: *mutable,
                                value,
                                span: *span,
                            });
                        }
                        Statement::Assign {
                            name,
                            name_span,
                            field,
                            value,
                            span,
                        } => {
                            let immutable_code = if field.is_some() {
                                "SPX-U107"
                            } else {
                                "SPX-U101"
                            };
                            let target = self.resolve_assign_target(
                                name,
                                *name_span,
                                &scope,
                                immutable_code,
                            )?;
                            let target_field = match field {
                                Some(field) => {
                                    Some(self.resolve_assign_field_target(&target, field)?)
                                }
                                None => None,
                            };
                            let value = self.resolve_expr_recursive_reference(
                                function,
                                value,
                                &scope,
                                &format!("{statement_path}.value"),
                            )?;
                            match &target_field {
                                Some((_, field_ty)) => {
                                    if value.ty != *field_ty {
                                        return Err(self.error(
                                            "SPX-U110",
                                            format!(
                                                "assigned value type `{}` does not exactly match field type `{}`",
                                                value.ty.identity_key(),
                                                field_ty.identity_key()
                                            ),
                                            value.span,
                                        ));
                                    }
                                }
                                None => {
                                    if value.ty != target.ty {
                                        return Err(self.error(
                                            "SPX-U102",
                                            format!(
                                                "assigned value type `{}` does not exactly match binding type `{}`",
                                                value.ty.identity_key(),
                                                target.ty.identity_key()
                                            ),
                                            value.span,
                                        ));
                                    }
                                    if value.ownership != OwnershipMode::Value
                                        || !is_scalar_resolved_type(&value.ty)
                                    {
                                        return Err(self.error(
                                            "SPX-U105",
                                            "explicit mutation v1 supports only scalar Copy values",
                                            value.span,
                                        ));
                                    }
                                }
                            }
                            resolved_statements.push(ResolvedStatement::Assign {
                                binding: target,
                                field: target_field.map(|(field_id, _)| field_id),
                                value,
                                span: *span,
                            });
                        }
                        Statement::Unsafe {
                            audit, body, span, ..
                        } => {
                            let body = self.resolve_expr_recursive_reference(
                                function,
                                body,
                                &scope,
                                &format!("{statement_path}.body"),
                            )?;
                            if body.ownership != OwnershipMode::Value
                                || !is_scalar_resolved_type(&body.ty)
                            {
                                return Err(self.error(
                                    "SPX-N104",
                                    "unsafe boundary bodies must produce a scalar Copy value",
                                    body.span,
                                ));
                            }
                            resolved_statements.push(ResolvedStatement::Unsafe {
                                audit: audit.clone(),
                                body: Box::new(body),
                                span: *span,
                            });
                        }
                        Statement::While {
                            condition,
                            body,
                            span,
                            ..
                        } => {
                            // Mirror the iterative admission and typing checks
                            // exactly, including path spellings.
                            self.reject_while_disallowed(condition)?;
                            self.reject_while_disallowed(body)?;
                            let resolved_condition = self.resolve_expr_recursive_reference(
                                function,
                                condition,
                                &scope,
                                &format!("{statement_path}.condition"),
                            )?;
                            if resolved_condition.ty != ResolvedType::Bool {
                                return Err(self.error(
                                    "SPX-T251",
                                    "`while` condition must be bool",
                                    condition.span,
                                ));
                            }
                            let resolved_body = self.resolve_expr_recursive_reference(
                                function,
                                body,
                                &scope,
                                &format!("{statement_path}.body"),
                            )?;
                            resolved_statements.push(ResolvedStatement::While {
                                condition: Box::new(resolved_condition),
                                body: Box::new(resolved_body),
                                span: condition.span.merge(*span),
                            });
                        }
                    }
                }
                let tail = self.resolve_expr_recursive_reference(
                    function,
                    tail,
                    &scope,
                    &format!("{path}.tail"),
                )?;
                let ty = tail.ty.clone();
                let ownership = tail.ownership;
                (
                    ResolvedExprKind::Block {
                        statements: resolved_statements,
                        tail: Box::new(tail),
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.resolve_expr_recursive_reference(
                    function,
                    condition,
                    bindings,
                    &format!("{path}.condition"),
                )?;
                let then_branch = self.resolve_expr_recursive_reference(
                    function,
                    then_branch,
                    bindings,
                    &format!("{path}.then"),
                )?;
                let else_branch = self.resolve_expr_recursive_reference(
                    function,
                    else_branch,
                    bindings,
                    &format!("{path}.else"),
                )?;
                let ty = then_branch.ty.clone();
                let ownership = then_branch.ownership;
                (
                    ResolvedExprKind::If {
                        condition: Box::new(condition),
                        then_branch: Box::new(then_branch),
                        else_branch: Box::new(else_branch),
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                fields,
                ..
            } => {
                let record = self
                    .declarations
                    .type_id(type_name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved record `{type_name}`"),
                            expr.span,
                        )
                    })?;
                if self.declarations.declaration(&record).is_none_or(|item| {
                    !matches!(item.kind, DeclarationKind::Record | DeclarationKind::Class)
                }) {
                    return Err(self.error(
                        "SPX-H001",
                        format!("constructor target `{type_name}` is not a record or class"),
                        expr.span,
                    ));
                }
                let arguments = type_arguments
                    .iter()
                    .map(|argument| self.resolve_type(argument, expr.span))
                    .collect::<Result<Vec<_>, _>>()?;
                let parameters = self.declarations.type_parameters(&record).ok_or_else(|| {
                    self.error(
                        "SPX-H006",
                        format!("record `{record}` has no parameter metadata"),
                        expr.span,
                    )
                })?;
                if arguments.len() != parameters.len()
                    || arguments
                        .iter()
                        .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
                {
                    return Err(self.error(
                        "SPX-H006",
                        format!("record `{record}` has invalid concrete arguments"),
                        expr.span,
                    ));
                }
                let mut resolved_fields = Vec::with_capacity(fields.len());
                for (index, initializer) in fields.iter().enumerate() {
                    let field = self
                        .declarations
                        .field_id(&record, &initializer.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("unresolved field `{}.{}`", type_name, initializer.name),
                                initializer.name_span,
                            )
                        })?;
                    let value = self.resolve_expr_recursive_reference(
                        function,
                        &initializer.value,
                        bindings,
                        &format!("{path}.field.{index}.value"),
                    )?;
                    resolved_fields.push(ResolvedFieldInitializer { field, value });
                }
                let ty = ResolvedType::Nominal {
                    declaration: record.clone(),
                    arguments,
                };
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                (
                    ResolvedExprKind::ConstructRecord {
                        record,
                        fields: resolved_fields,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                case_name,
                fields,
                ..
            } => {
                let variant = self
                    .declarations
                    .type_id(type_name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved variant `{type_name}`"),
                            expr.span,
                        )
                    })?;
                if self
                    .declarations
                    .declaration(&variant)
                    .is_none_or(|item| item.kind != DeclarationKind::Variant)
                {
                    return Err(self.error(
                        "SPX-H001",
                        format!("constructor target `{type_name}` is not a variant"),
                        expr.span,
                    ));
                }
                let case = self
                    .declarations
                    .case_id(&variant, case_name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved case `{type_name}::{case_name}`"),
                            expr.span,
                        )
                    })?;
                let mut resolved_fields = Vec::with_capacity(fields.len());
                for (index, initializer) in fields.iter().enumerate() {
                    let field = self
                        .declarations
                        .field_id(&case, &initializer.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!(
                                    "unresolved payload field `{type_name}::{case_name}.{}`",
                                    initializer.name
                                ),
                                initializer.name_span,
                            )
                        })?;
                    let value = self.resolve_expr_recursive_reference(
                        function,
                        &initializer.value,
                        bindings,
                        &format!("{path}.field.{index}.value"),
                    )?;
                    resolved_fields.push(ResolvedFieldInitializer { field, value });
                }
                let ty = ResolvedType::Nominal {
                    declaration: variant.clone(),
                    arguments: type_arguments
                        .iter()
                        .map(|argument| self.resolve_type(argument, expr.span))
                        .collect::<Result<Vec<_>, _>>()?,
                };
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                (
                    ResolvedExprKind::ConstructVariant {
                        variant,
                        case,
                        fields: resolved_fields,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                let mode = ResolvedMatchMode::from(*mode);
                let scrutinee = self.resolve_expr_recursive_reference(
                    function,
                    scrutinee,
                    bindings,
                    &format!("{path}.scrutinee"),
                )?;
                // Refutable Match v1: mirror of the iterative resolver's
                // Copy-scalar decision chain, producing identical identities.
                if matches!(
                    scrutinee.ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Usize
                        | ResolvedType::Char
                        | ResolvedType::Bool
                ) {
                    if mode != ResolvedMatchMode::Value {
                        return Err(self.error(
                            "SPX-O117",
                            "explicit match ownership modes require a non-Copy record scrutinee",
                            expr.span,
                        ));
                    }
                    if arms.is_empty() {
                        return Err(self.error(
                            "SPX-H006",
                            "resolved match has no arms",
                            expr.span,
                        ));
                    }
                    self.validate_refutable_match_admission(&scrutinee.ty, arms)?;
                    let mut resolved_arms: Vec<ResolvedMatchArm> = Vec::with_capacity(arms.len());
                    for (arm_index, arm) in arms.iter().enumerate() {
                        let mut arm_bindings = bindings.clone();
                        let pattern = match &arm.pattern {
                            MatchPattern::Wildcard { .. } => ResolvedMatchPattern::Wildcard,
                            MatchPattern::Binding { name, .. } => {
                                let binding = ResolvedBinding {
                                    id: ValueId::local(
                                        function,
                                        &format!("{path}.arm.{arm_index}.binding"),
                                    ),
                                    name: name.clone(),
                                    ownership: OwnershipMode::Value,
                                    ty: scrutinee.ty.clone(),
                                    span: arm.pattern.span(),
                                };
                                arm_bindings.insert(
                                    name.clone(),
                                    Binding {
                                        id: binding.id.clone(),
                                        ty: binding.ty.clone(),
                                        ownership: OwnershipMode::Value,
                                        mutable: false,
                                    },
                                );
                                ResolvedMatchPattern::Binding(binding)
                            }
                            MatchPattern::Literal { value, .. } => {
                                ResolvedMatchPattern::Literal(PatternValue::from_ast(*value))
                            }
                            MatchPattern::Or { alternatives, .. } => ResolvedMatchPattern::Or(
                                alternatives
                                    .iter()
                                    .map(|alternative| match alternative {
                                        MatchPattern::Literal { value, .. } => {
                                            ResolvedMatchPattern::Literal(PatternValue::from_ast(
                                                *value,
                                            ))
                                        }
                                        _ => unreachable!("or-pattern alternatives are literals"),
                                    })
                                    .collect(),
                            ),
                            MatchPattern::Variant { .. } | MatchPattern::Record { .. } => {
                                return Err(self.error(
                                    "SPX-H001",
                                    "aggregate pattern has a Copy-scalar scrutinee",
                                    arm.span,
                                ));
                            }
                        };
                        let guard = match &arm.guard {
                            Some(guard) => {
                                let resolved_guard = self.resolve_expr_recursive_reference(
                                    function,
                                    guard.as_ref(),
                                    &arm_bindings,
                                    &format!("{path}.arm.{arm_index}.guard"),
                                )?;
                                if resolved_guard.ty != ResolvedType::Bool {
                                    return Err(self.error(
                                        "SPX-T256",
                                        format!(
                                            "match guard must be bool; received {}",
                                            resolved_guard.ty.identity_key()
                                        ),
                                        arm.span,
                                    ));
                                }
                                Some(Box::new(resolved_guard))
                            }
                            None => None,
                        };
                        let value = self.resolve_expr_recursive_reference(
                            function,
                            &arm.value,
                            &arm_bindings,
                            &format!("{path}.arm.{arm_index}.value"),
                        )?;
                        if let Some(first) = resolved_arms.first() {
                            if first.value.ty != value.ty
                                || first.value.ownership != value.ownership
                            {
                                return Err(self.error(
                                    "SPX-T259",
                                    format!(
                                        "match arms disagree on the result type; expected {}",
                                        first.value.ty.identity_key()
                                    ),
                                    arm.span,
                                ));
                            }
                        }
                        resolved_arms.push(ResolvedMatchArm {
                            pattern,
                            guard,
                            value,
                            span: arm.span,
                        });
                    }
                    return Ok(ResolvedExpr {
                        id: ExpressionId::new(function, path),
                        ty: resolved_arms[0].value.ty.clone(),
                        ownership: resolved_arms[0].value.ownership,
                        kind: ResolvedExprKind::Match {
                            mode,
                            scrutinee: Box::new(scrutinee),
                            arms: resolved_arms,
                        },
                        span: expr.span,
                    });
                }
                let refutable_syntax = arms.iter().any(|arm| {
                    arm.guard.is_some()
                        || matches!(
                            &arm.pattern,
                            crate::ast::MatchPattern::Literal { .. }
                                | crate::ast::MatchPattern::Or { .. }
                                | crate::ast::MatchPattern::Binding { .. }
                        )
                });
                if refutable_syntax {
                    return Err(self.error(
                        "SPX-T254",
                        format!(
                            "guards and literal/or/binding patterns require a Copy-scalar \
                             scrutinee (i64/i32/u8/char/bool); received {}",
                            scrutinee.ty.identity_key()
                        ),
                        expr.span,
                    ));
                }
                let ResolvedType::Nominal {
                    declaration: matched_type,
                    arguments,
                } = &scrutinee.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve match on a non-record/non-variant value",
                        expr.span,
                    ));
                };
                let matched_kind = self
                    .declarations
                    .declaration(matched_type)
                    .map(|item| item.kind);
                if !matches!(
                    matched_kind,
                    Some(DeclarationKind::Record | DeclarationKind::Variant)
                ) {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve match on a non-record/non-variant value",
                        expr.span,
                    ));
                }
                let matched_kind = matched_kind.expect("matched kind checked above");
                let facts = self.declarations.type_facts(&scrutinee.ty).ok_or_else(|| {
                    self.error("SPX-H006", "match scrutinee has no type facts", expr.span)
                })?;
                match (matched_kind, mode) {
                    (DeclarationKind::Variant, ResolvedMatchMode::Value)
                        if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                    (DeclarationKind::Variant, ResolvedMatchMode::Own)
                        if resolver_admits_flat_owned_byte_variant(
                            &self.declarations,
                            &scrutinee.ty,
                        ) && facts.needs_drop
                            && !facts.copy
                            && scrutinee.ownership == OwnershipMode::Own => {}
                    (DeclarationKind::Variant, ResolvedMatchMode::Borrow)
                        if resolver_admits_flat_owned_byte_variant(
                            &self.declarations,
                            &scrutinee.ty,
                        ) && facts.needs_drop
                            && !facts.copy
                            && matches!(
                                scrutinee.ownership,
                                OwnershipMode::Own | OwnershipMode::Borrow
                            )
                            && matches!(
                                &scrutinee.kind,
                                ResolvedExprKind::Place(place) if place.projections.is_empty()
                            ) => {}
                    (DeclarationKind::Variant, _) => {
                        return Err(self.error(
                            "SPX-O117",
                            "match ownership mode disagrees with the admitted variant scrutinee",
                            expr.span,
                        ));
                    }
                    (DeclarationKind::Record, ResolvedMatchMode::Value)
                        if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                    (DeclarationKind::Record, ResolvedMatchMode::Own)
                        if facts.needs_drop
                            && !facts.copy
                            && scrutinee.ownership == OwnershipMode::Own => {}
                    (DeclarationKind::Record, ResolvedMatchMode::Borrow)
                        if facts.needs_drop
                            && !facts.copy
                            && matches!(
                                scrutinee.ownership,
                                OwnershipMode::Own | OwnershipMode::Borrow
                            )
                            && matches!(scrutinee.kind, ResolvedExprKind::Place(_)) => {}
                    (DeclarationKind::Record, _) => {
                        return Err(self.error(
                            "SPX-O117",
                            "match ownership mode disagrees with the record scrutinee",
                            expr.span,
                        ));
                    }
                    _ => unreachable!("matched kind was restricted above"),
                }
                let instance_arguments = arguments.clone();
                let matched_type = matched_type.clone();
                let mut resolved_arms = Vec::with_capacity(arms.len());
                for (arm_index, arm) in arms.iter().enumerate() {
                    let mut arm_bindings = bindings.clone();
                    let pattern = match &arm.pattern {
                        MatchPattern::Wildcard { span }
                            if matched_kind == DeclarationKind::Variant
                                && mode != ResolvedMatchMode::Value =>
                        {
                            return Err(self.error(
                                "SPX-O117",
                                "explicit ownership variant match requires every case pattern",
                                *span,
                            ));
                        }
                        MatchPattern::Wildcard { .. } => ResolvedMatchPattern::Wildcard,
                        MatchPattern::Variant {
                            case_name, fields, ..
                        } => {
                            if matched_kind != DeclarationKind::Variant {
                                return Err(self.error(
                                    "SPX-H001",
                                    "variant pattern has a record scrutinee",
                                    arm.span,
                                ));
                            }
                            let case = self
                                .declarations
                                .case_id(&matched_type, case_name)
                                .cloned()
                                .ok_or_else(|| {
                                    self.error(
                                        "SPX-H001",
                                        format!("unresolved case `{matched_type}::{case_name}`"),
                                        arm.span,
                                    )
                                })?;
                            let mut resolved_fields = Vec::with_capacity(fields.len());
                            for (field_index, field) in fields.iter().enumerate() {
                                let field_id = self
                                    .declarations
                                    .field_id(&case, &field.name)
                                    .cloned()
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H001",
                                            format!(
                                                "unresolved pattern field `{case}.{}`",
                                                field.name
                                            ),
                                            field.span,
                                        )
                                    })?;
                                let field_template = self
                                    .declarations
                                    .case_fields(&case)
                                    .and_then(|items| items.iter().find(|item| item.id == field_id))
                                    .map(|item| item.ty.clone())
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H001",
                                            format!("pattern field `{field_id}` has no type"),
                                            field.span,
                                        )
                                    })?;
                                let field_ty = substitute_type(
                                    &field_template,
                                    &matched_type,
                                    &instance_arguments,
                                )?;
                                let field_facts =
                                    self.declarations.type_facts(&field_ty).ok_or_else(|| {
                                        self.error(
                                            "SPX-H006",
                                            "variant pattern field has no authenticated type facts",
                                            field.span,
                                        )
                                    })?;
                                let ownership = if field_facts.needs_drop {
                                    match mode {
                                        ResolvedMatchMode::Own => OwnershipMode::Own,
                                        ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                                        ResolvedMatchMode::Value => OwnershipMode::Value,
                                    }
                                } else {
                                    OwnershipMode::Value
                                };
                                let binding = ResolvedBinding {
                                    id: ValueId::local(
                                        function,
                                        &format!("{path}.arm.{arm_index}.binding.{field_index}"),
                                    ),
                                    name: field.binding.clone(),
                                    ownership,
                                    ty: field_ty.clone(),
                                    span: field.binding_span,
                                };
                                arm_bindings.insert(
                                    field.binding.clone(),
                                    Binding {
                                        id: binding.id.clone(),
                                        ty: field_ty,
                                        ownership,
                                        mutable: false,
                                    },
                                );
                                resolved_fields.push(ResolvedMatchPatternField {
                                    field: field_id,
                                    binding,
                                });
                            }
                            ResolvedMatchPattern::Variant {
                                variant: matched_type.clone(),
                                case,
                                fields: resolved_fields,
                            }
                        }
                        MatchPattern::Record {
                            type_name,
                            fields,
                            span,
                            ..
                        } => {
                            if matched_kind != DeclarationKind::Record {
                                return Err(self.error(
                                    "SPX-H001",
                                    "record pattern has a variant scrutinee",
                                    arm.span,
                                ));
                            }
                            self.resolve_record_match_pattern(
                                function,
                                &scrutinee.ty,
                                type_name,
                                fields,
                                &mut arm_bindings,
                                &format!("{path}.arm.{arm_index}.record"),
                                *span,
                                mode,
                            )?
                        }
                        // Refutable Match v1 patterns on aggregate
                        // scrutinees were rejected during admission
                        // (SPX-T254); the legacy chain never sees them.
                        MatchPattern::Literal { span, .. }
                        | MatchPattern::Or { span, .. }
                        | MatchPattern::Binding { span, .. } => {
                            return Err(self.error(
                                "SPX-T254",
                                "guards and literal/or/binding patterns require a \
                                 Copy-scalar scrutinee",
                                *span,
                            ));
                        }
                    };
                    let value = self.resolve_expr_recursive_reference(
                        function,
                        &arm.value,
                        &arm_bindings,
                        &format!("{path}.arm.{arm_index}.value"),
                    )?;
                    resolved_arms.push(ResolvedMatchArm {
                        pattern,
                        // Aggregate matches reject guards with SPX-T254
                        // before any arm resolves.
                        guard: None,
                        value,
                        span: arm.span,
                    });
                }
                let first = resolved_arms.first().ok_or_else(|| {
                    self.error("SPX-H006", "resolved match has no arms", expr.span)
                })?;
                let ty = first.value.ty.clone();
                let ownership = first.value.ownership;
                (
                    ResolvedExprKind::Match {
                        mode,
                        scrutinee: Box::new(scrutinee),
                        arms: resolved_arms,
                    },
                    ty,
                    ownership,
                )
            }
            ExprKind::Try { operand } => {
                let operand = self.resolve_expr_recursive_reference(
                    function,
                    operand,
                    bindings,
                    &format!("{path}.operand"),
                )?;
                let operand_type = operand.ty.clone();
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &operand_type
                else {
                    return Err(self.error(
                        "SPX-H006",
                        "resolved `?` operand is not the ordinary Result",
                        expr.span,
                    ));
                };
                let target = self
                    .program
                    .functions
                    .iter()
                    .find(|candidate| {
                        matches!(
                            function,
                            FunctionExecutionId::Monomorphic(declaration)
                                if candidate.stable_id == declaration.as_str()
                        )
                    })
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H006",
                            format!("resolved `?` has unknown enclosing function `{function}`"),
                            expr.span,
                        )
                    })?;
                let residual_type = self.resolve_type(&target.return_type, target.span)?;
                match (declaration.as_str(), arguments.as_slice()) {
                    (crate::prelude::RESULT_ID, [ok_type, _]) => (
                        ResolvedExprKind::Try {
                            operand: Box::new(operand),
                            result: DeclarationId::new(crate::prelude::RESULT_ID),
                            ok_case: DeclarationId::new(crate::prelude::RESULT_OK_ID),
                            ok_field: DeclarationId::new(crate::prelude::RESULT_OK_VALUE_ID),
                            err_case: DeclarationId::new(crate::prelude::RESULT_ERR_ID),
                            err_field: DeclarationId::new(crate::prelude::RESULT_ERR_ERROR_ID),
                            residual_type,
                        },
                        ok_type.clone(),
                        OwnershipMode::Value,
                    ),
                    (crate::prelude::OPTION_ID, [some_type]) => (
                        ResolvedExprKind::TryOption {
                            operand: Box::new(operand),
                            option: DeclarationId::new(crate::prelude::OPTION_ID),
                            some_case: DeclarationId::new(crate::prelude::OPTION_SOME_ID),
                            some_field: DeclarationId::new(crate::prelude::OPTION_SOME_VALUE_ID),
                            none_case: DeclarationId::new(crate::prelude::OPTION_NONE_ID),
                            residual_type,
                        },
                        some_type.clone(),
                        OwnershipMode::Value,
                    ),
                    _ => {
                        return Err(self.error(
                            "SPX-H006",
                            "resolved `?` operand is not an ordinary Result or Option",
                            expr.span,
                        ));
                    }
                }
            }
            ExprKind::UpdateRecord { base, fields } => {
                let base = self.resolve_expr_recursive_reference(
                    function,
                    base,
                    bindings,
                    &format!("{path}.base"),
                )?;
                let ResolvedType::Nominal {
                    declaration: record,
                    arguments: _,
                } = &base.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve a record update on a non-record value",
                        expr.span,
                    ));
                };
                if self
                    .declarations
                    .declaration(record)
                    .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return Err(self.error(
                        "SPX-H001",
                        "cannot resolve a record update on a non-record value",
                        expr.span,
                    ));
                }
                let record = record.clone();
                let mut resolved_fields = Vec::with_capacity(fields.len());
                for (index, initializer) in fields.iter().enumerate() {
                    let field = self
                        .declarations
                        .field_id(&record, &initializer.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!(
                                    "unresolved replacement field `{}.{}`",
                                    record, initializer.name
                                ),
                                initializer.name_span,
                            )
                        })?;
                    let value = self.resolve_expr_recursive_reference(
                        function,
                        &initializer.value,
                        bindings,
                        &format!("{path}.field.{index}.value"),
                    )?;
                    resolved_fields.push(ResolvedFieldInitializer { field, value });
                }
                let ty = base.ty.clone();
                let ownership = self.expression_ownership(&ty, OwnershipMode::Own, expr.span)?;
                (
                    ResolvedExprKind::UpdateRecord {
                        base: Box::new(base),
                        record,
                        fields: resolved_fields,
                    },
                    ty,
                    ownership,
                )
            }
            // The test-only reference resolver never walks class-method
            // bodies; `super` resolution is owned by the iterative resolver.
            ExprKind::SuperMethod { method_span, .. } => {
                return Err(self.error(
                    "SPX-T231",
                    "`super` is only allowed inside a class-method override",
                    *method_span,
                ));
            }
            ExprKind::Project { base, field, .. } => {
                let base = self.resolve_expr_recursive_reference(
                    function,
                    base,
                    bindings,
                    &format!("{path}.base"),
                )?;
                let ResolvedType::Nominal {
                    declaration: record,
                    arguments,
                } = &base.ty
                else {
                    return Err(self.error(
                        "SPX-H001",
                        format!("cannot resolve field `{field}` on a non-record value"),
                        expr.span,
                    ));
                };
                if self
                    .declarations
                    .declaration(record)
                    .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return Err(self.error(
                        "SPX-H001",
                        format!("cannot resolve field `{field}` on a non-record value"),
                        expr.span,
                    ));
                }
                let instance_arguments = arguments.clone();
                let field_id = self
                    .declarations
                    .field_id(record, field)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("unresolved field `{field}` on record `{record}`"),
                            expr.span,
                        )
                    })?;
                let field_ty = self
                    .declarations
                    .record_fields(record)
                    .and_then(|fields| fields.iter().find(|item| item.id == field_id))
                    .map(|field| field.ty.clone())
                    .ok_or_else(|| {
                        self.error(
                            "SPX-H001",
                            format!("field `{field_id}` has no resolved type"),
                            expr.span,
                        )
                    })?;
                let field_ty = substitute_type(&field_ty, record, &instance_arguments)?;
                let ownership = self.expression_ownership(&field_ty, base.ownership, expr.span)?;
                let kind = match &base.kind {
                    ResolvedExprKind::Place(place) => {
                        let mut place = place.clone();
                        place
                            .projections
                            .push(PlaceProjection::Field(field_id.clone()));
                        ResolvedExprKind::Place(place)
                    }
                    _ => ResolvedExprKind::Project {
                        base: Box::new(base),
                        field: field_id,
                    },
                };
                (kind, field_ty, ownership)
            }
        };
        Ok(ResolvedExpr {
            id,
            ty,
            ownership,
            kind,
            span: expr.span,
        })
    }
}
