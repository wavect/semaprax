//! The `Enter` frame: the first visit to an expression, which either
//! produces a value directly or schedules the frames its operands need.

use crate::ast::{Expr, ExprKind, ImportResult, ParamMode, Statement, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::source_verify::arguments::{
    activate_borrowed_bytes_call_loans, source_byte_view_place_is_admitted,
};
use crate::source_verify::binding::{Availability, CheckedValue};
use crate::source_verify::declared_type::{
    check_declared_type, function_value_signature, generic_function_arguments_are_admitted,
    generic_function_arguments_are_forwarded, validation_specialize_signature,
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

impl<'a, 'v, 'p> IterativeVerifier<'a, 'v, 'p> {
    pub(super) fn before_components_frame_enter(
        &mut self,
        expression: &'p Expr,
        scope: usize,
    ) -> Result<(), Diagnostic> {
        #[cfg(test)]
        if self.recovery_legacy {
            return self.recovery_old_frame_enter(expression, scope);
        }

        self.recovery.begin(expression);
        match &expression.kind {
            ExprKind::VecConstructor { .. } => {
                let hygienic = *self.constructor_hygiene.get_or_insert_with(|| {
                    crate::ast::expression_statement::projection_roots(
                        self.current
                            .payload()
                            .requires
                            .iter()
                            .chain(std::iter::once(&self.current.payload().body))
                            .chain(&self.current.payload().ensures),
                        self.current
                            .payload()
                            .params
                            .iter()
                            .map(|param| param.name.as_str())
                            .chain(
                                self.current
                                    .payload()
                                    .type_parameters
                                    .iter()
                                    .map(|param| param.name.as_str()),
                            ),
                        crate::bounded_output::reserve_active_required,
                    )
                });
                if !hygienic {
                    return Err(error(
                        self.program,
                        "SPX-T318",
                        "Vec constructor generated owner names are not hygienic",
                        expression.span,
                    ));
                }
                let expansion = crate::ast::vec_constructor::inspect(expression)
                    .map_err(|message| error(self.program, "SPX-T318", message, expression.span))?;
                self.frames.push(VerifierFrame::Enter {
                    expression: expansion.value,
                    scope,
                });
            }
            ExprKind::LexicalExit { kind, value } => {
                if let Some(operand) = value {
                    self.frames.push(VerifierFrame::ResumeLexicalReturn {
                        expression,
                        operand,
                        scope,
                    });
                    self.frames.push(VerifierFrame::Enter {
                        expression: operand,
                        scope,
                    });
                } else if *kind != crate::ast::LexicalExitKind::Return {
                    self.values.push(Some(CheckedValue::value(Type::Never)));
                } else {
                    return Err(error(
                        self.program,
                        "SPX-T312",
                        "return requires an operand",
                        expression.span,
                    ));
                }
            }
            ExprKind::Closure { .. } => self.enter_closure(expression, scope)?,
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
            ExprKind::BorrowedUtf8Literal(text) => {
                self.values
                    .push(Some(crate::source_verify::static_text::checked(
                        self.program,
                        expression.span,
                        text,
                    )?))
            }
            ExprKind::Var(name) if name == "result" => {
                let value = self
                    .result_type
                    .map(|ty| CheckedValue::returned(ty.clone(), self.types.needs_drop(ty)));
                if value.is_none() {
                    self.diagnostics.push(
                        error(
                            self.program,
                            "SPX-T201",
                            "`result` is only available in postconditions",
                            expression.span,
                        )
                        .with_help(hints::RESULT_NAME_HELP),
                    );
                }
                self.values.push(value);
            }
            ExprKind::Var(name) => {
                if self.scopes[scope]
                    .bindings
                    .get(name)
                    .is_some_and(|binding| binding.traversal_row.is_some())
                    && !self
                        .row_loan_sites
                        .contains(&(expression.span.start, expression.span.end))
                {
                    return Err(error(
                        self.program,
                        "SPX-T284",
                        "record traversal row cannot escape or materialize aggregate storage; select a field",
                        expression.span,
                    ));
                }
                let value = self.scopes[scope].bindings.get(name).map(|binding| {
                    crate::source_verify::owning_closure::reject_escaping_read(
                        self.program,
                        name,
                        &binding.ty,
                        expression.span,
                        self.diagnostics,
                    );
                    match binding.availability {
                        Availability::Moved => self.diagnostics.push(
                            error(
                                self.program,
                                "SPX-O101",
                                format!("use of resource `{name}` after ownership was moved"),
                                expression.span,
                            )
                            .with_help(crate::source_verify::hints::moved_resource_help(
                                &binding.ty,
                                name,
                            )),
                        ),
                        Availability::MaybeMoved => self.diagnostics.push(
                            error(
                                self.program,
                                "SPX-O107",
                                format!("resource `{name}` may have been moved on another control-flow path"),
                                expression.span,
                            )
                            .with_help(crate::source_verify::hints::moved_resource_help(
                                &binding.ty,
                                name,
                            )),
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
                let value = value.or_else(|| {
                    self.functions
                        .get(name.as_str())
                        .and_then(|function| function_value_signature(function.payload()))
                        .map(CheckedValue::value)
                });
                if value.is_none() && !self.recovery.reads_refused(scope, name) {
                    self.diagnostics.push(hints::with_optional_help(
                        error(
                            self.program,
                            "SPX-T202",
                            format!("unknown value `{name}` in `{}`", self.current.name()),
                            expression.span,
                        ),
                        hints::variant_shorthand_help(name).map(str::to_owned),
                    ));
                }
                // Preserve every real binding/ownership check above before
                // attaching a separate expected-type refusal view to this read.
                let refused = value.is_some() && self.recovery.reads_value(scope, name, expression);
                self.values.push(if refused { None } else { value });
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
                let args: &[Expr] = if name == crate::vec_ops::sort_keys::NAME {
                    if !self.current.payload().type_parameters.is_empty()
                        || crate::source_verify::sort_keys::descriptor(
                            self.types,
                            type_arguments,
                            args,
                        )
                        .is_none()
                    {
                        self.diagnostics.push(error(self.program,"SPX-T311","vec_sort_by requires an explicit monomorphic record and 1..8 unique literal field:asc/field:desc keys",expression.span));
                        self.values.push(None);
                        return Ok(());
                    }
                    &args[..1]
                } else if name == crate::vec_ops::field_unique::NAME {
                    if !self.current.payload().type_parameters.is_empty()
                        || crate::source_verify::field_unique::descriptor(
                            self.types,
                            type_arguments,
                            args,
                            &self.scopes[scope].bindings,
                        )
                        .is_none()
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T319",
                            crate::vec_ops::field_unique::SHAPE,
                            expression.span,
                        ));
                        self.values.push(None);
                        return Ok(());
                    }
                    &args[..1]
                } else {
                    args
                };
                if let Some(result) = crate::source_verify::owning_closure::check_call(
                    self.program,
                    name,
                    type_arguments,
                    args,
                    expression.span,
                    self.allow_moves,
                    &mut self.scopes[scope].bindings,
                    self.types,
                    self.diagnostics,
                ) {
                    self.values.push(result);
                    return Ok(());
                }
                let native = self
                    .program
                    .interfaces()
                    .iter()
                    .flat_map(|interface| &interface.imports)
                    .find(|import| import.native_rust && import.name == *name);
                let callable_binding = self.scopes[scope].bindings.get(name).map(|binding| {
                    crate::source_verify::mutable_closure::invocation_signature(
                        self.program,
                        binding,
                        name,
                        args,
                        self.allow_moves,
                        expression.span,
                        self.diagnostics,
                    )
                });
                let target = if let Some(binding_type) = callable_binding {
                    match binding_type {
                        Type::Function { parameters, result } => {
                            if !type_arguments.is_empty() {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-T225",
                                    "function values do not accept type arguments",
                                    expression.span,
                                ));
                            }
                            if args.len() != parameters.len() {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-T204",
                                    format!(
                                        "`{name}` expects {} arguments, received {}",
                                        parameters.len(),
                                        args.len()
                                    ),
                                    expression.span,
                                ));
                            }
                            VerifierCallTarget::Ordinary(Some(
                                VerifierFunctionSignature::Specialized {
                                    params: parameters
                                        .into_iter()
                                        .enumerate()
                                        .map(|(index, ty)| crate::ast::Param {
                                            name: format!("argument{index}"),
                                            mode: ParamMode::Value,
                                            ty,
                                            span: expression.span,
                                        })
                                        .collect(),
                                    return_type: *result,
                                    implicit_unique_ownership: false,
                                },
                            ))
                        }
                        _ => {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T287",
                                format!("`{name}` is a bound value, not a function value"),
                                expression.span,
                            ));
                            VerifierCallTarget::Ordinary(None)
                        }
                    }
                } else if let Some(import) = native {
                    if !type_arguments.is_empty() || args.len() != import.params.len() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-B107",
                            "Native Rust Interop declaration set is unsupported: scalar value signature required",
                            expression.span,
                        ));
                    }
                    for effect in &import.effects {
                        if !self.current.payload().effects.contains(effect) {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-B107",
                                "Native Rust Interop declaration set is unsupported: effect or capability mismatch",
                                expression.span,
                            ));
                        }
                    }
                    VerifierCallTarget::Native(import)
                } else if let Some(op) = crate::map_ops::by_generic_name(name, type_arguments) {
                    let Some((params, return_type)) = op.ast_signature(type_arguments) else {
                        return Err(error(
                            self.program,
                            "SPX-T274",
                            "typed collection requires admitted key/value type arguments",
                            expression.span,
                        ));
                    };
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
                        return_type,
                        implicit_unique_ownership: false,
                    }))
                } else if name == crate::vec_field::NAME {
                    let Some((params, return_type)) = crate::source_verify::vec_field::signature(
                        self.types,
                        type_arguments,
                        args,
                        expression.span,
                    ) else {
                        self.diagnostics.push(error(self.program,"SPX-T310","vec_field requires one admitted owned record type, a named Vec and usize index, and an exact literal field selector",expression.span));
                        self.values.push(None);
                        return Ok(());
                    };
                    if !self.current.payload().type_parameters.is_empty() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T310",
                            "vec_field is not admitted in generic function templates",
                            expression.span,
                        ));
                    }
                    VerifierCallTarget::Ordinary(Some(VerifierFunctionSignature::Specialized {
                        params,
                        return_type,
                        implicit_unique_ownership: false,
                    }))
                } else if name == crate::literal_format::NAME {
                    if !type_arguments.is_empty() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T225",
                            "string_format does not accept type arguments",
                            expression.span,
                        ));
                    }
                    if !self.current.payload().type_parameters.is_empty() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T225",
                            "string_format is not admitted in generic function templates",
                            expression.span,
                        ));
                    }
                    let template = match args.first().map(|arg| &arg.kind) {
                        Some(ExprKind::String(value)) => value,
                        _ => {
                            self.diagnostics.push(error(self.program, "SPX-T204", "string_format requires a compile-time string literal as its first argument", expression.span));
                            self.values.push(None);
                            return Ok(());
                        }
                    };
                    let pieces = match crate::literal_format::scan(template) {
                        Ok(pieces) => pieces,
                        Err(reason) => {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T204",
                                reason.message(),
                                args[0].span,
                            ));
                            self.values.push(None);
                            return Ok(());
                        }
                    };
                    let fields = crate::literal_format::field_count(&pieces);
                    if args.len() - 1 != fields {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T204",
                            format!(
                                "string_format literal has {fields} fields, received {} values",
                                args.len() - 1
                            ),
                            expression.span,
                        ));
                    }
                    VerifierCallTarget::LiteralFormat
                } else if let Some(op) = crate::string_ops::by_name(name) {
                    // Compiler-owned string operations verify through
                    // the ordinary monomorphic machinery with one
                    // synthetic signature; consuming arguments use the
                    // established `own` transfer mode and borrowed
                    // arguments never mark their sources moved.
                    if op == crate::string_ops::StringOp::Take
                        && !crate::string_ops::take::source_operand(args, self.allow_moves)
                    {
                        self.diagnostics.push(error(self.program, "SPX-O105",
                            "string_take requires an available named owned String outside contracts", expression.span));
                    }
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
                } else if let Some(op) = crate::vec_ops::by_name(name) {
                    if op == crate::vec_ops::VecOp::StrAt
                        && (!self.current.payload().type_parameters.is_empty()
                            || crate::source_verify::string_slot::root(
                                type_arguments,
                                args,
                                &self.scopes[scope].bindings,
                            )
                            .is_none())
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T281",
                            crate::vec_ops::string_slot::SHAPE,
                            expression.span,
                        ));
                    }
                    let element = if op == crate::vec_ops::VecOp::StrAt {
                        Some(&Type::String)
                    } else {
                        type_arguments.first()
                    };
                    if (if op == crate::vec_ops::VecOp::StrAt { !type_arguments.is_empty() } else { type_arguments.len() != 1 })
                        || element.is_none_or(|ty| {
                            !crate::source_verify::declared_type::owned_leaf_collection::vec_operation_admitted(self.types, op, ty)
                                && !crate::vec_ops::source_parameter_is_admitted_ref(self.program, self.current, op, ty)
                        })
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T281",
                            format!("vector operation `{name}` requires one explicit admitted type argument"),
                            expression.span,
                        ));
                    }
                    if args.len() != op.arity() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T281",
                            format!(
                                "vector operation `{name}` expects {} arguments, received {}",
                                op.arity(),
                                args.len()
                            ),
                            expression.span,
                        ));
                    }
                    if op.capacity_argument().is_some_and(|index| {
                        matches!(args.get(index).map(|arg| &arg.kind), Some(ExprKind::Usize(value)) if *value > crate::vec_ops::MAX_CAPACITY)
                    })
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T282",
                            format!(
                                "`{name}` literal capacity must be no greater than {}",
                                crate::vec_ops::MAX_CAPACITY
                            ),
                            expression.span,
                        ));
                    }
                    VerifierCallTarget::Ordinary(element.map(|element| {
                        VerifierFunctionSignature::Specialized {
                            params: crate::vec_ops::ast_params_with_owned_element(
                                op,
                                element,
                                *element == crate::ast::Type::Bytes
                                    || crate::source_verify::declared_type::owned_leaf_collection::admitted(self.types, element)
                                    || crate::source_verify::declared_type::
                                        owned_leaf_collection::runtime_element(
                                            self.types, element,
                                        ),
                            ),
                            return_type: op.ast_return_type(element),
                            implicit_unique_ownership: false,
                        }
                    }))
                } else if let Some(op) = crate::list_ops::by_name(name) {
                    if !type_arguments.is_empty() || args.len() != op.argument_count() {
                        self.diagnostics.push(error(self.program, "SPX-T291", format!("immutable list operation `{name}` requires {} arguments and no type arguments", op.argument_count()), expression.span));
                    }
                    VerifierCallTarget::Ordinary(Some(VerifierFunctionSignature::Specialized {
                        params: op.ast_params(),
                        return_type: op.ast_return_type(),
                        implicit_unique_ownership: false,
                    }))
                } else if let Some(op) = crate::iterator_ops::by_name(name) {
                    let element = type_arguments.first();
                    if type_arguments.len() != 1
                        || args.len() != 1
                        || element.is_none_or(|ty| {
                            !crate::iterator_ops::ast_element_is_admitted(ty)
                                    && !crate::source_verify::declared_type::
                                        owned_leaf_collection::runtime_element(
                                            self.types,
                                            ty,
                                        )
                        })
                    {
                        self.diagnostics.push(error(self.program,"SPX-T290","iterator operations require one concrete scalar type argument and one owned argument",expression.span));
                    }
                    VerifierCallTarget::Ordinary(element.map(|element| {
                        VerifierFunctionSignature::Specialized {
                            params: crate::iterator_ops::ast_params(op, element),
                            return_type: op.ast_return_type(element),
                            implicit_unique_ownership: false,
                        }
                    }))
                } else if let Some(op) = crate::box_ops::by_name(name) {
                    let element = type_arguments.first();
                    if type_arguments.len() != 1
                        || element.is_none_or(|ty| {
                            !crate::box_ops::ast_operation_element_is_admitted(op, ty)
                                && !crate::box_ops::source_parameter_is_admitted(
                                    self.program,
                                    self.current.payload(),
                                    op,
                                    ty,
                                )
                        })
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T285",
                            format!("box operation `{name}` requires one explicit admitted type argument"),
                            expression.span,
                        ));
                    }
                    if args.len() != 1 {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T285",
                            format!(
                                "box operation `{name}` expects 1 argument, received {}",
                                args.len()
                            ),
                            expression.span,
                        ));
                    }
                    VerifierCallTarget::Ordinary(element.map(|element| {
                        VerifierFunctionSignature::Specialized {
                            params: crate::box_ops::ast_params(op, element),
                            return_type: op.ast_return_type(element),
                            implicit_unique_ownership: false,
                        }
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
                            if matches!(
                                op,
                                crate::byte_ops::ByteOp::StringAsStr
                                    | crate::byte_ops::ByteOp::StrAsBytes
                            ) {
                                format!(
                                    "borrowed view `{name}` requires a named String owner or an authenticated named-record path to a String field"
                                )
                            } else {
                                format!(
                                    "borrowed view `{name}` requires an exact admitted storage place"
                                )
                            },
                            expression.span,
                        );
                        self.diagnostics.push(match args.first() {
                            Some(argument) => {
                                diagnostic.with_help(hints::view_place_help(name, argument))
                            }
                            None => diagnostic,
                        });
                    }
                    if op.is_owned_buffer_chain() {
                        let reopen = self
                            .buffer_reopen_sites
                            .contains(&(expression.span.start, expression.span.end));
                        self.diagnostics
                            .extend(crate::source_verify::owned_buffer::check_call(
                                self.program,
                                expression,
                                op,
                                name,
                                args,
                                reopen,
                            ));
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
                } else if let Some(op) = crate::stdin_stream_ops::pure_by_name(name) {
                    if !type_arguments.is_empty() || args.len() != 1 {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T270",
                            "invalid streaming stdin inspection call shape",
                            expression.span,
                        ));
                    }
                    VerifierCallTarget::Ordinary(Some(VerifierFunctionSignature::Specialized {
                        params: op.params(),
                        return_type: op.ast_result(),
                        implicit_unique_ownership: false,
                    }))
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
                    let target = self.functions.get(name.as_str());
                    if target.is_none() {
                        self.diagnostics.push(hints::unknown_function(
                            self.program,
                            name,
                            self.functions,
                            expression.span,
                        ));
                    }
                    if target.is_some_and(|target| args.len() != target.payload().params.len()) {
                        let target = target.expect("checked above");
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T204",
                            format!(
                                "`{name}` expects {} arguments, received {}",
                                target.payload().params.len(),
                                args.len()
                            ),
                            expression.span,
                        ));
                    }
                    let specialized = target.and_then(|target| {
                        if target.payload().type_parameters.is_empty() {
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
                        let inferred_arguments;
                        let type_arguments = if type_arguments.is_empty() {
                            inferred_arguments = crate::source_verify::generic_inference::arguments_ref(
                                self.program,
                                self.current,
                                target,
                                args,
                                &self.scopes[scope].bindings,
                                self.functions,
                                self.types,
                            );
                            inferred_arguments.as_deref().unwrap_or(type_arguments)
                        } else {
                            type_arguments
                        };
                        if type_arguments.len() != target.payload().type_parameters.len() {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T225",
                                format!("generic function `{name}` expects {} explicit type arguments, received {}", target.payload().type_parameters.len(), type_arguments.len()),
                                expression.span,
                            ).with_help(hints::generic_call_help(name)));
                            return None;
                        }
                        if !generic_function_arguments_are_admitted(target.payload(), type_arguments, self.types)
                            && !generic_function_arguments_are_forwarded(
                                self.current.payload(),
                                target.payload(),
                                type_arguments,
                            )
                            && !crate::vec_ops::source_arguments_are_admitted_ref(
                                self.program,
                                &target,
                                type_arguments,
                            )
                            && !crate::box_ops::source_arguments_are_admitted_ref(self.program, &target, type_arguments)
                        {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T225",
                                format!("generic function `{name}` accepts only direct `i64` or `bool` type arguments, except admitted Copy scalars in an owned-record relay"),
                                expression.span,
                            ));
                            return None;
                        }
                        validation_specialize_signature(target.payload(), type_arguments).map(
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
                if let VerifierCallTarget::Ordinary(Some(VerifierFunctionSignature::Borrowed(
                    function,
                ))) = &target
                {
                    self.note_row_loan_arguments(function.payload(), args, scope);
                }
                let borrowed_bytes_loans = match &target {
                    VerifierCallTarget::Ordinary(Some(VerifierFunctionSignature::Borrowed(
                        function,
                    ))) => activate_borrowed_bytes_call_loans(
                        self.program,
                        args,
                        &function.payload().params,
                        &mut self.scopes[scope].bindings,
                        self.types,
                    ),
                    VerifierCallTarget::Ordinary(Some(
                        VerifierFunctionSignature::Specialized { params, .. },
                    )) if params.iter().any(|p| {
                        crate::source_verify::declared_type::collection_record::vector(
                            self.types, &p.ty,
                        )
                    }) =>
                    {
                        activate_borrowed_bytes_call_loans(
                            self.program,
                            args,
                            params,
                            &mut self.scopes[scope].bindings,
                            self.types,
                        )
                    }
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
                        VerifierCallTarget::LiteralFormat => {
                            CheckedValue::returned(Type::String, true)
                        }
                        VerifierCallTarget::Native(import) => {
                            let mut value = CheckedValue::returned(
                                import.result.value_type(),
                                import.result.is_owned(),
                            );
                            value.native_unit = import.result == ImportResult::Unit;
                            value
                        }
                        VerifierCallTarget::Byte(op) => {
                            let ty = op.ast_return_type();
                            let owns = self.types.needs_drop(&ty);
                            CheckedValue::returned(ty, owns)
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
                                operation => {
                                    match crate::command_io_ops::result_ownership(operation) {
                                        crate::hir::OwnershipMode::Own => ParamMode::Own,
                                        crate::hir::OwnershipMode::Borrow => ParamMode::Borrow,
                                        _ => ParamMode::Value,
                                    }
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
                self.recovery.fork(Some(scope), block_scope);
                self.scopes.push(VerifierScope {
                    bindings: self.scopes[scope].bindings.clone(),
                    local_borrow_count: self.scopes[scope].local_borrow_count,
                });
                if let Some(first_statement) = statements.first() {
                    if let Statement::Let {
                        name, name_span, ..
                    } = first_statement
                    {
                        if !source_identifier(name) {
                            self.diagnostics.push(hints::reserved_local_binding(
                                self.program,
                                name,
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
                    } else if let Statement::For {
                        item,
                        item_span,
                        values,
                        body,
                        ..
                    }
                    | Statement::ForBorrow {
                        item,
                        item_span,
                        values,
                        body,
                        ..
                    }
                    | Statement::ForOwn {
                        item,
                        item_span,
                        values,
                        body,
                        ..
                    } = first_statement
                    {
                        self.begin_for_statement(
                            expression,
                            statements,
                            tail,
                            scope,
                            block_scope,
                            0,
                            outer_names,
                            item,
                            *item_span,
                            values,
                            body,
                        );
                    } else {
                        self.check_string_replacement_entry(first_statement, block_scope);
                        self.note_owned_buffer_reopen(first_statement);
                        self.frames.push(VerifierFrame::ResumeBlockStatement {
                            expression,
                            statements,
                            tail,
                            parent_scope: scope,
                            block_scope,
                            index: 0,
                            outer_names,
                            recovery_checkpoint: self.recovery.checkpoint(self.diagnostics),
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
            // Resumable Effects v1 (issue #204). See
            // `iterative::calls::frame_resume_yield`.
            ExprKind::Yield { request } => {
                self.frames.push(VerifierFrame::ResumeYield {
                    expression,
                    request,
                });
                self.frames.push(VerifierFrame::Enter {
                    expression: request,
                    scope,
                });
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
                if let Some((root, value)) = crate::source_verify::record_traversal::project(
                    expression,
                    &self.scopes[scope].bindings,
                    self.types,
                ) {
                    let owner = Expr {
                        kind: ExprKind::Var(root),
                        span: expression.span,
                    };
                    if let Some(place) =
                        source_place(&owner, &self.scopes[scope].bindings, self.types)
                    {
                        check_source_place_availability(
                            self.program,
                            &place,
                            &self.scopes[scope].bindings,
                            expression.span,
                            self.diagnostics,
                        );
                    }
                    self.values.push(Some(value));
                    return Ok(());
                }
                let rooted_string_view = matches!(
                    self.frames.last(),
                    Some(VerifierFrame::ResumeCallArgument {
                        target: VerifierCallTarget::Byte(crate::byte_ops::ByteOp::StringAsStr),
                        ..
                    })
                ) && source_byte_view_place_is_admitted(
                    crate::byte_ops::ByteOp::StringAsStr,
                    expression,
                    &self.scopes[scope].bindings,
                    self.types,
                );
                if self.loop_depth != 0 && !rooted_string_view
                    && source_place(base, &self.scopes[scope].bindings, self.types).is_some_and(
                        |place| {
                            !crate::source_verify::declared_type::owned_leaf_collection::copy_or_leaf_admitted(
                                self.types, &place.ty,
                            ) && !crate::source_verify::declared_type::collection_record::admitted(&place.ty, self.types)
                        },
                    )
                {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "record field projection is not yet admitted in while bodies",
                        expression.span,
                    ));
                }
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
                if matches!(type_name.as_str(), "Map" | "Set") {
                    return Err(error(
                        self.program,
                        "SPX-T274",
                        "collections can only be created by map_new/set_new",
                        expression.span,
                    ));
                }
                if type_name == "Iter" {
                    return Err(error(
                        self.program,
                        "SPX-T290",
                        "Iter carriers can only be created by vec_into_iter",
                        expression.span,
                    ));
                }
                let instance = Type::Named {
                    name: type_name.clone(),
                    arguments: type_arguments.clone(),
                };
                let reported = self.diagnostics.len();
                check_declared_type(
                    self.program,
                    &instance,
                    expression.span,
                    self.types,
                    &HashSet::new(),
                    self.diagnostics,
                );
                let declared_fields = effective_record_fields(self.types, &instance);
                if declared_fields.is_none()
                    && !self.diagnostics[reported..]
                        .iter()
                        .any(|diagnostic| diagnostic.severity.is_error())
                {
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
                    let diagnostic = error(
                        self.program,
                        "SPX-T215",
                        format!("`{type_name}::{case_name}` is not a declared variant constructor"),
                        expression.span,
                    );
                    self.diagnostics.push(
                        match cases.and_then(|cases| {
                            crate::source_verify::hints::nearest_variant_case_name(case_name, cases)
                        }) {
                            Some(nearest) => diagnostic.with_help(format!(
                                "did you mean `{type_name}::{nearest} {{ ... }}`?"
                            )),
                            None => diagnostic,
                        },
                    );
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
                    } else if cases.is_some() {
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
                    checkpoint: self.recovery.checkpoint(self.diagnostics),
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
