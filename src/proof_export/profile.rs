//! `semaprax-lean-export-profile-v1`: the exact, deliberately tiny slice of
//! SEMAPRAX this tranche translates to Lean 4, and a closed reason for every
//! declaration it refuses.
//!
//! The admitted subset is a strict *narrowing* of the already-shipped
//! bounded SMT-discharge subset
//! ([`crate::assurance_manifest::smt_discharge::subset`]), whose
//! [`UnsupportedReason`] vocabulary this module reuses verbatim rather than
//! re-deriving. On top of it this profile additionally excludes:
//!
//! - generic functions (`type_parameters` non-empty),
//! - functions declaring any effect,
//! - any parameter whose mode is not `Value` (`own`/`borrow`/`shared`),
//! - `bool` in a *value* position (parameter, `let`, or return type) —
//!   booleans exist in this profile only as the sort of a contract clause
//!   or of a comparison/connective inside one,
//! - conditional (`if`) expressions,
//! - functions with no `ensures` clause, since an obligation to export is
//!   exactly an `ensures` clause.
//!
//! Every one of those exclusions is reported, never silently skipped: see
//! [`super::coverage`]. Silently dropping a construct and then reporting the
//! enclosing obligation as proved is the single failure mode issue #186
//! calls out by name, so the export refuses a declaration wholesale the
//! instant any part of it leaves this profile.

use crate::assurance_manifest::smt_discharge::{NumericMode, Sort, UnsupportedReason};
use crate::ast::{Function, ParamMode, Type};

/// The profile string embedded verbatim in every generated Lean header, in
/// the coverage report, and in every certificate's `profile` field, so a
/// reader never has to guess how far an accepted obligation reaches.
pub const PROFILE_V1: &str = "semaprax-lean-export-profile-v1: non-generic, effect-free, \
all-by-value pure functions over i64/i32/u8/usize; add/sub/mul/neg, comparisons and \
and/or/not inside contracts, immutable let, at least one `ensures`; no bool-valued \
parameters/lets/returns, no `if`, no division/remainder, no calls, no loops, no mutation, \
no aggregates; see docs/LEAN-OBLIGATION-EXPORT-V1.md";

/// Additive LAW-07 profile used only for source-bound finite aggregate
/// scalarization. The older profile and its golden bytes remain unchanged.
pub const PROFILE_STRUCTURED_V1: &str = "semaprax-lean-structured-profile-v1: scalarized immutable finite records/closed variants, guarded if/match paths, simp_all then omega; explicit checked source/field/case binding required";

/// One closed exclusion category. Either a reason the shared bounded-subset
/// checker already owns, or one of this profile's own extra narrowings.
/// Nothing here is a catch-all: every variant names exactly one construct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Excluded {
    /// Reused verbatim from the bounded SMT-discharge subset.
    Shared(UnsupportedReason),
    /// `fn f<T>(..)`: this profile has no polymorphism story in Lean yet.
    Generic { parameters: usize },
    /// The function declares at least one effect, so it is not pure.
    Effectful { effects: String },
    /// A parameter declared `own`/`borrow`/`shared`. Ownership is a
    /// compile-time discipline this translation does not model at all.
    OwnershipMode { index: usize, mode: &'static str },
    /// `bool` used as a value (parameter, `let`, or return type). Contract
    /// clauses are translated to Lean `Prop`s, so a `bool` *value* would
    /// need a `Bool`/`Prop` coercion this profile does not introduce.
    BoolValued { position: String },
    /// An `if` expression. Admitting one would put `ite` in every goal and
    /// take the emitted proof past the linear-arithmetic tactic budget this
    /// profile commits to; deferred rather than emitted unproved.
    Conditional,
    /// A comparison or connective whose operands did not both resolve to the
    /// sort that operator requires.
    OperandSort {
        op: &'static str,
        wanted: &'static str,
    },
    /// The function has `requires` but no `ensures`, so there is no
    /// postcondition obligation to export.
    NoEnsuresClause,
    /// A type outside `{i64, i32, u8, usize, bool}` in some named position.
    UnsupportedValueType { position: String, ty: String },
    /// A module-level declaration that is not a function at all (a record,
    /// class, variant, interface, protocol, implementation, or agent).
    /// Reported so a coverage reader sees every declaration in the module,
    /// not only the functions this profile could have exported.
    NonFunctionDeclaration { kind: &'static str },
}

impl Excluded {
    /// A short, stable, machine-comparable code. For [`Self::Shared`] this
    /// is exactly the shared checker's own code, so a reader comparing a
    /// Lean coverage report against an SMT one sees one vocabulary.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Shared(reason) => reason.code(),
            Self::Generic { .. } => "generic_function",
            Self::Effectful { .. } => "effectful_function",
            Self::OwnershipMode { .. } => "ownership_param_mode",
            Self::BoolValued { .. } => "bool_valued_position",
            Self::Conditional => "conditional_expression",
            Self::OperandSort { .. } => "operand_sort",
            Self::NoEnsuresClause => "no_ensures_clause",
            Self::UnsupportedValueType { .. } => "unsupported_value_type",
            Self::NonFunctionDeclaration { .. } => "non_function_declaration",
        }
    }

    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            Self::Shared(reason) => reason.detail(),
            Self::Generic { parameters } => {
                format!("function declares {parameters} type parameter(s)")
            }
            Self::Effectful { effects } => format!("function declares effects `{effects}`"),
            Self::OwnershipMode { index, mode } => {
                format!("parameter {index} is declared `{mode}`, not by value")
            }
            Self::BoolValued { position } => {
                format!("`bool` in value position ({position}); this profile admits bool only as a contract/operator sort")
            }
            Self::Conditional => {
                "`if` expression (deferred: outside this profile's linear-arithmetic tactic budget)"
                    .to_owned()
            }
            Self::OperandSort { op, wanted } => {
                format!("`{op}` requires {wanted} operands")
            }
            Self::NoEnsuresClause => {
                "function has no `ensures` clause, so there is no postcondition obligation to export"
                    .to_owned()
            }
            Self::UnsupportedValueType { position, ty } => {
                format!("type `{ty}` in {position} is outside {{i64,i32,u8,usize,bool}}")
            }
            Self::NonFunctionDeclaration { kind } => {
                format!("`{kind}` declarations are outside this profile, which exports functions only")
            }
        }
    }
}

