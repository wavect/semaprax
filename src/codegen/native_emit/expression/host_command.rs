//! Native lowering of the closed host-command operation family: Language
//! Command I/O v1 (arguments, stdin, two-channel output) and Bounded Language
//! Network I/O v1.
//!
//! `emit_host_command_expr` moved here verbatim from `expression.rs`; the
//! network arm now delegates to `emit_network_command_expr` instead of
//! failing closed.

use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedType};

use super::super::{backend_error, CEmitter, COutput, CValue, NativeOutputProfile};

// `format!` resolves to the bounded codegen macro declared before
// `mod native_emit`; it must never fall back to `std::format!` here.
impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_host_command_expr(
        &mut self,
        expr: &ResolvedExpr,
    ) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::HostCommandCall(call) => {
                use hir::ResolvedHostCommandOperation as Operation;

                if !self.output_profile.is_language_command() {
                    return Err(backend_error(
                        "command I/O operation requires the native language-command profile",
                    ));
                }
                if call.expression != expr.id {
                    return Err(backend_error(
                        "command I/O call identity disagrees with its expression",
                    ));
                }
                let expected = crate::command_io_ops::return_type(call.operation);
                self.require_type(&expr.ty, &expected, "command I/O result")?;
                match call.operation {
                    Operation::NetConnect
                    | Operation::NetSend
                    | Operation::NetRecv
                    | Operation::NetStreamStdout
                    | Operation::NetWait
                    | Operation::NetClose => self.emit_network_command_expr(expr, call)?,
                    Operation::ArgsLen => {
                        if !call.args.is_empty() {
                            return Err(backend_error("args_len arity disagrees with HIR"));
                        }
                        let temporary = self.temporary(&ResolvedType::Usize)?;
                        self.line(&format!(
                            "spx_status = spx_host_args_len_v1(spx_ctx, &{temporary});"
                        ));
                        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                        CValue {
                            code: temporary,
                            ty: ResolvedType::Usize,
                        }
                    }
                    Operation::ArgUtf8 => {
                        let [argument] = call.args.as_slice() else {
                            return Err(backend_error("arg_utf8 arity disagrees with HIR"));
                        };
                        let argument = self.emit_expr(argument)?;
                        self.require_type(&argument.ty, &ResolvedType::Usize, "arg_utf8 index")?;
                        let temporary = self.temporary(&ResolvedType::Str)?;
                        self.line(&format!(
                            "spx_status = spx_host_arg_utf8_v1(spx_ctx, {}, &{temporary});",
                            argument.code
                        ));
                        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                        CValue {
                            code: temporary,
                            ty: ResolvedType::Str,
                        }
                    }
                    Operation::StdinRead => {
                        if !call.args.is_empty() {
                            return Err(backend_error("stdin_read arity disagrees with HIR"));
                        }
                        let plan = self.bytes_plan.ok_or_else(|| {
                            backend_error("stdin_read owned result has no cleanup plan")
                        })?;
                        let temporary = plan
                            .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                            .to_owned();
                        self.line(&format!(
                            "spx_status = spx_host_stdin_read_v1(spx_ctx, &{temporary});"
                        ));
                        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                        let transitions = plan.apply_at(&expr.id)?;
                        for line in transitions.lines() {
                            self.line(line);
                        }
                        CValue {
                            code: plan
                                .result_at(&expr.id)
                                .ok_or_else(|| {
                                    backend_error(
                                        "stdin_read has no canonical owned result transfer",
                                    )
                                })?
                                .to_owned(),
                            ty: ResolvedType::Bytes,
                        }
                    }
                    Operation::StderrWrite => {
                        let [argument] = call.args.as_slice() else {
                            return Err(backend_error("stderr_write arity disagrees with HIR"));
                        };
                        let value = self.emit_expr(argument)?;
                        self.require_type(
                            &value.ty,
                            &ResolvedType::SliceU8,
                            "stderr_write argument",
                        )?;
                        let temporary = self.temporary(&ResolvedType::Usize)?;
                        self.line(&format!(
                            "{temporary} = spx_host_command_stderr_write_v1(spx_ctx, {});",
                            value.code
                        ));
                        CValue {
                            code: temporary,
                            ty: ResolvedType::Usize,
                        }
                    }
                    Operation::StdoutAppend | Operation::StderrAppend => {
                        let [argument] = call.args.as_slice() else {
                            return Err(backend_error("command append arity disagrees with HIR"));
                        };
                        let value = self.emit_expr(argument)?;
                        self.require_type(
                            &value.ty,
                            &ResolvedType::SliceU8,
                            "command append argument",
                        )?;
                        let temporary = self.temporary(&ResolvedType::Usize)?;
                        let helper = match call.operation {
                            Operation::StdoutAppend => "spx_host_command_stdout_append_v1",
                            Operation::StderrAppend => "spx_host_command_stderr_append_v1",
                            _ => unreachable!("append operation was matched above"),
                        };
                        self.line(&format!(
                            "spx_status = {helper}(spx_ctx, {}, &{temporary});",
                            value.code
                        ));
                        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                        if let Some(plan) = self.bytes_plan {
                            let transitions = plan.apply_at(&expr.id)?;
                            for line in transitions.lines() {
                                self.line(line);
                            }
                        }
                        CValue {
                            code: temporary,
                            ty: ResolvedType::Usize,
                        }
                    }
                }
            }
            _ => unreachable!("non-HostCommandCall expression reached emit_host_command_expr"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    /// Lower one network operation: arguments evaluate left to right, the host
    /// helper runs, a failed status aborts to the epilogue before any owned
    /// slot initializes, and `net_recv` then follows the `stdin_read` owned
    /// result plan exactly.
    fn emit_network_command_expr(
        &mut self,
        expr: &ResolvedExpr,
        call: &hir::ResolvedHostCommandCall,
    ) -> Result<CValue, Diagnostic> {
        use crate::network_io_ops as ops;
        use hir::ResolvedHostCommandOperation as Operation;

        if self.output_profile != NativeOutputProfile::NetworkCommandIo {
            return Err(backend_error(
                "network operation requires the native network-command profile",
            ));
        }
        if !ops::is_network(call.operation) {
            return Err(backend_error(
                "non-network operation reached network lowering",
            ));
        }
        let name = ops::name(call.operation);
        if call.args.len() != ops::arity(call.operation) {
            return Err(backend_error(format!("{name} arity disagrees with HIR")));
        }
        let mut staged = Vec::with_capacity(call.args.len());
        for (index, argument) in call.args.iter().enumerate() {
            let value = self.emit_expr(argument)?;
            let expected = if ops::accepts_resolved(call.operation, index, &ResolvedType::SliceU8) {
                ResolvedType::SliceU8
            } else {
                ResolvedType::Usize
            };
            self.require_type(&value.ty, &expected, &format!("{name} argument {index}"))?;
            staged.push(value.code);
        }
        let helper = match call.operation {
            Operation::NetConnect => "spx_host_net_connect_v1",
            Operation::NetSend => "spx_host_net_send_v1",
            Operation::NetRecv => "spx_host_net_recv_v1",
            Operation::NetStreamStdout => "spx_host_net_stream_stdout_v1",
            Operation::NetWait => "spx_host_net_wait_v1",
            Operation::NetClose => "spx_host_net_close_v1",
            _ => unreachable!("network operation membership was checked above"),
        };
        let arguments = staged.join(", ");
        if call.operation == Operation::NetRecv {
            let plan = self
                .bytes_plan
                .ok_or_else(|| backend_error("net_recv owned result has no cleanup plan"))?;
            let temporary = plan
                .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                .to_owned();
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {arguments}, &{temporary});"
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            let transitions = plan.apply_at(&expr.id)?;
            for line in transitions.lines() {
                self.line(line);
            }
            return Ok(CValue {
                code: plan
                    .result_at(&expr.id)
                    .ok_or_else(|| {
                        backend_error("net_recv has no canonical owned result transfer")
                    })?
                    .to_owned(),
                ty: ResolvedType::Bytes,
            });
        }
        let temporary = self.temporary(&ResolvedType::Usize)?;
        self.line(&format!(
            "spx_status = {helper}(spx_ctx, {arguments}, &{temporary});"
        ));
        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        if let Some(plan) = self.bytes_plan {
            let transitions = plan.apply_at(&expr.id)?;
            for line in transitions.lines() {
                self.line(line);
            }
        }
        Ok(CValue {
            code: temporary,
            ty: ResolvedType::Usize,
        })
    }
}
