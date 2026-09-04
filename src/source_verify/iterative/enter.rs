//! The `Enter` frame: the first visit to an expression, which either
//! produces a value directly or schedules the frames its operands need.

use crate::ast::{Expr, ExprKind, ImportResult, ParamMode, Statement, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::source_verify::arguments::{
    activate_borrowed_bytes_call_loans, source_byte_view_place_is_admitted,
};
use crate::source_verify::binding::{Availability, CheckedValue};
use crate::source_verify::declared_type::{
    check_declared_type, direct_function_type_argument, validation_specialize_signature,
};
use crate::source_verify::diagnostics::{error, source_identifier};
use crate::source_verify::hints;
use crate::source_verify::place::{
    check_source_place_availability, overlapping_place_state, source_place,
};
use crate::source_verify::scope::{
    VerifierCallTarget, VerifierFrame, VerifierFunctionSignature, VerifierScope,
};
use crate::source_verify::type_table::effective_record_fields;
use crate::source_verify::IterativeVerifier;
use std::collections::HashSet;

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    pub(super) fn frame_enter(
        &mut self,
        expression: &'p Expr,
        scope: usize,
    ) -> Result<(), Diagnostic> {
        match &expression.kind {
            ExprKind::Int(_) => self.values.push(Some(CheckedValue::value(Type::I64))),
            ExprKind::Int32(_) => self.values.push(Some(CheckedValue::value(Type::I32))),
            ExprKind::Char(_) => self.values.push(Some(CheckedValue::value(Type::Char))),
            ExprKind::Uint8(_) => self.values.push(Some(CheckedValue::value(Type::U8))),
            ExprKind::Usize(_) => self.values.push(Some(CheckedValue::value(Type::Usize))),
            ExprKind::ArrayU8(values) => self.values.push(Some(CheckedValue::value(
                Type::ArrayU8(values.len() as u32),
            ))),
            ExprKind::RepeatArrayU8 { count, .. } => self
                .values
                .push(Some(CheckedValue::value(Type::ArrayU8(*count)))),
            ExprKind::Float32(_) => self.values.push(Some(CheckedValue::value(Type::F32))),
            ExprKind::Float64(_) => self.values.push(Some(CheckedValue::value(Type::F64))),
            ExprKind::Bool(_) => self.values.push(Some(CheckedValue::value(Type::Bool))),
            ExprKind::String(_) => self.values.push(Some(CheckedValue::value(Type::String))),
            ExprKind::Var(name) if name == "result" => {
                let value = self
                    .result_type
                    .map(|ty| CheckedValue::returned(ty.clone(), self.types.needs_drop(ty)));
                if value.is_none() {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T201",
                        "`result` is only available in postconditions",
                        expression.span,
                    ));
                }
                self.values.push(value);
            }
            ExprKind::Var(name) => {
                let value = self.scopes[scope].bindings.get(name).map(|binding| {
                    match binding.availability {
                        Availability::Moved => self.diagnostics.push(
                            error(
                                self.program,
                                "SPX-O101",
                                format!("use of resource `{name}` after ownership was moved"),
                                expression.span,
                            )
                            .with_help(
                                "borrow the resource if the callee does not need ownership",
                            ),
                        ),
                        Availability::MaybeMoved => self.diagnostics.push(
                            error(
                                self.program,
                                "SPX-O107",
                                format!("resource `{name}` may have been moved on another control-flow path"),
                                expression.span,
                            )
                            .with_help("move the resource on every path or keep it borrowed"),
                        ),
                        Availability::Available => match overlapping_place_state(binding, &[]) {
                            Availability::Moved => self.diagnostics.push(
                                error(self.program, "SPX-O109", format!("use of partially moved place `{name}`"), expression.span)
                                    .with_help("use an available sibling field or avoid moving this place earlier"),
                            ),
                            Availability::MaybeMoved => self.diagnostics.push(
                                error(self.program, "SPX-O110", format!("place `{name}` may have been moved on another control-flow path"), expression.span)
                                    .with_help("move the field on every path or keep it borrowed"),
                            ),
                            Availability::Available => {}
                        },
                    }
                    if binding.native_unit_discard {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: scalar value signature required",
                            expression.span,
                        ));
                    }
                    CheckedValue { ty: binding.ty.clone(), mode: binding.mode, native_unit: binding.native_unit_discard }
                });
                if value.is_none() {
                    self.diagnostics.push(hints::with_optional_help(
                        error(
                            self.program,
                            "SPX-T202",
                            format!("unknown value `{name}` in `{}`", self.current.name),
                            expression.span,
                        ),
                        hints::variant_shorthand_help(name).map(str::to_owned),
                    ));
                }
                self.values.push(value);
            }
            ExprKind::Unary { op, value } => {
                self.frames.push(VerifierFrame::ResumeUnary {
                    expression,
                    operand: value,
                    op: *op,
                });
                self.frames.push(VerifierFrame::Enter {
                    expression: value,
                    scope,
                });
            }
            ExprKind::Binary { op, left, right } => {
                self.frames.push(VerifierFrame::ResumeBinaryLeft {
                    expression,
                    op: *op,
                    right,
                    scope,
                });
                self.frames.push(VerifierFrame::Enter {
                    expression: left,
                    scope,
                });
            }
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } => {
                let native = self
                    .program
                    .interfaces
                    .iter()
                    .flat_map(|interface| &interface.imports)
                    .find(|import| import.native_rust && import.name == *name);
                let target = if let Some(import) = native {
                    if !type_arguments.is_empty() || args.len() != import.params.len() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: scalar value signature required",
                            expression.span,
                        ));
                    }
                    for effect in &import.effects {
                        if !self.current.effects.contains(effect) {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-B107",
                                "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                                expression.span,
                            ));
                        }
                    }
                    VerifierCallTarget::Native(import)
                } else if let Some(op) = crate::string_ops::by_name(name) {
                    // Compiler-owned string operations verify through
                    // the ordinary monomorphic machinery with one
                    // synthetic signature; consuming arguments use the
                    // established `own` transfer mode and borrowed
                    // arguments never mark their sources moved.
                    let params = crate::string_ops::ast_params(op);
                    if !type_arguments.is_empty() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T225",
                            format!("monomorphic function `{name}` does not accept type arguments"),
                            expression.span,
                        ));
                    }
                    if args.len() != params.len() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T204",
                            format!(
                                "`{name}` expects {} arguments, received {}",
                                params.len(),
                                args.len()
                            ),
                            expression.span,
                        ));
                    }
                    VerifierCallTarget::Ordinary(Some(VerifierFunctionSignature::Specialized {
                        params,
                        return_type: op.ast_return_type(),
                        implicit_unique_ownership: false,
                    }))
                } else if let Some(op) = crate::str_ops::by_name(name) {
                    let params = crate::str_ops::ast_params(op);
                    if !type_arguments.is_empty() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T225",
                            format!("monomorphic function `{name}` does not accept type arguments"),
                            expression.span,
                        ));
                    }
                    if args.len() != params.len() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T204",
                            format!(
                                "`{name}` expects {} arguments, received {}",
                                params.len(),
                                args.len()
                            ),
                            expression.span,
                        ));
                    }
                    VerifierCallTarget::Ordinary(Some(VerifierFunctionSignature::Specialized {
                        params,
                        return_type: op.ast_return_type(),
                        implicit_unique_ownership: false,
                    }))
                } else if let Some(op) = crate::byte_ops::by_name(name) {
                    if !type_arguments.is_empty() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T263",
                            format!("byte operation `{name}` does not accept type arguments"),
                            expression.span,
                        ));
                    }
                    if args.len() != op.arity() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T263",
                            format!(
                                "byte operation `{name}` expects {} arguments, received {}",
                                op.arity(),
                                args.len()
                            ),
                            expression.span,
                        ));
                    }
                    if op.is_view()
                        && args.first().is_none_or(|argument| {
                            !source_byte_view_place_is_admitted(
                                op,
                                argument,
                                &self.scopes[scope].bindings,
                                self.types,
                            )
                        })
                    {
                        let diagnostic = error(
                            self.program,
                            "SPX-T266",
                            format!(
                                "borrowed view `{name}` requires an exact admitted storage place"
                            ),
                            expression.span,
                        );
                        self.diagnostics.push(match args.first() {
                            Some(argument) => {
                                diagnostic.with_help(hints::view_place_help(name, argument))
                            }
                            None => diagnostic,
                        });
                    }
                    VerifierCallTarget::Byte(op)
                } else if let Some(op) = crate::host_io_ops::by_name(name) {
                    let params = crate::host_io_ops::ast_params(op);
                    if !type_arguments.is_empty() || args.len() != params.len() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T269",
                            format!("invalid host I/O operation `{name}` call shape"),
                            expression.span,
                        ));
                    }
                    VerifierCallTarget::HostIo(op)
                } else if let Some(op) = crate::command_io_ops::by_name(name) {
                    let params = crate::command_io_ops::ast_params(op);
                    if !type_arguments.is_empty() || args.len() != params.len() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T270",
                            format!("invalid command I/O operation `{name}` call shape"),
                            expression.span,
                        ));
                    }
                    VerifierCallTarget::CommandIo(op)
                } else {
                    let target = self.functions.get(name.as_str()).copied();
                    if target.is_none() {
                        self.diagnostics.push(hints::unknown_function(
                            self.program,
                            name,
                            self.functions,
                            expression.span,
                        ));
                    }
                    if target.is_some_and(|target| args.len() != target.params.len()) {
                        let target = target.expect("checked above");
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T204",
                            format!(
                                "`{name}` expects {} arguments, received {}",
                                target.params.len(),
                                args.len()
                            ),
                            expression.span,
                        ));
                    }
                    let specialized = target.and_then(|target| {
                        if target.type_parameters.is_empty() {
                            if !type_arguments.is_empty() {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-T225",
                                    format!("monomorphic function `{name}` does not accept type arguments"),
                                    expression.span,
                                ));
                                return None;
                            }
                            return Some(VerifierFunctionSignature::Borrowed(target));
                        }
                        if !self.current.type_parameters.is_empty() {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T226",
                                format!("generic function `{}` cannot call generic function `{name}` in this slice", self.current.name),
                                expression.span,
                            ));
                        }
                        if type_arguments.len() != target.type_parameters.len() {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T225",
                                format!("generic function `{name}` expects {} explicit type arguments, received {}", target.type_parameters.len(), type_arguments.len()),
                                expression.span,
                            ).with_help(hints::generic_call_help(name)));
                            return None;
                        }
                        if type_arguments.iter().any(|argument| !direct_function_type_argument(argument)) {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T225",
                                format!("generic function `{name}` accepts only direct `i64` or `bool` type arguments"),
                                expression.span,
                            ));
                            return None;
                        }
                        validation_specialize_signature(target, type_arguments).map(
                            |(params, return_type)| {
                                VerifierFunctionSignature::Specialized {
                                    params,
                                    return_type,
                                    implicit_unique_ownership: true,
                                }
                            },
                        )
                    });
                    VerifierCallTarget::Ordinary(specialized)
                };
                let borrowed_bytes_loans = match &target {
                    VerifierCallTarget::Ordinary(Some(VerifierFunctionSignature::Borrowed(
                        function,
                    ))) => activate_borrowed_bytes_call_loans(
                        args,
                        &function.params,
                        &mut self.scopes[scope].bindings,
                        self.types,
                    ),
                    _ => Vec::new(),
                };
                if let Some(argument) = args.first() {
                    self.frames.push(VerifierFrame::ResumeCallArgument {
                        expression,
                        name,
                        args,
                        scope,
                        index: 0,
                        target,
                        borrowed_bytes_loans,
                    });
                    self.frames.push(VerifierFrame::Enter {
                        expression: argument,
                        scope,
                    });
                } else {
                    self.values.push(Some(match target {
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
                            value
                        }
                        VerifierCallTarget::Byte(op) => {
                            CheckedValue::returned(op.ast_return_type(), false)
                        }
                        VerifierCallTarget::HostIo(op) => {
                            CheckedValue::returned(op.ast_return_type(), false)
                        }
                        VerifierCallTarget::CommandIo(op) => CheckedValue {
                            ty: crate::command_io_ops::ast_return_type(op),
                            mode: match op {
                                crate::hir::ResolvedHostCommandOperation::ArgUtf8 => {
                                    ParamMode::Borrow
                                }
                                crate::hir::ResolvedHostCommandOperation::StdinRead => {
                                    ParamMode::Own
                                }
                                crate::hir::ResolvedHostCommandOperation::ArgsLen
                                | crate::hir::ResolvedHostCommandOperation::StderrWrite
                                | crate::hir::ResolvedHostCommandOperation::StdoutAppend
                                | crate::hir::ResolvedHostCommandOperation::StderrAppend => {
                                    ParamMode::Value
                                }
                            },
                            native_unit: false,
                        },
                        VerifierCallTarget::Ordinary(Some(target)) => CheckedValue::returned(
                            target.return_type().clone(),
                            self.types.needs_drop(target.return_type()),
                        ),
                        VerifierCallTarget::Ordinary(None) => {
                            self.values.push(None);
                            return Ok(());
                        }
                    }));
                }
            }
            ExprKind::MethodCall {
                receiver,
                method,
                type_arguments,
                args,
                ..
            } => {
                if !type_arguments.is_empty() {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T225",
                        format!("method `{method}` does not accept type arguments in this slice"),
                        expression.span,
                    ));
                }
                self.frames.push(VerifierFrame::ResumeMethodReceiver {
                    expression,
                    receiver,
                    method,
                    args,
                    scope,
                });
                self.frames.push(VerifierFrame::Enter {
                    expression: receiver,
                    scope,
                });
            }
            // These walkers only traverse generic-function and contract
            // expressions; `super` is meaningful only inside a class-method
            // override, whose body is resolved by the HIR layer instead.
            ExprKind::SuperMethod { method_span, .. } => {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T231",
                    "`super` is only allowed inside a class-method override",
                    *method_span,
                ));
                self.values.push(None);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.frames.push(VerifierFrame::ResumeIfCondition {
                    expression,
                    then_branch,
                    else_branch,
                    scope,
                });
                self.frames.push(VerifierFrame::Enter {
                    expression: condition,
                    scope,
                });
            }
            ExprKind::Block { statements, tail } => {
                let outer_names = self.scopes[scope]
                    .bindings
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>();
                let block_scope = self.scopes.len();
                self.scopes.push(VerifierScope {
                    bindings: self.scopes[scope].bindings.clone(),
                });
                if let Some(first_statement) = statements.first() {
                    if let Statement::Let {
                        name, name_span, ..
                    } = first_statement
                    {
                        if !source_identifier(name) {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-S109",
                                format!("`{name}` is reserved and cannot name a local binding"),
                                *name_span,
                            ));
                        }
                    }
                    if let Statement::While {
                        condition, body, ..
                    } = first_statement
                    {
                        self.begin_while_statement(
                            expression,
                            statements,
                            tail,
                            scope,
                            block_scope,
                            0,
                            outer_names,
                            condition,
                            body,
                        );
                    } else {
                        self.frames.push(VerifierFrame::ResumeBlockStatement {
                            expression,
                            statements,
                            tail,
                            parent_scope: scope,
                            block_scope,
                            index: 0,
                            outer_names,
                        });
                        self.frames.push(VerifierFrame::Enter {
                            expression: first_statement.value(),
                            scope: block_scope,
                        });
                    }
                } else {
                    self.frames.push(VerifierFrame::ResumeBlockTail {
                        parent_scope: scope,
                        block_scope,
                        outer_names,
                    });
                    self.frames.push(VerifierFrame::Enter {
                        expression: tail,
                        scope: block_scope,
                    });
                }
            }
            ExprKind::Try { operand } => {
                self.frames.push(VerifierFrame::ResumeTry {
                    expression,
                    operand,
                    scope,
                });
                self.frames.push(VerifierFrame::Enter {
                    expression: operand,
                    scope,
                });
            }
            ExprKind::Project { base, field, .. } => {
                if let Some(place) =
                    source_place(expression, &self.scopes[scope].bindings, self.types)
                {
                    check_source_place_availability(
                        self.program,
                        &place,
                        &self.scopes[scope].bindings,
                        expression.span,
                        self.diagnostics,
                    );
                    self.values.push(Some(CheckedValue {
                        ty: place.ty,
                        mode: place.mode,
                        native_unit: false,
                    }));
                } else {
                    self.frames.push(VerifierFrame::ResumeProject {
                        expression,
                        base,
                        field,
                    });
                    self.frames.push(VerifierFrame::Enter {
                        expression: base,
                        scope,
                    });
                }
            }
            ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                fields,
                ..
            } => {
                let instance = Type::Named {
                    name: type_name.clone(),
                    arguments: type_arguments.clone(),
                };
                check_declared_type(
                    self.program,
                    &instance,
                    expression.span,
                    self.types,
                    &HashSet::new(),
                    self.diagnostics,
                );
                let declared_fields = effective_record_fields(self.types, &instance);
                if declared_fields.is_none() {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T215",
                        format!("`{type_name}` is not a declared record type"),
                        expression.span,
                    ));
                }
                if !fields.is_empty() {
                    self.frames.push(VerifierFrame::PrepareRecordField {
                        expression,
                        type_name,
                        type_arguments,
                        fields,
                        declared_fields,
                        scope,
                        index: 0,
                        supplied: HashSet::new(),
                    });
                } else {
                    if let Some(declared_fields) = declared_fields {
                        for field in declared_fields {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T213",
                                format!(
                                    "record `{type_name}` construction is missing field `{}`",
                                    field.name
                                ),
                                expression.span,
                            ));
                        }
                        self.values.push(Some(CheckedValue::returned(
                            instance.clone(),
                            self.types.needs_drop(&instance),
                        )));
                    } else {
                        self.values.push(None);
                    }
                }
            }
            ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                case_name,
                fields,
                ..
            } => {
                let declaration = self.types.declaration(type_name);
                let instance = Type::Named {
                    name: type_name.clone(),
                    arguments: type_arguments.clone(),
                };
                check_declared_type(
                    self.program,
                    &instance,
                    expression.span,
                    self.types,
                    &HashSet::new(),
                    self.diagnostics,
                );
                let cases = declaration.and_then(|declaration| match &declaration.kind {
                    TypeDeclarationKind::Variant { cases } => Some(cases.as_slice()),
                    TypeDeclarationKind::Resource { .. }
                    | TypeDeclarationKind::Record { .. }
                    | TypeDeclarationKind::Class { .. } => None,
                });
                let case =
                    cases.and_then(|cases| cases.iter().find(|case| case.name == *case_name));
                if cases.is_none() || case.is_none() {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T215",
                        format!("`{type_name}::{case_name}` is not a declared variant constructor"),
                        expression.span,
                    ));
                }
                if !fields.is_empty() {
                    self.frames.push(VerifierFrame::PrepareVariantField {
                        expression,
                        type_name,
                        type_arguments,
                        case_name,
                        fields,
                        declaration,
                        case,
                        scope,
                        index: 0,
                        supplied: HashSet::new(),
                    });
                } else {
                    if let Some(case) = case {
                        for field in &case.fields {
                            self.diagnostics.push(error(self.program, "SPX-T213", format!("variant construction `{type_name}::{case_name}` is missing payload field `{}`", field.name), expression.span));
                        }
                        // Ownership belongs to the complete variant carrier, not
                        // only to the selected case. A zero-payload case of a
                        // variant whose other cases contain owned fields is still
                        // a fresh owned carrier and must cross return/call
                        // boundaries exactly once.
                        self.values.push(Some(CheckedValue::returned(
                            instance.clone(),
                            self.types.needs_drop(&instance),
                        )));
                    } else {
                        self.values.push(None);
                    }
                }
            }
            ExprKind::UpdateRecord { base, fields } => {
                self.frames.push(VerifierFrame::ResumeUpdateBase {
                    expression,
                    base,
                    fields,
                    scope,
                });
                self.frames.push(VerifierFrame::Enter {
                    expression: base,
                    scope,
                });
            }
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                self.frames.push(VerifierFrame::ResumeMatchScrutinee {
                    expression,
                    scrutinee,
                    arms,
                    scope,
                });
                self.frames.push(VerifierFrame::Enter {
                    expression: scrutinee,
                    scope,
                });
            }
        }
        Ok(())
    }
}
