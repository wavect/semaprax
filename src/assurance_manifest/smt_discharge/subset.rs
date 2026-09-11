//! The exact bounded expression/type subset this discharge module accepts,
//! and a closed reason for every rejection.
//!
//! See [`docs/SMT-DISCHARGE-V1.md`](../../../docs/SMT-DISCHARGE-V1.md)
//! "Supported subset" for the full rationale. This module never rejects the
//! whole program: an unsupported function simply yields
//! [`super::DischargeOutcome::Unsupported`] for its contract obligations,
//! and the existing `runtime_guarded` method record `derive.rs` already
//! attaches keeps standing.

use crate::ast::{BinaryOp, Expr, ExprKind, Function, Statement, Type};

/// One closed exclusion category. Every variant names exactly one AST shape
/// or type this tranche does not translate; nothing here is a stand-in for
/// "translation failed for some other reason".
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnsupportedReason {
    /// The function has no `requires`/`ensures` clause, so there is nothing
    /// for this module to discharge.
    NoContractClauses,
    /// A parameter's type is outside `{i64, i32, u8, usize, bool}`.
    ParamType { index: usize, ty: String },
    /// The return type is outside `{i64, i32, u8, usize, bool}`, so `result`
    /// cannot be given a sort in `ensures`.
    ReturnType { ty: String },
    /// An expression form outside the bounded pure grammar: closures, calls
    /// of any kind, floats, strings/bytes/chars/arrays, records, variants,
    /// match, try, field projection, or division/remainder (excluded
    /// pending a verified truncating-division encoding; see the spec's
    /// "Explicitly deferred" section).
    Expr { what: &'static str },
    /// A local binding declared `let mut`; this subset is effect-free and
    /// admits only single-assignment locals.
    MutableLocalBinding { name: String },
    /// A statement other than an immutable `let`: assignment, `unsafe`,
    /// `while`, or `for` all introduce mutation, iteration, or an audited
    /// escape hatch this bounded subset does not model.
    NonLetStatement { what: &'static str },
    /// A `let` bound two operand types that disagree (or a declared
    /// annotation that disagrees with the inferred type of its value).
    TypeMismatch { detail: String },
    /// Both operands of a comparison, arithmetic, or logical operator (or
    /// both branches of `if`) resolved to different sorts.
    OperandTypeMismatch { op: &'static str },
    /// A `Var` name that resolves to neither a parameter, a prior `let` in
    /// scope, nor (inside `ensures`) `result`.
    UnknownName { name: String },
}

impl UnsupportedReason {
    /// A short, stable machine-comparable code, independent of the
    /// human-readable `detail()` text.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NoContractClauses => "no_contract_clauses",
            Self::ParamType { .. } => "param_type",
            Self::ReturnType { .. } => "return_type",
            Self::Expr { .. } => "expr",
            Self::MutableLocalBinding { .. } => "mutable_local_binding",
            Self::NonLetStatement { .. } => "non_let_statement",
            Self::TypeMismatch { .. } => "type_mismatch",
            Self::OperandTypeMismatch { .. } => "operand_type_mismatch",
            Self::UnknownName { .. } => "unknown_name",
        }
    }

    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            Self::NoContractClauses => {
                "function has no requires/ensures clause to discharge".to_owned()
            }
            Self::ParamType { index, ty } => {
                format!("parameter {index} has type `{ty}`, outside {{i64,i32,u8,usize,bool}}")
            }
            Self::ReturnType { ty } => {
                format!("return type `{ty}` is outside {{i64,i32,u8,usize,bool}}")
            }
            Self::Expr { what } => format!("unsupported expression form: {what}"),
            Self::MutableLocalBinding { name } => {
                format!("`let mut {name}` introduces mutable local state")
            }
            Self::NonLetStatement { what } => format!("unsupported statement form: {what}"),
            Self::TypeMismatch { detail } => detail.clone(),
            Self::OperandTypeMismatch { op } => {
                format!("`{op}` operands resolved to different sorts")
            }
            Self::UnknownName { name } => format!("unresolved name `{name}`"),
        }
    }
}

/// One of the four numeric modes this subset admits, plus `Bool`. Every
/// admitted [`crate::ast::Type`] maps to exactly one [`Sort`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sort {
    Numeric(NumericMode),
    Bool,
}

