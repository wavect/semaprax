//! Unary, binary, and `if` frames: operand resumption, lazy boolean
//! operands, and the branch joins that follow them.

use crate::ast::{BinaryOp, Expr, ExprKind, Type, UnaryOp};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding, CheckedValue};
use crate::source_verify::diagnostics::{
    error, reject_aggregate_equality, reject_native_unit_value,
};
use crate::source_verify::hints;
use crate::source_verify::loans::join_conditional;
use crate::source_verify::place::{join_definitely_partial, join_moved_places};
use crate::source_verify::scope::{VerifierFrame, VerifierScope};
use crate::source_verify::IterativeVerifier;
use std::collections::HashMap;

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    pub(super) fn frame_resume_unary(
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

    pub(super) fn frame_resume_binary_left(
        &mut self,
        expression: &'p Expr,
        op: BinaryOp,
        right: &'p Expr,
        scope: usize,
    ) -> Result<(), Diagnostic> {
        let left_value = self.values.pop().unwrap_or(None);
        let lazy = matches!(op, BinaryOp::And | BinaryOp::Or);
        let baseline_names = if lazy {
            self.scopes[scope]
                .bindings
                .keys()
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let evaluated_scope = if lazy {
            let index = self.scopes.len();
            self.scopes.push(VerifierScope {
                bindings: self.scopes[scope].bindings.clone(),
                local_borrow_count: self.scopes[scope].local_borrow_count,
            });
            index
        } else {
            scope
        };
        let left = match &expression.kind {
            ExprKind::Binary { left, .. } => left.as_ref(),
            _ => unreachable!(),
        };
        self.frames.push(VerifierFrame::ResumeBinaryRight {
            expression,
            op,
            left,
            left_value,
            scope,
            evaluated_scope,
            baseline_names,
        });
        self.frames.push(VerifierFrame::Enter {
            expression: right,
            scope: evaluated_scope,
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_resume_binary_right(
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
        if !native_unit
            && matches!(op, BinaryOp::Rem)
            && (left_numeric.is_some() || left_integer.is_some() || left_narrow.is_some())
        {
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
        if !native_unit && !matches!(op, BinaryOp::Eq | BinaryOp::Ne) && string_operands {
            self.diagnostics.push(error(
                self.program,
                "SPX-T250",
                format!("operator `{}` does not support string operands", op.text()),
                expression.span,
            ));
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
                    reject_aggregate_equality(self.program, expression, value, self.diagnostics);
                }
                if !native_unit
                    && left_value.is_some()
                    && right_value.is_some()
                    && left_value.as_ref().map(|value| &value.ty)
                        != right_value.as_ref().map(|value| &value.ty)
                {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T207",
                        "equality operands must have the same type",
                        expression.span,
                    ));
                }
                self.values.push(Some(CheckedValue::value(Type::Bool)));
                return Ok(());
            }
        };
        if !native_unit
            && !string_operands
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

    pub(super) fn frame_resume_if_condition(
        &mut self,
        expression: &'p Expr,
        then_branch: &'p Expr,
        else_branch: &'p Expr,
        scope: usize,
    ) -> Result<(), Diagnostic> {
        let condition_value = self.values.pop().unwrap_or(None);
        let condition = match &expression.kind {
            ExprKind::If { condition, .. } => condition.as_ref(),
            _ => unreachable!(),
        };
        if let Some(value) = condition_value {
            if value.native_unit {
                reject_native_unit_value(self.program, condition, &value, self.diagnostics);
            } else if value.ty != Type::Bool {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T210",
                    "`if` condition must be bool",
                    condition.span,
                ));
            }
        }
        let baseline_names = self.scopes[scope]
            .bindings
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        let then_scope = self.scopes.len();
        self.scopes.push(VerifierScope {
            bindings: self.scopes[scope].bindings.clone(),
            local_borrow_count: self.scopes[scope].local_borrow_count,
        });
        self.frames.push(VerifierFrame::ResumeIfThen {
            expression,
            else_branch,
            scope,
            then_scope,
            baseline_names,
        });
        self.frames.push(VerifierFrame::Enter {
            expression: then_branch,
            scope: then_scope,
        });
        Ok(())
    }

    pub(super) fn frame_resume_if_then(
        &mut self,
        expression: &'p Expr,
        else_branch: &'p Expr,
        scope: usize,
        then_scope: usize,
        baseline_names: Vec<String>,
    ) -> Result<(), Diagnostic> {
        if then_scope + 1 != self.scopes.len() {
            return Err(Diagnostic::io(
                "SPX-H006",
                "then verifier scope is not the active child",
            ));
        }
        let then_value = self.values.pop().unwrap_or(None);
        let then_bindings = self
            .scopes
            .pop()
            .expect("active then scope index checked above")
            .bindings;
        let else_scope = self.scopes.len();
        self.scopes.push(VerifierScope {
            bindings: self.scopes[scope].bindings.clone(),
            local_borrow_count: self.scopes[scope].local_borrow_count,
        });
        let then_branch = match &expression.kind {
            ExprKind::If { then_branch, .. } => then_branch.as_ref(),
            _ => unreachable!(),
        };
        self.frames.push(VerifierFrame::ResumeIfElse {
            expression,
            then_branch,
            else_branch,
            scope,
            else_scope,
            baseline_names,
            then_value,
            then_bindings,
        });
        self.frames.push(VerifierFrame::Enter {
            expression: else_branch,
            scope: else_scope,
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_resume_if_else(
        &mut self,
        expression: &'p Expr,
        then_branch: &'p Expr,
        else_branch: &'p Expr,
        scope: usize,
        else_scope: usize,
        baseline_names: Vec<String>,
        then_value: Option<CheckedValue>,
        then_bindings: HashMap<String, Binding>,
    ) -> Result<(), Diagnostic> {
        if else_scope + 1 != self.scopes.len() {
            return Err(Diagnostic::io(
                "SPX-H006",
                "else verifier scope is not the active child",
            ));
        }
        let else_value = self.values.pop().unwrap_or(None);
        let else_bindings = self
            .scopes
            .pop()
            .expect("active else scope index checked above")
            .bindings;
        for name in &baseline_names {
            if let Some(binding) = self.scopes[scope].bindings.get_mut(name) {
                let then_state = then_bindings
                    .get(name)
                    .map_or(Availability::Available, |value| value.availability);
                let else_state = else_bindings
                    .get(name)
                    .map_or(Availability::Available, |value| value.availability);
                binding.availability = then_state.join(else_state);
                if let (Some(then_binding), Some(else_binding)) =
                    (then_bindings.get(name), else_bindings.get(name))
                {
                    binding.active_loans = then_binding
                        .active_loans
                        .union(&else_binding.active_loans)
                        .cloned()
                        .collect();
                    binding.moved_places = join_moved_places(then_binding, else_binding);
                    binding.definitely_partial =
                        join_definitely_partial(then_binding, else_binding);
                }
            }
        }
        let output = match (then_value, else_value) {
            (Some(then_value), Some(else_value)) => {
                if then_value.native_unit || else_value.native_unit {
                    reject_native_unit_value(
                        self.program,
                        then_branch,
                        &then_value,
                        self.diagnostics,
                    );
                    reject_native_unit_value(
                        self.program,
                        else_branch,
                        &else_value,
                        self.diagnostics,
                    );
                } else if then_value.ty != else_value.ty {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T211",
                        format!(
                            "`if` branches return different types: {} and {}",
                            then_value.ty, else_value.ty
                        ),
                        expression.span,
                    ));
                }
                if self.types.needs_drop(&then_value.ty) && then_value.mode != else_value.mode {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-O106",
                        "`if` branches must produce the same resource ownership mode",
                        expression.span,
                    ));
                }
                Some(then_value)
            }
            _ => None,
        };
        self.values.push(output);
        Ok(())
    }
}
