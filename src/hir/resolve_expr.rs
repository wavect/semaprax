//! The iterative expression resolver.
//!
//! One explicit frame stack lowers an AST expression tree into HIR
//! without recursing on the host stack.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, ExprKind, MatchPattern, Statement, TypeDeclarationKind, UnaryOp};
use crate::diagnostic::Diagnostic;

#[cfg(test)]
use super::capacity_probe::{note_iterative_phase_capacity, resolved_expr_owned_capacity};
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
#[cfg(test)]
use super::resolve_expr_frame::frame_owned_capacity;
use super::resolve_expr_frame::{take_results, Frame};
use super::{Binding, Place, PlaceProjection, Resolver};

impl Resolver<'_> {
    pub(super) fn resolve_expr_iterative(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        // Refutable Match v1 grew `ResolvedMatchPattern` (Literal/Or/
        // Binding), which grows this frame's arm-pattern payload.
        const { assert!(std::mem::size_of::<Frame<'static>>() == 592) };
        let mut frames = vec![Frame::Enter {
            expr,
            bindings: Rc::new(bindings.clone()),
            path: path.to_owned(),
        }];
        let mut results = Vec::new();

        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            {
                let mut seen_scopes = std::collections::HashSet::new();
                let frame_owned = frames.iter().fold(0_usize, |total, candidate| {
                    total.saturating_add(frame_owned_capacity(candidate, &mut seen_scopes))
                });
                let current_owned = frame_owned_capacity(&frame, &mut seen_scopes);
                note_iterative_phase_capacity(
                    0,
                    frames.capacity() * std::mem::size_of::<Frame<'_>>()
                        + results.capacity() * std::mem::size_of::<ResolvedExpr>()
                        + results
                            .iter()
                            .map(resolved_expr_owned_capacity)
                            .sum::<usize>()
                        + frame_owned
                        + current_owned,
                );
            }
            match frame {
                Frame::Enter {
                    expr,
                    bindings,
                    path,
                } => match &expr.kind {
                    ExprKind::Int(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::I64,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Int(*value),
                        span: expr.span,
                    }),
                    ExprKind::String(value) => {
                        let ty = ResolvedType::String;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ownership: self.expression_ownership(
                                &ty,
                                OwnershipMode::Own,
                                expr.span,
                            )?,
                            ty,
                            kind: ResolvedExprKind::String(value.clone()),
                            span: expr.span,
                        });
                    }
                    ExprKind::Int32(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::I32,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Int32(*value),
                        span: expr.span,
                    }),
                    ExprKind::Char(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::Char,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Char(*value),
                        span: expr.span,
                    }),
                    ExprKind::Uint8(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::U8,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Uint8(*value),
                        span: expr.span,
                    }),
                    ExprKind::Usize(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::Usize,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Usize(*value),
                        span: expr.span,
                    }),
                    ExprKind::ArrayU8(values) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::ArrayU8(values.len() as u32),
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::ArrayU8(values.clone()),
                        span: expr.span,
                    }),
                    ExprKind::RepeatArrayU8 { value, count } => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::ArrayU8(*count),
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::RepeatArrayU8 {
                            value: *value,
                            count: *count,
                        },
                        span: expr.span,
                    }),
                    ExprKind::Float32(bits) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::F32,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Float32(*bits),
                        span: expr.span,
                    }),
                    ExprKind::Float64(bits) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::F64,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Float64(*bits),
                        span: expr.span,
                    }),
                    ExprKind::Bool(value) => results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: ResolvedType::Bool,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Bool(*value),
                        span: expr.span,
                    }),
                    ExprKind::Var(name) => {
                        let binding = bindings.get(name).ok_or_else(|| {
                            self.error("SPX-H002", format!("unresolved value `{name}`"), expr.span)
                        })?;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty: binding.ty.clone(),
                            ownership: binding.ownership,
                            kind: ResolvedExprKind::Place(Place {
                                root: binding.id.clone(),
                                projections: Vec::new(),
                            }),
                            span: expr.span,
                        });
                    }
                    ExprKind::Call {
                        name,
                        type_arguments,
                        args,
                    } => {
                        if let Some(import_id) =
                            self.declarations.native_rust_import_id(name).cloned()
                        {
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
                            frames.push(Frame::FinishNativeCall {
                                span: expr.span,
                                path: path.clone(),
                                import: import_id,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "native-rust-arg",
                            });
                        } else if let Some(op) = crate::string_ops::by_name(name) {
                            // Compiler-owned string operations resolve to
                            // ordinary monomorphic calls carrying their
                            // reserved `core.string.*` identity; backends
                            // lower that identity intrinsically.
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
                            frames.push(Frame::FinishStringOp {
                                span: expr.span,
                                path: path.clone(),
                                op,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        } else if let Some(op) = crate::str_ops::by_name(name) {
                            if !type_arguments.is_empty() {
                                return Err(self.error(
                                    "SPX-H006",
                                    format!(
                                        "borrowed string operation `{name}` has type arguments"
                                    ),
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
                            frames.push(Frame::FinishStrOp {
                                span: expr.span,
                                path: path.clone(),
                                op,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        } else if let Some(op) = crate::byte_ops::by_name(name) {
                            if !type_arguments.is_empty() || args.len() != op.arity() {
                                return Err(self.error(
                                    "SPX-H006",
                                    format!("invalid byte operation `{name}` call shape"),
                                    expr.span,
                                ));
                            }
                            frames.push(Frame::FinishByteOp {
                                span: expr.span,
                                path: path.clone(),
                                op,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        } else if let Some(op) = crate::host_io_ops::by_name(name) {
                            if !type_arguments.is_empty() || args.len() != op.arity() {
                                return Err(self.error(
                                    "SPX-T269",
                                    format!("invalid host I/O operation `{name}` call shape"),
                                    expr.span,
                                ));
                            }
                            frames.push(Frame::FinishHostIoOp {
                                span: expr.span,
                                path: path.clone(),
                                op,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        } else if let Some(op) = crate::command_io_ops::by_name(name) {
                            if !type_arguments.is_empty()
                                || args.len() != crate::command_io_ops::arity(op)
                            {
                                return Err(self.error(
                                    "SPX-T270",
                                    format!("invalid command I/O operation `{name}` call shape"),
                                    expr.span,
                                ));
                            }
                            frames.push(Frame::FinishHostCommandOp {
                                span: expr.span,
                                path: path.clone(),
                                op,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        } else {
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
                                        format!(
                                            "function identity `{template}` has no declaration"
                                        ),
                                        expr.span,
                                    )
                                })?;
                            let resolved_arguments = type_arguments
                                .iter()
                                .map(|argument| self.resolve_type(argument, expr.span))
                                .collect::<Result<Vec<_>, _>>()?;
                            let (instance, return_source_type) = if target
                                .type_parameters
                                .is_empty()
                            {
                                if !resolved_arguments.is_empty() {
                                    return Err(self.error(
                                        "SPX-H006",
                                        format!(
                                            "monomorphic function `{template}` has type arguments"
                                        ),
                                        expr.span,
                                    ));
                                }
                                (None, target.return_type.clone())
                            } else {
                                if resolved_arguments.len() != target.type_parameters.len()
                                    || resolved_arguments.iter().any(|argument| {
                                        !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                                    })
                                {
                                    return Err(self.error(
                                            "SPX-H006",
                                            format!(
                                                "generic function `{template}` has invalid type arguments"
                                            ),
                                            expr.span,
                                        ));
                                }
                                let instance =
                                    FunctionInstanceId::derive(&template, &resolved_arguments);
                                let return_type = substitute_source_function_type(
                                        target,
                                        type_arguments,
                                        &target.return_type,
                                    )
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H006",
                                            format!(
                                                "generic function `{template}` return substitution failed"
                                            ),
                                            expr.span,
                                        )
                                    })?;
                                (Some(instance), return_type)
                            };
                            frames.push(Frame::FinishCall {
                                span: expr.span,
                                path: path.clone(),
                                callee: template,
                                type_arguments: resolved_arguments,
                                instance,
                                return_source_type,
                                target_span: target.span,
                                argument_count: args.len(),
                            });
                            frames.push(Frame::ChildNext {
                                children: args,
                                index: 0,
                                bindings,
                                path,
                                segment: "arg",
                            });
                        }
                    }
                    ExprKind::Unary { op, value } => {
                        frames.push(Frame::FinishUnary {
                            span: expr.span,
                            path: path.clone(),
                            op: *op,
                        });
                        frames.push(Frame::Enter {
                            expr: value,
                            bindings,
                            path: format!("{path}.value"),
                        });
                    }
                    ExprKind::Binary { op, left, right } => {
                        frames.push(Frame::AfterBinaryLeft {
                            span: expr.span,
                            path: path.clone(),
                            op: *op,
                            right,
                            bindings: bindings.clone(),
                        });
                        frames.push(Frame::Enter {
                            expr: left,
                            bindings,
                            path: format!("{path}.left"),
                        });
                    }
                    ExprKind::Block { statements, tail } => {
                        frames.push(Frame::BlockNext {
                            span: expr.span,
                            path,
                            statements,
                            tail,
                            index: 0,
                            scope: bindings,
                            resolved: Vec::with_capacity(statements.len()),
                        });
                    }
                    ExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        frames.push(Frame::AfterIfCondition {
                            span: expr.span,
                            path: path.clone(),
                            then_branch,
                            else_branch,
                            bindings: bindings.clone(),
                        });
                        frames.push(Frame::Enter {
                            expr: condition,
                            bindings,
                            path: format!("{path}.condition"),
                        });
                    }
                    ExprKind::ConstructRecord {
                        type_name,
                        type_arguments,
                        fields,
                        ..
                    } => {
                        let record =
                            self.declarations
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
                                format!(
                                    "constructor target `{type_name}` is not a record or class"
                                ),
                                expr.span,
                            ));
                        }
                        let arguments = type_arguments
                            .iter()
                            .map(|argument| self.resolve_type(argument, expr.span))
                            .collect::<Result<Vec<_>, _>>()?;
                        let parameters =
                            self.declarations.type_parameters(&record).ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("record `{record}` has no parameter metadata"),
                                    expr.span,
                                )
                            })?;
                        if arguments.len() != parameters.len()
                            || arguments.iter().any(|argument| {
                                !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                            })
                        {
                            return Err(self.error(
                                "SPX-H006",
                                format!("record `{record}` has invalid concrete arguments"),
                                expr.span,
                            ));
                        }
                        frames.push(Frame::RecordNext {
                            span: expr.span,
                            path,
                            type_name,
                            record,
                            arguments,
                            fields,
                            index: 0,
                            bindings,
                            resolved: Vec::with_capacity(fields.len()),
                        });
                    }
                    ExprKind::ConstructVariant {
                        type_name,
                        type_arguments,
                        case_name,
                        fields,
                        ..
                    } => {
                        let variant =
                            self.declarations
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
                        frames.push(Frame::VariantNext {
                            span: expr.span,
                            path,
                            type_name,
                            case_name,
                            variant,
                            case,
                            type_arguments,
                            fields,
                            index: 0,
                            bindings,
                            resolved: Vec::with_capacity(fields.len()),
                        });
                    }
                    ExprKind::Match {
                        mode,
                        scrutinee,
                        arms,
                    } => {
                        frames.push(Frame::AfterMatchScrutinee {
                            span: expr.span,
                            path: path.clone(),
                            mode: (*mode).into(),
                            arms,
                            bindings: bindings.clone(),
                        });
                        frames.push(Frame::Enter {
                            expr: scrutinee,
                            bindings,
                            path: format!("{path}.scrutinee"),
                        });
                    }
                    ExprKind::Try { operand } => {
                        frames.push(Frame::FinishTry {
                            span: expr.span,
                            path: path.clone(),
                        });
                        frames.push(Frame::Enter {
                            expr: operand,
                            bindings,
                            path: format!("{path}.operand"),
                        });
                    }
                    ExprKind::UpdateRecord { base, fields } => {
                        frames.push(Frame::AfterUpdateBase {
                            span: expr.span,
                            path: path.clone(),
                            fields,
                            bindings: bindings.clone(),
                        });
                        frames.push(Frame::Enter {
                            expr: base,
                            bindings,
                            path: format!("{path}.base"),
                        });
                    }
                    ExprKind::Project { base, field, .. } => {
                        frames.push(Frame::FinishProject {
                            span: expr.span,
                            path: path.clone(),
                            field,
                        });
                        frames.push(Frame::Enter {
                            expr: base,
                            bindings,
                            path: format!("{path}.base"),
                        });
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
                        let resolved_args = type_arguments
                            .iter()
                            .map(|a| self.resolve_type(a, expr.span))
                            .collect::<Result<Vec<_>, _>>()?;
                        frames.push(Frame::FinishMethodCall {
                            span: expr.span,
                            path: path.clone(),
                            method,
                            receiver,
                            bindings: bindings.clone(),
                            type_arguments: resolved_args,
                            args_len: args.len(),
                        });
                        if !args.is_empty() {
                            frames.push(Frame::MethodArgNext {
                                args,
                                index: 0,
                                bindings: bindings.clone(),
                                path: path.clone(),
                            });
                        }
                        // The receiver lowers to the call's first argument, so
                        // it carries the canonical `.arg.0` identity slot.
                        frames.push(Frame::Enter {
                            expr: receiver,
                            bindings,
                            path: format!("{path}.arg.0"),
                        });
                    }
                    ExprKind::SuperMethod {
                        method,
                        method_span,
                        args,
                    } => {
                        // `super` resolves against the enclosing class-method's
                        // owner; the enclosing method's own receiver becomes
                        // the callee's `self` argument.
                        let FunctionExecutionId::Monomorphic(template) = function else {
                            return Err(self.error(
                                "SPX-T231",
                                "`super` is only allowed inside a class-method override",
                                *method_span,
                            ));
                        };
                        let owner = self
                            .declarations
                            .declaration(template)
                            .and_then(|item| item.owner.clone())
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-T231",
                                    "`super` is only allowed inside a class-method override",
                                    *method_span,
                                )
                            })?;
                        let parent = self
                            .declarations
                            .class_parent(&owner)
                            .cloned()
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-T231",
                                    format!(
                                        "`super.{method}` requires a parent; the enclosing class has none"
                                    ),
                                    *method_span,
                                )
                            })?;
                        let (holder, callee) = self
                            .resolve_method_in_chain(&parent, method, *method_span)
                            .map_err(|_| {
                                self.error(
                                    "SPX-T231",
                                    format!("unresolved super method `{method}`"),
                                    *method_span,
                                )
                            })?;
                        frames.push(Frame::FinishSuperMethod {
                            span: expr.span,
                            method_span: *method_span,
                            path: path.clone(),
                            method,
                            holder: holder.clone(),
                            callee,
                            args_len: args.len(),
                        });
                        if !args.is_empty() {
                            frames.push(Frame::MethodArgNext {
                                args,
                                index: 0,
                                bindings: bindings.clone(),
                                path: path.clone(),
                            });
                        }
                        // The inherited receiver is the enclosing method's
                        // own `self` parameter. It is created here as the
                        // upcast source; the finish frame wraps it under the
                        // canonical `.arg.0` argument identity.
                        let owner_ty = ResolvedType::Nominal {
                            declaration: owner.clone(),
                            arguments: Vec::new(),
                        };
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &format!("{path}.arg.0.source")),
                            ty: owner_ty.clone(),
                            ownership: self.expression_ownership(
                                &owner_ty,
                                OwnershipMode::Own,
                                expr.span,
                            )?,
                            kind: ResolvedExprKind::Place(Place {
                                root: ValueId::parameter(function, 0),
                                projections: Vec::new(),
                            }),
                            span: expr.span,
                        });
                    }
                },
                Frame::FinishNativeCall {
                    span,
                    path,
                    import,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
                    let source_import = self
                        .program
                        .interfaces
                        .iter()
                        .flat_map(|interface| &interface.imports)
                        .find(|candidate| candidate.stable_id == import.as_str())
                        .expect("native Rust import identity remains indexed");
                    for (argument, parameter) in args.iter().zip(&source_import.params) {
                        if argument.ty != self.resolve_type(&parameter.ty, parameter.span)? {
                            return Err(self.error(
                                "SPX-B107",
                                "Native Rust Interop declaration set is unsupported: scalar value signature required",
                                argument.span,
                            ));
                        }
                    }
                    let result = match source_import.result {
                        crate::ast::ImportResult::Unit => ResolvedImportResultKind::Unit,
                        crate::ast::ImportResult::I64 => ResolvedImportResultKind::I64,
                        crate::ast::ImportResult::Bool => ResolvedImportResultKind::Bool,
                    };
                    let ty = match result {
                        ResolvedImportResultKind::Unit => ResolvedType::Unit,
                        ResolvedImportResultKind::I64 => ResolvedType::I64,
                        ResolvedImportResultKind::Bool => ResolvedType::Bool,
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::NativeRustImportCall(
                            ResolvedNativeRustImportCall {
                                expression: ExpressionId::new(function, &path),
                                import,
                                args,
                                result,
                            },
                        ),
                        span,
                    });
                }
                Frame::FinishCall {
                    span,
                    path,
                    callee,
                    type_arguments,
                    instance,
                    return_source_type,
                    target_span,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
                    let ty = self.resolve_type(&return_source_type, target_span)?;
                    let ownership =
                        self.expression_ownership(&ty, OwnershipMode::Own, target_span)?;
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            instance,
                            args,
                        },
                        span,
                    });
                }
                Frame::FinishStringOp {
                    span,
                    path,
                    op,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
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
                    let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span,
                    });
                }
                Frame::FinishStrOp {
                    span,
                    path,
                    op,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
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
                    let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span,
                    });
                }
                Frame::FinishByteOp {
                    span,
                    path,
                    op,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
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
                    let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
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
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership: OwnershipMode::Borrow,
                            kind: ResolvedExprKind::ByteRange {
                                operation: DeclarationId::new(crate::byte_ops::RANGE_ID),
                                source: Box::new(source),
                                start: Box::new(start),
                                end: Box::new(end),
                            },
                            span,
                        });
                        continue;
                    }
                    if op.is_view() {
                        let ResolvedExprKind::Place(place) = &args[0].kind else {
                            return Err(self.error(
                                "SPX-T266",
                                format!(
                                    "byte view `{}` requires an exact named storage root",
                                    op.name()
                                ),
                                args[0].span,
                            ));
                        };
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership: OwnershipMode::Borrow,
                            kind: ResolvedExprKind::BorrowPlace {
                                operation: DeclarationId::new(op.id()),
                                place: place.clone(),
                            },
                            span,
                        });
                        continue;
                    }
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span,
                    });
                }
                Frame::FinishHostIoOp {
                    span,
                    path,
                    op,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
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
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: op.return_type(),
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Call {
                            callee: DeclarationId::new(op.id()),
                            type_arguments: Vec::new(),
                            instance: None,
                            args,
                        },
                        span,
                    });
                }
                Frame::FinishHostCommandOp {
                    span,
                    path,
                    op,
                    argument_count,
                } => {
                    let args = take_results(&mut results, argument_count);
                    for (index, argument) in args.iter().enumerate() {
                        if !crate::command_io_ops::accepts_resolved(op, index, &argument.ty) {
                            return Err(self.error(
                                "SPX-T270",
                                format!(
                                    "command I/O operation `{}` argument {index} has the wrong type",
                                    crate::command_io_ops::name(op)
                                ),
                                argument.span,
                            ));
                        }
                    }
                    let expression = ExpressionId::new(function, &path);
                    results.push(ResolvedExpr {
                        id: expression.clone(),
                        ty: crate::command_io_ops::return_type(op),
                        ownership: crate::command_io_ops::result_ownership(op),
                        kind: ResolvedExprKind::HostCommandCall(ResolvedHostCommandCall {
                            expression,
                            operation: op,
                            args,
                        }),
                        span,
                    });
                }
                Frame::ChildNext {
                    children,
                    index,
                    bindings,
                    path,
                    segment,
                } => {
                    if index < children.len() {
                        frames.push(Frame::ChildNext {
                            children,
                            index: index + 1,
                            bindings: bindings.clone(),
                            path: path.clone(),
                            segment,
                        });
                        frames.push(Frame::Enter {
                            expr: &children[index],
                            bindings,
                            path: format!("{path}.{segment}.{index}"),
                        });
                    }
                }
                Frame::MethodArgNext {
                    args,
                    index,
                    bindings,
                    path,
                } => {
                    if index < args.len() {
                        frames.push(Frame::MethodArgNext {
                            args,
                            index: index + 1,
                            bindings: bindings.clone(),
                            path: path.clone(),
                        });
                        // Method arguments lower to call slots shifted by one
                        // so the receiver owns `.arg.0`.
                        frames.push(Frame::Enter {
                            expr: &args[index],
                            bindings,
                            path: format!("{path}.arg.{}", index + 1),
                        });
                    }
                }
                Frame::FinishUnary { span, path, op } => {
                    let value = results.pop().expect("unary child result retained");
                    // Negation keeps the numeric operand type; the validator
                    // and backends fail closed on any other shape.
                    let ty = match (&op, &value.ty) {
                        (UnaryOp::Neg, ResolvedType::F32) => ResolvedType::F32,
                        (UnaryOp::Neg, ResolvedType::F64) => ResolvedType::F64,
                        (UnaryOp::Neg, ResolvedType::I32) => ResolvedType::I32,
                        (UnaryOp::Neg, _) => ResolvedType::I64,
                        (UnaryOp::Not, _) => ResolvedType::Bool,
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Unary {
                            op,
                            value: Box::new(value),
                        },
                        span,
                    });
                }
                Frame::FinishBinary { span, path, op } => {
                    let mut children = take_results(&mut results, 2).into_iter();
                    let left = children.next().expect("binary left result retained");
                    let right = children.next().expect("binary right result retained");
                    // Arithmetic keeps the numeric operand type; the validator
                    // and backends reject mixed or float-remainder shapes.
                    let ty = match (&op, &left.ty) {
                        (
                            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div,
                            ResolvedType::I32,
                        ) => ResolvedType::I32,
                        (
                            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div,
                            ResolvedType::F32,
                        ) => ResolvedType::F32,
                        (
                            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div,
                            ResolvedType::F64,
                        ) => ResolvedType::F64,
                        (
                            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div,
                            ResolvedType::U8,
                        ) => ResolvedType::U8,
                        (
                            BinaryOp::Add
                            | BinaryOp::Sub
                            | BinaryOp::Mul
                            | BinaryOp::Div
                            | BinaryOp::Rem,
                            ResolvedType::Usize,
                        ) => ResolvedType::Usize,
                        (
                            BinaryOp::Add
                            | BinaryOp::Sub
                            | BinaryOp::Mul
                            | BinaryOp::Div
                            | BinaryOp::Rem,
                            _,
                        ) => ResolvedType::I64,
                        (
                            BinaryOp::Eq
                            | BinaryOp::Ne
                            | BinaryOp::Lt
                            | BinaryOp::Le
                            | BinaryOp::Gt
                            | BinaryOp::Ge
                            | BinaryOp::And
                            | BinaryOp::Or,
                            _,
                        ) => ResolvedType::Bool,
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership: OwnershipMode::Value,
                        kind: ResolvedExprKind::Binary {
                            op,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        span,
                    });
                }
                Frame::AfterBinaryLeft {
                    span,
                    path,
                    op,
                    right,
                    bindings,
                } => {
                    frames.push(Frame::FinishBinary {
                        span,
                        path: path.clone(),
                        op,
                    });
                    frames.push(Frame::Enter {
                        expr: right,
                        bindings,
                        path: format!("{path}.right"),
                    });
                }
                Frame::BlockNext {
                    span,
                    path,
                    statements,
                    tail,
                    index,
                    scope,
                    resolved,
                } => {
                    if index == statements.len() {
                        frames.push(Frame::FinishBlock {
                            span,
                            path: path.clone(),
                            statements: resolved,
                        });
                        frames.push(Frame::Enter {
                            expr: tail,
                            bindings: scope,
                            path: format!("{path}.tail"),
                        });
                    } else {
                        match &statements[index] {
                            Statement::Let { value, .. } => {
                                frames.push(Frame::BlockAfterLet {
                                    span,
                                    path: path.clone(),
                                    statements,
                                    tail,
                                    index,
                                    scope: scope.clone(),
                                    resolved,
                                });
                                frames.push(Frame::Enter {
                                    expr: value,
                                    bindings: scope,
                                    path: format!("{path}.s{index}.value"),
                                });
                            }
                            Statement::Assign {
                                name,
                                name_span,
                                field,
                                value,
                                ..
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
                                frames.push(Frame::BlockAfterAssign {
                                    span,
                                    path: path.clone(),
                                    statements,
                                    tail,
                                    index,
                                    scope: scope.clone(),
                                    resolved,
                                    target,
                                    target_field,
                                });
                                frames.push(Frame::Enter {
                                    expr: value,
                                    bindings: scope,
                                    path: format!("{path}.s{index}.value"),
                                });
                            }
                            Statement::Unsafe { body, .. } => {
                                // The body is an ordinary safe block; it
                                // resolves with the enclosing scope and its
                                // result is admitted (or rejected) when the
                                // boundary statement is assembled.
                                frames.push(Frame::BlockAfterUnsafe {
                                    span,
                                    path: path.clone(),
                                    statements,
                                    tail,
                                    index,
                                    scope: scope.clone(),
                                    resolved,
                                });
                                frames.push(Frame::Enter {
                                    expr: body,
                                    bindings: scope,
                                    path: format!("{path}.s{index}.body"),
                                });
                            }
                            Statement::While {
                                condition, body, ..
                            } => {
                                // Bounded While-Loops v1: admit only the
                                // Copy-scalar profile before resolving, so a
                                // loop can never introduce cleanup structure.
                                self.reject_while_disallowed(condition)?;
                                self.reject_while_disallowed(body)?;
                                frames.push(Frame::BlockWhileCondition {
                                    span,
                                    path: path.clone(),
                                    condition,
                                    body,
                                    statements,
                                    tail,
                                    index,
                                    scope: scope.clone(),
                                    resolved,
                                });
                                frames.push(Frame::Enter {
                                    expr: condition,
                                    bindings: scope,
                                    path: format!("{path}.s{index}.condition"),
                                });
                            }
                        }
                    }
                }
                Frame::BlockAfterLet {
                    span,
                    path,
                    statements,
                    tail,
                    index,
                    mut scope,
                    mut resolved,
                } => {
                    let value = results.pop().expect("let value result retained");
                    let Statement::Let {
                        name,
                        name_span,
                        mutable,
                        declared,
                        span: statement_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("let frame resumes at a let statement")
                    };
                    let statement_path = format!("{path}.s{index}");
                    // Class Inheritance v1: an explicit declared type accepts
                    // either the value's exact type or an ancestor class; a
                    // descendant value is consumed through a prefix upcast
                    // whose source re-resolves at the canonical `.source`
                    // identity below the binding slot.
                    if let Some(declared_ast) = declared {
                        let declared_ty = self.resolve_type(declared_ast, *name_span)?;
                        if value.ty != declared_ty {
                            let ResolvedType::Nominal {
                                declaration: child_id,
                                ..
                            } = &value.ty
                            else {
                                return Err(self.error(
                                    "SPX-T232",
                                    format!(
                                        "declared binding type `{}` does not accept value type `{}`",
                                        declared_ty.identity_key(),
                                        value.ty.identity_key()
                                    ),
                                    *name_span,
                                ));
                            };
                            let ResolvedType::Nominal {
                                declaration: parent_id,
                                ..
                            } = &declared_ty
                            else {
                                return Err(self.error(
                                    "SPX-T232",
                                    format!(
                                        "declared binding type `{}` does not accept value type `{}`",
                                        declared_ty.identity_key(),
                                        value.ty.identity_key()
                                    ),
                                    *name_span,
                                ));
                            };
                            self.check_upcast_admissible(child_id, parent_id, *name_span)?;
                            let slot_path = format!("{statement_path}.value");
                            frames.push(Frame::StartUpcast {
                                source: match &statements[index] {
                                    Statement::Let { value, .. } => value,
                                    _ => unreachable!("let frame resumes at a let statement"),
                                },
                                bindings: scope.clone(),
                                slot_path,
                                holder: parent_id.clone(),
                                span: value.span,
                                resume: Box::new(Frame::BlockAfterLet {
                                    span,
                                    path,
                                    statements,
                                    tail,
                                    index,
                                    scope,
                                    resolved,
                                }),
                            });
                            continue;
                        }
                    }
                    let binding = ResolvedBinding {
                        id: ValueId::local(function, &statement_path),
                        name: name.clone(),
                        ownership: value.ownership,
                        ty: value.ty.clone(),
                        span: *name_span,
                    };
                    Rc::make_mut(&mut scope).insert(
                        name.clone(),
                        Binding {
                            id: binding.id.clone(),
                            ty: binding.ty.clone(),
                            ownership: binding.ownership,
                            mutable: *mutable,
                        },
                    );
                    resolved.push(ResolvedStatement::Let {
                        binding,
                        mutable: *mutable,
                        value,
                        span: *statement_span,
                    });
                    frames.push(Frame::BlockNext {
                        span,
                        path,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        resolved,
                    });
                }
                Frame::BlockAfterAssign {
                    span,
                    path,
                    statements,
                    tail,
                    index,
                    scope,
                    mut resolved,
                    target,
                    target_field,
                } => {
                    // The assigned value is fully evaluated before the store;
                    // exact-type and scalar-Copy admission are checked here so
                    // failure statuses propagate exactly like initializers.
                    let value = results.pop().expect("assign value result retained");
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
                    let Statement::Assign {
                        span: statement_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("assign frame resumes at an assignment statement")
                    };
                    resolved.push(ResolvedStatement::Assign {
                        binding: target,
                        field: target_field.map(|(field_id, _)| field_id),
                        value,
                        span: *statement_span,
                    });
                    frames.push(Frame::BlockNext {
                        span,
                        path,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        resolved,
                    });
                }
                Frame::BlockAfterUnsafe {
                    span,
                    path,
                    statements,
                    tail,
                    index,
                    scope,
                    mut resolved,
                } => {
                    // The body block resolved like any ordinary nested block.
                    // Boundary admission mirrors the mutation checks: the
                    // discarded body result must be a scalar Copy value so no
                    // cleanup or ownership semantics are introduced.
                    let body = results.pop().expect("unsafe body result retained");
                    if body.ownership != OwnershipMode::Value || !is_scalar_resolved_type(&body.ty)
                    {
                        return Err(self.error(
                            "SPX-N104",
                            "unsafe boundary bodies must produce a scalar Copy value",
                            body.span,
                        ));
                    }
                    let Statement::Unsafe {
                        audit,
                        span: statement_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("unsafe frame resumes at an unsafe statement")
                    };
                    resolved.push(ResolvedStatement::Unsafe {
                        audit: audit.clone(),
                        body: Box::new(body),
                        span: *statement_span,
                    });
                    frames.push(Frame::BlockNext {
                        span,
                        path,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        resolved,
                    });
                }
                Frame::BlockWhileCondition {
                    span,
                    path,
                    condition,
                    body,
                    statements,
                    tail,
                    index,
                    scope,
                    resolved,
                } => {
                    // The condition is re-evaluated before every iteration and
                    // must be exactly `bool`.
                    let evaluated = results.pop().expect("while condition result retained");
                    if evaluated.ty != ResolvedType::Bool {
                        return Err(self.error(
                            "SPX-T251",
                            "`while` condition must be bool",
                            condition.span,
                        ));
                    }
                    frames.push(Frame::BlockWhileBody {
                        span,
                        path: path.clone(),
                        statements,
                        tail,
                        index,
                        scope: scope.clone(),
                        resolved,
                        condition: Box::new(evaluated),
                        condition_span: condition.span,
                    });
                    frames.push(Frame::Enter {
                        expr: body,
                        bindings: scope.clone(),
                        path: format!("{path}.s{index}.body"),
                    });
                }
                Frame::BlockWhileBody {
                    span,
                    path,
                    statements,
                    tail,
                    index,
                    scope,
                    mut resolved,
                    condition,
                    condition_span,
                } => {
                    // The body block resolved like any ordinary nested block;
                    // its value is discarded by the statement.
                    let body = results.pop().expect("while body result retained");
                    let Statement::While {
                        span: statement_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("while frame resumes at a while statement")
                    };
                    resolved.push(ResolvedStatement::While {
                        condition,
                        body: Box::new(body),
                        span: condition_span.merge(*statement_span),
                    });
                    frames.push(Frame::BlockNext {
                        span,
                        path,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        resolved,
                    });
                }
                Frame::FinishBlock {
                    span,
                    path,
                    statements,
                } => {
                    let tail = results.pop().expect("block tail result retained");
                    let ty = tail.ty.clone();
                    let ownership = tail.ownership;
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::Block {
                            statements,
                            tail: Box::new(tail),
                        },
                        span,
                    });
                }
                Frame::FinishIf { span, path } => {
                    let mut children = take_results(&mut results, 3).into_iter();
                    let condition = children.next().expect("if condition retained");
                    let then_branch = children.next().expect("if then branch retained");
                    let else_branch = children.next().expect("if else branch retained");
                    let ty = then_branch.ty.clone();
                    let ownership = then_branch.ownership;
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership,
                        kind: ResolvedExprKind::If {
                            condition: Box::new(condition),
                            then_branch: Box::new(then_branch),
                            else_branch: Box::new(else_branch),
                        },
                        span,
                    });
                }
                Frame::AfterIfCondition {
                    span,
                    path,
                    then_branch,
                    else_branch,
                    bindings,
                } => {
                    frames.push(Frame::AfterIfThen {
                        span,
                        path: path.clone(),
                        else_branch,
                        bindings: bindings.clone(),
                    });
                    frames.push(Frame::Enter {
                        expr: then_branch,
                        bindings,
                        path: format!("{path}.then"),
                    });
                }
                Frame::AfterIfThen {
                    span,
                    path,
                    else_branch,
                    bindings,
                } => {
                    frames.push(Frame::FinishIf {
                        span,
                        path: path.clone(),
                    });
                    frames.push(Frame::Enter {
                        expr: else_branch,
                        bindings,
                        path: format!("{path}.else"),
                    });
                }
                Frame::RecordNext {
                    span,
                    path,
                    type_name,
                    record,
                    arguments,
                    fields,
                    index,
                    bindings,
                    resolved,
                } => {
                    if index == fields.len() {
                        let ty = ResolvedType::Nominal {
                            declaration: record.clone(),
                            arguments,
                        };
                        let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership,
                            kind: ResolvedExprKind::ConstructRecord {
                                record,
                                fields: resolved,
                            },
                            span,
                        });
                    } else {
                        let initializer = &fields[index];
                        let field = self
                            .declarations
                            .field_id(&record, &initializer.name)
                            .cloned()
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H001",
                                    format!(
                                        "unresolved field `{}.{}`",
                                        type_name, initializer.name
                                    ),
                                    initializer.name_span,
                                )
                            })?;
                        frames.push(Frame::RecordAfterField {
                            span,
                            path: path.clone(),
                            type_name,
                            record,
                            arguments,
                            fields,
                            index,
                            bindings: bindings.clone(),
                            resolved,
                            field,
                        });
                        frames.push(Frame::Enter {
                            expr: &initializer.value,
                            bindings,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                Frame::RecordAfterField {
                    span,
                    path,
                    type_name,
                    record,
                    arguments,
                    fields,
                    index,
                    bindings,
                    mut resolved,
                    field,
                } => {
                    let value = results.pop().expect("record field result retained");
                    resolved.push(ResolvedFieldInitializer { field, value });
                    frames.push(Frame::RecordNext {
                        span,
                        path,
                        type_name,
                        record,
                        arguments,
                        fields,
                        index: index + 1,
                        bindings,
                        resolved,
                    });
                }
                Frame::VariantNext {
                    span,
                    path,
                    type_name,
                    case_name,
                    variant,
                    case,
                    type_arguments,
                    fields,
                    index,
                    bindings,
                    resolved,
                } => {
                    if index == fields.len() {
                        let arguments = type_arguments
                            .iter()
                            .map(|argument| self.resolve_type(argument, span))
                            .collect::<Result<Vec<_>, _>>()?;
                        let ty = ResolvedType::Nominal {
                            declaration: variant.clone(),
                            arguments,
                        };
                        let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership,
                            kind: ResolvedExprKind::ConstructVariant {
                                variant,
                                case,
                                fields: resolved,
                            },
                            span,
                        });
                    } else {
                        let initializer = &fields[index];
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
                        frames.push(Frame::VariantAfterField {
                            span,
                            path: path.clone(),
                            type_name,
                            case_name,
                            variant,
                            case,
                            type_arguments,
                            fields,
                            index,
                            bindings: bindings.clone(),
                            resolved,
                            field,
                        });
                        frames.push(Frame::Enter {
                            expr: &initializer.value,
                            bindings,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                Frame::VariantAfterField {
                    span,
                    path,
                    type_name,
                    case_name,
                    variant,
                    case,
                    type_arguments,
                    fields,
                    index,
                    bindings,
                    mut resolved,
                    field,
                } => {
                    let value = results.pop().expect("variant field result retained");
                    resolved.push(ResolvedFieldInitializer { field, value });
                    frames.push(Frame::VariantNext {
                        span,
                        path,
                        type_name,
                        case_name,
                        variant,
                        case,
                        type_arguments,
                        fields,
                        index: index + 1,
                        bindings,
                        resolved,
                    });
                }
                Frame::AfterMatchScrutinee {
                    span,
                    path,
                    mode,
                    arms,
                    bindings,
                } => {
                    let scrutinee = results.pop().expect("match scrutinee retained");
                    // Refutable Match v1: Copy-scalar scrutinees take the
                    // literal/guard decision chain; every aggregate or
                    // non-scalar type keeps the exact pre-feature surface.
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
                                span,
                            ));
                        }
                        if arms.is_empty() {
                            return Err(self.error("SPX-H006", "resolved match has no arms", span));
                        }
                        self.validate_refutable_match_admission(&scrutinee.ty, arms)?;
                        frames.push(Frame::ScalarMatchNext {
                            span,
                            path,
                            mode,
                            arms,
                            index: 0,
                            bindings,
                            scrutinee,
                            resolved: Vec::with_capacity(arms.len()),
                        });
                        continue;
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
                            span,
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
                            span,
                        ));
                    };
                    let matched_kind = self
                        .declarations
                        .declaration(matched_type)
                        .map(|item| item.kind)
                        .filter(|kind| {
                            matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
                        })
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                "cannot resolve match on a non-record/non-variant value",
                                span,
                            )
                        })?;
                    let facts = self.declarations.type_facts(&scrutinee.ty).ok_or_else(|| {
                        self.error("SPX-H006", "match scrutinee has no type facts", span)
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
                                span,
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
                                span,
                            ));
                        }
                        _ => unreachable!("matched kind was restricted above"),
                    }
                    let matched_type = matched_type.clone();
                    let instance_arguments = arguments.clone();
                    frames.push(Frame::MatchNext {
                        span,
                        path,
                        mode,
                        arms,
                        index: 0,
                        bindings,
                        scrutinee,
                        matched_type,
                        instance_arguments,
                        matched_kind,
                        resolved: Vec::with_capacity(arms.len()),
                    });
                }
                Frame::MatchNext {
                    span,
                    path,
                    mode,
                    arms,
                    index,
                    bindings,
                    scrutinee,
                    matched_type,
                    instance_arguments,
                    matched_kind,
                    resolved,
                } => {
                    if index == arms.len() {
                        let first = resolved.first().ok_or_else(|| {
                            self.error("SPX-H006", "resolved match has no arms", span)
                        })?;
                        let ty = first.value.ty.clone();
                        let ownership = first.value.ownership;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership,
                            kind: ResolvedExprKind::Match {
                                mode,
                                scrutinee: Box::new(scrutinee),
                                arms: resolved,
                            },
                            span,
                        });
                    } else {
                        let arm = &arms[index];
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
                                            format!(
                                                "unresolved case `{matched_type}::{case_name}`"
                                            ),
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
                                        .and_then(|items| {
                                            items.iter().find(|item| item.id == field_id)
                                        })
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
                                    let field_facts = self
                                        .declarations
                                        .type_facts(&field_ty)
                                        .ok_or_else(|| {
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
                                            &format!("{path}.arm.{index}.binding.{field_index}"),
                                        ),
                                        name: field.binding.clone(),
                                        ownership,
                                        ty: field_ty.clone(),
                                        span: field.binding_span,
                                    };
                                    Rc::make_mut(&mut arm_bindings).insert(
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
                                span: pattern_span,
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
                                    Rc::make_mut(&mut arm_bindings),
                                    &format!("{path}.arm.{index}.record"),
                                    *pattern_span,
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
                        frames.push(Frame::MatchAfterArm {
                            span,
                            path: path.clone(),
                            mode,
                            arms,
                            index,
                            bindings,
                            scrutinee,
                            matched_type,
                            instance_arguments,
                            matched_kind,
                            resolved,
                            pattern,
                        });
                        frames.push(Frame::Enter {
                            expr: &arm.value,
                            bindings: arm_bindings,
                            path: format!("{path}.arm.{index}.value"),
                        });
                    }
                }
                Frame::MatchAfterArm {
                    span,
                    path,
                    mode,
                    arms,
                    index,
                    bindings,
                    scrutinee,
                    matched_type,
                    instance_arguments,
                    matched_kind,
                    mut resolved,
                    pattern,
                } => {
                    let value = results.pop().expect("match arm value retained");
                    // Aggregate matches reject guards with SPX-T254 before
                    // any arm resolves, so pre-feature arms carry no guard.
                    resolved.push(ResolvedMatchArm {
                        pattern,
                        guard: None,
                        value,
                        span: arms[index].span,
                    });
                    frames.push(Frame::MatchNext {
                        span,
                        path,
                        mode,
                        arms,
                        index: index + 1,
                        bindings,
                        scrutinee,
                        matched_type,
                        instance_arguments,
                        matched_kind,
                        resolved,
                    });
                }
                Frame::ScalarMatchNext {
                    span,
                    path,
                    mode,
                    arms,
                    index,
                    bindings,
                    scrutinee,
                    resolved,
                } => {
                    if index == arms.len() {
                        // SPX-T257 already guaranteed a trailing catch-all
                        // during admission, so at least one arm exists and
                        // all arm values unified to one type.
                        let ty = resolved[0].value.ty.clone();
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership: resolved[0].value.ownership,
                            kind: ResolvedExprKind::Match {
                                mode,
                                scrutinee: Box::new(scrutinee),
                                arms: resolved,
                            },
                            span,
                        });
                    } else {
                        let arm = &arms[index];
                        let mut arm_bindings = bindings.clone();
                        let pattern = match &arm.pattern {
                            crate::ast::MatchPattern::Wildcard { .. } => {
                                ResolvedMatchPattern::Wildcard
                            }
                            crate::ast::MatchPattern::Binding { name, .. } => {
                                let binding = ResolvedBinding {
                                    id: ValueId::local(
                                        function,
                                        &format!("{path}.arm.{index}.binding"),
                                    ),
                                    name: name.clone(),
                                    ownership: OwnershipMode::Value,
                                    ty: scrutinee.ty.clone(),
                                    span: arm.pattern.span(),
                                };
                                Rc::make_mut(&mut arm_bindings).insert(
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
                            crate::ast::MatchPattern::Literal { value, .. } => {
                                ResolvedMatchPattern::Literal(PatternValue::from_ast(*value))
                            }
                            crate::ast::MatchPattern::Or { alternatives, .. } => {
                                ResolvedMatchPattern::Or(
                                    alternatives
                                        .iter()
                                        .map(|alternative| match alternative {
                                            crate::ast::MatchPattern::Literal { value, .. } => {
                                                ResolvedMatchPattern::Literal(
                                                    PatternValue::from_ast(*value),
                                                )
                                            }
                                            // SPX-M105 rejected non-literal
                                            // alternatives during admission.
                                            _ => {
                                                unreachable!("or-pattern alternatives are literals")
                                            }
                                        })
                                        .collect(),
                                )
                            }
                            // Aggregate patterns on scalar scrutinees were
                            // rejected during admission.
                            crate::ast::MatchPattern::Variant { .. }
                            | crate::ast::MatchPattern::Record { .. } => {
                                return Err(self.error(
                                    "SPX-H001",
                                    "aggregate pattern has a Copy-scalar scrutinee",
                                    arm.span,
                                ));
                            }
                        };
                        frames.push(Frame::ScalarMatchAfterArm {
                            span,
                            path: path.clone(),
                            mode,
                            arms,
                            index,
                            bindings,
                            scrutinee,
                            resolved,
                            pattern,
                        });
                        if let Some(guard) = &arm.guard {
                            frames.push(Frame::Enter {
                                expr: guard.as_ref(),
                                bindings: arm_bindings.clone(),
                                path: format!("{path}.arm.{index}.guard"),
                            });
                        }
                        frames.push(Frame::Enter {
                            expr: &arm.value,
                            bindings: arm_bindings,
                            path: format!("{path}.arm.{index}.value"),
                        });
                    }
                }
                Frame::ScalarMatchAfterArm {
                    span,
                    path,
                    mode,
                    arms,
                    index,
                    bindings,
                    scrutinee,
                    mut resolved,
                    pattern,
                } => {
                    // The guard's Enter resolved after the value's, so the
                    // guard's result sits on top of the results stack.
                    let guard = arms[index]
                        .guard
                        .is_some()
                        .then(|| Box::new(results.pop().expect("scalar match arm guard retained")));
                    let value = results.pop().expect("scalar match arm value retained");
                    if let Some(guard) = &guard {
                        if guard.ty != ResolvedType::Bool {
                            return Err(self.error(
                                "SPX-T256",
                                format!(
                                    "match guard must be bool; received {}",
                                    guard.ty.identity_key()
                                ),
                                arms[index].span,
                            ));
                        }
                    }
                    if let Some(first) = resolved.first() {
                        if first.value.ty != value.ty || first.value.ownership != value.ownership {
                            return Err(self.error(
                                "SPX-T259",
                                format!(
                                    "match arms disagree on the result type; expected {}",
                                    first.value.ty.identity_key()
                                ),
                                arms[index].span,
                            ));
                        }
                    }
                    resolved.push(ResolvedMatchArm {
                        pattern,
                        guard,
                        value,
                        span: arms[index].span,
                    });
                    frames.push(Frame::ScalarMatchNext {
                        span,
                        path,
                        mode,
                        arms,
                        index: index + 1,
                        bindings,
                        scrutinee,
                        resolved,
                    });
                }
                Frame::FinishTry { span, path } => {
                    let operand = results.pop().expect("try operand retained");
                    let operand_type = operand.ty.clone();
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = &operand_type
                    else {
                        return Err(self.error(
                            "SPX-H006",
                            "resolved `?` operand is not the ordinary Result",
                            span,
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
                                span,
                            )
                        })?;
                    let residual_type = self.resolve_type(&target.return_type, target.span)?;
                    let (kind, ty) = match (declaration.as_str(), arguments.as_slice()) {
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
                        ),
                        (crate::prelude::OPTION_ID, [some_type]) => (
                            ResolvedExprKind::TryOption {
                                operand: Box::new(operand),
                                option: DeclarationId::new(crate::prelude::OPTION_ID),
                                some_case: DeclarationId::new(crate::prelude::OPTION_SOME_ID),
                                some_field: DeclarationId::new(
                                    crate::prelude::OPTION_SOME_VALUE_ID,
                                ),
                                none_case: DeclarationId::new(crate::prelude::OPTION_NONE_ID),
                                residual_type,
                            },
                            some_type.clone(),
                        ),
                        _ => {
                            return Err(self.error(
                                "SPX-H006",
                                "resolved `?` operand is not an ordinary Result or Option",
                                span,
                            ));
                        }
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty,
                        ownership: OwnershipMode::Value,
                        kind,
                        span,
                    });
                }
                Frame::AfterUpdateBase {
                    span,
                    path,
                    fields,
                    bindings,
                } => {
                    let base = results.pop().expect("record update base retained");
                    let ResolvedType::Nominal {
                        declaration: record,
                        ..
                    } = &base.ty
                    else {
                        return Err(self.error(
                            "SPX-H001",
                            "cannot resolve a record update on a non-record value",
                            span,
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
                            span,
                        ));
                    }
                    let record = record.clone();
                    frames.push(Frame::UpdateNext {
                        span,
                        path,
                        base,
                        record,
                        fields,
                        index: 0,
                        bindings,
                        resolved: Vec::with_capacity(fields.len()),
                    });
                }
                Frame::UpdateNext {
                    span,
                    path,
                    base,
                    record,
                    fields,
                    index,
                    bindings,
                    resolved,
                } => {
                    if index == fields.len() {
                        let ty = base.ty.clone();
                        let ownership = self.expression_ownership(&ty, OwnershipMode::Own, span)?;
                        results.push(ResolvedExpr {
                            id: ExpressionId::new(function, &path),
                            ty,
                            ownership,
                            kind: ResolvedExprKind::UpdateRecord {
                                base: Box::new(base),
                                record,
                                fields: resolved,
                            },
                            span,
                        });
                    } else {
                        let initializer = &fields[index];
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
                        frames.push(Frame::UpdateAfterField {
                            span,
                            path: path.clone(),
                            base,
                            record,
                            fields,
                            index,
                            bindings: bindings.clone(),
                            resolved,
                            field,
                        });
                        frames.push(Frame::Enter {
                            expr: &initializer.value,
                            bindings,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                Frame::UpdateAfterField {
                    span,
                    path,
                    base,
                    record,
                    fields,
                    index,
                    bindings,
                    mut resolved,
                    field,
                } => {
                    let value = results.pop().expect("record replacement result retained");
                    resolved.push(ResolvedFieldInitializer { field, value });
                    frames.push(Frame::UpdateNext {
                        span,
                        path,
                        base,
                        record,
                        fields,
                        index: index + 1,
                        bindings,
                        resolved,
                    });
                }
                Frame::FinishProject { span, path, field } => {
                    let base = results.pop().expect("projection base retained");
                    let ResolvedType::Nominal {
                        declaration: owner,
                        arguments,
                    } = &base.ty
                    else {
                        return Err(self.error(
                            "SPX-H001",
                            format!("cannot resolve field `{field}` on a non-record value"),
                            span,
                        ));
                    };
                    if self.declarations.declaration(owner).is_none_or(|item| {
                        !matches!(item.kind, DeclarationKind::Record | DeclarationKind::Class)
                    }) {
                        return Err(self.error(
                            "SPX-H001",
                            format!("cannot resolve field `{field}` on a non-record value"),
                            span,
                        ));
                    }
                    let field_id = self
                        .declarations
                        .field_id(owner, field)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("unresolved field `{field}` on `{owner}`"),
                                span,
                            )
                        })?;
                    let field_ty = self
                        .declarations
                        .record_fields(owner)
                        .and_then(|fields| fields.iter().find(|item| item.id == field_id))
                        .map(|item| item.ty.clone())
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("field `{field_id}` has no resolved type"),
                                span,
                            )
                        })?;
                    let field_ty = substitute_type(&field_ty, owner, arguments)?;
                    let ownership = self.expression_ownership(&field_ty, base.ownership, span)?;
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
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: field_ty,
                        ownership,
                        kind,
                        span,
                    });
                }
                Frame::FinishMethodCall {
                    span,
                    path,
                    method,
                    receiver: receiver_ast,
                    bindings,
                    type_arguments,
                    args_len,
                } => {
                    if !type_arguments.is_empty() {
                        return Err(self.error(
                            "SPX-P106",
                            "method generic arguments are not supported in this slice",
                            span,
                        ));
                    }
                    let mut all = take_results(&mut results, args_len + 1);
                    let receiver = all.remove(0);
                    let args = all;
                    let receiver_class = match &receiver.ty {
                        ResolvedType::Nominal {
                            declaration: class_id,
                            arguments,
                        } => Some((class_id.clone(), arguments.clone())),
                        _ => None,
                    };
                    let Some((class_id, class_args)) = receiver_class else {
                        return Err(self.error(
                            "SPX-H001",
                            format!("cannot resolve method `{method}` on a non-class value"),
                            span,
                        ));
                    };
                    let class_decl = self.declarations.declaration(&class_id).ok_or_else(|| {
                        self.error("SPX-H001", format!("unknown class `{class_id}`"), span)
                    })?;
                    if class_decl.kind != DeclarationKind::Class {
                        return Err(self.error(
                            "SPX-H001",
                            format!(
                                "method `{method}` requires a class receiver, found `{class_id}`"
                            ),
                            span,
                        ));
                    }
                    // Class Inheritance v1: method resolution walks the
                    // declared receiver's ancestor chain nearest-first, so an
                    // override replaces the inherited symbol for receivers of
                    // its own class while unoverridden parents stay callable.
                    let (holder, method_id) =
                        self.resolve_method_in_chain(&class_id, method, span)?;
                    // An inherited receiver is consumed through a prefix
                    // upcast: re-enter the receiver expression at the
                    // canonical `.source` identity and wrap its result before
                    // the method-call continuation resumes.
                    if holder != class_id {
                        self.check_upcast_admissible(&class_id, &holder, receiver.span)?;
                        frames.push(Frame::StartUpcast {
                            source: receiver_ast,
                            bindings: bindings.clone(),
                            slot_path: format!("{path}.arg.0"),
                            holder: holder.clone(),
                            span: receiver.span,
                            resume: Box::new(Frame::FinishMethodCall {
                                span,
                                path,
                                method,
                                receiver: receiver_ast,
                                bindings,
                                type_arguments,
                                args_len,
                            }),
                        });
                        continue;
                    }
                    let holder_ast = self
                        .program
                        .types
                        .iter()
                        .find(|t| t.stable_id == holder.as_str())
                        .ok_or_else(|| {
                            self.error("SPX-H006", format!("class `{holder}` has no AST"), span)
                        })?;
                    let TypeDeclarationKind::Class { methods, .. } = &holder_ast.kind else {
                        return Err(self.error(
                            "SPX-H006",
                            format!("`{holder}` is not a class"),
                            span,
                        ));
                    };
                    let method_ast =
                        methods.iter().find(|m| m.name == *method).ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("unresolved method `{method}` on class `{holder}`"),
                                span,
                            )
                        })?;
                    if method_ast.params.is_empty() {
                        return Err(self.error(
                            "SPX-H001",
                            format!("method `{method}` has no self parameter"),
                            method_ast.span,
                        ));
                    }
                    let self_param = &method_ast.params[0];
                    let self_ty = self.resolve_type(&self_param.ty, self_param.span)?;
                    let expected_self = ResolvedType::Nominal {
                        declaration: holder.clone(),
                        arguments: class_args.clone(),
                    };
                    if self_ty != expected_self {
                        return Err(self.error(
                            "SPX-H001",
                            format!(
                                "method `{method}` self parameter type `{:?}` does not match class `{holder}`",
                                self_ty
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
                            span,
                        ));
                    }
                    for (arg, param) in args.iter().zip(method_ast.params.iter().skip(1)) {
                        let param_ty = self.resolve_type(&param.ty, param.span)?;
                        if arg.ty != param_ty {
                            return Err(self.error(
                                "SPX-H001",
                                format!(
                                    "method `{method}` argument `{}` expects type `{}`, found `{}`",
                                    param.name,
                                    param_ty.identity_key(),
                                    arg.ty.identity_key()
                                ),
                                arg.span,
                            ));
                        }
                    }
                    let return_ty = self.resolve_type(&method_ast.return_type, method_ast.span)?;
                    let ownership =
                        self.expression_ownership(&return_ty, OwnershipMode::Own, span)?;
                    let callee = method_id;
                    let mut call_args = Vec::with_capacity(1 + args.len());
                    call_args.push(receiver);
                    call_args.extend(args);
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: return_ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee,
                            type_arguments: Vec::new(),
                            instance: None,
                            args: call_args,
                        },
                        span,
                    });
                }
                Frame::FinishSuperMethod {
                    span,
                    method_span,
                    path,
                    method,
                    holder,
                    callee,
                    args_len,
                } => {
                    let mut all = take_results(&mut results, args_len + 1);
                    let receiver = all.remove(0);
                    let args = all;
                    // The inherited receiver is the enclosing override's own
                    // `self`, upcast to the declaring ancestor exactly like a
                    // declared-type binding. The source place is synthesized
                    // here, so both identities are canonical by construction.
                    let ResolvedType::Nominal {
                        declaration: self_class,
                        ..
                    } = &receiver.ty
                    else {
                        return Err(self.error(
                            "SPX-H006",
                            "super receiver is not a class value",
                            method_span,
                        ));
                    };
                    let receiver = if *self_class == holder {
                        receiver
                    } else {
                        self.check_upcast_admissible(self_class, &holder, method_span)?;
                        let source = ResolvedExpr {
                            id: ExpressionId::new(function, &format!("{path}.arg.0.source")),
                            ty: receiver.ty.clone(),
                            ownership: receiver.ownership,
                            kind: receiver.kind,
                            span: receiver.span,
                        };
                        ResolvedExpr {
                            id: ExpressionId::new(function, &format!("{path}.arg.0")),
                            ty: ResolvedType::Nominal {
                                declaration: holder.clone(),
                                arguments: Vec::new(),
                            },
                            ownership: self.expression_ownership(
                                &ResolvedType::Nominal {
                                    declaration: holder.clone(),
                                    arguments: Vec::new(),
                                },
                                OwnershipMode::Own,
                                span,
                            )?,
                            kind: ResolvedExprKind::Upcast {
                                source: Box::new(source),
                            },
                            span: receiver.span,
                        }
                    };
                    let holder_ast = self
                        .program
                        .types
                        .iter()
                        .find(|t| t.stable_id == holder.as_str())
                        .ok_or_else(|| {
                            self.error("SPX-H006", format!("class `{holder}` has no AST"), span)
                        })?;
                    let TypeDeclarationKind::Class { methods, .. } = &holder_ast.kind else {
                        return Err(self.error(
                            "SPX-H006",
                            format!("`{holder}` is not a class"),
                            span,
                        ));
                    };
                    let method_ast =
                        methods.iter().find(|m| m.name == *method).ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!("unresolved super method `{method}` on `{holder}`"),
                                method_span,
                            )
                        })?;
                    if method_ast.params.is_empty() {
                        return Err(self.error(
                            "SPX-H001",
                            format!("super method `{method}` has no self parameter"),
                            method_ast.span,
                        ));
                    }
                    if method_ast.params.len() - 1 != args.len() {
                        return Err(self.error(
                            "SPX-T231",
                            format!(
                                "`super.{method}` expects {} arguments, found {}",
                                method_ast.params.len() - 1,
                                args.len()
                            ),
                            span,
                        ));
                    }
                    for (arg, param) in args.iter().zip(method_ast.params.iter().skip(1)) {
                        let param_ty = self.resolve_type(&param.ty, param.span)?;
                        if arg.ty != param_ty {
                            return Err(self.error(
                                "SPX-T231",
                                format!(
                                    "`super.{method}` argument `{}` expects type `{}`, found `{}`",
                                    param.name,
                                    param_ty.identity_key(),
                                    arg.ty.identity_key()
                                ),
                                arg.span,
                            ));
                        }
                    }
                    let return_ty = self.resolve_type(&method_ast.return_type, method_ast.span)?;
                    let ownership =
                        self.expression_ownership(&return_ty, OwnershipMode::Own, span)?;
                    let mut call_args = Vec::with_capacity(1 + args.len());
                    call_args.push(receiver);
                    call_args.extend(args);
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &path),
                        ty: return_ty,
                        ownership,
                        kind: ResolvedExprKind::Call {
                            callee,
                            type_arguments: Vec::new(),
                            instance: None,
                            args: call_args,
                        },
                        span,
                    });
                }
                Frame::StartUpcast {
                    source,
                    bindings,
                    slot_path,
                    holder,
                    span,
                    resume,
                } => {
                    // Re-resolve the consumed expression at the canonical
                    // `.source` identity below its occupied slot, then wrap
                    // and resume the interrupted continuation.
                    frames.push(*resume);
                    frames.push(Frame::FinishUpcast {
                        slot_path: slot_path.clone(),
                        holder,
                        span,
                    });
                    frames.push(Frame::Enter {
                        expr: source,
                        bindings,
                        path: format!("{slot_path}.source"),
                    });
                }
                Frame::FinishUpcast {
                    slot_path,
                    holder,
                    span,
                } => {
                    let source = results.pop().expect("upcast source result retained");
                    let declared = ResolvedType::Nominal {
                        declaration: holder,
                        arguments: Vec::new(),
                    };
                    results.push(ResolvedExpr {
                        id: ExpressionId::new(function, &slot_path),
                        ty: declared.clone(),
                        ownership: self.expression_ownership(
                            &declared,
                            OwnershipMode::Own,
                            span,
                        )?,
                        kind: ResolvedExprKind::Upcast {
                            source: Box::new(source),
                        },
                        span,
                    });
                }
            }
        }

        if results.len() != 1 {
            return Err(self.error(
                "SPX-H006",
                "iterative expression resolver finished with an invalid result stack",
                expr.span,
            ));
        }
        results.pop().ok_or_else(|| {
            self.error(
                "SPX-H006",
                "iterative expression resolver lost its root result",
                expr.span,
            )
        })
    }
}
