//! Core-Wasm lowering for the internal owned bounded byte-buffer operations.

use super::*;

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
}
