use crate::ast::{BinaryOp, Expr, ExprKind, Type, UnaryOp};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::CheckedValue;
use crate::source_verify::diagnostics::{
    error, reject_aggregate_equality, reject_native_unit_value,
};
use crate::source_verify::hints;
use crate::source_verify::loans::join_conditional;
use crate::source_verify::IterativeVerifier;
impl<'a, 'v, 'p> IterativeVerifier<'a, 'v, 'p> {
    pub(in crate::source_verify) fn recovery_old_frame_resume_unary(
        &mut self,
        expression: &'p Expr,
        operand: &'p Expr,
        op: UnaryOp,
    ) -> Result<(), Diagnostic> {
        let Some(actual) = self.values.pop().flatten() else {
            self.values.push(None);
            return Ok(());
        };
        let numeric = matches!(op, UnaryOp::Neg)
            .then(|| actual.ty.clone())
            .filter(|ty| matches!(ty, Type::I64 | Type::I32 | Type::F32 | Type::F64));
        let expected = match (&op, &numeric) {
            (UnaryOp::Neg, Some(ty)) => ty.clone(),
            (UnaryOp::Neg, None) => Type::I64,
            (UnaryOp::Not, _) => Type::Bool,
        };
        if !actual.native_unit && actual.ty != expected {
            self.diagnostics.push(error(
                self.program,
                "SPX-T206",
                format!("unary operator expects {expected}, received {}", actual.ty),
                expression.span,
            ));
        }
        reject_native_unit_value(self.program, operand, &actual, self.diagnostics);
        self.values.push(Some(CheckedValue::value(expected)));
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::source_verify) fn recovery_old_frame_resume_binary_right(
        &mut self,
        expression: &'p Expr,
        op: BinaryOp,
        left: &'p Expr,
        left_value: Option<CheckedValue>,
        scope: usize,
        evaluated_scope: usize,
        baseline_names: Vec<String>,
    ) -> Result<(), Diagnostic> {
        let right_value = self.values.pop().unwrap_or(None);
        if evaluated_scope != scope {
            if evaluated_scope + 1 != self.scopes.len() {
                return Err(Diagnostic::io(
                    "SPX-H006",
                    "lazy verifier scope is not the active child",
                ));
            }
            let evaluated = self
                .scopes
                .pop()
                .expect("active lazy scope index checked above")
                .bindings;
            join_conditional(
                &mut self.scopes[scope].bindings,
                &evaluated,
                &baseline_names,
            );
        }
        if let Some(value) = &left_value {
            reject_native_unit_value(self.program, left, value, self.diagnostics);
        }
        let right = match &expression.kind {
            ExprKind::Binary { right, .. } => right.as_ref(),
            _ => unreachable!(),
        };
        if let Some(value) = &right_value {
            reject_native_unit_value(self.program, right, value, self.diagnostics);
        }
        let native_unit = left_value.as_ref().is_some_and(|value| value.native_unit)
            || right_value.as_ref().is_some_and(|value| value.native_unit);
        let left_ordered = left_value
            .as_ref()
            .map(|value| value.ty.clone())
            .filter(|ty| {
                matches!(
                    ty,
                    Type::I64
                        | Type::I32
                        | Type::Char
                        | Type::U8
                        | Type::Usize
                        | Type::F32
                        | Type::F64
                        | Type::String
                )
            });
        let left_narrow = left_value
            .as_ref()
            .map(|value| value.ty.clone())
            .filter(|ty| matches!(ty, Type::U8));
        let left_usize = left_value
            .as_ref()
            .map(|value| value.ty.clone())
            .filter(|ty| matches!(ty, Type::Usize));
        let left_numeric = left_value
            .as_ref()
            .map(|value| value.ty.clone())
            .filter(|ty| matches!(ty, Type::F32 | Type::F64));
        let left_integer = left_value
            .as_ref()
            .map(|value| value.ty.clone())
            .filter(|ty| matches!(ty, Type::I32));
        if !native_unit && matches!(op, BinaryOp::Rem) && left_numeric.is_some() {
            self.diagnostics.push(error(
                self.program,
                "SPX-T208",
                format!("operator `{}` expects i64 operands", op.text()),
                expression.span,
            ));
        }
        let string_operands = left_value
            .as_ref()
            .is_some_and(|value| value.ty == Type::String)
            || right_value
                .as_ref()
                .is_some_and(|value| value.ty == Type::String);
        if !native_unit
            && !matches!(
                op,
                BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
            )
            && string_operands
        {
            self.diagnostics.push(
                error(
                    self.program,
                    "SPX-T250",
                    format!("operator `{}` does not support string operands", op.text()),
                    expression.span,
                )
                .with_help(crate::source_verify::hints::STRING_OPERATOR_HELP),
            );
        }
        let (expected, output) = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                let expected = left_numeric
                    .clone()
                    .or(left_integer)
                    .or(left_narrow)
                    .or(left_usize)
                    .unwrap_or(Type::I64);
                (expected.clone(), expected)
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let expected = left_ordered.unwrap_or(Type::I64);
                (expected, Type::Bool)
            }
            BinaryOp::And | BinaryOp::Or => (Type::Bool, Type::Bool),
            BinaryOp::Eq | BinaryOp::Ne => {
                if let Some(value) = &left_value {
                    reject_aggregate_equality(
                        self.program,
                        self.types,
                        expression,
                        value,
                        self.diagnostics,
                    );
                }
                if !native_unit
                    && left_value.is_some()
                    && right_value.is_some()
                    && left_value.as_ref().map(|value| &value.ty)
                        != right_value.as_ref().map(|value| &value.ty)
                {
                    self.diagnostics.push(hints::with_optional_help(
                        error(
                            self.program,
                            "SPX-T207",
                            "equality operands must have the same type",
                            expression.span,
                        ),
                        hints::equality_types_help(
                            left_value.as_ref().map(|value| &value.ty),
                            right_value.as_ref().map(|value| &value.ty),
                            left,
                            right,
                        ),
                    ));
                }
                self.values.push(Some(CheckedValue::value(Type::Bool)));
                return Ok(());
            }
        };
        if !native_unit
            && (!string_operands
                || matches!(
                    op,
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
                ))
            && (left_value
                .as_ref()
                .is_some_and(|value| value.ty != expected)
                || right_value
                    .as_ref()
                    .is_some_and(|value| value.ty != expected))
        {
            self.diagnostics.push(hints::with_optional_help(
                error(
                    self.program,
                    "SPX-T208",
                    format!("operator `{}` expects {expected} operands", op.text()),
                    expression.span,
                ),
                hints::literal_suffix_help(&expected, left, right),
            ));
        }
        self.values.push(Some(CheckedValue::value(output)));
        Ok(())
    }
}
