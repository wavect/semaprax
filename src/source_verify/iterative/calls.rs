//! Call, method call, `?`, and projection frames: argument resumption and
//! the ownership transfers at each call boundary.

use crate::ast::{Expr, Function, ImportResult, ParamMode, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::arguments::{
    check_argument_ownership, release_borrowed_bytes_call_loans,
};
use crate::source_verify::binding::{CheckedValue, SourceLoanId};
use crate::source_verify::declared_type::{ordinary_option_argument, ordinary_result_arguments};
use crate::source_verify::diagnostics::{error, reject_native_unit_value};
use crate::source_verify::hints;
use crate::source_verify::scope::{VerifierCallTarget, VerifierFrame, VerifierFunctionSignature};
use crate::source_verify::type_table::{effective_record_fields, resolve_class_method};
use crate::source_verify::IterativeVerifier;

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_resume_call_argument(
        &mut self,
        expression: &'p Expr,
        name: &'p str,
        args: &'p [Expr],
        scope: usize,
        index: usize,
        target: VerifierCallTarget<'p>,
        borrowed_bytes_loans: Vec<(String, SourceLoanId)>,
    ) -> Result<(), Diagnostic> {
        let actual = self.values.pop().unwrap_or(None);
        let argument = &args[index];
        match &target {
            VerifierCallTarget::Native(import) => {
                if let (Some(actual), Some(parameter)) = (actual.as_ref(), import.params.get(index))
                {
                    reject_native_unit_value(self.program, argument, actual, self.diagnostics);
                    if !actual.native_unit
                        && (actual.ty != parameter.ty || actual.mode != ParamMode::Value)
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: scalar value signature required",
                            argument.span,
                        ));
                    }
                }
            }
            VerifierCallTarget::Ordinary(specialized) => {
                if let Some(param) = specialized
                    .as_ref()
                    .and_then(|target| target.params().get(index))
                {
                    if let Some(actual) = &actual {
                        reject_native_unit_value(self.program, argument, actual, self.diagnostics);
                    }
                    if actual
                        .as_ref()
                        .is_some_and(|actual| !actual.native_unit && actual.ty != param.ty)
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T205",
                            format!(
                                "argument `{}` to `{name}` expects {}, received {}",
                                param.name,
                                param.ty,
                                actual.as_ref().expect("type checked above").ty
                            ),
                            argument.span,
                        ));
                    }
                    check_argument_ownership(
                        self.program,
                        self.current,
                        name,
                        argument,
                        param,
                        actual.as_ref(),
                        &mut self.scopes[scope].bindings,
                        self.types,
                        self.allow_moves,
                        specialized
                            .as_ref()
                            .is_some_and(VerifierFunctionSignature::implicit_unique_ownership),
                        matches!(
                            specialized.as_ref(),
                            Some(VerifierFunctionSignature::Borrowed(function))
                                if function.type_parameters.is_empty()
                        ),
                        self.diagnostics,
                    );
                }
            }
            VerifierCallTarget::Byte(op) => {
                if let Some(actual) = &actual {
                    reject_native_unit_value(self.program, argument, actual, self.diagnostics);
                    if !actual.native_unit && !op.accepts_ast(index, &actual.ty) {
                        self.diagnostics.push(hints::with_optional_help(
                            error(
                                self.program,
                                "SPX-T263",
                                format!(
                                    "byte operation `{name}` argument {index} has the wrong type"
                                ),
                                argument.span,
                            ),
                            hints::view_argument_help(name, &actual.ty),
                        ));
                    }
                }
            }
            VerifierCallTarget::HostIo(op) => {
                if let Some(actual) = &actual {
                    reject_native_unit_value(self.program, argument, actual, self.diagnostics);
                    if !actual.native_unit && !op.accepts_ast(index, &actual.ty) {
                        self.diagnostics.push(hints::with_optional_help(
                            error(
                                self.program,
                                "SPX-T269",
                                format!(
                                    "host I/O operation `{name}` argument {index} has the wrong type"
                                ),
                                argument.span,
                            ),
                            hints::view_argument_help(name, &actual.ty),
                        ));
                    }
                }
            }
            VerifierCallTarget::CommandIo(op) => {
                if let Some(actual) = &actual {
                    reject_native_unit_value(self.program, argument, actual, self.diagnostics);
                    if !actual.native_unit
                        && !crate::command_io_ops::accepts_ast(*op, index, &actual.ty)
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T270",
                            format!("command I/O operation `{name}` argument {index} has the wrong type"),
                            argument.span,
                        ));
                    }
                }
            }
        }
        let next = index + 1;
        if let Some(argument) = args.get(next) {
            self.frames.push(VerifierFrame::ResumeCallArgument {
                expression,
                name,
                args,
                scope,
                index: next,
                target,
                borrowed_bytes_loans,
            });
            self.frames.push(VerifierFrame::Enter {
                expression: argument,
                scope,
            });
        } else {
            release_borrowed_bytes_call_loans(
                &mut self.scopes[scope].bindings,
                &borrowed_bytes_loans,
            );
            let output = match target {
                VerifierCallTarget::Native(import) => {
                    let mut value = CheckedValue::value(match import.result {
                        ImportResult::Unit => Type::Named {
                            name: "\0native-rust-unit".to_owned(),
                            arguments: Vec::new(),
                        },
                        ImportResult::I64 => Type::I64,
                        ImportResult::Bool => Type::Bool,
                    });
                    value.native_unit = import.result == ImportResult::Unit;
                    Some(value)
                }
                VerifierCallTarget::Byte(op) => {
                    Some(CheckedValue::returned(op.ast_return_type(), false))
                }
                VerifierCallTarget::HostIo(op) => {
                    Some(CheckedValue::returned(op.ast_return_type(), false))
                }
                VerifierCallTarget::CommandIo(op) => Some(CheckedValue {
                    ty: crate::command_io_ops::ast_return_type(op),
                    mode: match op {
                        crate::hir::ResolvedHostCommandOperation::ArgUtf8 => ParamMode::Borrow,
                        crate::hir::ResolvedHostCommandOperation::StdinRead => ParamMode::Own,
                        crate::hir::ResolvedHostCommandOperation::ArgsLen
                        | crate::hir::ResolvedHostCommandOperation::StderrWrite
                        | crate::hir::ResolvedHostCommandOperation::StdoutAppend
                        | crate::hir::ResolvedHostCommandOperation::StderrAppend => {
                            ParamMode::Value
                        }
                    },
                    native_unit: false,
                }),
                VerifierCallTarget::Ordinary(Some(target)) => Some(CheckedValue::returned(
                    target.return_type().clone(),
                    self.types.needs_drop(target.return_type()),
                )),
                VerifierCallTarget::Ordinary(None) => None,
            };
            self.values.push(output);
        }
        Ok(())
    }

    pub(super) fn frame_resume_method_receiver(
        &mut self,
        expression: &'p Expr,
        receiver: &'p Expr,
        method: &'p str,
        args: &'p [Expr],
        scope: usize,
    ) -> Result<(), Diagnostic> {
        let actual = self.values.pop().unwrap_or(None);
        let Some(receiver_value) = actual else {
            self.values.push(None);
            return Ok(());
        };
        let Type::Named {
            name: class_name,
            arguments: class_arguments,
        } = &receiver_value.ty
        else {
            self.diagnostics.push(error(
                self.program,
                "SPX-T203",
                format!(
                    "method `{method}` requires a class receiver, found `{}`",
                    receiver_value.ty
                ),
                receiver.span,
            ));
            self.values.push(None);
            return Ok(());
        };
        if !class_arguments.is_empty() {
            self.diagnostics.push(error(
                self.program,
                "SPX-T203",
                format!(
                    "method `{method}` on generic class `{class_name}` is not supported in this slice"
                ),
                expression.span,
            ));
            self.values.push(None);
            return Ok(());
        }
        let declaration = self.types.declaration(class_name);
        let _ = declaration;
        // Class Inheritance v1: resolution walks the receiver's
        // ancestor chain nearest-first; an inherited method's
        // declared class owns the expected `self` type.
        let Some((holder_name, method_fn)) = resolve_class_method(self.types, class_name, method)
        else {
            self.diagnostics.push(error(
                self.program,
                "SPX-T203",
                format!("unknown method `{method}` on `{class_name}`"),
                expression.span,
            ));
            self.values.push(None);
            return Ok(());
        };
        let Some(self_param) = method_fn.params.first() else {
            self.diagnostics.push(error(
                self.program,
                "SPX-T205",
                format!("method `{method}` on `{class_name}` has no `self` parameter"),
                method_fn.span,
            ));
            self.values.push(None);
            return Ok(());
        };
        let expected_self = Type::Named {
            name: holder_name.to_owned(),
            arguments: Vec::new(),
        };
        if self_param.mode != ParamMode::Value || self_param.ty != expected_self {
            self.diagnostics.push(error(
                self.program,
                "SPX-T205",
                format!(
                    "method `{method}` expects a value-mode `self: {holder_name}` receiver, found `{}`",
                    self_param.ty
                ),
                self_param.span,
            ));
            self.values.push(None);
            return Ok(());
        }
        if method_fn.params.len() - 1 != args.len() {
            self.diagnostics.push(error(
                self.program,
                "SPX-T204",
                format!(
                    "`{}.{}` expects {} arguments, received {}",
                    class_name,
                    method,
                    method_fn.params.len() - 1,
                    args.len()
                ),
                expression.span,
            ));
            self.values.push(None);
            return Ok(());
        }
        check_argument_ownership(
            self.program,
            self.current,
            method_fn.name.as_str(),
            receiver,
            self_param,
            Some(&receiver_value),
            &mut self.scopes[scope].bindings,
            self.types,
            self.allow_moves,
            true,
            false,
            self.diagnostics,
        );
        if args.is_empty() {
            self.values.push(Some(CheckedValue::returned(
                method_fn.return_type.clone(),
                self.types.needs_drop(&method_fn.return_type),
            )));
            return Ok(());
        }
        self.frames.push(VerifierFrame::ResumeMethodArgument {
            expression,
            method: method_fn,
            args,
            scope,
            index: 0,
        });
        self.frames.push(VerifierFrame::Enter {
            expression: &args[0],
            scope,
        });
        Ok(())
    }

    pub(super) fn frame_resume_method_argument(
        &mut self,
        expression: &'p Expr,
        method: &'p Function,
        args: &'p [Expr],
        scope: usize,
        index: usize,
    ) -> Result<(), Diagnostic> {
        let actual = self.values.pop().unwrap_or(None);
        let argument = &args[index];
        if let Some(param) = method.params.get(index + 1) {
            if let Some(actual) = &actual {
                reject_native_unit_value(self.program, argument, actual, self.diagnostics);
            }
            if actual
                .as_ref()
                .is_some_and(|actual| !actual.native_unit && actual.ty != param.ty)
            {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T205",
                    format!(
                        "argument `{}` to `{}` expects {}, received {}",
                        param.name,
                        method.name,
                        param.ty,
                        actual.as_ref().expect("type checked above").ty
                    ),
                    argument.span,
                ));
            }
            check_argument_ownership(
                self.program,
                self.current,
                method.name.as_str(),
                argument,
                param,
                actual.as_ref(),
                &mut self.scopes[scope].bindings,
                self.types,
                self.allow_moves,
                true,
                false,
                self.diagnostics,
            );
        }
        let next = index + 1;
        if let Some(argument) = args.get(next) {
            self.frames.push(VerifierFrame::ResumeMethodArgument {
                expression,
                method,
                args,
                scope,
                index: next,
            });
            self.frames.push(VerifierFrame::Enter {
                expression: argument,
                scope,
            });
        } else {
            self.values.push(Some(CheckedValue::returned(
                method.return_type.clone(),
                self.types.needs_drop(&method.return_type),
            )));
        }
        Ok(())
    }

    pub(super) fn frame_resume_try(
        &mut self,
        expression: &'p Expr,
        operand: &'p Expr,
        scope: usize,
    ) -> Result<(), Diagnostic> {
        let Some(operand_value) = self.values.pop().flatten() else {
            self.values.push(None);
            return Ok(());
        };
        reject_native_unit_value(self.program, operand, &operand_value, self.diagnostics);
        if !self.allow_moves {
            self.diagnostics.push(error(
                self.program,
                "SPX-T218",
                "`?` is only valid in an executable function body",
                expression.span,
            ));
        }
        if self.scopes[scope]
            .bindings
            .values()
            .any(|binding| self.types.needs_drop(&binding.ty))
        {
            self.diagnostics.push(error(
                self.program,
                "SPX-T218",
                "`?` with a live resource binding is not supported yet",
                expression.span,
            ));
        }
        if let Some((ok, error_ty)) = ordinary_result_arguments(&operand_value.ty) {
            let Some((_, residual_error_ty)) = ordinary_result_arguments(&self.current.return_type)
            else {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T218",
                    format!(
                        "function `{}` must return the ordinary compiler-owned Result to propagate a Result with `?`",
                        self.current.name
                    ),
                    expression.span,
                ));
                self.values.push(Some(CheckedValue::value(ok.clone())));
                return Ok(());
            };
            if error_ty != residual_error_ty {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T219",
                    format!("`?` cannot propagate error type {error_ty} into Result error type {residual_error_ty}"),
                    expression.span,
                ));
            }
            if !matches!(ok, Type::I64 | Type::Bool)
                || !matches!(error_ty, Type::I64 | Type::Bool)
                || !matches!(residual_error_ty, Type::I64 | Type::Bool)
            {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T218",
                    "Result `?` accepts only direct `i64` or `bool` success and error payloads",
                    expression.span,
                ));
            }
            self.values.push(Some(CheckedValue::value(ok.clone())));
            return Ok(());
        }
        if let Some(some) = ordinary_option_argument(&operand_value.ty) {
            let outer = ordinary_option_argument(&self.current.return_type);
            if outer.is_none() {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T218",
                    format!("function `{}` must return the ordinary compiler-owned Option to propagate an Option with `?`", self.current.name),
                    expression.span,
                ));
            } else if !matches!(some, Type::I64 | Type::Bool)
                || outer.is_some_and(|value| !matches!(value, Type::I64 | Type::Bool))
            {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T218",
                    "Option `?` accepts only direct `i64` or `bool` source and enclosing payloads",
                    expression.span,
                ));
            }
            self.values.push(Some(CheckedValue::value(some.clone())));
            return Ok(());
        }
        self.diagnostics.push(error(
            self.program,
            "SPX-T218",
            format!(
                "`?` operand must be an ordinary compiler-owned Result or Option, received {}",
                operand_value.ty
            ),
            expression.span,
        ));
        self.values.push(None);
        Ok(())
    }

    pub(super) fn frame_resume_project(
        &mut self,
        expression: &'p Expr,
        base: &'p Expr,
        field: &'p str,
    ) -> Result<(), Diagnostic> {
        let Some(base_value) = self.values.pop().flatten() else {
            self.values.push(None);
            return Ok(());
        };
        reject_native_unit_value(self.program, base, &base_value, self.diagnostics);
        let Some(fields) = effective_record_fields(self.types, &base_value.ty) else {
            self.diagnostics.push(error(
                self.program,
                "SPX-T214",
                format!("cannot project field `{field}` from `{}`", base_value.ty),
                expression.span,
            ));
            self.values.push(None);
            return Ok(());
        };
        let Some(declared) = fields.iter().find(|candidate| candidate.name == field) else {
            self.diagnostics.push(error(
                self.program,
                "SPX-T214",
                format!("record `{}` has no field `{field}`", base_value.ty),
                expression.span,
            ));
            self.values.push(None);
            return Ok(());
        };
        let projected = self
            .types
            .record_field_type(&base_value.ty, declared)
            .unwrap_or_else(|| declared.ty.clone());
        let mode = if self.types.needs_drop(&projected) {
            base_value.mode
        } else {
            ParamMode::Value
        };
        self.values.push(Some(CheckedValue {
            ty: projected,
            mode,
            native_unit: false,
        }));
        Ok(())
    }
}