/// The name this profile uses for a parameter mode in a rejection.
#[must_use]
pub const fn mode_name(mode: ParamMode) -> &'static str {
    match mode {
        ParamMode::Value => "value",
        ParamMode::Own => "own",
        ParamMode::Borrow => "borrow",
        ParamMode::Shared => "shared",
    }
}

/// Map one [`Type`] to the [`Sort`] the shared bounded subset gives it.
///
/// `smt_discharge`'s own `subset::sort_of_type` is private to that module
/// (only the `Sort`/`NumericMode`/`UnsupportedReason` *vocabulary* is
/// re-exported), so this profile restates the five-line mapping rather than
/// widening another module's public surface from here. The types — and
/// therefore `NumericMode::min`/`max`, the checked-range facts the whole
/// translation rests on — are still the shared ones, not copies.
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

/// Map a type to the numeric mode this profile admits in a *value*
/// position. `bool` deliberately fails here even though the shared subset
/// admits it: see [`Excluded::BoolValued`].
pub fn value_mode(ty: &Type, position: &str) -> Result<NumericMode, Excluded> {
    match sort_of_type(ty) {
        Some(Sort::Numeric(mode)) => Ok(mode),
        Some(Sort::Bool) => Err(Excluded::BoolValued {
            position: position.to_owned(),
        }),
        None => Err(Excluded::UnsupportedValueType {
            position: position.to_owned(),
            ty: ty.to_string(),
        }),
    }
}

/// Declaration-level admission: everything checkable without walking a
/// single expression. Runs before the body walk so a caller gets the most
/// specific available reason, and so a rejected declaration never reaches
/// the renderer at all.
pub fn admit_declaration(function: &Function) -> Result<(), Excluded> {
    if !function.type_parameters.is_empty() {
        return Err(Excluded::Generic {
            parameters: function.type_parameters.len(),
        });
    }
    if !function.effects.is_empty() {
        return Err(Excluded::Effectful {
            effects: function.effects.join(","),
        });
    }
    if function.ensures.is_empty() {
        return Err(if function.requires.is_empty() {
            Excluded::Shared(UnsupportedReason::NoContractClauses)
        } else {
            Excluded::NoEnsuresClause
        });
    }
    for (index, param) in function.params.iter().enumerate() {
        if param.mode != ParamMode::Value {
            return Err(Excluded::OwnershipMode {
                index,
                mode: mode_name(param.mode),
            });
        }
        if sort_of_type(&param.ty).is_none() {
            return Err(Excluded::Shared(UnsupportedReason::ParamType {
                index,
                ty: param.ty.to_string(),
            }));
        }
        value_mode(&param.ty, &format!("parameter {index}"))?;
    }
    if sort_of_type(&function.return_type).is_none() {
        return Err(Excluded::Shared(UnsupportedReason::ReturnType {
            ty: function.return_type.to_string(),
        }));
    }
    value_mode(&function.return_type, "return type")?;
    Ok(())
}