/// The exact representable range of one admitted numeric type, matching the
/// runtime's checked (trapping, never-wrapping) semantics: an operation
/// whose mathematical result leaves this range does not silently wrap, it
/// traps. See [`Self::min`]/[`Self::max`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericMode {
    I64,
    I32,
    U8,
    /// SEMAPRAX's `usize` is a target-independent checked unsigned 64-bit
    /// semantic integer (see `ast::Type::Usize`), not a host pointer width.
    Usize,
}

impl NumericMode {
    #[must_use]
    pub const fn min(self) -> i128 {
        match self {
            Self::I64 => i64::MIN as i128,
            Self::I32 => i32::MIN as i128,
            Self::U8 | Self::Usize => 0,
        }
    }

    #[must_use]
    pub const fn max(self) -> i128 {
        match self {
            Self::I64 => i64::MAX as i128,
            Self::I32 => i32::MAX as i128,
            Self::U8 => u8::MAX as i128,
            Self::Usize => u64::MAX as i128,
        }
    }

    #[must_use]
    pub const fn is_signed(self) -> bool {
        matches!(self, Self::I64 | Self::I32)
    }
}

/// Map one [`Type`] to its [`Sort`], or `None` if it is outside the
/// admitted subset.
#[must_use]
pub fn sort_of_type(ty: &Type) -> Option<Sort> {
    match ty {
        Type::I64 => Some(Sort::Numeric(NumericMode::I64)),
        Type::I32 => Some(Sort::Numeric(NumericMode::I32)),
        Type::U8 => Some(Sort::Numeric(NumericMode::U8)),
        Type::Usize => Some(Sort::Numeric(NumericMode::Usize)),
        Type::Bool => Some(Sort::Bool),
        _ => None,
    }
}

/// `true` when `function` has at least one contract clause and every
/// parameter/return type is admitted. This is a fast, shallow pre-check;
/// [`super::translate::translate_function`] performs the full expression
/// walk and is the authority for expression-level rejections.
pub fn check_declaration_supported(function: &Function) -> Result<(), UnsupportedReason> {
    if function.requires.is_empty() && function.ensures.is_empty() {
        return Err(UnsupportedReason::NoContractClauses);
    }
    for (index, param) in function.params.iter().enumerate() {
        if sort_of_type(&param.ty).is_none() {
            return Err(UnsupportedReason::ParamType {
                index,
                ty: param.ty.to_string(),
            });
        }
    }
    if !function.ensures.is_empty() && sort_of_type(&function.return_type).is_none() {
        return Err(UnsupportedReason::ReturnType {
            ty: function.return_type.to_string(),
        });
    }
    Ok(())
}

/// Classify one expression node's shape without recursing: used by the
/// translator to produce a closed reason the instant it meets an
/// unsupported node, and independently reusable so tests can assert
/// rejection without constructing a full function.
#[must_use]
pub fn expr_reason(expr: &Expr) -> Option<UnsupportedReason> {
    let what = match &expr.kind {
        ExprKind::Closure { .. } => "closure",
        ExprKind::Char(_) => "char literal",
        ExprKind::ArrayU8(_) => "byte array literal",
        ExprKind::RepeatArrayU8 { .. } => "repeated byte array literal",
        ExprKind::Float32(_) | ExprKind::Float64(_) => "floating point",
        ExprKind::String(_) => "string literal",
        ExprKind::Call { .. } => "call",
        ExprKind::MethodCall { .. } => "method call",
        ExprKind::SuperMethod { .. } => "super method call",
        ExprKind::Block { .. } => return None, // handled structurally by the translator
        ExprKind::ConstructRecord { .. } => "record construction",
        ExprKind::ConstructVariant { .. } => "variant construction",
        ExprKind::Match { .. } => "match",
        ExprKind::Try { .. } => "try",
        ExprKind::UpdateRecord { .. } => "record update",
        ExprKind::Project { .. } => "field projection",
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Bool(_)
        | ExprKind::Var(_)
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::If { .. } => return None,
    };
    Some(UnsupportedReason::Expr { what })
}

/// Classify one binary operator that the translator does not lower:
/// division and remainder are excluded pending a verified truncating-
/// division-to-`QF_LIA` encoding (see the spec's "Explicitly deferred").
#[must_use]
pub fn binary_op_reason(op: BinaryOp) -> Option<UnsupportedReason> {
    match op {
        BinaryOp::Div => Some(UnsupportedReason::Expr {
            what: "division (truncating-division encoding deferred)",
        }),
        BinaryOp::Rem => Some(UnsupportedReason::Expr {
            what: "remainder (truncating-division encoding deferred)",
        }),
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Eq
        | BinaryOp::Ne
        | BinaryOp::Lt
        | BinaryOp::Le
        | BinaryOp::Gt
        | BinaryOp::Ge
        | BinaryOp::And
        | BinaryOp::Or => None,
    }
}

