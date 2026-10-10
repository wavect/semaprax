//! Source-bound literal-format lowering. All dynamic values enter ordinary
//! CallArgument epochs before any rendering, then one private worker owns the
//! String values and its partial accumulator through every failure exit.
use super::{
    backend_error, c_string, is_direct_plan_owned, resolved_expr_children, CEmitter, COutput,
    CValue,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedType};

pub(super) fn emit_runtime(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    strings: super::StringRuntimeSelection,
) {
    if strings.length_delimited && program_uses(program, strings.include_instances) {
        output.push_str(RUNTIME_C);
    }
}

pub(super) fn program_uses(program: &ResolvedProgram, include_instances: bool) -> bool {
    let mut pending = Vec::new();
    for function in super::string_runtime_functions(program, include_instances) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if matches!(expression.kind, ResolvedExprKind::LiteralFormat { .. }) {
            return true;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_literal_format(
        &mut self,
        expression: &ResolvedExpr,
        template: &str,
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        self.require_type(
            &expression.ty,
            &ResolvedType::String,
            "literal format result",
        )?;
        if !self.output_profile.string_runtime().length_delimited {
            return Err(backend_error(
                "literal format requires length-delimited native String",
            ));
        }
        let pieces = crate::literal_format::scan(template)
            .map_err(|reason| backend_error(reason.message()))?;
        if crate::literal_format::field_count(&pieces) != args.len() {
            return Err(backend_error(
                "literal format field count changed after validation",
            ));
        }
        let mut values = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            let value = self.emit_expr(arg)?;
            self.require_type(&value.ty, &arg.ty, "literal format argument")?;
            values.push(if arg.ty == ResolvedType::String {
                self.stage_bytes_call_argument(
                    &expression.id,
                    index,
                    arg,
                    OwnershipMode::Own,
                    value,
                )?
            } else {
                let snapshot = self.temporary(&value.ty)?;
                self.line(&format!("{snapshot} = {};", value.code));
                CValue {
                    code: snapshot,
                    ty: value.ty,
                }
            });
        }
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("literal format lacks cleanup plan"))?;
        plan.authenticate_literal_format_commit(&expression.id, args)?;
        let result = plan
            .value(&crate::cleanup_plan::StorageId::Temporary(
                expression.id.clone(),
            ))?
            .to_owned();
        let result_destination = plan
            .result_at(&expression.id)
            .ok_or_else(|| backend_error("literal format result lacks canonical transfer"))?
            .to_owned();
        let flags = values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                if value.ty == ResolvedType::String {
                    plan.call_argument(&expression.id, index as u32)
                        .map(|(_, flag, _)| Some(flag.to_owned()))
                } else {
                    Ok(None)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let serial = self.next_local;
        self.next_local += 1;
        let prefix = format!("spx_fmt_{serial}");
        let failure = format!("{prefix}_failure");
        let done = format!("{prefix}_done");
        for (index, value) in values.iter().enumerate() {
            if let Some(flag) = &flags[index] {
                self.line(&format!("char *{prefix}_owner_{index} = {};", value.code));
                self.line(&format!("{flag} = false;"));
            }
        }
        self.line(&format!(
            "char *{prefix}_acc = NULL, *{prefix}_part = NULL, *{prefix}_next = NULL;"
        ));
        self.line(&format!(
            "spx_status = spx_format_copy_v1(spx_ctx, \"\", UINT64_C(0), &{prefix}_acc);"
        ));
        self.line(&format!(
            "if (spx_status != SPX_STATUS_SUCCESS) goto {failure};"
        ));
        let mut field = 0;
        for piece in pieces {
            match piece {
                crate::literal_format::Piece::Literal(text) => {
                    self.line(&format!("spx_status = spx_format_copy_v1(spx_ctx, \"{}\", UINT64_C({}), &{prefix}_part);", c_string(&text), text.len()));
                    self.line(&format!(
                        "if (spx_status != SPX_STATUS_SUCCESS) goto {failure};"
                    ));
                }
                crate::literal_format::Piece::Field => {
                    let value = &values[field];
                    match value.ty {
                        ResolvedType::String => {
                            self.line(&format!("{prefix}_part = {prefix}_owner_{field};"));
                            self.line(&format!("{prefix}_owner_{field} = NULL;"));
                        }
                        ResolvedType::I64 | ResolvedType::U8 => {
                            self.line(&format!("spx_status = spx_format_i64_v1(spx_ctx, (int64_t)({}), &{prefix}_part);", value.code));
                            self.line(&format!(
                                "if (spx_status != SPX_STATUS_SUCCESS) goto {failure};"
                            ));
                        }
                        ResolvedType::Usize => {
                            self.line(&format!("spx_status = spx_format_u64_v1(spx_ctx, (uint64_t)({}), &{prefix}_part);", value.code));
                            self.line(&format!(
                                "if (spx_status != SPX_STATUS_SUCCESS) goto {failure};"
                            ));
                        }
                        ResolvedType::Bool => {
                            self.line(&format!("spx_status = spx_format_copy_v1(spx_ctx, ({}) ? \"true\" : \"false\", ({}) ? UINT64_C(4) : UINT64_C(5), &{prefix}_part);", value.code, value.code));
                            self.line(&format!(
                                "if (spx_status != SPX_STATUS_SUCCESS) goto {failure};"
                            ));
                        }
                        _ => {
                            return Err(backend_error(
                                "literal format argument has unsupported type",
                            ))
                        }
                    }
                    field += 1;
                }
            }
            self.line(&format!("spx_status = spx_format_join_v1(spx_ctx, {prefix}_acc, {prefix}_part, &{prefix}_next);"));
            self.line(&format!(
                "if (spx_status != SPX_STATUS_SUCCESS) goto {failure};"
            ));
            self.line(&format!(
                "spx_string_drop({prefix}_acc); spx_string_drop({prefix}_part);"
            ));
            self.line(&format!(
                "{prefix}_acc = {prefix}_next; {prefix}_part = NULL; {prefix}_next = NULL;"
            ));
        }
        self.line(&format!("{result} = {prefix}_acc; {prefix}_acc = NULL;"));
        self.line(&format!("goto {done};"));
        self.line(&format!("{failure}:"));
        self.line(&format!("if ({prefix}_acc) spx_string_drop({prefix}_acc);"));
        self.line(&format!(
            "if ({prefix}_part) spx_string_drop({prefix}_part);"
        ));
        self.line(&format!(
            "if ({prefix}_next) spx_string_drop({prefix}_next);"
        ));
        for (index, value) in values.iter().enumerate() {
            if value.ty == ResolvedType::String {
                self.line(&format!(
                    "if ({prefix}_owner_{index}) spx_string_drop({prefix}_owner_{index});"
                ));
            }
        }
        self.line("goto spx_epilogue;");
        self.line(&format!("{done}:;"));
        if is_direct_plan_owned(self.program, &ResolvedType::String) {
            self.apply_owned_plan_at_value(
                &expression.id,
                &CValue {
                    code: result,
                    ty: ResolvedType::String,
                },
            )?;
            Ok(CValue {
                code: result_destination,
                ty: ResolvedType::String,
            })
        } else {
            Ok(CValue {
                code: result,
                ty: ResolvedType::String,
            })
        }
    }
}

