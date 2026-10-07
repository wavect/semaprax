//! Optional private String intrinsic imports for aggregate Wasm profiles.
//!
//! Aggregate String carriers use the existing pointer-high/length-low Bytes
//! representation. These imports extend that carrier runtime only; they do
//! not expose a source capability or alter the internal-arena String ABI.

use super::*;

pub(super) const IMPORT_COUNT: u32 = 7;
pub(super) const CONCAT: u32 = 0;
pub(super) const FROM_CHAR: u32 = 1;
pub(super) const LEN_CHARS: u32 = 2;
pub(super) const FROM_I64: u32 = 3;
pub(super) const FROM_USIZE: u32 = 4;
pub(super) const STARTS_WITH: u32 = 5;
pub(super) const CONTAINS: u32 = 6;

pub(super) fn requires_runtime(operation: crate::string_ops::StringOp) -> bool {
    matches!(
        operation,
        crate::string_ops::StringOp::Concat
            | crate::string_ops::StringOp::FromChar
            | crate::string_ops::StringOp::LenChars
            | crate::string_ops::StringOp::FromI64
            | crate::string_ops::StringOp::FromUsize
            | crate::string_ops::StringOp::StartsWith
            | crate::string_ops::StringOp::Contains
    )
}

pub(super) fn program_uses_runtime(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            expression_uses_runtime(&function.body)
                || function.requires.iter().any(expression_uses_runtime)
                || function.ensures.iter().any(expression_uses_runtime)
        })
}

fn expression_uses_runtime(expression: &ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str()).is_some_and(requires_runtime) {
                return true;
            }
        }
        if let ResolvedExprKind::Binary { op, left, .. } = &expression.kind {
            if matches!(op, BinaryOp::Eq | BinaryOp::Ne) && left.ty == ResolvedType::String {
                return true;
            }
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

pub(super) fn import_name(offset: u32) -> Option<&'static str> {
    match offset {
        CONCAT => Some("spx_string_concat_v1"),
        FROM_CHAR => Some("spx_string_from_char_v1"),
        LEN_CHARS => Some("spx_string_len_chars_v1"),
        FROM_I64 => Some("spx_string_from_i64_v1"),
        FROM_USIZE => Some("spx_string_from_usize_v1"),
        STARTS_WITH => Some("spx_string_starts_with_v1"),
        CONTAINS => Some("spx_string_contains_v1"),
        _ => None,
    }
}

pub(super) fn import_offset(operation: crate::string_ops::StringOp) -> Option<u32> {
    match operation {
        crate::string_ops::StringOp::Concat => Some(CONCAT),
        crate::string_ops::StringOp::FromChar => Some(FROM_CHAR),
        crate::string_ops::StringOp::LenChars => Some(LEN_CHARS),
        crate::string_ops::StringOp::FromI64 => Some(FROM_I64),
        crate::string_ops::StringOp::FromUsize => Some(FROM_USIZE),
        crate::string_ops::StringOp::StartsWith => Some(STARTS_WITH),
        crate::string_ops::StringOp::Contains => Some(CONTAINS),
        _ => None,
    }
}

pub(super) fn emit_imports(
    output: &mut Vec<u8>,
    binary: u32,
    unary: u32,
    from_char: u32,
    text_binary: u32,
) {
    for offset in 0..IMPORT_COUNT {
        let ty = if offset == FROM_CHAR {
            from_char
        } else if offset == CONCAT {
            binary
        } else if offset == STARTS_WITH || offset == CONTAINS {
            text_binary
        } else {
            unary
        };
        function_import(
            output,
            "env",
            import_name(offset).expect("String runtime import offset is bounded"),
            ty,
        );
    }
}

pub(super) fn insert_function_indexes(indexes: &mut HashMap<FunctionExecutionId, u32>, base: u32) {
    for operation in crate::string_ops::StringOp::ALL {
        if let Some(offset) = import_offset(operation) {
            indexes.insert(
                FunctionExecutionId::Monomorphic(DeclarationId::new(operation.id())),
                base + offset,
            );
        }
    }
}

