//! Deterministic translation of one supported [`Function`]'s contract
//! clauses and body into `QF_LIA` SMT-LIB2 terms, plus the explicit
//! overflow/well-definedness side obligations each arithmetic operation
//! contributes.
//!
//! See [`docs/SMT-DISCHARGE-V1.md`](../../../docs/SMT-DISCHARGE-V1.md)
//! "Numeric encoding" for why mathematical (`QF_LIA`) integers, bounded by
//! an explicit range axiom per declared variable and an explicit range
//! obligation per arithmetic operation, are the semantics-preserving
//! encoding of SEMAPRAX's checked (trapping, never-wrapping) arithmetic —
//! not an approximation of it.

use crate::ast::{BinaryOp, Expr, ExprKind, Function, Statement, UnaryOp};

use super::subset::{
    self, binary_op_reason, check_declaration_supported, expr_reason, statement_reason,
    NumericMode, Sort, UnsupportedReason,
};

/// One side condition that must hold whenever `guard` holds, or the
/// operation it describes would trap at runtime instead of producing the
/// value the surrounding term assumes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SideObligation {
    /// SMT-LIB2 `Bool` term: the accumulated path condition (branch guards,
    /// short-circuit guards) under which the real runtime would actually
    /// evaluate this operation.
    pub guard: String,
    /// SMT-LIB2 `Bool` term: the condition that must hold for the operation
    /// not to trap (currently always a range membership check).
    pub formula: String,
    /// Human-readable locator for diagnostics, e.g. `"ensure:0 op:2 add"`.
    pub label: String,
}

impl SideObligation {
    /// `(=> guard formula)`, the exact assertable form.
    #[must_use]
    pub fn implication(&self) -> String {
        format!("(=> {} {})", self.guard, self.formula)
    }
}

/// One free SMT-LIB2 constant this encoding declares: a function parameter,
/// or (only inside an `ensures` encoding) the special `result` binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub sort: Sort,
}

/// `v` rendered as a signed SMT-LIB2 numeral: non-negative numerals are
/// bare digits; SMT-LIB2 has no numeral token for negative numbers, so a
/// negative value must be `(- <magnitude>)`.
#[must_use]
pub fn int_literal(v: i128) -> String {
    if v >= 0 {
        v.to_string()
    } else {
        format!("(- {})", -v)
    }
}

fn range_formula(term: &str, mode: NumericMode) -> String {
    format!(
        "(and (>= {term} {}) (<= {term} {}))",
        int_literal(mode.min()),
        int_literal(mode.max())
    )
}

/// One translated expression: its SMT-LIB2 term text and inferred sort.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Translated {
    term: String,
    sort: Sort,
}

/// Translation state threaded through one function's requires, body, and
/// ensures. `scope` supports shadowing: `let` pushes, the owning block pops.
struct Ctx {
    declarations: Vec<Declaration>,
    range_axioms: Vec<String>,
    /// Unconditional definitional equalities: `result = <body term>` and
    /// each `let`-bound skolem constant's `<name> = <value term>`. These
    /// are hard facts, not obligations — a fresh symbol defined equal to
    /// an already-translated term can never fail to satisfy that equality,
    /// so asserting it inside the negated goal (as this module's first
    /// working version incorrectly did) leaves the symbol's value
    /// unconstrained there and lets the solver "refute" the goal by
    /// picking any value other than the one it is defined to have. See
    /// `docs/SMT-DISCHARGE-V1.md` "Numeric encoding" for why this
    /// distinction — hard fact vs. provable obligation — is exactly the
    /// line between "always true by construction" and "must be proved".
    definitions: Vec<String>,
    obligations: Vec<SideObligation>,
    scope: Vec<(String, Translated)>,
    fresh: u32,
    label_prefix: String,
}

impl Ctx {
    fn lookup(&self, name: &str) -> Option<Translated> {
        self.scope
            .iter()
            .rev()
            .find(|(bound, _)| bound == name)
            .map(|(_, value)| value.clone())
    }

