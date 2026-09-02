//! HIR expression, statement, and pattern nodes.
//!
//! The value-level half of the resolved data model, including the
//! authored-order child accessors both validators walk.

use crate::ast::{BinaryOp, Span, UnaryOp};

use super::ids::{DeclarationId, ExpressionId, FunctionInstanceId};
use super::nodes::{
    OwnershipMode, ResolvedBinding, ResolvedHostCommandCall, ResolvedMatchMode,
    ResolvedNativeRustImportCall, ResolvedType,
};
use super::Place;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExpr {
    pub id: ExpressionId,
    pub ty: ResolvedType,
    pub ownership: OwnershipMode,
    pub kind: ResolvedExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedExprKind {
    Int(i64),
    /// An `i32` literal held as its exact value.
    Int32(i32),
    /// A `char` literal held as its exact Unicode scalar value.
    Char(u32),
    /// A `u8` literal held as its exact value.
    Uint8(u8),
    /// A target-independent `usize` literal held as its exact u64 value.
    Usize(u64),
    ArrayU8(Vec<u8>),
    RepeatArrayU8 {
        value: u8,
        count: u32,
    },
    /// An `f32` literal held as its exact IEEE-754 bit pattern.
    Float32(u32),
    /// An `f64` literal held as its exact IEEE-754 bit pattern.
    Float64(u64),
    Bool(bool),
    /// A string literal held as its exact owned UTF-8 contents.
    String(String),
    Place(Place),
    /// A compiler-owned, non-consuming view of one exact authenticated place.
    BorrowPlace {
        operation: DeclarationId,
        place: Place,
    },
    /// A compiler-owned fallible half-open subview of one exact named slice.
    /// This is deliberately not an ordinary call: its borrowed result and
    /// parent provenance must be independently reconstructable from HIR.
    ByteRange {
        operation: DeclarationId,
        source: Box<ResolvedExpr>,
        start: Box<ResolvedExpr>,
        end: Box<ResolvedExpr>,
    },
    Call {
        callee: DeclarationId,
        type_arguments: Vec<ResolvedType>,
        instance: Option<FunctionInstanceId>,
        args: Vec<ResolvedExpr>,
    },
    NativeRustImportCall(ResolvedNativeRustImportCall),
    HostCommandCall(ResolvedHostCommandCall),
    Unary {
        op: UnaryOp,
        value: Box<ResolvedExpr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<ResolvedExpr>,
        right: Box<ResolvedExpr>,
    },
    Block {
        statements: Vec<ResolvedStatement>,
        tail: Box<ResolvedExpr>,
    },
    If {
        condition: Box<ResolvedExpr>,
        then_branch: Box<ResolvedExpr>,
        else_branch: Box<ResolvedExpr>,
    },
    ConstructRecord {
        record: DeclarationId,
        fields: Vec<ResolvedFieldInitializer>,
    },
    ConstructVariant {
        variant: DeclarationId,
        case: DeclarationId,
        fields: Vec<ResolvedFieldInitializer>,
    },
    Match {
        mode: ResolvedMatchMode,
        scrutinee: Box<ResolvedExpr>,
        arms: Vec<ResolvedMatchArm>,
    },
    Try {
        operand: Box<ResolvedExpr>,
        result: DeclarationId,
        ok_case: DeclarationId,
        ok_field: DeclarationId,
        err_case: DeclarationId,
        err_field: DeclarationId,
        residual_type: ResolvedType,
    },
    TryOption {
        operand: Box<ResolvedExpr>,
        option: DeclarationId,
        some_case: DeclarationId,
        some_field: DeclarationId,
        none_case: DeclarationId,
        residual_type: ResolvedType,
    },
    UpdateRecord {
        base: Box<ResolvedExpr>,
        record: DeclarationId,
        fields: Vec<ResolvedFieldInitializer>,
    },
    Project {
        base: Box<ResolvedExpr>,
        field: DeclarationId,
    },
    /// Class Inheritance v1: implicit prefix upcast of an owned descendant
    /// class value to an ancestor class value. The source is consumed; its
    /// inherited leaves transfer into the ancestor-typed result, so the
    /// child-declared suffix must be cleanup-inert (checked at resolution).
    /// Backends copy the ancestor prefix field-by-field from the source.
    Upcast {
        source: Box<ResolvedExpr>,
    },
}

impl ResolvedMatchArm {
    /// Refutable Match v1 interpreter admission: literal and or-of-literal
    /// patterns are refutable; wildcard/binding are irrefutable; aggregate
    /// patterns never join the scalar profile.
    pub fn pattern_is_literal_or_irrefutable(&self) -> bool {
        match &self.pattern {
            ResolvedMatchPattern::Wildcard
            | ResolvedMatchPattern::Binding(_)
            | ResolvedMatchPattern::Literal(_) => true,
            ResolvedMatchPattern::Or(alternatives) => alternatives
                .iter()
                .all(|alternative| matches!(alternative, ResolvedMatchPattern::Literal(_))),
            ResolvedMatchPattern::Variant { .. } | ResolvedMatchPattern::Record { .. } => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMatchArm {
    pub pattern: ResolvedMatchPattern,
    /// Refutable Match v1: ordinary bool expression evaluated once after the
    /// pattern matches; a false result falls through to the following arms.
    /// `None` for every pre-feature arm.
    pub guard: Option<Box<ResolvedExpr>>,
    pub value: ResolvedExpr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedMatchPattern {
    Variant {
        variant: DeclarationId,
        case: DeclarationId,
        fields: Vec<ResolvedMatchPatternField>,
    },
    Record {
        record: DeclarationId,
        instance: ResolvedType,
        fields: Vec<ResolvedRecordMatchPatternField>,
    },
    Wildcard,
    /// Refutable Match v1: one exact scalar literal compared against the
    /// scrutinee with exact equality. The literal's type equals the
    /// scrutinee type; floats are never admitted.
    Literal(PatternValue),
    /// Refutable Match v1: `a | b` flattened to same-typed literal
    /// alternatives. Never empty; nesting is rejected at resolution.
    Or(Vec<ResolvedMatchPattern>),
    /// Refutable Match v1: irrefutable whole-scrutinee binding of a Copy
    /// scalar.
    Binding(ResolvedBinding),
}

/// The exact scalar value carried by [`ResolvedMatchPattern::Literal`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatternValue {
    Int(i64),
    Int32(i32),
    Uint8(u8),
    Usize(u64),
    Char(u32),
    Bool(bool),
}

impl PatternValue {
    pub fn from_ast(value: crate::ast::PatternLiteral) -> Self {
        match value {
            crate::ast::PatternLiteral::Int(inner) => Self::Int(inner),
            crate::ast::PatternLiteral::Int32(inner) => Self::Int32(inner),
            crate::ast::PatternLiteral::Uint8(inner) => Self::Uint8(inner),
            crate::ast::PatternLiteral::Usize(inner) => Self::Usize(inner),
            crate::ast::PatternLiteral::Char(inner) => Self::Char(inner),
            crate::ast::PatternLiteral::Bool(inner) => Self::Bool(inner),
        }
    }

    /// The scalar scrutinee type this literal compares against.
    pub fn ty(&self) -> ResolvedType {
        match self {
            Self::Int(_) => ResolvedType::I64,
            Self::Int32(_) => ResolvedType::I32,
            Self::Uint8(_) => ResolvedType::U8,
            Self::Usize(_) => ResolvedType::Usize,
            Self::Char(_) => ResolvedType::Char,
            Self::Bool(_) => ResolvedType::Bool,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMatchPatternField {
    pub field: DeclarationId,
    pub binding: ResolvedBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRecordMatchPatternField {
    pub field: DeclarationId,
    pub pattern: ResolvedRecordMatchFieldPattern,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedRecordMatchFieldPattern {
    Binding(ResolvedBinding),
    Wildcard,
    Record {
        record: DeclarationId,
        instance: ResolvedType,
        fields: Vec<ResolvedRecordMatchPatternField>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldInitializer {
    pub field: DeclarationId,
    pub value: ResolvedExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedStatement {
    Let {
        binding: ResolvedBinding,
        /// Explicit Mutation v1: `true` when the source declared `let mut`.
        mutable: bool,
        value: ResolvedExpr,
        span: Span,
    },
    /// Explicit Mutation v1: `<binding> = <expr>;`. The target reuses the
    /// original binding's `ValueId`; no new value identity is created.
    /// Field Mutation v1: `field` names the one direct scalar field of a
    /// `<binding>.<field>` target; the store replaces that whole field.
    Assign {
        binding: ResolvedBinding,
        field: Option<DeclarationId>,
        value: ResolvedExpr,
        span: Span,
    },
    /// Unsafe Boundary Mechanics v1: `@audit("...") unsafe { ... }`. The body
    /// is an ordinary checked safe block expression; the audit summary is
    /// recorded verbatim. No raw pointers or memory operations exist.
    Unsafe {
        audit: String,
        body: Box<ResolvedExpr>,
        span: Span,
    },
    /// Bounded While-Loops v1: `while <condition> { <body> }`. The condition
    /// must be exactly `bool` and the body is an ordinary checked block whose
    /// value is discarded. The statement produces no value.
    While {
        condition: Box<ResolvedExpr>,
        body: Box<ResolvedExpr>,
        span: Span,
    },
}

impl ResolvedStatement {
    /// The statement's evaluated expression. While statements carry two
    /// evaluated expressions and must be traversed with
    /// [`ResolvedStatement::child`] instead.
    pub fn value(&self) -> &ResolvedExpr {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => value,
            Self::Unsafe { body, .. } => body,
            Self::While { .. } => {
                panic!("while statements expose condition and body children")
            }
        }
    }

    /// Mutable access to the statement's evaluated expression.
    pub fn value_mut(&mut self) -> &mut ResolvedExpr {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => value,
            Self::Unsafe { body, .. } => body,
            Self::While { .. } => {
                panic!("while statements expose condition and body children")
            }
        }
    }

    /// The statement's target or declared binding. Only `let` and assignment
    /// statements carry one.
    pub fn binding(&self) -> &ResolvedBinding {
        match self {
            Self::Let { binding, .. } | Self::Assign { binding, .. } => binding,
            Self::Unsafe { .. } | Self::While { .. } => {
                panic!("only let and assignment statements declare a binding")
            }
        }
    }

    /// The verbatim audit summary of an unsafe boundary statement.
    pub fn audit(&self) -> Option<&str> {
        match self {
            Self::Unsafe { audit, .. } => Some(audit),
            _ => None,
        }
    }

    /// `true` for assignment statements.
    pub fn is_assign(&self) -> bool {
        matches!(self, Self::Assign { .. })
    }

    /// Number of directly nested evaluated expressions. `let`, assignment,
    /// and unsafe statements contribute one; while statements contribute its
    /// condition then its body, in evaluation order.
    pub fn child_count(&self) -> usize {
        match self {
            Self::Let { .. } | Self::Assign { .. } | Self::Unsafe { .. } => 1,
            Self::While { .. } => 2,
        }
    }

    /// One directly nested evaluated expression in left-to-right order.
    pub fn child(&self, index: usize) -> Option<&ResolvedExpr> {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => (index == 0).then_some(value),
            Self::Unsafe { body, .. } => (index == 0).then_some(body.as_ref()),
            Self::While {
                condition, body, ..
            } => [condition.as_ref(), body.as_ref()].get(index).copied(),
        }
    }
}
