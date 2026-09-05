//! HTTPS Client I/O v1 lowering for the aggregate Wasm emitter.

use super::super::http_io as boundary;
use super::*;

const MAX_URL_BYTES: i64 = 2_048;
const MAX_RESPONSE_BYTES: i64 = crate::network_io_ops::MAX_CHUNK_BYTES as i64;
const INVALID_URL: i32 = crate::network_io_ops::HTTP_INVALID_URL as i32;
const RESPONSE_TOO_LARGE: i32 = crate::network_io_ops::HTTP_RESPONSE_TOO_LARGE as i32;
const LAST_STATUS: i32 = crate::network_io_ops::HTTP_AUTHORITY_DENIED as i32;

impl Emitter<'_> {
    pub(super) fn emit_http_command_call(
        &mut self,
        expr: &ResolvedExpr,
        call: &crate::hir::ResolvedHostCommandCall,
        arguments: &[Value],
        local: u32,
        pointer: Pointer,
    ) -> Result<Value, Diagnostic> {
        use crate::hir::ResolvedHostCommandOperation as Op;
        let stdout = super::super::host_output::COMMAND_STDOUT_GLOBALS;
        let stderr = super::super::host_output::COMMAND_STDERR_GLOBALS;
        if call.operation != Op::HttpsGet {
            return Err(error("non-HTTPS operation reached the HTTPS lowering"));
        }
        self.require_scalar(&arguments[0], &ResolvedType::SliceU8, "https_get URL")?;
        self.require_scalar(
            &arguments[1],
            &ResolvedType::Usize,
            "https_get response bound",
        )?;
        self.stage_slice_carrier(&arguments[0], local);
        self.emit_carrier_length(local);
        self.emit_outside_one_to(MAX_URL_BYTES);
        self.emit_http_failure_if_code(&expr.id, INVALID_URL)?;
        self.get_scalar(&arguments[1]);
        self.emit_outside_one_to(MAX_RESPONSE_BYTES);
        self.emit_http_failure_if_code(&expr.id, RESPONSE_TOO_LARGE)?;

        self.emit_carrier_root_and_length(local);
        self.get_scalar(&arguments[1]);
        self.output.push(0xa7);
        self.emit_pointer(pointer);
        self.output.push(0x10);
        write_u32(self.output, boundary::GET_IMPORT);
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);

        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.push(0x41);
        write_i64(self.output, i64::from(LAST_STATUS));
        self.output.push(0x4b);
        self.emit_command_failure_if(&expr.id, stdout, stderr)?;
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.emit_http_failure_if(&expr.id)?;

        // Authenticate the provider-owned result exactly as `stdin_read` and
        // `net_recv`: nonzero tagged token, length at most the caller's bound,
        // and exact membership in the host arena.
        self.emit_load_out(pointer);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
        write_i64(self.output, i64::from(i32::MIN));
        self.output.extend([0x71, 0x45]);
        self.emit_load_out(pointer);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
        write_i64(self.output, i64::from(0x7fff_ffff_u32));
        self.output.extend([0x71, 0x45, 0x72]);
        self.emit_load_out(pointer);
        self.output.push(0xa7);
        self.get_scalar(&arguments[1]);
        self.output.extend([0xa7, 0x4b, 0x72]);
        self.emit_command_failure_if(&expr.id, stdout, stderr)?;
        self.emit_load_out(pointer);
        self.output.push(0x10);
        write_u32(
            self.output,
            super::super::command_io::OWNED_BYTES_VALIDATE_IMPORT,
        );
        self.emit_command_failure_if(&expr.id, stdout, stderr)?;
        self.emit_load_out(pointer);
        self.output.push(0x21);
        write_u32(self.output, local);
        Ok(Value::Scalar {
            local,
            ty: expr.ty.clone(),
        })
    }

    fn emit_http_failure_if_code(
        &mut self,
        expression: &ExpressionId,
        code: i32,
    ) -> Result<(), Diagnostic> {
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(self.output, i64::from(code));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.emit_http_exit(expression)?;
        self.output.push(0x0b);
        Ok(())
    }

    fn emit_http_failure_if(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic> {
        self.output.extend([0x04, 0x40]);
        self.emit_http_exit(expression)?;
        self.output.push(0x0b);
        Ok(())
    }

    fn emit_http_exit(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic> {
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