    fn declare(&mut self, name: String, sort: Sort) {
        if let Sort::Numeric(mode) = sort {
            self.range_axioms.push(range_formula(&name, mode));
        }
        self.declarations.push(Declaration { name, sort });
    }

    fn fresh_name(&mut self, hint: &str) -> String {
        self.fresh += 1;
        format!("smt_discharge_{hint}_{}", self.fresh)
    }
}

fn translate_expr(
    ctx: &mut Ctx,
    expr: &Expr,
    guard: &str,
) -> Result<Translated, UnsupportedReason> {
    if let Some(reason) = expr_reason(expr) {
        return Err(reason);
    }
    match &expr.kind {
        ExprKind::Int(v) => Ok(Translated {
            term: int_literal(i128::from(*v)),
            sort: Sort::Numeric(NumericMode::I64),
        }),
        ExprKind::Int32(v) => Ok(Translated {
            term: int_literal(i128::from(*v)),
            sort: Sort::Numeric(NumericMode::I32),
        }),
        ExprKind::Uint8(v) => Ok(Translated {
            term: int_literal(i128::from(*v)),
            sort: Sort::Numeric(NumericMode::U8),
        }),
        ExprKind::Usize(v) => Ok(Translated {
            term: int_literal(i128::from(*v)),
            sort: Sort::Numeric(NumericMode::Usize),
        }),
        ExprKind::Bool(b) => Ok(Translated {
            term: b.to_string(),
            sort: Sort::Bool,
        }),
        ExprKind::Var(name) => ctx
            .lookup(name)
            .ok_or_else(|| UnsupportedReason::UnknownName { name: name.clone() }),
        ExprKind::Unary { op, value } => translate_unary(ctx, *op, value, guard),
        ExprKind::Binary { op, left, right } => translate_binary(ctx, *op, left, right, guard),
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => translate_if(ctx, condition, then_branch, else_branch, guard),
        ExprKind::Block { statements, tail } => translate_block(ctx, statements, tail, guard),
        _ => unreachable!("expr_reason already rejected every other ExprKind"),
    }
}

fn translate_unary(
    ctx: &mut Ctx,
    op: UnaryOp,
    value: &Expr,
    guard: &str,
) -> Result<Translated, UnsupportedReason> {
    let inner = translate_expr(ctx, value, guard)?;
    match op {
        UnaryOp::Not => {
            if inner.sort != Sort::Bool {
                return Err(UnsupportedReason::OperandTypeMismatch { op: "not" });
            }
            Ok(Translated {
                term: format!("(not {})", inner.term),
                sort: Sort::Bool,
            })
        }
        UnaryOp::Neg => {
            let Sort::Numeric(mode) = inner.sort else {
                return Err(UnsupportedReason::OperandTypeMismatch { op: "neg" });
            };
            if !mode.is_signed() {
                return Err(UnsupportedReason::OperandTypeMismatch { op: "neg" });
            }
            let term = format!("(- {})", inner.term);
            ctx.obligations.push(SideObligation {
                guard: guard.to_owned(),
                formula: range_formula(&term, mode),
                label: format!("{} neg", ctx.label_prefix),
            });
            Ok(Translated {
                term,
                sort: Sort::Numeric(mode),
            })
        }
    }
}