impl Emitter<'_> {
    pub(super) fn emit_aggregate_string_equality(
        &mut self,
        op: BinaryOp,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        require_type(value_type(right), &ResolvedType::String, "String equality")?;
        self.get_scalar(left);
        self.output.push(0xa7); // i32.wrap_i64: left byte length
        self.get_scalar(right);
        self.output.extend([0xa7, 0x47, 0x04, I32]); // length !=; if (result i32)
        self.control_depth += 1;
        self.output.extend([0x41, 0x00]); // unequal lengths cannot have equal text
        self.output.push(0x05);
        self.get_scalar(left);
        self.get_scalar(right);
        let runtime = self
            .function_indexes
            .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                crate::string_ops::STARTS_WITH_ID,
            )))
            .copied()
            .ok_or_else(|| error("aggregate String starts-with runtime import is not indexed"))?;
        self.output.push(0x10);
        write_u32(self.output, runtime);
        self.output.push(0x0b);
        self.control_depth -= 1;
        if op == BinaryOp::Ne {
            self.output.push(0x45); // i32.eqz
        }
        self.output.push(0x21);
        write_u32(self.output, destination);
        Ok(())
    }

    pub(super) fn emit_aggregate_string_operation(
        &mut self,
        expr: &ResolvedExpr,
        operation: crate::string_ops::StringOp,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        use crate::string_ops::StringOp;
        if operation.is_wasm_refused() {
            return Err(crate::string_ops::text_toolkit_wasm_refusal(operation));
        }
        if args.len() != operation.arity() {
            return Err(error("String operation arity disagrees with resolved HIR"));
        }
        require_type(
            &expr.ty,
            &operation.return_type(),
            "String operation result",
        )?;
        let mut values = Vec::with_capacity(args.len());
        for (parameter_index, (argument, expected)) in
            args.iter().zip(operation.param_types()).enumerate()
        {
            let value = self.emit_expr(argument)?;
            require_type(value_type(&value), expected, "String operation argument")?;
            if operation.consumes_arguments() {
                let epoch = crate::cleanup_plan::StorageId::CallArgument {
                    call: expr.id.clone(),
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| error("String operation argument index overflows u32"))?,
                    value_expression: argument.id.clone(),
                };
                let carrier = self
                    .plan
                    .cleanup_call_argument_carriers
                    .get(&epoch)
                    .copied()
                    .ok_or_else(|| {
                        error("consuming aggregate String operation has no canonical call epoch")
                    })?;
                values.push(Value::Scalar {
                    local: carrier,
                    ty: expected.clone(),
                });
            } else {
                values.push(value);
            }
        }
        self.apply_call_commit(&expr.id)?;
        let destination = self.plan.expr_scalar(expr)?;
        match operation {
            // Aggregate String carriers use the existing Bytes runtime's
            // pointer-high/byte-length-low representation. These borrowed
            // operations therefore require no second owner runtime.
            StringOp::I64FromU8 => {
                self.get_scalar(&values[0]);
                self.output.push(0xad); // i64.extend_i32_u
            }
            StringOp::Len => {
                self.get_scalar(&values[0]);
                self.output.extend([0xa7, 0xad]); // i32.wrap_i64; i64.extend_i32_u
            }
            StringOp::IsEmpty => {
                self.get_scalar(&values[0]);
                self.output.extend([0xa7, 0x45]); // i32.wrap_i64; i32.eqz
            }
            StringOp::StartsWith | StringOp::Contains => {
                self.get_scalar(&values[0]);
                self.get_scalar(&values[1]);
                let runtime = self
                    .function_indexes
                    .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                        operation.id(),
                    )))
                    .copied()
                    .ok_or_else(|| error("aggregate String text runtime import is not indexed"))?;
                self.output.push(0x10);
                write_u32(self.output, runtime);
            }
            StringOp::Concat
            | StringOp::LenChars
            | StringOp::FromChar
            | StringOp::FromI64
            | StringOp::FromUsize => {
                for value in &values {
                    self.get_scalar(value);
                }
                let offset = import_offset(operation).ok_or_else(|| {
                    error("aggregate String runtime operation has no import offset")
                })?;
                let runtime = self
                    .function_indexes
                    .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                        operation.id(),
                    )))
                    .copied()
                    .ok_or_else(|| {
                        error(format!(
                            "aggregate String runtime import {} is not indexed",
                            offset
                        ))
                    })?;
                self.output.push(0x10);
                write_u32(self.output, runtime);
            }
            _ => return Err(crate::string_ops::text_toolkit_wasm_refusal(operation)),
        }
        self.output.push(0x21);
        write_u32(self.output, destination);
        if operation == StringOp::Concat {
            for value in &values {
                self.drop_internal_string(value)?;
            }
        }
        Ok(Value::Scalar {
            local: destination,
            ty: expr.ty.clone(),
        })
    }
}
