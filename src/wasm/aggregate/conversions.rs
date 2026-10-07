//! Additive checked scalar conversion lowering; no new host authority/imports.
use super::*;
use crate::string_ops::StringOp;

pub(super) const OUT_OF_RANGE: i32 = 21;
pub(super) const NAN: i32 = 22;

pub(super) fn admitted(operation: StringOp) -> bool {
    matches!(
        operation,
        StringOp::F64FromI64
            | StringOp::I64FromF64
            | StringOp::UsizeFromI64
            | StringOp::I64FromUsize
    )
}

pub(in crate::wasm) fn program_uses_numeric(program: &ResolvedProgram) -> bool {
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
        if matches!(&expression.kind, ResolvedExprKind::Call { callee, .. } if crate::string_ops::by_id(callee.as_str()).is_some_and(admitted))
        {
            return true;
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

pub(in crate::wasm) fn validate_public_profile(
    program: &ResolvedProgram,
    public: bool,
) -> Result<(), Diagnostic> {
    if public && program_uses_numeric(program) {
        crate::string_ops::refuse_collections_for_wasm(program)?;
    }
    Ok(())
}

impl Emitter<'_> {
    pub(super) fn emit_scalar_conversion(
        &mut self,
        expression: &ResolvedExpr,
        operation: StringOp,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let [argument] = args else {
            return Err(error("numeric conversion requires exactly one operand"));
        };
        if !admitted(operation) {
            return Err(error(
                "numeric conversion operation is outside the additive profile",
            ));
        }
        require_type(
            &expression.ty,
            &operation.return_type(),
            "numeric conversion result",
        )?;
        let value = self.emit_expr(argument)?;
        require_type(
            value_type(&value),
            &operation.param_types()[0],
            "numeric conversion operand",
        )?;
        self.apply_call_commit(&expression.id)?;
        let previous = self.failure_expression.replace(expression.id.clone());
        match operation {
            StringOp::F64FromI64 => {
                self.get_scalar(&value);
                self.output.push(0xb9);
            }
            StringOp::I64FromF64 => {
                self.get_scalar(&value);
                self.get_scalar(&value);
                self.output.push(0x62); // f64.ne: NaN only
                self.fail_if(NAN)?;
                self.get_scalar(&value);
                self.f64_conversion_bound(-9223372036854775808.0);
                self.output.push(0x63); // f64.lt
                self.get_scalar(&value);
                self.f64_conversion_bound(9223372036854775808.0);
                self.output.extend([0x66, 0x72]); // f64.ge; i32.or
                self.fail_if(OUT_OF_RANGE)?;
                self.get_scalar(&value);
                self.output.push(0xb0); // i64.trunc_f64_s: range already proved
            }
            StringOp::UsizeFromI64 | StringOp::I64FromUsize => {
                self.get_scalar(&value);
                self.output.extend([0x42, 0x00, 0x53]); // signed high bit catches negative / usize > i64::MAX
                self.fail_if(OUT_OF_RANGE)?;
                self.get_scalar(&value);
            }
            _ => unreachable!("admitted above"),
        }
        self.failure_expression = previous;
        let destination = self.plan.expr_scalar(expression)?;
        self.output.push(0x21);
        write_u32(self.output, destination);
        Ok(Value::Scalar {
            local: destination,
            ty: expression.ty.clone(),
        })
    }

    fn f64_conversion_bound(&mut self, value: f64) {
        self.output.push(0x44);
        self.output.extend(value.to_bits().to_le_bytes());
    }
}
