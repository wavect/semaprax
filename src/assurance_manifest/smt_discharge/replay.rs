//! Independent, checked-arithmetic reference evaluation used to validate a
//! solver's `sat` model before it is ever trusted as a genuine
//! counterexample.
//!
//! This evaluator is written directly against
//! [`crate::ast`] and deliberately shares no code with
//! [`super::translate`]'s SMT-LIB2 lowering: if the two disagree about what
//! one concrete input does, that disagreement is exactly the kind of
//! translation bug this module exists to catch, and it must not be masked
//! by both halves making the same mistake. See
//! [`docs/SMT-DISCHARGE-V1.md`](../../../docs/SMT-DISCHARGE-V1.md)
//! "Counterexample validation".

use std::collections::BTreeMap;

use crate::ast::{BinaryOp, Expr, ExprKind, Function, Statement, UnaryOp};

use super::model::{Model, ModelValue};
use super::subset::{sort_of_type, NumericMode, Sort};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Value {
    Numeric(i128, NumericMode),
    Bool(bool),
}

fn checked_range(mode: NumericMode, raw: i128) -> Option<i128> {
    if raw >= mode.min() && raw <= mode.max() {
        Some(raw)
    } else {
        None
    }
}

/// The outcome of replaying one solver model against `function`'s actual
/// checked semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayOutcome {
    /// A genuine, validated concrete counterexample: with these exact
    /// input values, the runtime traps before `ensures` is ever reached.
    Trapped { detail: String },
    /// A genuine, validated concrete counterexample: the runtime returns
    /// normally, but the named `ensures` clause evaluates to `false`.
    EnsuresViolated { ensures_index: usize },
    /// The model does not reproduce any failure this evaluator can find:
    /// either `requires` is false under this model (meaning the model
    /// contradicts a fact the SMT query asserted, i.e. a translation bug),
    /// or every `ensures` clause evaluates to `true` and nothing traps
    /// (meaning the `sat` verdict is spurious relative to this
    /// evaluator). Either way, this model is not a validated
    /// counterexample and must never be reported as one.
    Inconsistent { detail: String },
}

fn model_value_to_runtime(name: &str, sort: Sort, model: &Model) -> Value {
    // "Counterexample models may omit unconstrained values": a name absent
    // from the model means the query's own formula does not depend on it,
    // so any representable value is an equally valid witness. Zero/false
    // are always representable in every admitted sort's range.
    match (model.get(name), sort) {
        (Some(ModelValue::Int(v)), Sort::Numeric(mode)) => Value::Numeric(*v, mode),
        (Some(ModelValue::Bool(v)), Sort::Bool) => Value::Bool(*v),
        (None, Sort::Numeric(mode)) => Value::Numeric(0, mode),
        (None, Sort::Bool) => Value::Bool(false),
        // A sort mismatch between what the model reports and what this
        // declaration's type requires is itself a translation
        // inconsistency, surfaced by the caller finding no matching
        // variant here; fall back to the sort's zero value so evaluation
        // can continue and still (most likely) land on `Inconsistent`.
        (Some(ModelValue::Bool(_)), Sort::Numeric(mode)) => Value::Numeric(0, mode),
        (Some(ModelValue::Int(_)), Sort::Bool) => Value::Bool(false),
    }
}

struct Env {
    scope: Vec<(String, Value)>,
}

impl Env {
    fn get(&self, name: &str) -> Option<Value> {
        self.scope
            .iter()
            .rev()
            .find(|(bound, _)| bound == name)
            .map(|(_, value)| *value)
    }
}

