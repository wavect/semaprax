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
            arguments.push(if op.param_ownership(index) == hir::OwnershipMode::Own {
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
            crate::string_ops::StringOp::Compare
            | crate::string_ops::StringOp::MapNew
            | crate::string_ops::StringOp::MapAdd
            | crate::string_ops::StringOp::MapSet
            | crate::string_ops::StringOp::MapGetOr
            | crate::string_ops::StringOp::MapHas
            | crate::string_ops::StringOp::MapLen
            | crate::string_ops::StringOp::MapKeyAt
            | crate::string_ops::StringOp::MapValueAt
            | crate::string_ops::StringOp::MapRemove => {
                self.emit_collection_op(op, &arguments, &temporary, expression)?;
            }
            crate::string_ops::StringOp::FromStr
            | crate::string_ops::StringOp::F64FromI64
            | crate::string_ops::StringOp::I64FromF64
            | crate::string_ops::StringOp::UsizeFromI64
            | crate::string_ops::StringOp::I64FromU8
            | crate::string_ops::StringOp::I64FromI32
            | crate::string_ops::StringOp::UsizeFromU8
            | crate::string_ops::StringOp::U8FromI64
            | crate::string_ops::StringOp::CharFromU8
            | crate::string_ops::StringOp::CharFromI64
            | crate::string_ops::StringOp::I64FromUsize => {
                self.emit_conversion_op(op, &arguments[0].code, &temporary)?;
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

    /// Conversions v1 (`docs/LANGUAGE-ERGONOMICS-V1.md`). A checked operand
    /// is copied into a fresh local so it is read exactly once; a value the
    /// target type cannot hold records the checked `semaprax.convert.v1`
    /// status and leaves the result slot unwritten.
    fn emit_conversion_op(
        &mut self,
        op: crate::string_ops::StringOp,
        operand: &str,
        temporary: &str,
    ) -> Result<(), Diagnostic> {
        use crate::string_ops::{StringOp, CONVERT_NAN_CODE, CONVERT_OUT_OF_RANGE_CODE};
        let (input_type, target) = match op {
            StringOp::FromStr => {
                self.line(&format!(
                    "{temporary} = spx_string_from_literal((const char *)({operand}).data, ({operand}).len);"
                ));
                return Ok(());
            }
            StringOp::F64FromI64 => {
                // C converts to the nearest double, ties to even, under the
                // default rounding mode the runtime never changes.
                self.line(&format!("{temporary} = (double)({operand});"));
                return Ok(());
            }
            StringOp::I64FromU8 | StringOp::I64FromI32 | StringOp::UsizeFromU8 => {
                let target = if op == StringOp::UsizeFromU8 {
                    "uint64_t"
                } else {
                    "int64_t"
                };
                self.line(&format!("{temporary} = ({target})({operand});"));
                return Ok(());
            }
            StringOp::CharFromU8 => {
                self.line(&format!("{temporary} = (uint32_t)({operand});"));
                return Ok(());
            }
            StringOp::I64FromF64 => (ResolvedType::F64, "int64_t"),
            StringOp::UsizeFromI64 => (ResolvedType::I64, "uint64_t"),
            StringOp::I64FromUsize => (ResolvedType::Usize, "int64_t"),
            StringOp::U8FromI64 => (ResolvedType::I64, "uint8_t"),
            StringOp::CharFromI64 => (ResolvedType::I64, "uint32_t"),
            _ => {
                return Err(super::backend_error(
                    "operation is not a Conversions v1 call",
                ))
            }
        };
        let input = self.temporary(&input_type)?;
        self.line(&format!("{input} = {operand};"));
        let failures = match op {
            StringOp::I64FromF64 => vec![
                (format!("{input} != {input}"), CONVERT_NAN_CODE),
                (
                    format!(
                        "!({input} >= -9223372036854775808.0 && {input} < 9223372036854775808.0)"
                    ),
                    CONVERT_OUT_OF_RANGE_CODE,
                ),
            ],
            StringOp::UsizeFromI64 => {
                vec![(format!("{input} < INT64_C(0)"), CONVERT_OUT_OF_RANGE_CODE)]
            }
            StringOp::U8FromI64 => vec![(
                format!("{input} < INT64_C(0) || {input} > INT64_C(255)"),
                CONVERT_OUT_OF_RANGE_CODE,
            )],
            StringOp::CharFromI64 => vec![(
                format!("{input} < INT64_C(0) || {input} > INT64_C(1114111) || ({input} >= INT64_C(55296) && {input} <= INT64_C(57343))"),
                CONVERT_OUT_OF_RANGE_CODE,
            )],
            _ => vec![(
                format!("{input} > (uint64_t)INT64_MAX"),
                CONVERT_OUT_OF_RANGE_CODE,
            )],
        };
        for (condition, code) in failures {
            self.line(&format!(
                "if ({condition}) {{ if (!spx_status_record_adapter(spx_ctx, \"{}\", UINT32_C({code}), SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &spx_status)) spx_runtime_invariant_failure(\"conversion status could not be recorded\"); goto spx_epilogue; }}",
                crate::string_ops::CONVERT_STATUS_DOMAIN
            ));
        }
        self.line(&format!("{temporary} = ({target}){input};"));
        Ok(())
    }
}
