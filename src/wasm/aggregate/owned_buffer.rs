//! Core-Wasm lowering for the internal owned bounded byte-buffer operations.

use super::*;

pub(super) struct ImportTypes {
    pub(super) set: Option<u32>,
    pub(super) set5: Option<u32>,
    pub(super) set1_or5: Option<u32>,
    pub(super) set1_or6_or48: Option<u32>,
}

pub(super) fn import_types(
    uses_owned_buffer: bool,
    types: &mut Vec<Signature>,
    type_indexes: &mut HashMap<Signature, u32>,
) -> ImportTypes {
    let set = uses_owned_buffer.then(|| {
        intern_type(
            Signature {
                params: vec![I64, I64, I32],
                results: vec![I64],
            },
            types,
            type_indexes,
        )
    });
    let set5 = uses_owned_buffer.then(|| {
        intern_type(
            Signature {
                params: vec![I64, I64, I32, I32, I32, I32, I32],
                results: vec![I64],
            },
            types,
            type_indexes,
        )
    });
    let set1_or5 = uses_owned_buffer.then(|| {
        intern_type(
            Signature {
                params: vec![I64, I64, I32, I64, I64],
                results: vec![I64],
            },
            types,
            type_indexes,
        )
    });
    let set1_or6_or48 = uses_owned_buffer.then(|| {
        intern_type(
            Signature {
                params: vec![I64, I64, I32, I64, I64],
                results: vec![I64],
            },
            types,
            type_indexes,
        )
    });
    ImportTypes {
        set,
        set5,
        set1_or5,
        set1_or6_or48,
    }
}

pub(super) fn emit_imports(imports: &mut Vec<u8>, byte_unary: u32, types: &ImportTypes) {
    function_import(imports, "env", "spx_bytes_zeroed", byte_unary);
    for (name, ty) in [
        ("spx_bytes_set", types.set),
        ("spx_bytes_set5", types.set5),
        ("spx_bytes_set1_or5", types.set1_or5),
        ("spx_bytes_set1_or6_or48", types.set1_or6_or48),
    ] {
        function_import(imports, "env", name, ty.expect("owned buffer import type"));
    }
}