/// Evaluate one supported expression. Returns `Err` only for a trap
/// (overflow); this module assumes `expr` already passed
/// [`super::translate::translate_function`]'s admission, so no
/// unsupported-shape error path exists here.
fn eval(env: &mut Env, expr: &Expr) -> Result<Value, String> {
    match &expr.kind {
        ExprKind::Int(v) => Ok(Value::Numeric(i128::from(*v), NumericMode::I64)),
        ExprKind::Int32(v) => Ok(Value::Numeric(i128::from(*v), NumericMode::I32)),
        ExprKind::Uint8(v) => Ok(Value::Numeric(i128::from(*v), NumericMode::U8)),
        ExprKind::Usize(v) => Ok(Value::Numeric(i128::from(*v), NumericMode::Usize)),
        ExprKind::Bool(b) => Ok(Value::Bool(*b)),
        ExprKind::Var(name) => env
            .get(name)
            .ok_or_else(|| format!("replay: unbound name `{name}`")),
        ExprKind::Unary { op, value } => {
            let inner = eval(env, value)?;
            match (op, inner) {
                (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                (UnaryOp::Neg, Value::Numeric(v, mode)) => checked_range(mode, -v)
                    .map(|raw| Value::Numeric(raw, mode))
                    .ok_or_else(|| "checked negation overflow".to_owned()),
                _ => Err("replay: ill-typed unary operand".to_owned()),
            }
        }
        ExprKind::Binary { op, left, right } => eval_binary(env, *op, left, right),
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => match eval(env, condition)? {
            Value::Bool(true) => eval(env, then_branch),
            Value::Bool(false) => eval(env, else_branch),
            Value::Numeric(..) => Err("replay: non-boolean if condition".to_owned()),
        },
        ExprKind::Block { statements, tail } => {
            let depth = env.scope.len();
            let result = eval_block(env, statements, tail);
            env.scope.truncate(depth);
            result
        }
        other => Err(format!("replay: unsupported expression {other:?}")),
    }
}

fn eval_block(env: &mut Env, statements: &[Statement], tail: &Expr) -> Result<Value, String> {
    for statement in statements {
        let Statement::Let { name, value, .. } = statement else {
            return Err("replay: unsupported statement".to_owned());
        };
        let bound = eval(env, value)?;
        env.scope.push((name.clone(), bound));
    }
    eval(env, tail)
}

fn eval_binary(env: &mut Env, op: BinaryOp, left: &Expr, right: &Expr) -> Result<Value, String> {
    match op {
        BinaryOp::And => {
            let Value::Bool(l) = eval(env, left)? else {
                return Err("replay: non-boolean `&&` operand".to_owned());
            };
            if !l {
                return Ok(Value::Bool(false));
            }
            eval(env, right)
        }
        BinaryOp::Or => {
            let Value::Bool(l) = eval(env, left)? else {
                return Err("replay: non-boolean `||` operand".to_owned());
            };
            if l {
                return Ok(Value::Bool(true));
            }
            eval(env, right)
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
            let (Value::Numeric(l, lm), Value::Numeric(r, rm)) =
                (eval(env, left)?, eval(env, right)?)
            else {
                return Err("replay: non-numeric arithmetic operand".to_owned());
            };
            if lm != rm {
                return Err("replay: arithmetic operand mode mismatch".to_owned());
            }
            let raw = match op {
                BinaryOp::Add => l + r,
                BinaryOp::Sub => l - r,
                BinaryOp::Mul => l * r,
                _ => unreachable!(),
            };
            checked_range(lm, raw)
                .map(|v| Value::Numeric(v, lm))
                .ok_or_else(|| format!("checked {} overflow", op.text()))
        }
        BinaryOp::Eq | BinaryOp::Ne => {
            let (l, r) = (eval(env, left)?, eval(env, right)?);
            let equal = match (l, r) {
                (Value::Numeric(a, am), Value::Numeric(b, bm)) => am == bm && a == b,
                (Value::Bool(a), Value::Bool(b)) => a == b,
                _ => return Err("replay: `==`/`!=` sort mismatch".to_owned()),
            };
            Ok(Value::Bool(if op == BinaryOp::Eq { equal } else { !equal }))
        }
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            let (Value::Numeric(l, lm), Value::Numeric(r, rm)) =
                (eval(env, left)?, eval(env, right)?)
            else {
                return Err("replay: non-numeric comparison operand".to_owned());
            };
            if lm != rm {
                return Err("replay: comparison operand mode mismatch".to_owned());
            }
            let result = match op {
                BinaryOp::Lt => l < r,
                BinaryOp::Le => l <= r,
                BinaryOp::Gt => l > r,
                BinaryOp::Ge => l >= r,
                _ => unreachable!(),
            };
            Ok(Value::Bool(result))
        }
        BinaryOp::Div | BinaryOp::Rem => {
            Err("replay: division/remainder are outside the supported subset".to_owned())
        }
    }
}

