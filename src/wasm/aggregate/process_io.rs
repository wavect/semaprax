//! Checked single-owner process request/result lowering; no host launch authority here.
use super::super::process_io as boundary;
use super::*;
use crate::process_ops as ops;

impl Emitter<'_> {
    fn process_get(&mut self, local: u32) {
        self.output.push(0x20);
        write_u32(self.output, local);
    }
    fn process_set(&mut self, local: u32) {
        self.output.push(0x21);
        write_u32(self.output, local);
    }
    fn process_integer(&mut self, value: u64) {
        self.output.push(0x42);
        write_i64(self.output, value as i64);
    }
    fn process_failure_if(
        &mut self,
        expr: &ExpressionId,
        code: i32,
        owned: Option<u32>,
    ) -> Result<(), Diagnostic> {
        self.output.extend([0x04, 0x40]);
        if let Some(carrier) = owned {
            self.process_get(carrier);
            self.output.push(0x10);
            write_u32(self.output, BYTE_DROP_IMPORT);
        }
        self.output.push(0x41);
        write_i64(self.output, code as i64);
        self.process_set(self.plan.status);
        if code == STATUS_INTERNAL_INVALID_TAG {
            let plan = self.cleanup_plan.clone();
            self.emit_success_cleanup(&plan)?;
        } else {
            self.process_get(self.plan.status);
            self.output.push(0x24);
            write_u32(self.output, boundary::STATUS_GLOBAL);
            self.emit_failure_cleanup(expr, StatusLane::OperationFailure)?;
        }
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        Ok(())
    }
    /// Read a little-endian word through the authenticated byte provider, never raw linear memory.
    #[allow(clippy::too_many_arguments)]
    fn process_word(
        &mut self,
        carrier: u32,
        offset: Option<u32>,
        fixed: u64,
        bytes: u64,
        destination: u32,
        expr: &ExpressionId,
        owned: Option<u32>,
    ) -> Result<(), Diagnostic> {
        let byte = self
            .plan
            .command_byte
            .ok_or_else(|| error("process byte scratch absent"))?;
        self.process_integer(0);
        self.process_set(destination);
        for i in 0..bytes {
            self.process_get(carrier);
            if let Some(offset) = offset {
                self.process_get(offset);
                self.process_integer(fixed + i);
                self.output.push(0x7c);
            } else {
                self.process_integer(fixed + i);
            }
            self.output.push(0x10);
            write_u32(self.output, BYTE_GET_IMPORT);
            self.output.push(0x22);
            write_u32(self.output, byte);
            self.output.push(0x41);
            write_i64(self.output, 255);
            self.output.push(0x4b);
            self.process_failure_if(expr, STATUS_INTERNAL_INVALID_TAG, owned)?;
            self.process_get(destination);
            self.process_get(byte);
            self.output.push(0xad);
            self.process_integer(i * 8);
            self.output.extend([0x86, 0x84]);
            self.process_set(destination);
        }
        Ok(())
    }
    pub(super) fn emit_process_command_call(
        &mut self,
        expr: &ResolvedExpr,
        args: &[Value],
        local: u32,
        pointer: Pointer,
    ) -> Result<Value, Diagnostic> {
        if args.len() != 8 {
            return Err(error("process request needs eight arguments"));
        }
        let [offset, count, remaining, length, word, capacity] = self
            .plan
            .process_scan
            .ok_or_else(|| error("process scan locals absent"))?;
        for (i, value) in args.iter().enumerate() {
            self.require_scalar(
                value,
                if matches!(i, 1 | 3) {
                    &ResolvedType::SliceU8
                } else {
                    &ResolvedType::Usize
                },
                "process argument",
            )?;
        }
        self.validate_byte_slice(&args[1]);
        self.validate_byte_slice(&args[3]);
        // Match ProcessRequest::from_wire ordering before any reservation or host entry.
        for (slice, length_arg) in [(1, 2), (3, 4)] {
            self.get_scalar(&args[length_arg]);
            self.get_scalar(&args[slice]);
            self.output.extend([0xa7, 0xad, 0x56]);
            self.process_failure_if(&expr.id, ops::INVALID_INPUT as i32, None)?;
        }
        self.get_scalar(&args[5]);
        self.output.push(0x50);
        self.get_scalar(&args[5]);
        self.process_integer(ops::MAX_WAIT_MILLIS);
        self.output.extend([0x56, 0x72]);
        self.process_failure_if(&expr.id, ops::INVALID_INPUT as i32, None)?;
        self.get_scalar(&args[2]);
        self.get_scalar(&args[4]);
        self.output.push(0x7c);
        self.process_integer(ops::MAX_INPUT_BYTES);
        self.output.push(0x56);
        self.process_failure_if(&expr.id, ops::CAPACITY_EXCEEDED as i32, None)?;
        for i in [6, 7] {
            self.get_scalar(&args[i]);
            self.process_integer(ops::MAX_OUTPUT_BYTES - ops::HEADER_BYTES);
            self.output.push(0x56);
            self.process_failure_if(&expr.id, ops::CAPACITY_EXCEEDED as i32, None)?;
        }
        self.get_scalar(&args[6]);
        self.get_scalar(&args[7]);
        self.output.push(0x7c);
        self.process_integer(ops::MAX_OUTPUT_BYTES - ops::HEADER_BYTES);
        self.output.push(0x56);
        self.process_failure_if(&expr.id, ops::CAPACITY_EXCEEDED as i32, None)?;
        self.get_scalar(&args[2]);
        self.process_integer(4);
        self.output.push(0x54);
        self.process_failure_if(&expr.id, ops::INVALID_INPUT as i32, None)?;
        self.get_scalar(&args[1]);
        self.process_set(local);
        self.process_word(local, None, 0, 4, count, &expr.id, None)?;
        self.process_get(count);
        self.process_integer(ops::MAX_ARGUMENTS);
        self.output.push(0x56);
        self.process_failure_if(&expr.id, ops::CAPACITY_EXCEEDED as i32, None)?;
        self.process_get(count);
        self.process_set(remaining);
        self.process_integer(4);
        self.process_set(offset);
        self.output.extend([0x02, 0x40, 0x03, 0x40]);
        self.control_depth += 2;
        self.process_get(remaining);
        self.output.extend([0x50, 0x0d, 1]);
        self.get_scalar(&args[2]);
        self.process_get(offset);
        self.output.push(0x7d);
        self.process_integer(4);
        self.output.push(0x54);
        self.process_failure_if(&expr.id, ops::INVALID_INPUT as i32, None)?;
        self.process_word(local, Some(offset), 0, 4, length, &expr.id, None)?;
        self.process_get(offset);
        self.process_integer(4);
        self.output.push(0x7c);
        self.process_set(offset);
        self.process_get(length);
        self.get_scalar(&args[2]);
        self.process_get(offset);
        self.output.extend([0x7d, 0x56]);
        self.process_failure_if(&expr.id, ops::INVALID_INPUT as i32, None)?;
        self.output.extend([0x02, 0x40, 0x03, 0x40]);
        self.control_depth += 2;
        self.process_get(length);
        self.output.extend([0x50, 0x0d, 1]);
        self.process_word(local, Some(offset), 0, 1, word, &expr.id, None)?;
        self.process_get(word);
        self.output.push(0x50);
        self.process_failure_if(&expr.id, ops::INVALID_INPUT as i32, None)?;
        self.process_get(offset);
        self.process_integer(1);
        self.output.push(0x7c);
        self.process_set(offset);
        self.process_get(length);
        self.process_integer(1);
        self.output.push(0x7d);
        self.process_set(length);
        self.output.extend([0x0c, 0, 0x0b, 0x0b]);
        self.control_depth -= 2;
        self.process_get(remaining);
        self.process_integer(1);
        self.output.push(0x7d);
        self.process_set(remaining);
        self.output.extend([0x0c, 0, 0x0b, 0x0b]);
        self.control_depth -= 2;
        self.process_get(offset);
        self.get_scalar(&args[2]);
        self.output.push(0x52);
        self.process_failure_if(&expr.id, ops::INVALID_INPUT as i32, None)?;
        self.get_scalar(&args[6]);
        self.get_scalar(&args[7]);
        self.output.push(0x7c);
        self.process_integer(ops::HEADER_BYTES);
        self.output.push(0x7c);
        self.process_set(capacity);
        self.process_get(capacity);
        self.get_scalar(&args[2]);
        self.output.push(0x7c);
        self.get_scalar(&args[4]);
        self.output.push(0x7c);
        self.process_set(word);
        self.output.push(0x23);
        write_u32(self.output, boundary::RUN_COUNT_GLOBAL);
        self.process_integer(ops::MAX_OPERATIONS);
        self.output.push(0x5a);
        self.process_failure_if(&expr.id, ops::CAPACITY_EXCEEDED as i32, None)?;
        self.output.push(0x23);
        write_u32(self.output, boundary::BYTE_COUNT_GLOBAL);
        self.process_get(word);
        self.output.push(0x7c);
        self.process_integer(ops::MAX_TOTAL_BYTES);
        self.output.push(0x56);
        self.process_failure_if(&expr.id, ops::CAPACITY_EXCEEDED as i32, None)?;
        self.output.push(0x23);
        write_u32(self.output, boundary::BYTE_COUNT_GLOBAL);
        self.process_get(word);
        self.output.extend([0x7c, 0x24]);
        write_u32(self.output, boundary::BYTE_COUNT_GLOBAL);
        self.output.push(0x23);
        write_u32(self.output, boundary::RUN_COUNT_GLOBAL);
        self.process_integer(1);
        self.output.extend([0x7c, 0x24]);
        write_u32(self.output, boundary::RUN_COUNT_GLOBAL);
        for value in args {
            self.get_scalar(value);
        }
        self.emit_pointer(pointer);
        self.output.push(0x10);
        write_u32(self.output, boundary::RUN_IMPORT);
        self.process_set(self.plan.status);
        self.process_get(self.plan.status);
        self.output.extend([0x41, 7, 0x4b]);
        self.process_failure_if(&expr.id, STATUS_INTERNAL_INVALID_TAG, None)?;
        self.process_get(self.plan.status);
        self.output.extend([0x04, 0x40]);
        self.process_get(self.plan.status);
        self.output.push(0x24);
        write_u32(self.output, boundary::STATUS_GLOBAL);
        self.emit_failure_cleanup(&expr.id, StatusLane::OperationFailure)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        self.emit_pointer(pointer);
        self.load_scalar(&ResolvedType::Bytes);
        self.process_set(local);
        self.process_get(local);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
        write_i64(self.output, i32::MIN as i64);
        self.output.extend([0x71, 0x45]);
        self.process_get(local);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
        write_i64(self.output, 0x7fff_ffff);
        self.output.extend([0x71, 0x45, 0x72]);
        self.process_failure_if(&expr.id, STATUS_INTERNAL_INVALID_TAG, None)?;
        self.process_get(local);
        self.output.push(0x10);
        write_u32(
            self.output,
            super::super::command_io::OWNED_BYTES_VALIDATE_IMPORT,
        );
        self.process_failure_if(&expr.id, STATUS_INTERNAL_INVALID_TAG, None)?;
        self.process_get(local);
        self.output.extend([0xa7, 0xad]);
        self.process_get(capacity);
        self.output.push(0x56);
        self.process_failure_if(&expr.id, ops::CAPACITY_EXCEEDED as i32, Some(local))?;
        self.process_get(local);
        self.output.extend([0xa7, 0xad]);
        self.process_integer(32);
        self.output.push(0x54);
        self.process_failure_if(&expr.id, ops::IO_FAILURE as i32, Some(local))?;
        for (offset, destination) in [(0, offset), (8, count), (16, remaining), (24, length)] {
            self.process_word(local, None, offset, 8, destination, &expr.id, Some(local))?;
        }
        self.process_get(offset);
        self.process_integer(1);
        self.output.push(0x52);
        self.process_failure_if(&expr.id, ops::IO_FAILURE as i32, Some(local))?;
        // !(kind0/u32 code || kind1/nonzero byte signal)
        self.process_get(count);
        self.process_integer(3);
        self.output.extend([0x83, 0x50]);
        self.process_get(count);
        self.process_integer(2);
        self.output.push(0x88);
        self.process_integer(u32::MAX as u64);
        self.output.extend([0x58, 0x71]);
        self.process_get(count);
        self.process_integer(3);
        self.output.push(0x83);
        self.process_integer(1);
        self.output.push(0x51);
        self.process_get(count);
        self.process_integer(2);
        self.output.push(0x88);
        self.process_integer(1);
        self.output.extend([0x5a, 0x71]);
        self.process_get(count);
        self.process_integer(2);
        self.output.push(0x88);
        self.process_integer(255);
        self.output.extend([0x58, 0x71, 0x72, 0x45]);
        self.process_failure_if(&expr.id, ops::IO_FAILURE as i32, Some(local))?;
        for (len, max) in [(remaining, 6), (length, 7)] {
            self.process_get(len);
            self.get_scalar(&args[max]);
            self.output.push(0x56);
            self.process_failure_if(&expr.id, ops::CAPACITY_EXCEEDED as i32, Some(local))?;
        }
        self.process_get(remaining);
        self.process_get(length);
        self.output.push(0x7c);
        self.process_integer(32);
        self.output.push(0x7c);
        self.process_get(local);
        self.output.extend([0xa7, 0xad, 0x52]);
        self.process_failure_if(&expr.id, ops::IO_FAILURE as i32, Some(local))?;
        Ok(Value::Scalar {
            local,
            ty: ResolvedType::Bytes,
        })
    }
}
