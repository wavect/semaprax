//! Native lowering for Text Toolkit v1 (`docs/TEXT-TOOLKIT-V1.md`).
//!
//! The helpers read the length-delimited String header, so they exist only in
//! profiles that select that representation. Fallible helpers write their
//! out-parameter only on success; the caller jumps to the epilogue first, so a
//! failed call never initializes its cleanup-plan result slot.

use super::{backend_error, c_case_symbol, c_field_symbol, CEmitter, COutput, CValue};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedType};
use crate::string_ops::StringOp;

/// Whether any resolved body or contract reaches a Text Toolkit v1 operation.
pub(super) fn program_uses_text_toolkit(
    program: &crate::hir::ResolvedProgram,
    include_instances: bool,
) -> bool {
    let mut pending = Vec::new();
    for function in super::string_runtime_functions(program, include_instances) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if let crate::hir::ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str()).is_some_and(StringOp::is_text_toolkit) {
                return true;
            }
        }
        pending.extend(super::function_value::resolved_expr_children(expression));
    }
    false
}

impl<O: COutput> CEmitter<'_, O> {
    /// Lower one Text Toolkit v1 call into `temporary`, whose storage the
    /// caller selected from the operation's result type.
    pub(super) fn emit_text_toolkit_op(
        &mut self,
        op: StringOp,
        arguments: &[CValue],
        temporary: &str,
    ) -> Result<(), Diagnostic> {
        if !self.output_profile.string_runtime().length_delimited {
            return Err(backend_error(format!(
                "Text Toolkit v1 operation `{}` requires a length-delimited native String profile",
                op.name()
            )));
        }
        let argument = |index: usize| arguments[index].code.as_str();
        let fallible = match op {
            StringOp::Slice => format!(
                "spx_string_slice_v1(spx_ctx, {}, {}, {}, &{temporary})",
                argument(0),
                argument(1),
                argument(2)
            ),
            StringOp::Find => format!(
                "spx_string_find_v1(spx_ctx, {}, {}, {}, &{temporary})",
                argument(0),
                argument(1),
                argument(2)
            ),
            StringOp::ByteAt => format!(
                "spx_string_byte_at_v1(spx_ctx, {}, {}, &{temporary})",
                argument(0),
                argument(1)
            ),
            StringOp::FileReadText => {
                if !matches!(
                    self.output_profile,
                    super::NativeOutputProfile::SourceCommand
                        | super::NativeOutputProfile::SourceResourceCommand
                ) {
                    return Err(backend_error(
                        "file_read_text requires the native single-file command profile",
                    ));
                }
                format!(
                    "spx_host_file_read_text_v1(spx_ctx, {}, &{temporary})",
                    argument(0)
                )
            }
            StringOp::Trim => {
                self.line(&format!(
                    "{temporary} = spx_string_trim_v1({});",
                    argument(0)
                ));
                return Ok(());
            }
            StringOp::ToI64 => return self.emit_text_to_i64(argument(0), temporary),
            _ => return Err(backend_error("operation is not part of Text Toolkit v1")),
        };
        self.line(&format!("spx_status = {fallible};"));
        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        Ok(())
    }

    fn emit_text_to_i64(&mut self, input: &str, temporary: &str) -> Result<(), Diagnostic> {
        let layout = self.variant_layout(&crate::string_ops::option_i64())?;
        let none_id = DeclarationId::new(crate::prelude::OPTION_NONE_ID);
        let some_id = DeclarationId::new(crate::prelude::OPTION_SOME_ID);
        let value_id = DeclarationId::new(crate::prelude::OPTION_SOME_VALUE_ID);
        let none = layout
            .case(&none_id)
            .ok_or_else(|| backend_error("Option<i64> layout has no compiler-owned None case"))?;
        let some = layout
            .case(&some_id)
            .ok_or_else(|| backend_error("Option<i64> layout has no compiler-owned Some case"))?;
        let field = some.field(&value_id).ok_or_else(|| {
            backend_error("Option<i64> layout has no compiler-owned Some payload")
        })?;
        self.require_type(&field.ty, &ResolvedType::I64, "string_to_i64 Some payload")?;
        let payload = format!(
            "{temporary}.spx_payload.{}.{}",
            c_case_symbol(&some_id),
            c_field_symbol(&value_id)
        );
        self.line(&format!("memset(&{temporary}, 0, sizeof({temporary}));"));
        self.line(&format!(
            "{temporary}.spx_tag = spx_string_to_i64_v1({input}, &{payload}) ? UINT32_C({}) : UINT32_C({});",
            some.tag, none.tag
        ));
        Ok(())
    }
}