/// Replay `model` (already type-checked against `function`'s parameter and
/// return sorts by the caller) against `function`'s real checked
/// semantics, and classify what the concrete inputs actually do.
pub fn replay_function(function: &Function, model: &Model) -> Result<ReplayOutcome, String> {
    let mut bindings: BTreeMap<String, Value> = BTreeMap::new();
    let mut scope = Vec::new();
    for param in &function.params {
        let sort = sort_of_type(&param.ty)
            .ok_or_else(|| format!("replay: parameter `{}` has an unsupported type", param.name))?;
        let value = model_value_to_runtime(&param.name, sort, model);
        bindings.insert(param.name.clone(), value);
        scope.push((param.name.clone(), value));
    }
    let mut env = Env { scope };

    for (index, clause) in function.requires.iter().enumerate() {
        match eval(&mut env, clause) {
            Ok(Value::Bool(true)) => {}
            Ok(Value::Bool(false)) => {
                return Ok(ReplayOutcome::Inconsistent {
                    detail: format!(
                        "model satisfies the SMT encoding but requires clause {index} is false \
                         under checked replay"
                    ),
                })
            }
            Ok(Value::Numeric(..)) => return Err("replay: non-boolean requires clause".to_owned()),
            Err(detail) => {
                return Ok(ReplayOutcome::Inconsistent {
                    detail: format!("requires clause {index} failed to evaluate: {detail}"),
                })
            }
        }
    }

    if function.ensures.is_empty() {
        return Ok(ReplayOutcome::Inconsistent {
            detail: "no ensures clauses to violate".to_owned(),
        });
    }

    let body_value = match eval(&mut env, &function.body) {
        Ok(value) => value,
        Err(detail) => return Ok(ReplayOutcome::Trapped { detail }),
    };
    env.scope.push(("result".to_owned(), body_value));

    for (index, clause) in function.ensures.iter().enumerate() {
        match eval(&mut env, clause) {
            Ok(Value::Bool(true)) => {}
            Ok(Value::Bool(false)) => {
                return Ok(ReplayOutcome::EnsuresViolated {
                    ensures_index: index,
                })
            }
            Ok(Value::Numeric(..)) => return Err("replay: non-boolean ensures clause".to_owned()),
            Err(detail) => return Ok(ReplayOutcome::Trapped { detail }),
        }
    }

    Ok(ReplayOutcome::Inconsistent {
        detail: "model satisfies the SMT encoding but replay found no trap or ensures violation"
            .to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function(source: &str) -> Function {
        let mut program = crate::parse(source, "replay-test.spx").expect("parse");
        program.functions.swap_remove(0)
    }

    fn model(entries: &[(&str, ModelValue)]) -> Model {
        entries
            .iter()
            .map(|(name, value)| ((*name).to_owned(), *value))
            .collect()
    }

    #[test]
    fn validates_a_genuine_ensures_violation() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    ensures result > a\n{ a }\n",
        );
        let m = model(&[("a", ModelValue::Int(5))]);
        assert_eq!(
            replay_function(&f, &m).unwrap(),
            ReplayOutcome::EnsuresViolated { ensures_index: 0 }
        );
    }

    #[test]
    fn validates_a_genuine_overflow_trap() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    ensures result >= 0\n{ a + 1 }\n",
        );
        let m = model(&[("a", ModelValue::Int(i64::MAX as i128))]);
        assert!(matches!(
            replay_function(&f, &m).unwrap(),
            ReplayOutcome::Trapped { .. }
        ));
    }

    #[test]
    fn a_model_that_actually_satisfies_the_contract_is_inconsistent_not_a_counterexample() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
        );
        let m = model(&[("a", ModelValue::Int(5))]);
        assert!(matches!(
            replay_function(&f, &m).unwrap(),
            ReplayOutcome::Inconsistent { .. }
        ));
    }

    #[test]
    fn a_model_violating_requires_is_reported_as_inconsistent_not_a_counterexample() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
        );
        let m = model(&[("a", ModelValue::Int(-1))]);
        assert!(matches!(
            replay_function(&f, &m).unwrap(),
            ReplayOutcome::Inconsistent { .. }
        ));
    }

    #[test]
    fn a_missing_model_entry_defaults_to_zero_and_can_still_reproduce_a_violation() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    ensures result > 0\n{ a }\n",
        );
        let m = model(&[]);
        assert_eq!(
            replay_function(&f, &m).unwrap(),
            ReplayOutcome::EnsuresViolated { ensures_index: 0 }
        );
    }
}