fn translate_binary(
    ctx: &mut Ctx,
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    guard: &str,
) -> Result<Translated, UnsupportedReason> {
    if let Some(reason) = binary_op_reason(op) {
        return Err(reason);
    }
    match op {
        BinaryOp::And | BinaryOp::Or => {
            let l = translate_expr(ctx, left, guard)?;
            if l.sort != Sort::Bool {
                return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
            }
            let right_guard = if op == BinaryOp::And {
                format!("(and {guard} {})", l.term)
            } else {
                format!("(and {guard} (not {}))", l.term)
            };
            let r = translate_expr(ctx, right, &right_guard)?;
            if r.sort != Sort::Bool {
                return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
            }
            let connective = if op == BinaryOp::And { "and" } else { "or" };
            Ok(Translated {
                term: format!("({connective} {} {})", l.term, r.term),
                sort: Sort::Bool,
            })
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
            let l = translate_expr(ctx, left, guard)?;
            let r = translate_expr(ctx, right, guard)?;
            let (Sort::Numeric(lm), Sort::Numeric(rm)) = (l.sort, r.sort) else {
                return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
            };
            if lm != rm {
                return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
            }
            let smt_op = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                _ => unreachable!(),
            };
            let term = format!("({smt_op} {} {})", l.term, r.term);
            ctx.obligations.push(SideObligation {
                guard: guard.to_owned(),
                formula: range_formula(&term, lm),
                label: format!("{} {}", ctx.label_prefix, op.text()),
            });
            Ok(Translated {
                term,
                sort: Sort::Numeric(lm),
            })
        }
        BinaryOp::Eq | BinaryOp::Ne => {
            let l = translate_expr(ctx, left, guard)?;
            let r = translate_expr(ctx, right, guard)?;
            if std::mem::discriminant(&l.sort) != std::mem::discriminant(&r.sort)
                || matches!((l.sort, r.sort), (Sort::Numeric(lm), Sort::Numeric(rm)) if lm != rm)
            {
                return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
            }
            let term = if op == BinaryOp::Eq {
                format!("(= {} {})", l.term, r.term)
            } else {
                format!("(distinct {} {})", l.term, r.term)
            };
            Ok(Translated {
                term,
                sort: Sort::Bool,
            })
        }
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            let l = translate_expr(ctx, left, guard)?;
            let r = translate_expr(ctx, right, guard)?;
            let (Sort::Numeric(lm), Sort::Numeric(rm)) = (l.sort, r.sort) else {
                return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
            };
            if lm != rm {
                return Err(UnsupportedReason::OperandTypeMismatch { op: op.text() });
            }
            Ok(Translated {
                term: format!("({} {} {})", op.text(), l.term, r.term),
                sort: Sort::Bool,
            })
        }
        BinaryOp::Div | BinaryOp::Rem => unreachable!("binary_op_reason already rejected this"),
    }
}

fn translate_if(
    ctx: &mut Ctx,
    condition: &Expr,
    then_branch: &Expr,
    else_branch: &Expr,
    guard: &str,
) -> Result<Translated, UnsupportedReason> {
    let cond = translate_expr(ctx, condition, guard)?;
    if cond.sort != Sort::Bool {
        return Err(UnsupportedReason::OperandTypeMismatch { op: "if" });
    }
    let then_guard = format!("(and {guard} {})", cond.term);
    let else_guard = format!("(and {guard} (not {}))", cond.term);
    let then_value = translate_expr(ctx, then_branch, &then_guard)?;
    let else_value = translate_expr(ctx, else_branch, &else_guard)?;
    if then_value.sort != else_value.sort {
        return Err(UnsupportedReason::OperandTypeMismatch { op: "if" });
    }
    Ok(Translated {
        term: format!(
            "(ite {} {} {})",
            cond.term, then_value.term, else_value.term
        ),
        sort: then_value.sort,
    })
}

fn translate_block(
    ctx: &mut Ctx,
    statements: &[Statement],
    tail: &Expr,
    guard: &str,
) -> Result<Translated, UnsupportedReason> {
    let scope_depth = ctx.scope.len();
    let result = translate_block_body(ctx, statements, tail, guard);
    // A `let` bound inside this block must not leak into the surrounding
    // scope, on either success or failure.
    ctx.scope.truncate(scope_depth);
    result
}

