//! Bounded Language Network I/O v1 lowering for the aggregate emitter.
//!
//! The caller has already staged the arguments left to right, applied the
//! call-commit transitions, and poisoned the provider out-slot. This module
//! narrows the scalar domains the closed import ABI carries as i32 (handles,
//! ports, chunk and wait bounds) into normalized `semaprax.network.v1`
//! failures before the provider is reached, calls the provider, and then fails
//! closed on any answer outside the exact contract before a value can reach
//! the program. Normalized failures (codes 1..=6) mark the exported network
//! status global; provider contract violations take the same backend
//! fail-stop edge command providers already use.

use super::super::network_io as boundary;
use super::*;

const MAX_HANDLES: i64 = crate::network_io_ops::MAX_HANDLES as i64;
const MAX_HOST_BYTES: i64 = crate::network_io_ops::MAX_HOST_BYTES as i64;
const MAX_PORT: i64 = crate::network_io_ops::MAX_PORT as i64;
const MAX_CHUNK_BYTES: i64 = crate::network_io_ops::MAX_CHUNK_BYTES as i64;
const MAX_WAIT_MILLIS: i64 = crate::network_io_ops::MAX_WAIT_MILLIS as i64;
const WAIT_CLOSED: i64 = crate::network_io_ops::WAIT_CLOSED as i64;
const INVALID_ENDPOINT: i32 = crate::network_io_ops::INVALID_ENDPOINT as i32;
const UNKNOWN_HANDLE: i32 = crate::network_io_ops::UNKNOWN_HANDLE as i32;
const CAPACITY_EXCEEDED: i32 = crate::network_io_ops::CAPACITY_EXCEEDED as i32;
/// The largest normalized status code; anything above it is a provider fault.
const LAST_STATUS: i32 = crate::network_io_ops::AUTHORITY_DENIED as i32;

