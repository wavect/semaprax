//! Host-command lowering kept separate to preserve aggregate emitter bounds.
use super::*;

impl Emitter<'_> {
    pub(super) fn emit_host_command_call(
        &mut self,
        expr: &ResolvedExpr,
        call: &crate::hir::ResolvedHostCommandCall,
    ) -> Result<Value, Diagnostic> {
        use crate::hir::ResolvedHostCommandOperation as Op;

        if call.args.len() != crate::command_io_ops::arity(call.operation) {
            return Err(error(
                "host command operation arity disagrees with resolved HIR",
            ));
        }
        let mut arguments = Vec::with_capacity(call.args.len());
        for argument in &call.args {
            arguments.push(self.emit_expr(argument)?);
        }
        self.apply_call_commit(&expr.id)?;
        let local = self.plan.expr_scalar(expr)?;
        if call.operation == Op::ArgsLen {
            self.output.push(0x10);
            write_u32(self.output, super::super::command_io::ARGS_LEN_IMPORT);
            self.output.push(0x21);
            write_u32(self.output, local);
            self.output.push(0x20);
            write_u32(self.output, local);
            self.output.extend([0x42]);
            write_i64(self.output, crate::command_io_ops::MAX_ARGUMENTS as i64);
            self.output.push(0x56); // i64.gt_u
            self.emit_command_failure_if(
                &expr.id,
                super::super::host_output::COMMAND_STDOUT_GLOBALS,
                super::super::host_output::COMMAND_STDERR_GLOBALS,
            )?;
            return Ok(Value::Scalar {
                local,
                ty: ResolvedType::Usize,
            });
        }

        let offset = self
            .plan
            .call_out
            .get(&expr.id)
            .copied()
            .ok_or_else(|| error("host command result has no exact out slot"))?;
        let pointer = Pointer {
            local: self.plan.frame_base,
            offset,
        };
        // Poison the provider out-slot before entry. A conforming provider
        // writes it only on status zero.
        self.emit_pointer(pointer);
        if call.operation == Op::EnvLen {
            self.output.extend([0x41]);
            write_i64(self.output, -1);
            self.output.extend([0x36, 0x02, 0x00]);
        } else {
            self.output.push(0x42);
            write_i64(
                self.output,
                if crate::environment_ops::is_environment(call.operation)
                    || crate::process_ops::is_process(call.operation)
                {
                    -1
                } else {
                    0
                },
            );
            self.output.extend([0x37, 0x03, 0x00]);
        }
        match call.operation {
            Op::ProcessRun => {
                return self.emit_process_command_call(expr, &arguments, local, pointer)
            }
            Op::EnvLen => {
                self.emit_pointer(pointer);
                self.output.push(0x10);
                write_u32(self.output, super::super::environment_io::LEN_IMPORT);
            }
            Op::EnvNameUtf8 | Op::EnvValueUtf8 => {
                self.require_scalar(&arguments[0], &ResolvedType::Usize, "environment index")?;
                self.get_scalar(&arguments[0]);
                self.emit_pointer(pointer);
                self.output.push(0x10);
                write_u32(
                    self.output,
                    if call.operation == Op::EnvNameUtf8 {
                        super::super::environment_io::NAME_UTF8_IMPORT
                    } else {
                        super::super::environment_io::VALUE_UTF8_IMPORT
                    },
                );
            }
            filesystem if crate::filesystem_ops::is_filesystem(filesystem) => {
                return self.emit_filesystem_command_call(expr, call, &arguments, local, pointer);
            }
            http if crate::network_io_ops::is_http(http) => {
                return self.emit_http_command_call(expr, call, &arguments, local, pointer);
            }
            network if crate::network_io_ops::is_network(network) => {
                return self.emit_network_command_call(expr, call, &arguments, local, pointer);
            }
            Op::ArgUtf8 => {
                self.require_scalar(&arguments[0], &ResolvedType::Usize, "arg_utf8 index")?;
                self.get_scalar(&arguments[0]);
                self.emit_pointer(pointer);
                self.output.push(0x10);
                write_u32(self.output, super::super::command_io::ARG_UTF8_IMPORT);
            }
            Op::StdinRead => {
                self.emit_pointer(pointer);
                self.output.push(0x10);
                write_u32(self.output, super::super::command_io::STDIN_READ_IMPORT);
            }
            Op::StderrWrite => {
                self.require_scalar(
                    &arguments[0],
                    &ResolvedType::SliceU8,
                    "stderr_write argument",
                )?;
                self.get_scalar(&arguments[0]);
                self.output.push(0x21);
                write_u32(self.output, local);
                let staged = Value::Scalar {
                    local,
                    ty: ResolvedType::SliceU8,
                };
                self.validate_byte_slice(&staged);
                self.emit_command_transcript_write(
                    &expr.id,
                    local,
                    super::super::host_output::COMMAND_STDERR_GLOBALS,
                    super::super::host_output::COMMAND_STDOUT_GLOBALS,
                )?;
                return Ok(Value::Scalar {
                    local,
                    ty: ResolvedType::Usize,
                });
            }
            Op::StdoutAppend | Op::StderrAppend => {
                self.require_scalar(
                    &arguments[0],
                    &ResolvedType::SliceU8,
                    "command append argument",
                )?;
                self.get_scalar(&arguments[0]);
                self.output.push(0x21);
                write_u32(self.output, local);
                let staged = Value::Scalar {
                    local,
                    ty: ResolvedType::SliceU8,
                };
                self.validate_byte_slice(&staged);
                let (channel, other) = if call.operation == Op::StdoutAppend {
                    (
                        super::super::host_output::COMMAND_STDOUT_GLOBALS,
                        super::super::host_output::COMMAND_STDERR_GLOBALS,
                    )
                } else {
                    (
                        super::super::host_output::COMMAND_STDERR_GLOBALS,
                        super::super::host_output::COMMAND_STDOUT_GLOBALS,
                    )
                };
                self.emit_command_transcript_append(&expr.id, local, channel, other)?;
                return Ok(Value::Scalar {
                    local,
                    ty: ResolvedType::Usize,
                });
            }
            Op::ArgsLen => unreachable!("handled above"),
            _ => unreachable!("network operations return above"),
        }
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);

        // Fail closed if an independently supplied provider returns a code
        // outside the operation's exact normalized sub-domain.
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        match call.operation {
            Op::ArgUtf8 => self.output.extend([0x41, 0x02, 0x4b]), // status > 2
            Op::EnvLen | Op::EnvNameUtf8 | Op::EnvValueUtf8 => {
                self.output.extend([0x41, 0x04, 0x4b]) // status > 4
            }
            Op::StdinRead => {
                self.output.extend([0x41, 0x03, 0x49, 0x20]); // status < 3 ||
                write_u32(self.output, self.plan.status);
                self.output.extend([0x41, 0x04, 0x4b, 0x72]);
                self.output.push(0x20);
                write_u32(self.output, self.plan.status);
                self.output.extend([0x45, 0x45, 0x71]); // and status != 0
            }
            _ => unreachable!("fallible operation checked above"),
        }
        self.output.extend([0x04, 0x40, 0x41]);
        write_i64(self.output, i64::from(STATUS_INTERNAL_INVALID_TAG));
        self.output.push(0x21);
        write_u32(self.output, self.plan.status);
        self.output.push(0x0b);
        if call.operation == Op::StdinRead {
            // Status zero is not enough: stdin must return one tagged,
            // nonzero owned-arena token within the invocation capacity.
            self.output.push(0x20);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x45, 0x04, 0x40]);
            self.control_depth += 1;
            self.emit_pointer(pointer);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
            write_i64(self.output, i64::from(i32::MIN));
            self.output.extend([0x71, 0x45]);
            self.emit_pointer(pointer);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
            write_i64(self.output, i64::from(0x7fff_ffff_u32));
            self.output.extend([0x71, 0x45, 0x72]);
            self.emit_pointer(pointer);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.extend([0xa7, 0x41]);
            write_i64(self.output, crate::command_io_ops::MAX_INPUT_BYTES as i64);
            self.output.extend([0x4b, 0x72, 0x04, 0x40, 0x41]);
            write_i64(self.output, i64::from(STATUS_INTERNAL_INVALID_TAG));
            self.output.push(0x21);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x0b]);

            // Structural tagging is insufficient: authenticate exact arena
            // membership and recorded length through a closed recoverable
            // 0=member / 1=not-member provider contract before CleanupPlan is
            // allowed to initialize the owned result slot. This also checks
            // zero-length carriers instead of treating length zero as proof.
            self.output.push(0x20);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x45, 0x04, 0x40]);
            self.emit_pointer(pointer);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.push(0x10);
            write_u32(
                self.output,
                super::super::command_io::OWNED_BYTES_VALIDATE_IMPORT,
            );
            self.output.push(0x22);
            write_u32(
                self.output,
                self.plan
                    .command_byte
                    .ok_or_else(|| error("command provider validation local is absent"))?,
            );
            self.output.extend([0x41, 0x01, 0x4b, 0x20]); // status > 1 || status == 1
            write_u32(
                self.output,
                self.plan
                    .command_byte
                    .ok_or_else(|| error("command provider validation local is absent"))?,
            );
            self.output
                .extend([0x41, 0x01, 0x46, 0x72, 0x04, 0x40, 0x41]);
            write_i64(self.output, i64::from(STATUS_INTERNAL_INVALID_TAG));
            self.output.push(0x21);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x0b, 0x0b, 0x0b]);
            self.control_depth -= 1;
        }
        if matches!(call.operation, Op::EnvNameUtf8 | Op::EnvValueUtf8) {
            self.output.push(0x20);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x45, 0x04, 0x40]);
            self.control_depth += 1;
            self.emit_environment_view_validation(
                &expr.id,
                pointer,
                local,
                call.operation == Op::EnvNameUtf8,
            )?;
            self.control_depth -= 1;
            self.output.push(0x0b);
        }
        if call.operation == Op::EnvLen {
            self.output.push(0x20);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x45, 0x04, 0x40]);
            self.control_depth += 1;
            self.emit_pointer(pointer);
            self.output.extend([0x28, 0x02, 0x00, 0xad]);
            self.output.push(0x42);
            write_i64(self.output, crate::environment_ops::MAX_ENTRIES as i64);
            self.output.push(0x56);
            self.emit_command_failure_if(
                &expr.id,
                super::super::host_output::COMMAND_STDOUT_GLOBALS,
                super::super::host_output::COMMAND_STDERR_GLOBALS,
            )?;
            self.control_depth -= 1;
            self.output.push(0x0b);
        }
        // Authenticate the operation-specific command-input domain separately
        // from the shared language status code. Arithmetic, contract, and
        // internal fail-stop statuses leave this marker at zero.
        let (first_code, second_code) = match call.operation {
            Op::ArgUtf8 => (1, 2),
            Op::StdinRead => (3, 4),
            Op::EnvLen | Op::EnvNameUtf8 | Op::EnvValueUtf8 => (1, 4),
            _ => unreachable!("only fallible command operations reach the marker"),
        };
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41]);
        write_i64(self.output, first_code);
        self.output.extend([0x4f, 0x20]);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41]);
        write_i64(self.output, second_code);
        self.output.extend([0x4d, 0x71, 0x04, 0x40, 0x20]);
        write_u32(self.output, self.plan.status);
        self.output.push(0x24);
        write_u32(
            self.output,
            if crate::environment_ops::is_environment(call.operation) {
                super::super::environment_io::STATUS_GLOBAL
            } else {
                super::super::command_io::INPUT_STATUS_GLOBAL
            },
        );
        self.output.push(0x0b);
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x04, 0x40]);
        self.emit_failure_cleanup(&expr.id, StatusLane::OperationFailure)?;
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth + self.status_exit_extra_depth,
        );
        self.output.push(0x0b);
        self.emit_pointer(pointer);
        if call.operation == Op::EnvLen {
            self.output.extend([0x28, 0x02, 0x00, 0xad]);
        } else {
            self.load_scalar(&expr.ty);
        }
        self.output.push(0x21);
        write_u32(self.output, local);
        Ok(Value::Scalar {
            local,
            ty: expr.ty.clone(),
        })
    }

    /// Validate the success-only borrowed environment view directly against
    /// the fixed untagged input arena.  Provider status zero is never proof:
    /// tagged roots, out-of-arena ranges, NUL bytes, and `=` in a name all
    /// fail through the ordinary command cleanup edge before publication.
    fn emit_environment_view_validation(
        &mut self,
        expression: &ExpressionId,
        pointer: Pointer,
        carrier: u32,
        name: bool,
    ) -> Result<(), Diagnostic> {
        let byte = self
            .plan
            .command_byte
            .ok_or_else(|| error("environment validation byte local is absent"))?;
        self.emit_pointer(pointer);
        self.load_scalar(&ResolvedType::Str);
        self.output.push(0x21);
        write_u32(self.output, carrier);
        // The high word must be an untagged fixed-memory root; its low word
        // plus the carrier length stays within the isolated first 64 KiB.
        self.output.push(0x20);
        write_u32(self.output, carrier);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x41]);
        write_i64(self.output, crate::environment_ops::MAX_INPUT_BYTES as i64);
        self.output.push(0x4b);
        self.output.push(0x20);
        write_u32(self.output, carrier);
        self.output.extend([0xa7, 0x41]);
        write_i64(self.output, crate::environment_ops::MAX_INPUT_BYTES as i64);
        self.output.extend([0x4b, 0x72]);
        self.output.push(0x20);
        write_u32(self.output, carrier);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x20]);
        write_u32(self.output, carrier);
        self.output.extend([0xa7, 0x6a, 0x41]);
        write_i64(self.output, crate::environment_ops::MAX_INPUT_BYTES as i64);
        self.output.extend([0x4b, 0x72]);
        self.output.push(0x20);
        write_u32(self.output, carrier);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x20]);
        write_u32(self.output, carrier);
        self.output.extend([0xa7, 0x6a, 0x20]);
        write_u32(self.output, carrier);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x49, 0x72]);
        if name {
            self.output.push(0x20);
            write_u32(self.output, carrier);
            self.output.extend([0xa7, 0x45, 0x72]);
        }
        self.emit_command_failure_if(
            expression,
            super::super::host_output::COMMAND_STDOUT_GLOBALS,
            super::super::host_output::COMMAND_STDERR_GLOBALS,
        )?;
        let validator = self
            .environment_utf8_index
            .ok_or_else(|| error("environment UTF-8 validator is absent"))?;
        self.output.push(0x20);
        write_u32(self.output, carrier);
        self.output.extend([0x42, 0x20, 0x88, 0xa7]);
        self.output.push(0x20);
        write_u32(self.output, carrier);
        self.output.extend([0xa7, 0x10]);
        write_u32(self.output, validator);
        self.emit_command_failure_if(
            expression,
            super::super::host_output::COMMAND_STDOUT_GLOBALS,
            super::super::host_output::COMMAND_STDERR_GLOBALS,
        )?;
        // Reuse the status local as a bounded cursor after its zero status
        // has been consumed. Every memory load is dominated by the complete
        // arena range check above.
        self.output.extend([0x41, 0x00, 0x21]);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x02, 0x40, 0x03, 0x40, 0x20]);
        self.control_depth += 2;
        write_u32(self.output, self.plan.status);
        self.output.push(0x20);
        write_u32(self.output, carrier);
        self.output.extend([0xa7, 0x4f, 0x0d, 0x01]);
        self.output.push(0x20);
        write_u32(self.output, carrier);
        self.output.extend([0x42, 0x20, 0x88, 0xa7, 0x20]);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x6a, 0x2d, 0x00, 0x00, 0x21]);
        write_u32(self.output, byte);
        self.output.push(0x20);
        write_u32(self.output, byte);
        self.output.extend([0x45]);
        if name {
            self.output.push(0x20);
            write_u32(self.output, byte);
            self.output.extend([0x41, 0x3d, 0x46, 0x72]);
        }
        self.emit_command_failure_if(
            expression,
            super::super::host_output::COMMAND_STDOUT_GLOBALS,
            super::super::host_output::COMMAND_STDERR_GLOBALS,
        )?;
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41, 0x01, 0x6a, 0x21]);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x0c, 0x00, 0x0b, 0x0b]);
        self.control_depth -= 2;
        self.output.extend([0x41, 0x00, 0x21]);
        write_u32(self.output, self.plan.status);
        Ok(())
    }
}