fn translate_block_body(
    ctx: &mut Ctx,
    statements: &[Statement],
    tail: &Expr,
    guard: &str,
) -> Result<Translated, UnsupportedReason> {
    for statement in statements {
        if let Some(reason) = statement_reason(statement) {
            return Err(reason);
        }
        let Statement::Let {
            name,
            declared,
            value,
            ..
        } = statement
        else {
            unreachable!("statement_reason already rejected every non-Let statement")
        };
        let translated = translate_expr(ctx, value, guard)?;
        if let Some(declared_ty) = declared {
            let declared_sort = subset::sort_of_type(declared_ty).ok_or_else(|| {
                UnsupportedReason::TypeMismatch {
                    detail: format!("`let {name}` declares an unsupported type `{declared_ty}`"),
                }
            })?;
            if declared_sort != translated.sort {
                return Err(UnsupportedReason::TypeMismatch {
                    detail: format!(
                        "`let {name}` declares `{declared_ty}` but its value has a different sort"
                    ),
                });
            }
        }
        // A named skolem constant, not a raw substitution: keeps the
        // rendered script readable and stable regardless of how many times
        // `name` is referenced downstream.
        let bound_name = ctx.fresh_name(name);
        let sort = translated.sort;
        ctx.declarations.push(Declaration {
            name: bound_name.clone(),
            sort,
        });
        ctx.definitions
            .push(format!("(= {bound_name} {})", translated.term));
        ctx.scope.push((
            name.clone(),
            Translated {
                term: bound_name,
                sort,
            },
        ));
    }
    translate_expr(ctx, tail, guard)
}

/// One admitted `ensures` clause's translation: its own term/sort and the
/// side obligations introduced strictly inside that clause (not shared with
/// other clauses, unlike `requires`/body obligations).
pub struct EnsuresEncoding {
    pub term: String,
    pub obligations: Vec<SideObligation>,
}

/// The complete deterministic encoding of one supported function: shared
/// declarations/axioms/obligations (from parameters, `requires`, and the
/// body), plus one independent [`EnsuresEncoding`] per `ensures` clause.
pub struct FunctionEncoding {
    pub declarations: Vec<Declaration>,
    pub range_axioms: Vec<String>,
    /// Unconditional definitional equalities (`result` and every `let`);
    /// see [`Ctx::definitions`]. Always asserted as hard facts, never as
    /// part of a provable goal.
    pub definitions: Vec<String>,
    pub requires_terms: Vec<String>,
    /// Obligations from parameters/`requires`/body: shared by every
    /// `ensures` query, since all of them execute unconditionally before
    /// any `ensures` clause is reached.
    pub shared_obligations: Vec<SideObligation>,
    pub ensures: Vec<EnsuresEncoding>,
}

