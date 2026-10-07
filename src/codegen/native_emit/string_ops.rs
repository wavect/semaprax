//! Native lowering for compiler-owned owned-string operations.

use super::{backend_error, c_value_type, is_direct_plan_owned, CEmitter, COutput, CValue};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ExpressionId, ResolvedExpr, ResolvedType};

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(in crate::codegen::native_emit) fn temporary(
        &mut self,
        ty: &ResolvedType,
    ) -> Result<String, Diagnostic> {
        if matches!(ty, ResolvedType::ArrayU8(0)) {
            return Ok("UINT8_C(0)".to_owned());
        }
        let name = format!("spx_internal_{}", self.next_local);
        self.next_local += 1;
        if matches!(ty, ResolvedType::String) {
            if let Some(cells) = &mut self.owned_strings {
                cells.register(&name, true)?;
                self.string_require_dead(&name);
                return Ok(name);
            }
        }
        // A runtime helper writes its out-parameter only on success and the
        // caller jumps to the epilogue first, but a compiler that cannot prove
        // that across the status check reports the slot as maybe-uninitialized.
        // Zero it the way the function result slot already is. Mark as unused
        // to avoid -Werror=unused-but-set-variable when the String path is
        // later optimized away and the slot remains set but not read.
        self.line(&format!(
            "{} __attribute__((unused)) {name} = {{0}};",
            c_value_type(self.program, self.resource_abi, ty)?
        ));
        Ok(name)
    }

    pub(in crate::codegen::native_emit) fn call_result_temporary(
        &mut self,
        ty: &ResolvedType,
    ) -> Result<String, Diagnostic> {
        if matches!(ty, ResolvedType::ArrayU8(0)) {
            let name = format!("spx_internal_{}", self.next_local);
            self.next_local += 1;
            self.line(&format!("uint8_t {name} = UINT8_C(0);"));
            Ok(name)
        } else {
            self.temporary(ty)
        }
    }

    pub(super) fn emit_string_op(
        &mut self,
        op: crate::string_ops::StringOp,
        args: &[ResolvedExpr],
        result_type: &ResolvedType,
        expression: &ExpressionId,
    ) -> Result<CValue, Diagnostic> {
        // Arguments evaluate left-to-right. Only concat reaches a consuming
        // CallArgument epoch; borrowed temporaries remain plan-live until the
        // canonical region finalizer selects their cleanup.
        let mut arguments = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let value = self.emit_expr(argument)?;
            self.require_type(
                &value.ty,
                &op.param_types()[index],
                "string operation argument",
            )?;
            arguments.push(if op.consumes_arguments() {
                self.stage_bytes_call_argument(
                    expression,
                    index,
                    argument,
                    hir::OwnershipMode::Own,
                    value,
                )?
            } else {
                value
            });
        }
        self.require_type(result_type, &op.return_type(), "string operation result")?;
        let temporary = if is_direct_plan_owned(self.program, &op.return_type()) {
            self.bytes_plan
                .ok_or_else(|| backend_error("owned String operation has no cleanup plan"))?
                .value(&crate::cleanup_plan::StorageId::Temporary(
                    expression.clone(),
                ))?
                .to_owned()
        } else {
            self.temporary(&op.return_type())?
        };
        match op {
            crate::string_ops::StringOp::Len => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_len({input});"));
            }
            crate::string_ops::StringOp::IsEmpty => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_is_empty({input});"));
            }
            crate::string_ops::StringOp::Concat => {
                let left = &arguments[0].code;
                let right = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_concat({left}, {right});"
                ));
                self.string_drop(left);
                self.string_drop(right);
            }
            crate::string_ops::StringOp::StartsWith => {
                let value = &arguments[0].code;
                let prefix = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_starts_with({value}, {prefix});"
                ));
            }
            crate::string_ops::StringOp::Contains => {
                let value = &arguments[0].code;
                let needle = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_contains({value}, {needle});"
                ));
            }
            crate::string_ops::StringOp::LenChars => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_len_chars({input});"));
            }
            crate::string_ops::StringOp::FromChar => {
                let scalar = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_from_char({scalar});"));
            }
            crate::string_ops::StringOp::FromI64 => {
                let value = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_from_i64({value});"));
            }
            crate::string_ops::StringOp::FromUsize => {
                let value = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_from_usize({value});"));
            }
            crate::string_ops::StringOp::Slice
            | crate::string_ops::StringOp::Find
            | crate::string_ops::StringOp::ToI64
            | crate::string_ops::StringOp::Trim
            | crate::string_ops::StringOp::ByteAt
            | crate::string_ops::StringOp::FileReadText => {
                self.emit_text_toolkit_op(op, &arguments, &temporary)?;
            }
        }
        let code = if is_direct_plan_owned(self.program, &op.return_type()) {
            self.apply_owned_plan_at_value(
                expression,
                &CValue {
                    code: temporary.clone(),
                    ty: op.return_type(),
                },
            )?;
            self.bytes_plan
                .and_then(|plan| plan.result_at(expression))
                .ok_or_else(|| {
                    backend_error("owned String operation has no canonical result transfer")
                })?
                .to_owned()
        } else {
            temporary
        };
        Ok(CValue {
            code,
            ty: op.return_type(),
        })
    }
}