/// Text Toolkit v1 helpers over the length-delimited String header.
pub(super) const RUNTIME_C: &str = r#"#define SPX_TEXT_STATUS_DOMAIN_V1 "semaprax.text.v1"
static __attribute__((unused)) spx_status_token spx_text_failure_v1(
    struct spx_context *spx_ctx, uint32_t code
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(spx_ctx, SPX_TEXT_STATUS_DOMAIN_V1, code,
            SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token))
        spx_runtime_invariant_failure("text status could not be recorded");
    return token;
}
static __attribute__((unused)) bool spx_text_boundary_v1(const char *value, uint64_t length, uint64_t offset) {
    return offset == UINT64_C(0) || offset == length ||
        (((const uint8_t *)value)[offset] & UINT8_C(0xc0)) != UINT8_C(0x80);
}
static __attribute__((unused)) spx_status_token spx_string_slice_v1(
    struct spx_context *spx_ctx, const char *value, int64_t start, int64_t end, char **result_out
) {
    uint64_t length = spx_string_length_v10(value);
    if (start < INT64_C(0) || start > end || (uint64_t)end > length)
        return spx_text_failure_v1(spx_ctx, UINT32_C(1));
    if (!spx_text_boundary_v1(value, length, (uint64_t)start) ||
        !spx_text_boundary_v1(value, length, (uint64_t)end))
        return spx_text_failure_v1(spx_ctx, UINT32_C(2));
    *result_out = spx_string_from_literal(value + start, (uint64_t)(end - start));
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_string_find_v1(
    struct spx_context *spx_ctx, const char *value, const char *needle, int64_t from, int64_t *result_out
) {
    uint64_t length = spx_string_length_v10(value), needle_length = spx_string_length_v10(needle);
    if (from < INT64_C(0) || (uint64_t)from > length) return spx_text_failure_v1(spx_ctx, UINT32_C(1));
    int64_t found = INT64_C(-1);
    if (needle_length == UINT64_C(0)) found = from;
    else if (needle_length <= length)
        for (uint64_t offset = (uint64_t)from; offset <= length - needle_length; ++offset)
            if (memcmp(value + offset, needle, (size_t)needle_length) == 0) { found = (int64_t)offset; break; }
    *result_out = found;
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_string_byte_at_v1(
    struct spx_context *spx_ctx, const char *value, int64_t index, int64_t *result_out
) {
    if (index < INT64_C(0) || (uint64_t)index >= spx_string_length_v10(value))
        return spx_text_failure_v1(spx_ctx, UINT32_C(1));
    *result_out = (int64_t)((const uint8_t *)value)[index];
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) bool spx_text_space_v1(uint8_t byte) {
    return byte == UINT8_C(0x20) || (byte >= UINT8_C(0x09) && byte <= UINT8_C(0x0d));
}
static __attribute__((unused)) char *spx_string_trim_v1(const char *value) {
    const uint8_t *bytes = (const uint8_t *)value;
    uint64_t start = UINT64_C(0), end = spx_string_length_v10(value);
    while (start < end && spx_text_space_v1(bytes[start])) ++start;
    while (end > start && spx_text_space_v1(bytes[end - UINT64_C(1)])) --end;
    return spx_string_from_literal(value + start, end - start);
}
static __attribute__((unused)) bool spx_string_to_i64_v1(const char *value, int64_t *result_out) {
    uint64_t length = spx_string_length_v10(value), offset = UINT64_C(0);
    bool negative = length != UINT64_C(0) && value[0] == '-';
    if (negative) offset = UINT64_C(1);
    if (offset == length) return false;
    int64_t parsed = INT64_C(0);
    for (; offset < length; ++offset) {
        uint8_t byte = ((const uint8_t *)value)[offset];
        if (byte < (uint8_t)'0' || byte > (uint8_t)'9') return false;
        int64_t digit = (int64_t)(byte - (uint8_t)'0');
        if (negative) {
            if (parsed < (INT64_MIN + digit) / INT64_C(10)) return false;
            parsed = parsed * INT64_C(10) - digit;
        } else {
            if (parsed > (INT64_MAX - digit) / INT64_C(10)) return false;
            parsed = parsed * INT64_C(10) + digit;
        }
    }
    *result_out = parsed;
    return true;
}
"#;