impl<'a> Emitter<'a> {
    pub(super) fn emit_owned_buffer_set5(
        &mut self,
        expr: &ResolvedExpr,
        values: &[Value],
    ) -> Result<Value, Diagnostic> {
        let local = self.plan.expr_scalar(expr)?;
        for value in values {
            self.get_scalar(value);
        }
        self.output.push(0x10);
        write_u32(
            self.output,
            self.function_indexes
                .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                    crate::byte_ops::SET5_ID,
                )))
                .copied()
                .unwrap_or(BYTE_SET5_IMPORT),
        );
        self.output.push(0x21);
        write_u32(self.output, local);
        Ok(Value::Scalar {
            local,
            ty: ResolvedType::Bytes,
        })
    }

    pub(super) fn emit_owned_buffer_set1_or5(
        &mut self,
        expr: &ResolvedExpr,
        values: &[Value],
    ) -> Result<Value, Diagnostic> {
        let local = self.plan.expr_scalar(expr)?;
        for value in values {
            self.get_scalar(value);
        }
        self.output.push(0x10);
        write_u32(
            self.output,
            self.function_indexes
                .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                    crate::byte_ops::SET1_OR5_ID,
                )))
                .copied()
                .unwrap_or(BYTE_SET1_OR5_IMPORT),
        );
        self.output.push(0x21);
        write_u32(self.output, local);
        Ok(Value::Scalar {
            local,
            ty: ResolvedType::Bytes,
        })
    }

    pub(super) fn emit_owned_buffer_set1_or6_or48(
        &mut self,
        expr: &ResolvedExpr,
        values: &[Value],
    ) -> Result<Value, Diagnostic> {
        let local = self.plan.expr_scalar(expr)?;
        for value in values {
            self.get_scalar(value);
        }
        self.output.push(0x10);
        write_u32(
            self.output,
            self.function_indexes
                .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                    crate::byte_ops::SET1_OR6_OR48_ID,
                )))
                .copied()
                .unwrap_or(BYTE_SET1_OR6_OR48_IMPORT),
        );
        self.output.push(0x21);
        write_u32(self.output, local);
        Ok(Value::Scalar {
            local,
            ty: ResolvedType::Bytes,
        })
    }

    /// Select the one-byte or five-byte destination failure from the selector
    /// tag before the owner transfer commits.
    pub(super) fn emit_owned_buffer_set1_or5_failure(
        &mut self,
        expression: &ExpressionId,
        buffer: &Value,
        index: &Value,
        selector: &Value,
    ) -> Result<(), Diagnostic> {
        self.get_scalar(selector);
        self.output.push(0x42); // i64.const
        write_i64(self.output, i64::MIN);
        self.output.extend([0x83, 0x50, 0x45]); // i64.and, i64.eqz, i32.eqz
        self.output.extend([0x04, 0x40]);
        // Nested bounds failures must branch past this selector to the status exit.
        self.control_depth += 1;
        self.emit_owned_buffer_set5_failure(expression, buffer, index)?;
        self.output.push(0x05);
        self.emit_owned_buffer_index_failure(expression, buffer, index)?;
        self.control_depth -= 1;
        self.output.push(0x0b);
        Ok(())
    }

    pub(super) fn emit_owned_buffer_set1_or6_or48_failure(
        &mut self,
        expression: &ExpressionId,
        buffer: &Value,
        index: &Value,
        selector: &Value,
    ) -> Result<(), Diagnostic> {
        self.get_scalar(selector);
        self.output.push(0x42);
        write_i64(self.output, i64::MIN);
        self.output.extend([0x83, 0x50, 0x45]);
        self.output.extend([0x04, 0x40]);
        self.control_depth += 1;
        self.get_scalar(selector);
        self.output.push(0x42);
        write_i64(self.output, 1_i64 << 62);
        self.output.extend([0x83, 0x50, 0x45]);
        self.output.extend([0x04, 0x40]);
        self.control_depth += 1;
        self.emit_owned_buffer_set48_failure(expression, buffer, index)?;
        self.output.push(0x05);
        self.emit_owned_buffer_set6_failure(expression, buffer, index)?;
        self.control_depth -= 1;
        self.output.push(0x0b);
        self.output.push(0x05);
        self.emit_owned_buffer_index_failure(expression, buffer, index)?;
        self.control_depth -= 1;
        self.output.push(0x0b);
        Ok(())
    }

    /// Select the single owned-buffer failure when the computed element index
    /// is at or above the transferred buffer's length. The carrier's low word
    /// is its byte length, which is the same predicate the reference
    /// interpreter and the native backend apply.
    pub(super) fn emit_owned_buffer_index_failure(
        &mut self,
        expression: &ExpressionId,
        buffer: &Value,
        index: &Value,
    ) -> Result<(), Diagnostic> {
        self.get_scalar(index);
        self.get_scalar(buffer);
        self.output.extend([0xa7, 0xad, 0x5a]); // index >= carrier length
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(
            self.output,
            i64::from(STATUS_BYTE_BUFFER_INDEX_OUT_OF_BOUNDS),
        );
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.emit_failure_cleanup(expression, StatusLane::OperationFailure)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        Ok(())
    }

    /// Select the same owned-buffer failure when a five-byte interval does
    /// not wholly fit. The first predicate preserves failure when subtraction
    /// wraps for `index > length`, so no overflowing endpoint is admitted.
    pub(super) fn emit_owned_buffer_set5_failure(
        &mut self,
        expression: &ExpressionId,
        buffer: &Value,
        index: &Value,
    ) -> Result<(), Diagnostic> {
        self.get_scalar(index);
        self.get_scalar(buffer);
        self.output.extend([0xa7, 0xad, 0x56]); // index > carrier length
        self.get_scalar(buffer);
        self.output.extend([0xa7, 0xad]); // widen carrier length to i64
        self.get_scalar(index);
        self.output.push(0x7d); // carrier length - index
        self.output.push(0x42);
        write_i64(self.output, 5);
        self.output.push(0x54); // remaining < five
        self.output.push(0x72); // either condition selects the failure
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(
            self.output,
            i64::from(STATUS_BYTE_BUFFER_INDEX_OUT_OF_BOUNDS),
        );
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.emit_failure_cleanup(expression, StatusLane::OperationFailure)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        Ok(())
    }

    pub(super) fn emit_owned_buffer_set6_failure(
        &mut self,
        expression: &ExpressionId,
        buffer: &Value,
        index: &Value,
    ) -> Result<(), Diagnostic> {
        self.get_scalar(index);
        self.get_scalar(buffer);
        self.output.extend([0xa7, 0xad, 0x56]);
        self.get_scalar(buffer);
        self.output.extend([0xa7, 0xad]);
        self.get_scalar(index);
        self.output.push(0x7d);
        self.output.push(0x42);
        write_i64(self.output, 6);
        self.output.push(0x54);
        self.output.push(0x72);
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(
            self.output,
            i64::from(STATUS_BYTE_BUFFER_INDEX_OUT_OF_BOUNDS),
        );
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.emit_failure_cleanup(expression, StatusLane::OperationFailure)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        Ok(())
    }

    pub(super) fn emit_owned_buffer_set48_failure(
        &mut self,
        expression: &ExpressionId,
        buffer: &Value,
        index: &Value,
    ) -> Result<(), Diagnostic> {
        self.get_scalar(index);
        self.get_scalar(buffer);
        self.output.extend([0xa7, 0xad, 0x56]);
        self.get_scalar(buffer);
        self.output.extend([0xa7, 0xad]);
        self.get_scalar(index);
        self.output.push(0x7d);
        self.output.push(0x42);
        write_i64(self.output, 48);
        self.output.push(0x54);
        self.output.push(0x72);
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(
            self.output,
            i64::from(STATUS_BYTE_BUFFER_INDEX_OUT_OF_BOUNDS),
        );
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.emit_failure_cleanup(expression, StatusLane::OperationFailure)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        Ok(())
    }
}
