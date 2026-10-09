//! Additive integer conversions: direct extension and checked portable u64 ranges.
use super::{call_import, local_get, local_set, write_i64, ByteOutput, Diagnostic, LocalLayout};
use crate::hir::{ResolvedExprKind, ResolvedProgram};
use crate::string_ops::StringOp;

pub(super) const OUT_OF_RANGE_STATUS: i32 = 21;

pub(super) fn emit_scalar(
    output: &mut impl ByteOutput,
    op: StringOp,
    layout: &LocalLayout,
) -> Result<(), Diagnostic> {
    match op {
        StringOp::I64FromU8 | StringOp::UsizeFromU8 => output.push(0xad),
        StringOp::I64FromI32 => output.push(0xac),
        StringOp::CharFromU8 => {}
        StringOp::U8FromI64 => {
            let scratch = layout.wide_scratch[0];
            local_set(output, scratch);
            for (bound, comparison) in [(0, 0x53), (255, 0x55)] {
                local_get(output, scratch);
                output.push(0x42);
                write_i64(output, bound);
                output.push(comparison); // i64.lt_s / i64.gt_s
                output.extend_bytes(&[0x04, 0x40, 0x41]);
                write_i64(output, i64::from(OUT_OF_RANGE_STATUS));
                call_import(output, 6);
                output.extend_bytes(&[0x00, 0x0b]);
            }
            local_get(output, scratch);
            output.push(0xa7); // i32.wrap_i64, safe after the checked range
        }
        StringOp::CharFromI64 => {
            let scratch = layout.wide_scratch[0];
            local_set(output, scratch);
            for (bound, comparison) in [(0, 0x53), (0x10ffff, 0x55)] {
                local_get(output, scratch);
                output.push(0x42);
                write_i64(output, bound);
                output.push(comparison); // i64.lt_s / i64.gt_s
                scalar_range_failure(output);
            }
            local_get(output, scratch);
            output.push(0x42);
            write_i64(output, 0xd800);
            output.push(0x59); // i64.ge_s
            local_get(output, scratch);
            output.push(0x42);
            write_i64(output, 0xdfff);
            output.extend_bytes(&[0x57, 0x71]); // i64.le_s; i32.and
            scalar_range_failure(output);
            local_get(output, scratch);
            output.push(0xa7); // i32.wrap_i64, after Unicode scalar validation
        }
        StringOp::I64FromUsize | StringOp::UsizeFromI64 => {
            let scratch = layout.wide_scratch[0];
            local_set(output, scratch);
            local_get(output, scratch);
            output.push(0x42);
            write_i64(
                output,
                if op == StringOp::I64FromUsize {
                    i64::MAX
                } else {
                    0
                },
            );
            output.push(if op == StringOp::I64FromUsize {
                0x56
            } else {
                0x53
            }); // gt_u / lt_s
            output.extend_bytes(&[0x04, 0x40, 0x41]);
            write_i64(output, i64::from(OUT_OF_RANGE_STATUS));
            call_import(output, 6);
            output.extend_bytes(&[0x00, 0x0b]);
            local_get(output, scratch);
        }
        _ => return Err(crate::string_ops::text_toolkit_wasm_refusal(op)),
    }
    Ok(())
}

fn scalar_range_failure(output: &mut impl ByteOutput) {
    output.extend_bytes(&[0x04, 0x40, 0x41]);
    write_i64(output, i64::from(OUT_OF_RANGE_STATUS));
    call_import(output, 6);
    output.extend_bytes(&[0x00, 0x0b]);
}

pub(super) fn expression_uses_integer_conversion(expression: &crate::hir::ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if matches!(&expression.kind, ResolvedExprKind::Call { callee, .. } if crate::string_ops::by_id(callee.as_str()).is_some_and(|op| op.is_integer_conversion()))
        {
            return true;
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

pub(crate) fn used(program: &ResolvedProgram) -> bool {
    let mut pending = Vec::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if expression_uses_integer_conversion(expression) {
            return true;
        }
    }
    false
}

pub(super) fn runtime(program: &ResolvedProgram) -> String {
    let runtime = super::browser_runtime();
    if used(program) {
        runtime.replace("    spx_contract_fail: code => {", "    spx_contract_fail: code => {\n      if (code === 21) throw new SpxSemanticFailure(\"semaprax.convert.v1\", 1, \"SEMAPRAX conversion out of range\");")
    } else {
        runtime.to_owned()
    }
}

pub(super) fn declarations(text: String, program: &ResolvedProgram) -> String {
    if used(program) {
        text.replace("export type ScalarStatus =", "export type ScalarStatus = Readonly<{ schema: \"semaprax.status.v1\"; domain_id: \"semaprax.convert.v1\"; code: 1 }> |")
    } else {
        text
    }
}