pub(super) const RUNTIME_C: &str = r#"#include <inttypes.h>
#define SPX_FORMAT_STATUS_DOMAIN_V1 "semaprax.string-format.v1"
static __attribute__((unused)) spx_status_token spx_format_failure_v1(struct spx_context *ctx) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(ctx, SPX_FORMAT_STATUS_DOMAIN_V1, UINT32_C(1),
            SPX_STATUS_CLASS_ADAPTER, SPX_RETRYABILITY_FALSE, &token))
        spx_runtime_invariant_failure("literal format status could not be recorded");
    return token;
}
static __attribute__((unused)) spx_status_token spx_format_copy_v1(
    struct spx_context *ctx, const char *source, uint64_t length, char **out
) {
    if (length > (uint64_t)SIZE_MAX - (uint64_t)offsetof(struct spx_string_v10, data) - UINT64_C(1))
        return spx_format_failure_v1(ctx);
    struct spx_string_v10 *value = (struct spx_string_v10 *)malloc(
        offsetof(struct spx_string_v10, data) + (size_t)length + 1u);
    if (!value) return spx_format_failure_v1(ctx);
    value->len = length;
    if (length) memcpy(value->data, source, (size_t)length);
    value->data[length] = '\0';
    *out = value->data;
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_format_join_v1(
    struct spx_context *ctx, const char *left, const char *right, char **out
) {
    uint64_t l = spx_string_length_v10(left), r = spx_string_length_v10(right);
    if (r > UINT64_MAX - l) return spx_format_failure_v1(ctx);
    uint64_t length = l + r;
    if (length > (uint64_t)SIZE_MAX - (uint64_t)offsetof(struct spx_string_v10, data) - UINT64_C(1))
        return spx_format_failure_v1(ctx);
    struct spx_string_v10 *value = (struct spx_string_v10 *)malloc(
        offsetof(struct spx_string_v10, data) + (size_t)length + 1u);
    if (!value) return spx_format_failure_v1(ctx);
    value->len = length;
    if (l) memcpy(value->data, left, (size_t)l);
    if (r) memcpy(value->data + l, right, (size_t)r);
    value->data[length] = '\0';
    *out = value->data;
    return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_format_i64_v1(struct spx_context *ctx, int64_t value, char **out) {
    char text[32]; int n = snprintf(text, sizeof(text), "%" PRId64, value);
    if (n < 0 || n >= (int)sizeof(text)) return spx_format_failure_v1(ctx);
    return spx_format_copy_v1(ctx, text, (uint64_t)n, out);
}
static __attribute__((unused)) spx_status_token spx_format_u64_v1(struct spx_context *ctx, uint64_t value, char **out) {
    char text[32]; int n = snprintf(text, sizeof(text), "%" PRIu64, value);
    if (n < 0 || n >= (int)sizeof(text)) return spx_format_failure_v1(ctx);
    return spx_format_copy_v1(ctx, text, (uint64_t)n, out);
}
"#;