/// Classify one statement: only an immutable `let` with no declared-type
/// mismatch continues the walk; everything else is a closed rejection.
pub fn statement_reason(statement: &Statement) -> Option<UnsupportedReason> {
    match statement {
        Statement::Let { name, mutable, .. } if *mutable => {
            Some(UnsupportedReason::MutableLocalBinding { name: name.clone() })
        }
        Statement::Let { .. } => None,
        Statement::Assign { .. } => Some(UnsupportedReason::NonLetStatement { what: "assign" }),
        Statement::Unsafe { .. } => Some(UnsupportedReason::NonLetStatement { what: "unsafe" }),
        Statement::While { .. } => Some(UnsupportedReason::NonLetStatement { what: "while" }),
        Statement::For { .. } => Some(UnsupportedReason::NonLetStatement { what: "for" }),
        // Any further statement variants this crate may add later are
        // conservatively unsupported until explicitly admitted.
        #[allow(unreachable_patterns)]
        _ => Some(UnsupportedReason::NonLetStatement { what: "statement" }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function(source: &str) -> Function {
        let mut program = crate::parse(source, "subset-test.spx").expect("parse");
        program.functions.swap_remove(0)
    }

    #[test]
    fn rejects_a_function_with_no_contract_clauses() {
        let f = function("module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64 { a }\n");
        assert_eq!(
            check_declaration_supported(&f),
            Err(UnsupportedReason::NoContractClauses)
        );
    }

    #[test]
    fn rejects_an_unsupported_parameter_type() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: str) -> i64\n    ensures result >= 0\n{ 0 }\n",
        );
        assert!(matches!(
            check_declaration_supported(&f),
            Err(UnsupportedReason::ParamType { index: 0, .. })
        ));
    }

    #[test]
    fn rejects_an_unsupported_return_type() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> f64\n    ensures result >= 0.0\n{ 0.0 }\n",
        );
        assert!(matches!(
            check_declaration_supported(&f),
            Err(UnsupportedReason::ReturnType { .. })
        ));
    }

    #[test]
    fn accepts_a_plain_numeric_contract_function() {
        let f = function(
            "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n",
        );
        assert_eq!(check_declaration_supported(&f), Ok(()));
    }

    #[test]
    fn division_and_remainder_are_closed_rejections_not_generic_ones() {
        assert_eq!(
            binary_op_reason(BinaryOp::Div),
            Some(UnsupportedReason::Expr {
                what: "division (truncating-division encoding deferred)"
            })
        );
        assert_eq!(
            binary_op_reason(BinaryOp::Rem),
            Some(UnsupportedReason::Expr {
                what: "remainder (truncating-division encoding deferred)"
            })
        );
        assert_eq!(binary_op_reason(BinaryOp::Add), None);
    }

    #[test]
    fn mutable_let_is_rejected_but_immutable_let_is_not() {
        fn let_statement(mutable: bool) -> Statement {
            Statement::Let {
                name: "x".to_owned(),
                name_span: Default::default(),
                mutable,
                declared: None,
                value: Expr {
                    kind: ExprKind::Int(0),
                    span: Default::default(),
                },
                span: Default::default(),
            }
        }
        assert!(matches!(
            statement_reason(&let_statement(true)),
            Some(UnsupportedReason::MutableLocalBinding { .. })
        ));
        assert_eq!(statement_reason(&let_statement(false)), None);
    }

    #[test]
    fn every_reason_code_is_a_stable_short_token() {
        let reasons = [
            UnsupportedReason::NoContractClauses,
            UnsupportedReason::ParamType {
                index: 0,
                ty: "str".to_owned(),
            },
            UnsupportedReason::ReturnType {
                ty: "f64".to_owned(),
            },
            UnsupportedReason::Expr { what: "call" },
            UnsupportedReason::MutableLocalBinding {
                name: "x".to_owned(),
            },
            UnsupportedReason::NonLetStatement { what: "assign" },
            UnsupportedReason::TypeMismatch {
                detail: "x".to_owned(),
            },
            UnsupportedReason::OperandTypeMismatch { op: "+" },
            UnsupportedReason::UnknownName {
                name: "x".to_owned(),
            },
        ];
        for reason in reasons {
            assert!(!reason.code().is_empty());
            assert!(!reason.detail().is_empty());
        }
    }
}
