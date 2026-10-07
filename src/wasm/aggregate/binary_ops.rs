//! Scalar and String binary lowering, with authored lazy evaluation.
use super::*;
impl Emitter<'_> {
    pub(super) fn emit_binary(
        &mut self,
        expr: &ResolvedExpr,
        op: BinaryOp,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let left = self.emit_expr(left)?;
        let destination = self.plan.expr_scalar(expr)?;
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            self.require_scalar(&left, &ResolvedType::Bool, "lazy left operand")?;
            self.get_scalar(&left);
            self.output.extend([0x04, 0x40]);
            self.control_depth += 1;
            if op == BinaryOp::And {
                let right = self.emit_expr(right)?;
                self.require_scalar(&right, &ResolvedType::Bool, "lazy right operand")?;
                self.get_scalar(&right);
                self.output.push(0x21);
                write_u32(self.output, destination);
                self.output.push(0x05);
                self.output.extend([0x41, 0x00, 0x21]);
                write_u32(self.output, destination);
            } else {
                self.output.extend([0x41, 0x01, 0x21]);
                write_u32(self.output, destination);
                self.output.push(0x05);
                let right = self.emit_expr(right)?;
                self.require_scalar(&right, &ResolvedType::Bool, "lazy right operand")?;
                self.get_scalar(&right);
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            self.control_depth -= 1;
            self.output.push(0x0b);
            return Ok(Value::Scalar {
                local: destination,
                ty: ResolvedType::Bool,
            });
        }

        let right = self.emit_expr(right)?;
        if value_type(&left) == &ResolvedType::String
            && matches!(
                op,
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
            )
        {
            self.emit_aggregate_string_ordering(op, &left, &right, destination)?;
            return Ok(Value::Scalar {
                local: destination,
                ty: ResolvedType::Bool,
            });
        }
        if self.standalone_strings && value_type(&left) == &ResolvedType::String {
            if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                return Err(error("standalone String binary operation is not equality"));
            }
            require_type(value_type(&right), &ResolvedType::String, "String equality")?;
            self.get_scalar(&left);
            self.get_scalar(&right);
            self.output
                .extend([0x10, internal_strings::EQ_IMPORT as u8]);
            if op == BinaryOp::Ne {
                self.output.push(0x45);
            }
            self.output.push(0x21);
            write_u32(self.output, destination);
            return Ok(Value::Scalar {
                local: destination,
                ty: ResolvedType::Bool,
            });
        }
        if matches!(value_type(&left), ResolvedType::F32 | ResolvedType::F64)
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            return self.emit_float_binary(expr, op, &left, &right, destination);
        }
        let int32_operands = matches!(value_type(&left), ResolvedType::I32);
        if matches!(value_type(&left), ResolvedType::U8)
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            let saved = self.failure_expression.replace(expr.id.clone());
            let result = self.emit_u8_binary(expr, op, &left, &right, destination);
            self.failure_expression = saved;
            return result;
        }
        if matches!(value_type(&left), ResolvedType::Usize)
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            let saved = self.failure_expression.replace(expr.id.clone());
            let result = self.emit_usize_binary(expr, op, &left, &right, destination);
            self.failure_expression = saved;
            return result;
        }
        let saved_failure = self.failure_expression.replace(expr.id.clone());
        match op {
            BinaryOp::Add if int32_operands => {
                self.emit_checked_i32_add(&left, &right, destination)?
            }
            BinaryOp::Sub if int32_operands => {
                self.emit_checked_i32_sub(&left, &right, destination)?
            }
            BinaryOp::Mul if int32_operands => {
                self.emit_checked_i32_mul(&left, &right, destination)?
            }
            BinaryOp::Div if int32_operands => {
                self.emit_checked_i32_div_rem(&left, &right, destination, false)?
            }
            BinaryOp::Rem if int32_operands => {
                self.emit_checked_i32_div_rem(&left, &right, destination, true)?
            }
            BinaryOp::Add => self.emit_checked_add(&left, &right, destination)?,
            BinaryOp::Sub => self.emit_checked_sub(&left, &right, destination)?,
            BinaryOp::Mul => self.emit_checked_mul(&left, &right, destination)?,
            BinaryOp::Div => self.emit_checked_div_rem(&left, &right, destination, false)?,
            BinaryOp::Rem => self.emit_checked_div_rem(&left, &right, destination, true)?,
            BinaryOp::Eq | BinaryOp::Ne => {
                if value_type(&left) == &ResolvedType::String {
                    self.emit_aggregate_string_equality(op, &left, &right, destination)?;
                } else if self.program.is_payload_free_variant(value_type(&left)) {
                    self.emit_case_equality(op, &left, &right, destination)?;
                } else if is_aggregate(self.program, value_type(&left))? {
                    return Err(error("record equality is outside executable records v1"));
                } else {
                    require_type(value_type(&left), value_type(&right), "equality operands")?;
                    self.get_scalar(&left);
                    self.get_scalar(&right);
                    self.output.push(match (value_type(&left), op) {
                        (ResolvedType::I64 | ResolvedType::Usize, BinaryOp::Eq) => 0x51,
                        (ResolvedType::I64 | ResolvedType::Usize, BinaryOp::Ne) => 0x52,
                        (ResolvedType::F32, BinaryOp::Eq) => 0x5b,
                        (ResolvedType::F32, BinaryOp::Ne) => 0x5c,
                        (ResolvedType::F64, BinaryOp::Eq) => 0x61,
                        (ResolvedType::F64, BinaryOp::Ne) => 0x62,
                        (_, BinaryOp::Eq) => 0x46,
                        (_, BinaryOp::Ne) => 0x47,
                        _ => unreachable!(),
                    });
                    self.output.push(0x21);
                    write_u32(self.output, destination);
                }
            }
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
                let operand_ty = value_type(&left);
                if !matches!(
                    operand_ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::Char
                        | ResolvedType::U8
                        | ResolvedType::Usize
                        | ResolvedType::F32
                        | ResolvedType::F64
                ) {
                    return Err(error(format!(
                        "ordered comparison requires a scalar operand, found `{}`",
                        operand_ty.identity_key()
                    )));
                }
                require_type(
                    value_type(&right),
                    &operand_ty.clone(),
                    "ordered right operand",
                )?;
                self.get_scalar(&left);
                self.get_scalar(&right);
                self.output.push(match (&operand_ty, op) {
                    (ResolvedType::Char, BinaryOp::Lt) => 0x49,
                    (ResolvedType::Char, BinaryOp::Gt) => 0x4b,
                    (ResolvedType::Char, BinaryOp::Le) => 0x4d,
                    (ResolvedType::Char, BinaryOp::Ge) => 0x4f,
                    (ResolvedType::U8, BinaryOp::Lt) => 0x49,
                    (ResolvedType::U8, BinaryOp::Gt) => 0x4b,
                    (ResolvedType::U8, BinaryOp::Le) => 0x4d,
                    (ResolvedType::U8, BinaryOp::Ge) => 0x4f,
                    (ResolvedType::Usize, BinaryOp::Lt) => 0x54,
                    (ResolvedType::Usize, BinaryOp::Gt) => 0x56,
                    (ResolvedType::Usize, BinaryOp::Le) => 0x58,
                    (ResolvedType::Usize, BinaryOp::Ge) => 0x5a,
                    (ResolvedType::F32, BinaryOp::Lt) => 0x5d,
                    (ResolvedType::F32, BinaryOp::Gt) => 0x5e,
                    (ResolvedType::F32, BinaryOp::Le) => 0x5f,
                    (ResolvedType::F32, BinaryOp::Ge) => 0x60,
                    (ResolvedType::F64, BinaryOp::Lt) => 0x63,
                    (ResolvedType::F64, BinaryOp::Gt) => 0x64,
                    (ResolvedType::F64, BinaryOp::Le) => 0x65,
                    (ResolvedType::F64, BinaryOp::Ge) => 0x66,
                    (ResolvedType::F32 | ResolvedType::F64, _)
                        if matches!(op, BinaryOp::Rem | BinaryOp::And | BinaryOp::Or) =>
                    {
                        unreachable!("float remainder/lazy operation was matched above")
                    }
                    (ResolvedType::I32, BinaryOp::Lt) => 0x48,
                    (ResolvedType::I32, BinaryOp::Gt) => 0x4a,
                    (ResolvedType::I32, BinaryOp::Le) => 0x4c,
                    (ResolvedType::I32, BinaryOp::Ge) => 0x4e,
                    (_, BinaryOp::Lt) => 0x53,
                    (_, BinaryOp::Gt) => 0x55,
                    (_, BinaryOp::Le) => 0x57,
                    (_, BinaryOp::Ge) => 0x59,
                    _ => unreachable!("ordered operation was matched above"),
                });
                self.output.push(0x21);
                write_u32(self.output, destination);
            }
            BinaryOp::And | BinaryOp::Or => {
                unreachable!("lazy boolean operations were short-circuited above")
            }
        }
        self.failure_expression = saved_failure;
        Ok(Value::Scalar {
            local: destination,
            ty: expr.ty.clone(),
        })
    }
}