impl Emitter<'_> {
    pub(super) fn emit_network_command_call(
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

        if call.operation == Op::NetConnect {
            self.require_scalar(&arguments[0], &ResolvedType::SliceU8, "net_connect host")?;
            self.require_scalar(&arguments[1], &ResolvedType::Usize, "net_connect port")?;
            self.stage_slice_carrier(&arguments[0], local);
            self.emit_carrier_length(local);
            self.emit_outside_one_to(MAX_HOST_BYTES);
            self.emit_network_failure_if_code(&expr.id, INVALID_ENDPOINT)?;
            self.get_scalar(&arguments[1]);
            self.emit_outside_one_to(MAX_PORT);
            self.emit_network_failure_if_code(&expr.id, INVALID_ENDPOINT)?;
            self.emit_carrier_root_and_length(local);
            self.get_scalar(&arguments[1]);
            self.output.push(0xa7);
            self.emit_pointer(pointer);
            self.output.push(0x10);
            write_u32(self.output, boundary::CONNECT_IMPORT);
        } else {
            self.require_scalar(&arguments[0], &ResolvedType::Usize, "network handle")?;
            self.get_scalar(&arguments[0]);
            self.emit_outside_one_to(MAX_HANDLES);
            self.emit_network_failure_if_code(&expr.id, UNKNOWN_HANDLE)?;
            match call.operation {
                Op::NetSend => {
                    self.require_scalar(&arguments[1], &ResolvedType::SliceU8, "net_send value")?;
                    self.stage_slice_carrier(&arguments[1], local);
                    self.emit_handle(&arguments[0]);
                    self.emit_carrier_root_and_length(local);
                    self.emit_pointer(pointer);
                    self.output.push(0x10);
                    write_u32(self.output, boundary::SEND_IMPORT);
                }
                Op::NetRecv | Op::NetStreamStdout => {
                    self.require_scalar(
                        &arguments[1],
                        &ResolvedType::Usize,
                        "network chunk bound",
                    )?;
                    self.get_scalar(&arguments[1]);
                    self.output.push(0x42);
                    write_i64(self.output, MAX_CHUNK_BYTES);
                    self.output.push(0x56); // i64.gt_u
                    self.emit_network_failure_if_code(&expr.id, CAPACITY_EXCEEDED)?;
                    self.emit_handle(&arguments[0]);
                    if call.operation == Op::NetStreamStdout {
                        self.output.push(0x41);
                        write_i64(self.output, i64::from(boundary::STREAM_SCRATCH_BASE));
                    }
                    self.get_scalar(&arguments[1]);
                    self.output.push(0xa7);
                    self.emit_pointer(pointer);
                    self.output.push(0x10);
                    write_u32(
                        self.output,
                        if call.operation == Op::NetRecv {
                            boundary::RECV_IMPORT
                        } else {
                            boundary::STREAM_STDOUT_IMPORT
                        },
                    );
                }
                Op::NetWait => {
                    self.require_scalar(&arguments[1], &ResolvedType::Usize, "net_wait timeout")?;
                    self.get_scalar(&arguments[1]);
                    self.output.push(0x42);
                    write_i64(self.output, MAX_WAIT_MILLIS);
                    self.output.push(0x56);
                    self.emit_network_failure_if_code(&expr.id, CAPACITY_EXCEEDED)?;
                    self.emit_handle(&arguments[0]);
                    self.get_scalar(&arguments[1]);
                    self.output.push(0xa7);
                    self.emit_pointer(pointer);
                    self.output.push(0x10);
                    write_u32(self.output, boundary::WAIT_IMPORT);
                }
                Op::NetClose => {
                    self.emit_handle(&arguments[0]);
                    self.output.push(0x10);
                    write_u32(self.output, boundary::CLOSE_IMPORT);
                }
                _ => return Err(error("non-network operation reached the network lowering")),
            }
        }
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);

        // A provider answer outside 0..=6 is a contract violation, not a
        // program-visible failure.
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.push(0x41);
        write_i64(self.output, i64::from(LAST_STATUS));
        self.output.push(0x4b); // i32.gt_u
        self.emit_command_failure_if(&expr.id, stdout, stderr)?;
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.emit_network_failure_if(&expr.id)?;

        // Status zero: authenticate the out-slot before it becomes a value.
        match call.operation {
            Op::NetClose => {
                self.output.extend([0x42, 0x00, 0x21]);
                write_u32(self.output, local);
            }
            Op::NetConnect => {
                self.emit_load_out(pointer);
                self.output.push(0x22);
                write_u32(self.output, local);
                self.emit_outside_one_to(MAX_HANDLES);
                self.emit_command_failure_if(&expr.id, stdout, stderr)?;
            }
            Op::NetSend => {
                // A successful send accepted exactly the staged length.
                self.emit_carrier_length(local);
                self.emit_load_out(pointer);
                self.output.push(0x22);
                write_u32(self.output, local);
                self.output.push(0x52); // i64.ne
                self.emit_command_failure_if(&expr.id, stdout, stderr)?;
            }
            Op::NetWait => {
                self.emit_load_out(pointer);
                self.output.push(0x22);
                write_u32(self.output, local);
                self.output.push(0x42);
                write_i64(self.output, WAIT_CLOSED);
                self.output.push(0x56);
                self.emit_command_failure_if(&expr.id, stdout, stderr)?;
            }
            Op::NetRecv => {
                // One tagged, nonzero owned-arena token of at most `max` bytes,
                // then exact arena membership through the closed 0/1 validator,
                // exactly as `stdin_read` is authenticated.
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
            }
            Op::NetStreamStdout => {
                self.emit_load_out(pointer);
                self.output.push(0x22);
                write_u32(self.output, local);
                self.get_scalar(&arguments[1]);
                self.output.push(0x56); // count > max
                self.emit_command_failure_if(&expr.id, stdout, stderr)?;
                // The two staged channels share one cumulative budget; test
                // the whole chunk before a byte joins the transcript.
                self.output.push(0x20);
                write_u32(self.output, local);
                self.output.extend([0xa7, 0x41]);
                write_i64(
                    self.output,
                    i64::from(super::super::host_output::TRANSCRIPT_CAPACITY),
                );
                self.output.push(0x23);
                write_u32(self.output, stdout.staged_length);
                self.output.extend([0x6b, 0x23]);
                write_u32(self.output, stderr.staged_length);
                self.output.extend([0x6b, 0x4b]);
                self.emit_network_failure_if_code(&expr.id, CAPACITY_EXCEEDED)?;
                // Append from the scratch range, then clear the scratch so the
                // published range holds nothing until the wrapper publishes.
                self.output.push(0x41);
                write_i64(self.output, i64::from(stdout.range_base));
                self.output.push(0x23);
                write_u32(self.output, stdout.staged_length);
                self.output.extend([0x6a, 0x41]);
                write_i64(self.output, i64::from(boundary::STREAM_SCRATCH_BASE));
                self.output.push(0x20);
                write_u32(self.output, local);
                self.output.extend([0xa7, 0xfc, 0x0a, 0x00, 0x00]);
                self.output.push(0x41);
                write_i64(self.output, i64::from(boundary::STREAM_SCRATCH_BASE));
                self.output.extend([0x41, 0x00, 0x20]);
                write_u32(self.output, local);
                self.output.extend([0xa7, 0xfc, 0x0b, 0x00]);
                self.output.push(0x23);
                write_u32(self.output, stdout.staged_length);
                self.output.push(0x20);
                write_u32(self.output, local);
                self.output.extend([0xa7, 0x6a, 0x24]);
                write_u32(self.output, stdout.staged_length);
            }
            _ => unreachable!("non-network operations were rejected above"),
        }
        Ok(Value::Scalar {
            local,
            ty: expr.ty.clone(),
        })
    }

    /// Park a `Slice<u8>` carrier in the result local and authenticate it
    /// while its source is live; the local is overwritten by the result later.
    fn stage_slice_carrier(&mut self, value: &Value, local: u32) {
        self.get_scalar(value);
        self.output.push(0x21);
        write_u32(self.output, local);
        self.validate_byte_slice(&Value::Scalar {
            local,
            ty: ResolvedType::SliceU8,
        });
    }

    /// Push the carrier's low 32-bit length as an i64.
    fn emit_carrier_length(&mut self, local: u32) {
        self.output.push(0x20);
        write_u32(self.output, local);
        self.output.extend([0xa7, 0xad]);
    }

    /// Push the carrier's root word and length as two i32 provider arguments.
    fn emit_carrier_root_and_length(&mut self, local: u32) {
        self.output.push(0x20);
        write_u32(self.output, local);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x20]);
        write_u32(self.output, local);
        self.output.push(0xa7);
    }

    /// Consume an i64 and push whether it lies outside `1..=max`.
    fn emit_outside_one_to(&mut self, max: i64) {
        self.output.extend([0x42, 0x01, 0x7d, 0x42]);
        write_i64(self.output, max);
        self.output.push(0x5a); // i64.ge_u
    }

    /// Push an already range-checked handle as the provider's i32 argument.
    fn emit_handle(&mut self, value: &Value) {
        self.get_scalar(value);
        self.output.push(0xa7);
    }

    fn emit_load_out(&mut self, pointer: Pointer) {
        self.emit_pointer(pointer);
        self.output.extend([0x29, 0x03, 0x00]);
    }

    /// Consume an i32 condition; when set, fail with the normalized `code`.
    fn emit_network_failure_if_code(
        &mut self,
        expression: &ExpressionId,
        code: i32,
    ) -> Result<(), Diagnostic> {
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(self.output, i64::from(code));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.emit_network_exit(expression)?;
        self.output.push(0x0b);
        Ok(())
    }

    /// Consume the status local; when nonzero, take the normalized exit.
    fn emit_network_failure_if(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic> {
        self.output.extend([0x04, 0x40]);
        self.emit_network_exit(expression)?;
        self.output.push(0x0b);
        Ok(())
    }

    /// Mark the network sub-domain with the sticky status, run the exact
    /// failure cleanup for this expression, and leave the function body.
    fn emit_network_exit(&mut self, expression: &ExpressionId) -> Result<(), Diagnostic> {
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
