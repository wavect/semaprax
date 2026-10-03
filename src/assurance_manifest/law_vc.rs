//! Typed, backend-neutral scalar verification subject.
//!
//! Both proof emitters audit their operation goals against this walk. The
//! tree retains binding identity, authored evaluation order, stage, and lazy
//! path conditions; rendered SMT/Lean text is deliberately absent.

use crate::ast::{BinaryOp, Expr, ExprKind, Function, Statement, UnaryOp};

use super::smt_discharge::{check_declaration_supported, NumericMode, Sort, UnsupportedReason};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Term {
    pub sort: Sort,
    pub kind: TermKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TermKind {
    Number(i128),
    Bool(bool),
    Binding {
        id: usize,
        name: String,
    },
    Unary {
        op: UnaryOp,
        value: Box<Term>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Term>,
        right: Box<Term>,
    },
    If {
        condition: Box<Term>,
        then_value: Box<Term>,
        else_value: Box<Term>,
    },
    Block {
        definitions: Vec<(usize, Term)>,
        tail: Box<Term>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Requires(usize),
    Body,
    Ensures(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathChoice {
    pub condition: Term,
    pub value: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operation {
    pub stage: Stage,
    pub ordinal: usize,
    pub mode: NumericMode,
    pub term: Term,
    pub path: Vec<PathChoice>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub sort: Sort,
    pub binding: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subject {
    pub parameters: Vec<Parameter>,
    pub requires: Vec<Term>,
    pub result: Option<Term>,
    pub ensures: Vec<Term>,
    pub operations: Vec<Operation>,
}

struct Builder {
    scope: Vec<(String, usize, Sort)>,
    next_binding: usize,
    operations: Vec<Operation>,
}

impl Builder {
    fn binding(&mut self, name: &str, sort: Sort) -> usize {
        let id = self.next_binding;
        self.next_binding += 1;
        self.scope.push((name.to_owned(), id, sort));
        id
    }

    fn expression(
        &mut self,
        expr: &Expr,
        stage: Stage,
        path: &[PathChoice],
    ) -> Result<Term, UnsupportedReason> {
        let simple = |kind, sort| Ok(Term { sort, kind });
        match &expr.kind {
            ExprKind::Int(value) => simple(
                TermKind::Number(i128::from(*value)),
                Sort::Numeric(NumericMode::I64),
            ),
            ExprKind::Int32(value) => simple(
                TermKind::Number(i128::from(*value)),
                Sort::Numeric(NumericMode::I32),
            ),
            ExprKind::Uint8(value) => simple(
                TermKind::Number(i128::from(*value)),
                Sort::Numeric(NumericMode::U8),
            ),
            ExprKind::Usize(value) => simple(
                TermKind::Number(i128::from(*value)),
                Sort::Numeric(NumericMode::Usize),
            ),
            ExprKind::Bool(value) => simple(TermKind::Bool(*value), Sort::Bool),
            ExprKind::Var(name) => {
                let (_, id, sort) = self
                    .scope
                    .iter()
                    .rev()
                    .find(|(bound, _, _)| bound == name)
                    .ok_or_else(|| UnsupportedReason::UnknownName { name: name.clone() })?;
                simple(
                    TermKind::Binding {
                        id: *id,
                        name: name.clone(),
                    },
                    *sort,
                )
            }
            ExprKind::Unary { op, value } => {
                let inner = self.expression(value, stage, path)?;
                let sort = match (op, inner.sort) {
                    (UnaryOp::Not, Sort::Bool) => Sort::Bool,
                    (UnaryOp::Neg, Sort::Numeric(mode)) if mode.is_signed() => Sort::Numeric(mode),
                    _ => return Err(UnsupportedReason::OperandTypeMismatch { op: "unary" }),
                };
                let term = Term {
                    sort,
                    kind: TermKind::Unary {
                        op: *op,
                        value: Box::new(inner),
                    },
                };
                if let Sort::Numeric(mode) = sort {
                    self.record(stage, mode, &term, path);
                }
                Ok(term)
            }
            ExprKind::Binary { op, left, right } => {
                if matches!(op, BinaryOp::Div | BinaryOp::Rem) {
                    return Err(UnsupportedReason::Expr {
                        what: "division or remainder",
                    });
                }
                let lhs = self.expression(left, stage, path)?;
                let mut rhs_path = path.to_vec();
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    rhs_path.push(PathChoice {
                        condition: lhs.clone(),
                        value: *op == BinaryOp::And,
                    });
                }
                let rhs = self.expression(right, stage, &rhs_path)?;
                let sort = match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                        let (Sort::Numeric(a), Sort::Numeric(b)) = (lhs.sort, rhs.sort) else {
                            return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
                        };
                        if a != b {
                            return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
                        }
                        Sort::Numeric(a)
                    }
                    BinaryOp::Eq | BinaryOp::Ne => {
                        if lhs.sort != rhs.sort {
                            return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
                        }
                        Sort::Bool
                    }
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                        if lhs.sort != rhs.sort || !matches!(lhs.sort, Sort::Numeric(_)) {
                            return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
                        }
                        Sort::Bool
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        if lhs.sort != Sort::Bool || rhs.sort != Sort::Bool {
                            return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
                        }
                        Sort::Bool
                    }
                    BinaryOp::Div | BinaryOp::Rem => unreachable!(),
                };
                let term = Term {
                    sort,
                    kind: TermKind::Binary {
                        op: *op,
                        left: Box::new(lhs),
                        right: Box::new(rhs),
                    },
                };
                if matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) {
                    let Sort::Numeric(mode) = sort else {
                        unreachable!()
                    };
                    self.record(stage, mode, &term, path);
                }
                Ok(term)
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.expression(condition, stage, path)?;
                if cond.sort != Sort::Bool {
                    return Err(UnsupportedReason::OperandTypeMismatch { op: "if" });
                }
                let mut then_path = path.to_vec();
                then_path.push(PathChoice {
                    condition: cond.clone(),
                    value: true,
                });
                let mut else_path = path.to_vec();
                else_path.push(PathChoice {
                    condition: cond.clone(),
                    value: false,
                });
                let then_value = self.expression(then_branch, stage, &then_path)?;
                let else_value = self.expression(else_branch, stage, &else_path)?;
                if then_value.sort != else_value.sort {
                    return Err(UnsupportedReason::OperandTypeMismatch { op: "if" });
                }
                Ok(Term {
                    sort: then_value.sort,
                    kind: TermKind::If {
                        condition: Box::new(cond),
                        then_value: Box::new(then_value),
                        else_value: Box::new(else_value),
                    },
                })
            }
            ExprKind::Block { statements, tail } => {
                let depth = self.scope.len();
                let mut definitions = Vec::new();
                for statement in statements {
                    let Statement::Let {
                        name,
                        mutable: false,
                        declared,
                        value,
                        ..
                    } = statement
                    else {
                        self.scope.truncate(depth);
                        return Err(UnsupportedReason::NonLetStatement {
                            what: "mutable or non-let",
                        });
                    };
                    let value = self.expression(value, stage, path)?;
                    if let Some(ty) = declared {
                        if super::smt_discharge::sort_of_type(ty) != Some(value.sort) {
                            self.scope.truncate(depth);
                            return Err(UnsupportedReason::TypeMismatch {
                                detail: format!("typed VC let `{name}` annotation mismatch"),
                            });
                        }
                    }
                    let id = self.binding(name, value.sort);
                    definitions.push((id, value));
                }
                let tail = self.expression(tail, stage, path)?;
                self.scope.truncate(depth);
                Ok(Term {
                    sort: tail.sort,
                    kind: TermKind::Block {
                        definitions,
                        tail: Box::new(tail),
                    },
                })
            }
            _ => Err(UnsupportedReason::Expr {
                what: "outside typed scalar VC",
            }),
        }
    }

    fn record(&mut self, stage: Stage, mode: NumericMode, term: &Term, path: &[PathChoice]) {
        self.operations.push(Operation {
            stage,
            ordinal: self.operations.len(),
            mode,
            term: term.clone(),
            path: path.to_vec(),
        });
    }
}

pub fn build(function: &Function) -> Result<Subject, UnsupportedReason> {
    check_declaration_supported(function)?;
    let mut builder = Builder {
        scope: Vec::new(),
        next_binding: 0,
        operations: Vec::new(),
    };
    let mut parameters = Vec::new();
    for param in &function.params {
        let sort = super::smt_discharge::sort_of_type(&param.ty).expect("checked above");
        let binding = builder.binding(&param.name, sort);
        parameters.push(Parameter {
            name: param.name.clone(),
            sort,
            binding,
        });
    }
    let mut requires = Vec::new();
    for (index, clause) in function.requires.iter().enumerate() {
        let term = builder.expression(clause, Stage::Requires(index), &[])?;
        if term.sort != Sort::Bool {
            return Err(UnsupportedReason::OperandTypeMismatch { op: "requires" });
        }
        requires.push(term);
    }
    let result = if function.ensures.is_empty() {
        None
    } else {
        let term = builder.expression(&function.body, Stage::Body, &[])?;
        if Some(term.sort) != super::smt_discharge::sort_of_type(&function.return_type) {
            return Err(UnsupportedReason::TypeMismatch {
                detail: "typed VC result sort mismatch".to_owned(),
            });
        }
        builder.binding("result", term.sort);
        Some(term)
    };
    let mut ensures = Vec::new();
    for (index, clause) in function.ensures.iter().enumerate() {
        let term = builder.expression(clause, Stage::Ensures(index), &[])?;
        if term.sort != Sort::Bool {
            return Err(UnsupportedReason::OperandTypeMismatch { op: "ensures" });
        }
        ensures.push(term);
    }
    Ok(Subject {
        parameters,
        requires,
        result,
        ensures,
        operations: builder.operations,
    })
}
