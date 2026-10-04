//! Checked native lowering for compiler-owned immutable List operations.

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedType};
use crate::list_ops::{self, ListOp};

use super::super::{backend_error, c_case_symbol, c_field_symbol, CEmitter, COutput, CValue};

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_list_op(
        &mut self,
        expr: &ResolvedExpr,
        op: ListOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        if !type_arguments.is_empty() || args.len() != op.argument_count() {
            return Err(backend_error(
                "immutable List operation has invalid resolved shape",
            ));
        }
        self.require_type(
            &expr.ty,
            &op.resolved_return_type(),
            "immutable List result",
        )?;
        match op {
            ListOp::Nil => Ok(CValue {
                code: "NULL".to_owned(),
                ty: expr.ty.clone(),
            }),
            ListOp::Cons => {
                let head = self.emit_expr(&args[0])?;
                self.require_type(&head.ty, &ResolvedType::I64, "immutable List head")?;
                let tail = self.emit_expr(&args[1])?;
                self.require_type(&tail.ty, &list_ops::resolved_list(), "immutable List tail")?;
                let destination = self.temporary(&expr.ty)?;
                self.line(&format!(
                    "spx_status = spx_list_cons(spx_ctx, {}, {}, &{destination});",
                    head.code, tail.code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                Ok(CValue {
                    code: destination,
                    ty: expr.ty.clone(),
                })
            }
            ListOp::Uncons => {
                let list = self.emit_expr(&args[0])?;
                self.require_type(
                    &list.ty,
                    &list_ops::resolved_list(),
                    "immutable List operand",
                )?;
                let destination = self.temporary(&expr.ty)?;
                self.line(&format!(
                    "memset(&{destination}, 0, sizeof({destination}));"
                ));
                self.line(&format!("if ({} != NULL) {{", list.code));
                self.indent += 1;
                self.line(&format!("{destination}.spx_tag = UINT32_C(1);"));
                let case = c_case_symbol(&crate::hir::DeclarationId::new(list_ops::CONS_CASE_ID));
                let head = c_field_symbol(&crate::hir::DeclarationId::new(list_ops::HEAD_ID));
                let tail = c_field_symbol(&crate::hir::DeclarationId::new(list_ops::TAIL_ID));
                self.line(&format!(
                    "{destination}.spx_payload.{case}.{head} = {}->head;",
                    list.code
                ));
                self.line(&format!(
                    "{destination}.spx_payload.{case}.{tail} = {}->tail;",
                    list.code
                ));
                self.indent -= 1;
                self.line("}");
                Ok(CValue {
                    code: destination,
                    ty: expr.ty.clone(),
                })
            }
        }
    }
}
