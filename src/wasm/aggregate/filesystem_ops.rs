//! Filesystem I/O v1 lowering for the aggregate Wasm emitter.

use super::super::filesystem_ops as boundary;
use super::*;

const INVALID_PATH: i32 = crate::filesystem_ops::INVALID_PATH as i32;
const CAPACITY_EXCEEDED: i32 = crate::filesystem_ops::CAPACITY_EXCEEDED as i32;
const LAST_STATUS: i32 = crate::filesystem_ops::INVALID_FILE_TYPE as i32;

impl Emitter<'_> {
    pub(super) fn emit_filesystem_command_call(
        &mut self,
        expr: &ResolvedExpr,
        call: &crate::hir::ResolvedHostCommandCall,
        arguments: &[Value],
        local: u32,
        pointer: Pointer,
    ) -> Result<Value, Diagnostic> {
        use crate::hir::ResolvedHostCommandOperation as Op;
        self.require_scalar(&arguments[0], &ResolvedType::SliceU8, "filesystem path")?;
        self.require_scalar(
            &arguments[1],
            &ResolvedType::Usize,
            "filesystem path length",
        )?;
        match call.operation {
            Op::FileRead => {
                self.require_scalar(&arguments[2], &ResolvedType::Usize, "file_read max")?;
                self.reserve_filesystem_work(&arguments[2], &expr.id)?;
            }
            Op::FileWriteNew => {
                self.require_scalar(&arguments[2], &ResolvedType::SliceU8, "file_write_new data")?;
                self.require_scalar(
                    &arguments[3],
                    &ResolvedType::Usize,
                    "file_write_new data length",
                )?;
                self.reserve_filesystem_work(&arguments[3], &expr.id)?;
            }
            _ => {
                return Err(error(
                    "non-filesystem operation reached filesystem lowering",
                ))
            }
        }
        self.stage_slice_carrier(&arguments[0], local);
        self.get_scalar(&arguments[1]);
        self.output.push(0x50); // i64.eqz
        self.emit_filesystem_failure_if_code(&expr.id, INVALID_PATH)?;
        self.get_scalar(&arguments[1]);
        self.emit_carrier_length(local);
        self.output.push(0x56);
        self.emit_filesystem_failure_if_code(&expr.id, INVALID_PATH)?;
        self.get_scalar(&arguments[1]);
        self.output.push(0x42);
        write_i64(self.output, crate::filesystem_ops::MAX_PATH_BYTES as i64);
        self.output.push(0x56);
        self.emit_filesystem_failure_if_code(&expr.id, INVALID_PATH)?;
        self.emit_filesystem_path_validation(local, &arguments[1], &expr.id)?;

        match call.operation {
            Op::FileRead => {
                self.get_scalar(&arguments[2]);
                self.output.push(0x42);
                write_i64(self.output, crate::filesystem_ops::MAX_FILE_BYTES as i64);
                self.output.push(0x56);
                self.emit_filesystem_failure_if_code(&expr.id, CAPACITY_EXCEEDED)?;
                self.emit_carrier_root_and_length(local);
                self.get_scalar(&arguments[1]);
                self.output.push(0xa7);
                self.get_scalar(&arguments[2]);
                self.output.push(0xa7);
                self.emit_pointer(pointer);
                self.output.push(0x10);
                write_u32(self.output, boundary::READ_IMPORT);
            }
            Op::FileWriteNew => {
                self.stage_slice_carrier(&arguments[2], local);
                self.get_scalar(&arguments[3]);
                self.emit_carrier_length(local);
                self.output.push(0x56);
                self.emit_filesystem_failure_if_code(&expr.id, CAPACITY_EXCEEDED)?;
                self.get_scalar(&arguments[3]);
                self.output.push(0x42);
                write_i64(self.output, crate::filesystem_ops::MAX_FILE_BYTES as i64);
                self.output.push(0x56);
                self.emit_filesystem_failure_if_code(&expr.id, CAPACITY_EXCEEDED)?;
                self.stage_slice_carrier(&arguments[0], local);
                self.emit_carrier_root_and_length(local);
                self.get_scalar(&arguments[1]);
                self.output.push(0xa7);
                self.stage_slice_carrier(&arguments[2], local);
                self.emit_carrier_root_and_length(local);
                self.get_scalar(&arguments[3]);
                self.output.push(0xa7);
                self.emit_pointer(pointer);
                self.output.push(0x10);
                write_u32(self.output, boundary::WRITE_NEW_IMPORT);
            }
            _ => {
                return Err(error(
                    "non-filesystem operation reached filesystem lowering",
                ))
            }
        }
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.push(0x41);
        write_i64(self.output, i64::from(LAST_STATUS));
        self.output.push(0x4b);
        self.emit_command_failure_if(
            &expr.id,
            super::super::host_output::COMMAND_STDOUT_GLOBALS,
            super::super::host_output::COMMAND_STDERR_GLOBALS,
        )?;
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.emit_filesystem_failure_if(&expr.id)?;
        if call.operation == Op::FileRead {
            self.emit_load_out(pointer);
            self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
            write_i64(self.output, i64::from(i32::MIN));
            self.output.extend([0x71, 0x45]);
            self.emit_load_out(pointer);
            self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
            write_i64(self.output, i64::from(0x7fff_ffff_u32));
            self.output.extend([0x71, 0x45, 0x72]);
            self.emit_command_failure_if(
                &expr.id,
                super::super::host_output::COMMAND_STDOUT_GLOBALS,
                super::super::host_output::COMMAND_STDERR_GLOBALS,
            )?;
            self.emit_load_out(pointer);
            self.output.push(0x10);
            write_u32(
                self.output,
                super::super::command_io::OWNED_BYTES_VALIDATE_IMPORT,
            );
            self.emit_command_failure_if(
                &expr.id,
                super::super::host_output::COMMAND_STDOUT_GLOBALS,
                super::super::host_output::COMMAND_STDERR_GLOBALS,
            )?;
            self.emit_load_out(pointer);
            self.output.push(0xa7);
            self.get_scalar(&arguments[2]);
            self.output.extend([0xa7, 0x4b]);
            self.output.extend([0x04, 0x40]);
            // status_exit_extra_depth already includes this failure branch.
            self.emit_load_out(pointer);
            self.output.push(0x10);
            write_u32(self.output, BYTE_DROP_IMPORT);
            self.output.push(0x41);
            write_i64(self.output, i64::from(CAPACITY_EXCEEDED));
            self.output.push(0x21);
            write_u32(self.output, self.plan.status);
            self.emit_filesystem_exit(&expr.id)?;
            self.output.push(0x0b);
            self.emit_load_out(pointer);
            self.output.push(0x21);
            write_u32(self.output, local);
        } else {
            self.emit_load_out(pointer);
            self.output.push(0x22);
            write_u32(self.output, local);
            self.get_scalar(&arguments[3]);
            self.output.push(0x52);
            self.emit_filesystem_failure_if_code(
                &expr.id,
                crate::filesystem_ops::IO_FAILURE as i32,
            )?;
        }
        Ok(Value::Scalar {
            local,
            ty: expr.ty.clone(),
        })
    }

    fn emit_filesystem_failure_if_code(
        &mut self,
        expression: &ExpressionId,
        code: i32,
    ) -> Result<(), Diagnostic> {
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(self.output, i64::from(code));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.emit_filesystem_exit(expression)?;
        self.output.push(0x0b);
        Ok(())
    }

    /// Validate the logical path prefix in guest code before the provider is
    /// entered. Component state is 0 for empty, 1 for `.`, 2 for `..`, and 3
    /// for every admitted non-dot component.
    fn emit_filesystem_path_validation(
        &mut self,
        carrier: u32,
        logical_length: &Value,
        expression: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        let (index, component, byte) = self
            .plan
            .filesystem_scan
            .ok_or_else(|| error("filesystem path scan locals are absent"))?;
        for local in [index, component] {
            self.output.extend([0x42, 0x00, 0x21]);
            write_u32(self.output, local);
        }

        self.output.extend([0x02, 0x40, 0x03, 0x40]);
        self.control_depth += 2;
        self.output.push(0x20);
        write_u32(self.output, index);
        self.get_scalar(logical_length);
        self.output.extend([0x5a, 0x0d, 0x01]); // index >= length -> exit

        self.output.push(0x20);
        write_u32(self.output, carrier);
        self.output.push(0x20);
        write_u32(self.output, index);
        self.output.push(0x10);
        write_u32(self.output, BYTE_GET_IMPORT);
        self.output.push(0x22);
        write_u32(self.output, self.plan.status);
        self.output.push(0x41);
        write_i64(self.output, 255);
        self.output.push(0x4b); // i32.gt_u: forged carrier/member access
        self.emit_command_failure_if(
            expression,
            super::super::host_output::COMMAND_STDOUT_GLOBALS,
            super::super::host_output::COMMAND_STDERR_GLOBALS,
        )?;
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0xad, 0x21]);
        write_u32(self.output, byte);

        // NUL, backslash, and colon are forbidden everywhere.
        for (position, forbidden) in [0i64, 92, 58].into_iter().enumerate() {
            self.output.push(0x20);
            write_u32(self.output, byte);
            self.output.push(0x42);
            write_i64(self.output, forbidden);
            self.output.push(0x51); // i64.eq
            if position != 0 {
                self.output.push(0x72); // i32.or
            }
        }
        self.emit_filesystem_failure_if_code(expression, INVALID_PATH)?;

        self.output.push(0x20);
        write_u32(self.output, byte);
        self.output.push(0x42);
        write_i64(self.output, 47);
        self.output.extend([0x51, 0x04, 0x40]); // if slash
        self.control_depth += 1;
        self.output.push(0x20);
        write_u32(self.output, component);
        self.output.push(0x42);
        write_i64(self.output, 2);
        self.output.push(0x58); // i64.le_u: empty, `.` or `..`
        self.emit_filesystem_failure_if_code(expression, INVALID_PATH)?;
        self.output.extend([0x42, 0x00, 0x21]);
        write_u32(self.output, component);
        self.output.push(0x05); // else non-slash

        // Preserve the dot-only states; any other spelling becomes state 3.
        self.output.push(0x20);
        write_u32(self.output, component);
        self.output.push(0x50); // component == 0
        self.output.extend([0x04, 0x40]);
        self.control_depth += 1;
        self.output.push(0x20);
        write_u32(self.output, byte);
        self.output.push(0x42);
        write_i64(self.output, 46);
        self.output
            .extend([0x51, 0x04, 0x7e, 0x42, 0x01, 0x05, 0x42, 0x03, 0x0b]);
        self.output.push(0x21);
        write_u32(self.output, component);
        self.output.push(0x05);
        self.output.push(0x20);
        write_u32(self.output, component);
        self.output.push(0x42);
        write_i64(self.output, 1);
        self.output.extend([0x51, 0x04, 0x40]);
        self.control_depth += 1;
        self.output.push(0x20);
        write_u32(self.output, byte);
        self.output.push(0x42);
        write_i64(self.output, 46);
        self.output
            .extend([0x51, 0x04, 0x7e, 0x42, 0x02, 0x05, 0x42, 0x03, 0x0b]);
        self.output.push(0x21);
        write_u32(self.output, component);
        self.output.push(0x05);
        self.output.extend([0x42, 0x03, 0x21]);
        write_u32(self.output, component);
        self.output.push(0x0b);
        self.control_depth -= 1;
        self.output.push(0x0b);
        self.control_depth -= 1;
        self.output.push(0x0b);
        self.control_depth -= 1;

        self.output.push(0x20);
        write_u32(self.output, index);
        self.output.extend([0x42, 0x01, 0x7c, 0x21]);
        write_u32(self.output, index);
        self.output.extend([0x0c, 0x00, 0x0b, 0x0b]);
        self.control_depth -= 2;

        self.output.push(0x20);
        write_u32(self.output, component);
        self.output.push(0x42);
        write_i64(self.output, 2);
        self.output.push(0x58);
        self.emit_filesystem_failure_if_code(expression, INVALID_PATH)?;
        Ok(())
    }

    fn emit_filesystem_failure_if(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic> {
        self.output.extend([0x04, 0x40]);
        self.emit_filesystem_exit(expression)?;
        self.output.push(0x0b);
        Ok(())
    }

    /// Reserve the operation and its declared byte budget before the provider
    /// is entered. Reservations are deliberately not refunded on a provider
    /// failure, making retry cost explicit and deterministic.
    fn reserve_filesystem_work(
        &mut self,
        charge: &Value,
        expression: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        self.output.push(0x23);
        write_u32(self.output, boundary::OPERATION_COUNT_GLOBAL);
        self.output.push(0x42);
        write_i64(self.output, crate::filesystem_ops::MAX_OPERATIONS as i64);
        self.output.push(0x5a);
        self.emit_filesystem_failure_if_code(expression, CAPACITY_EXCEEDED)?;
        self.output.push(0x23);
        write_u32(self.output, boundary::OPERATION_COUNT_GLOBAL);
        self.output.push(0x42);
        write_i64(self.output, 1);
        self.output.push(0x7c);
        self.output.push(0x24);
        write_u32(self.output, boundary::OPERATION_COUNT_GLOBAL);

        self.get_scalar(charge);
        self.output.push(0x42);
        write_i64(self.output, crate::filesystem_ops::MAX_TOTAL_BYTES as i64);
        self.output.push(0x56);
        self.emit_filesystem_failure_if_code(expression, CAPACITY_EXCEEDED)?;
        self.output.push(0x23);
        write_u32(self.output, boundary::BYTE_COUNT_GLOBAL);
        self.output.push(0x42);
        write_i64(self.output, crate::filesystem_ops::MAX_TOTAL_BYTES as i64);
        self.get_scalar(charge);
        self.output.push(0x7d);
        self.output.push(0x56);
        self.emit_filesystem_failure_if_code(expression, CAPACITY_EXCEEDED)?;
        self.output.push(0x23);
        write_u32(self.output, boundary::BYTE_COUNT_GLOBAL);
        self.get_scalar(charge);
        self.output.push(0x7c);
        self.output.push(0x24);
        write_u32(self.output, boundary::BYTE_COUNT_GLOBAL);
        Ok(())
    }

    fn emit_filesystem_exit(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic> {
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.push(0x24);
        write_u32(self.output, boundary::STATUS_GLOBAL);
        self.emit_failure_cleanup(expression, StatusLane::OperationFailure)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        Ok(())
    }
}