/// Translate `function`. Returns a closed [`UnsupportedReason`] the moment
/// any clause or the body leaves the admitted subset; a caller with a
/// partially unsupported function still gets nothing rendered — this
/// module discharges a function wholesale or not at all, matching the
/// spec's "wholesale, not per-clause" note.
pub fn translate_function(function: &Function) -> Result<FunctionEncoding, UnsupportedReason> {
    check_declaration_supported(function)?;

    let mut ctx = Ctx {
        declarations: Vec::new(),
        range_axioms: Vec::new(),
        definitions: Vec::new(),
        obligations: Vec::new(),
        scope: Vec::new(),
        fresh: 0,
        label_prefix: "requires".to_owned(),
    };
    for param in &function.params {
        let sort = subset::sort_of_type(&param.ty).expect("checked by check_declaration_supported");
        ctx.declare(param.name.clone(), sort);
        ctx.scope.push((
            param.name.clone(),
            Translated {
                term: param.name.clone(),
                sort,
            },
        ));
    }

    let mut requires_terms = Vec::new();
    for require in &function.requires {
        let translated = translate_expr(&mut ctx, require, "true")?;
        if translated.sort != Sort::Bool {
            return Err(UnsupportedReason::OperandTypeMismatch { op: "requires" });
        }
        requires_terms.push(translated.term);
    }

    let mut ensures = Vec::new();
    if !function.ensures.is_empty() {
        ctx.label_prefix = "body".to_owned();
        let body = translate_expr(&mut ctx, &function.body, "true")?;
        let return_sort = subset::sort_of_type(&function.return_type)
            .expect("checked by check_declaration_supported");
        if body.sort != return_sort {
            return Err(UnsupportedReason::TypeMismatch {
                detail: "function body's inferred sort does not match its declared return type"
                    .to_owned(),
            });
        }
        let result_name = "result".to_owned();
        // Deliberately not `ctx.declare()`: that also asserts an
        // unconditional range axiom, which would be unsound here. `result`
        // is defined equal to `body.term`, which may itself be an
        // out-of-range (trapping) value along some input — that is exactly
        // the property the range *obligation* on `body.term` below exists
        // to prove is unreachable, not something to assume as an axiom on
        // `result` too. Asserting both would make the two facts
        // contradictory precisely when the body overflows, which makes the
        // whole query vacuously (and wrongly) unsat — see
        // `docs/SMT-DISCHARGE-V1.md` "Numeric encoding" for the general
        // rule this is one instance of: only a real input (a parameter) is
        // axiomatically in range; every derived value's range is a
        // provable obligation, never an assumption.
        ctx.declarations.push(Declaration {
            name: result_name.clone(),
            sort: return_sort,
        });
        ctx.definitions
            .push(format!("(= {result_name} {})", body.term));
        ctx.scope.push((
            result_name.clone(),
            Translated {
                term: result_name,
                sort: return_sort,
            },
        ));

        for (index, clause) in function.ensures.iter().enumerate() {
            ctx.label_prefix = format!("ensure:{index}");
            let before = ctx.obligations.len();
            let translated = translate_expr(&mut ctx, clause, "true")?;
            if translated.sort != Sort::Bool {
                return Err(UnsupportedReason::OperandTypeMismatch { op: "ensures" });
            }
            let clause_obligations = ctx.obligations.split_off(before);
            ensures.push(EnsuresEncoding {
                term: translated.term,
                obligations: clause_obligations,
            });
        }
    }

    Ok(FunctionEncoding {
        declarations: ctx.declarations,
        range_axioms: ctx.range_axioms,
        definitions: ctx.definitions,
        requires_terms,
        shared_obligations: ctx.obligations,
        ensures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function(source: &str) -> Function {
        let mut program = crate::parse(source, "translate-test.spx").expect("parse");
        program.functions.swap_remove(0)
    }

    #[test]
    fn int_literal_renders_smt_lib2_negative_numerals() {
        assert_eq!(int_literal(5), "5");
        assert_eq!(int_literal(0), "0");
        assert_eq!(int_literal(-5), "(- 5)");
        assert_eq!(
            int_literal(i64::MIN as i128),
            format!("(- {})", -(i64::MIN as i128))
        );
    }

    #[test]
    fn translates_a_plain_precondition_and_postcondition() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
        );
        let encoding = translate_function(&f).expect("supported");
        assert_eq!(encoding.requires_terms, vec!["(>= a 0)".to_owned()]);
        assert_eq!(encoding.ensures.len(), 1);
        assert_eq!(encoding.ensures[0].term, "(>= result 0)");
        // `a` is a real input: it gets an unconditional i64 range axiom.
        assert!(encoding
            .range_axioms
            .iter()
            .any(|axiom| axiom.contains('a')));
        // `result` must NOT get one: it is a derived value defined via
        // `definitions`, and its range is a provable obligation elsewhere,
        // never an axiom — asserting both would be unsound whenever the
        // body's value actually leaves range (see
        // `translate_function`'s comment at the `result` declaration and
        // `docs/SMT-DISCHARGE-V1.md` "Numeric encoding").
        assert!(!encoding
            .range_axioms
            .iter()
            .any(|axiom| axiom.contains("result")));
        assert!(encoding
            .definitions
            .iter()
            .any(|definition| definition == "(= result a)"));
    }

    #[test]
    fn addition_contributes_a_range_obligation() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64, b: i64) -> i64\n    requires a >= 0\n    requires b >= 0\n    ensures result >= 0\n{ a + b }\n",
        );
        let encoding = translate_function(&f).expect("supported");
        assert!(encoding
            .shared_obligations
            .iter()
            .any(|obligation| obligation.formula.contains("(+ a b)")));
    }

    #[test]
    fn if_branches_guard_their_own_arithmetic_obligations() {
        let f = function(
            r#"
module app.t;
@id("app.t.f")
fn f(a: i64) -> i64
    ensures result >= 0
{ if a > 0 { a + 1 } else { 0 } }
"#,
        );
        let encoding = translate_function(&f).expect("supported");
        let add_obligation = encoding
            .shared_obligations
            .iter()
            .find(|obligation| obligation.formula.contains("(+ a 1)"))
            .expect("the then-branch addition contributes an obligation");
        assert!(add_obligation.guard.contains("(> a 0)"));
    }

    #[test]
    fn or_short_circuit_guards_the_second_operand() {
        let f = function(
            r#"
module app.t;
@id("app.t.f")
fn f(a: i64) -> i64
    requires a == 0 || a + 1 > 0
    ensures result >= 0
{ a }
"#,
        );
        let encoding = translate_function(&f).expect("supported");
        let add_obligation = encoding
            .shared_obligations
            .iter()
            .find(|obligation| obligation.formula.contains("(+ a 1)"))
            .expect("the right operand's addition contributes an obligation");
        assert!(add_obligation.guard.contains("(not (= a 0))"));
    }

    #[test]
    fn division_is_rejected_with_the_closed_reason() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64, b: i64) -> i64\n    ensures result >= 0\n{ a / b }\n",
        );
        assert!(matches!(
            translate_function(&f),
            Err(UnsupportedReason::Expr { .. })
        ));
    }

    #[test]
    fn immutable_let_bindings_are_supported() {
        let f = function(
            r#"
module app.t;
@id("app.t.f")
fn f(a: i64) -> i64
    requires a >= 0
    ensures result >= 0
{
    let doubled = a + a;
    doubled
}
"#,
        );
        let encoding = translate_function(&f).expect("supported");
        assert_eq!(encoding.ensures.len(), 1);
        assert!(encoding
            .shared_obligations
            .iter()
            .any(|obligation| obligation.formula.contains("(+ a a)")));
    }

    #[test]
    fn mutable_let_bindings_are_rejected() {
        let f = function(
            r#"
module app.t;
@id("app.t.f")
fn f(a: i64) -> i64
    ensures result >= 0
{
    let mut doubled = a + a;
    doubled
}
"#,
        );
        assert!(matches!(
            translate_function(&f),
            Err(UnsupportedReason::MutableLocalBinding { .. })
        ));
    }

    #[test]
    fn mismatched_arithmetic_operand_modes_are_rejected() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64, b: i32) -> i64\n    ensures result >= 0\n{ a }\n",
        );
        // `a` (i64) compared/added against `b` (i32) is exercised through a
        // synthetic body below rather than this parsed one, since the
        // parser's own type checker is out of scope here; the translator
        // must independently reject the mismatch if asked to.
        let mismatched = Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr {
                    kind: ExprKind::Var("a".to_owned()),
                    span: Default::default(),
                }),
                right: Box::new(Expr {
                    kind: ExprKind::Var("b".to_owned()),
                    span: Default::default(),
                }),
            },
            span: Default::default(),
        };
        let mut ctx = Ctx {
            declarations: Vec::new(),
            range_axioms: Vec::new(),
            definitions: Vec::new(),
            obligations: Vec::new(),
            scope: vec![
                (
                    "a".to_owned(),
                    Translated {
                        term: "a".to_owned(),
                        sort: Sort::Numeric(NumericMode::I64),
                    },
                ),
                (
                    "b".to_owned(),
                    Translated {
                        term: "b".to_owned(),
                        sort: Sort::Numeric(NumericMode::I32),
                    },
                ),
            ],
            fresh: 0,
            label_prefix: "test".to_owned(),
        };
        assert!(matches!(
            translate_expr(&mut ctx, &mismatched, "true"),
            Err(UnsupportedReason::OperandTypeMismatch { .. })
        ));
        let _ = f;
    }

    #[test]
    fn a_let_bound_name_does_not_leak_past_its_block() {
        // Two sibling `if` branches each bind an internal `let` with the
        // same name; translation must not let the second see the first's
        // binding (which would be a scope leak, not just a naming clash).
        let f = function(
            r#"
module app.t;
@id("app.t.f")
fn f(a: i64) -> i64
    ensures result >= 0
{
    if a > 0 {
        let n = a;
        n
    } else {
        let n = 0;
        n
    }
}
"#,
        );
        let encoding = translate_function(&f).expect("supported");
        assert_eq!(encoding.ensures.len(), 1);
    }
}
