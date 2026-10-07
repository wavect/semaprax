//! Explicit streaming-command lowering of the sealed reader and its views.

use crate::diagnostic::Diagnostic;
use crate::hir::{self, Place, ResolvedExpr, ResolvedHostCommandCall, ResolvedType};

use super::super::{backend_error, CEmitter, COutput, CValue};

impl<'a, O: COutput> CEmitter<'a, O> {
    fn require_stdin_stream_profile(&self) -> Result<(), Diagnostic> {
        if !self.output_profile.is_stdin_stream() {
            return Err(backend_error(
                "stdin reader requires the explicit native streaming-command profile",
            ));
        }
        Ok(())
    }

    pub(super) fn emit_stdin_stream_chunk(
        &mut self,
        expr: &ResolvedExpr,
        place: &Place,
    ) -> Result<CValue, Diagnostic> {
        self.require_stdin_stream_profile()?;
        if !place.projections.is_empty() {
            return Err(backend_error("stdin chunk requires the whole reader place"));
        }
        let reader = self.emit_place(place)?;
        self.require_type(
            &reader.ty,
            &crate::stdin_stream_ops::reader(),
            "stdin chunk reader",
        )?;
        self.require_type(&expr.ty, &ResolvedType::SliceU8, "stdin chunk result")?;
        let temporary = self.temporary(&ResolvedType::SliceU8)?;
        self.line(&format!(
            "{temporary} = spx_stdin_stream_chunk_v1(spx_ctx, {});",
            reader.code
        ));
        Ok(CValue {
            code: temporary,
            ty: ResolvedType::SliceU8,
        })
    }

    pub(super) fn emit_stdin_stream_eof(
        &mut self,
        expr: &ResolvedExpr,
        args: &[ResolvedExpr],
        type_arguments: &[ResolvedType],
    ) -> Result<CValue, Diagnostic> {
        self.require_stdin_stream_profile()?;
        let [argument] = args else {
            return Err(backend_error("stdin EOF requires one reader argument"));
        };
        if !type_arguments.is_empty() {
            return Err(backend_error("stdin EOF has no type arguments"));
        }
        let reader = self.emit_expr(argument)?;
        self.require_type(
            &reader.ty,
            &crate::stdin_stream_ops::reader(),
            "stdin EOF reader",
        )?;
        self.require_type(&expr.ty, &ResolvedType::Bool, "stdin EOF result")?;
        let temporary = self.temporary(&ResolvedType::Bool)?;
        self.line(&format!(
            "{temporary} = spx_stdin_stream_eof_v1(spx_ctx, {});",
            reader.code
        ));
        Ok(CValue {
            code: temporary,
            ty: ResolvedType::Bool,
        })
    }

    pub(super) fn emit_stdin_stream_host(
        &mut self,
        expr: &ResolvedExpr,
        call: &ResolvedHostCommandCall,
    ) -> Result<CValue, Diagnostic> {
        self.require_stdin_stream_profile()?;
        let ty = crate::stdin_stream_ops::reader();
        self.require_type(&expr.ty, &ty, "stdin reader result")?;
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("stdin reader has no cleanup plan"))?;
        let destination = plan
            .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
            .to_owned();
        match call.operation {
            hir::ResolvedHostCommandOperation::StdinStreamOpen => {
                if !call.args.is_empty() {
                    return Err(backend_error("stdin stream Open has no arguments"));
                }
                self.line(&format!(
                    "spx_status = spx_host_stdin_stream_open_v1(spx_ctx, &{destination});"
                ));
            }
            hir::ResolvedHostCommandOperation::StdinStreamNext => {
                let [argument] = call.args.as_slice() else {
                    return Err(backend_error("stdin stream Next requires one owned reader"));
                };
                let reader = self.emit_expr(argument)?;
                self.require_type(&reader.ty, &ty, "stdin stream Next reader")?;
                let reader = self.stage_bytes_call_argument(
                    &expr.id,
                    0,
                    argument,
                    hir::OwnershipMode::Own,
                    reader,
                )?;
                // Keep the staged owner live until success: failure cleanup owns
                // it, and the host helper publishes no result on failure.
                self.line(&format!(
                    "spx_status = spx_host_stdin_stream_next_v1(spx_ctx, {}, &{destination});",
                    reader.code
                ));
            }
            _ => {
                return Err(backend_error(
                    "foreign operation reached stdin stream lowering",
                ))
            }
        }
        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        if call.operation == hir::ResolvedHostCommandOperation::StdinStreamNext {
            // The successful host call consumed the canonical staged argument.
            // Failure leaves that epoch live for the caller's cleanup plan.
            let (_, flag, _) = plan.call_argument(&expr.id, 0)?;
            self.line(&format!("{flag} = false;"));
        }
        for line in plan.apply_at(&expr.id)?.lines() {
            self.line(line);
        }
        let code = plan
            .result_at(&expr.id)
            .ok_or_else(|| backend_error("stdin reader has no canonical result transfer"))?
            .to_owned();
        Ok(CValue { code, ty })
    }
}
