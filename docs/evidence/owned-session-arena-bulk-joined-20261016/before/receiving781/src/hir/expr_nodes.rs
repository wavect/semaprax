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
pub struct ResolvedClosureCapture {
    /// Private body parameter receiving an admitted snapshot or borrowed view.
    pub binding: ResolvedBinding,
    /// Exact outer-scope place read when the closure is constructed.
    pub value: ResolvedExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedExprKind {
    /// Authenticated source constructor; the value executes an ordinary block.
    VecConstructor {
        element: ResolvedType,
        count: u32,
        statements: Vec<ResolvedStatement>,
        tail: Box<ResolvedExpr>,
    },
    /// A typed terminal flow, independently bound to its exact lexical target.
    LexicalExit {
        kind: crate::ast::LexicalExitKind,
        target: super::lexical_exit::ControlTarget,
        value: Option<Box<ResolvedExpr>>,
    },
    Closure {
        parameters: Vec<ResolvedBinding>,
        captures: Vec<ResolvedClosureCapture>,
        body: Box<ResolvedExpr>,
    },
    FunctionReference {
        target: DeclarationId,
    },
    Invoke {
        callable: Box<ResolvedExpr>,
        args: Vec<ResolvedExpr>,
    },
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
    /// Immutable decoded module bytes; a nonescaping view, never an owner.
    BorrowedUtf8Literal(super::StaticUtf8),
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
    /// Compiler-owned literal rendering. The template is the decoded source
    /// literal; the children are dynamic arguments only, in evaluation order.
    LiteralFormat {
        template: String,
        args: Vec<ResolvedExpr>,
    },
    /// Nonallocating read of one explicit field in a borrowed Vec generation.
    /// `args` are exactly [named Vec place, usize index], in authored order.
    /// The literal selector is resolved into `field`, never a runtime String.
    /// `bytes` denotes only the exact fused str_as_bytes(String-field) form.
    VecFieldRead {
        origin: super::VecFieldReadOrigin,
        element: ResolvedType,
        field: DeclarationId,
        bytes: bool,
        args: Vec<ResolvedExpr>,
    },
    /// Row Loan v1: a call-only borrow of one frozen owned-leaf Vec row. It
    /// appears only as a direct argument of an ordinary monomorphic call whose
    /// matching parameter is the descriptor's exact `borrow R` parameter.
    /// `args` are exactly [named Vec place, row index place], authored order.
    /// No record value is materialized; see `hir::row_loan`.
    RowLoan {
        loan: Box<super::row_loan::RowLoan>,
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
    /// Resumable Effects v1 (issue #204): `yield <request>`. `request`'s
    /// type is the enclosing function's declared `yields` request type; the
    /// whole node's own `ResolvedExpr::ty` is the declared response type.
    /// Admitted only as a direct top-level statement or tail expression of
    /// a `yields`-declaring function's own body block, exactly once --
    /// `hir::validation::yields` enforces both. See
    /// `docs/RESUMABLE-EFFECTS-V1.md`.
    Yield {
        request: Box<ResolvedExpr>,
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

/// Borrowed authored case traversal. Duplicate alternatives remain distinct.
pub(crate) struct VariantCaseIter<'a> {
    single: Option<&'a DeclarationId>,
    alternatives: std::slice::Iter<'a, ResolvedMatchPattern>,
}
impl<'a> Iterator for VariantCaseIter<'a> {
    type Item = &'a DeclarationId;
    fn next(&mut self) -> Option<Self::Item> {
        self.single.take().or_else(|| {
            self.alternatives.next().map(|pattern| {
                let ResolvedMatchPattern::Variant { case, .. } = pattern else {
                    unreachable!("borrowed variant alternatives were validated");
                };
                case
            })
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.alternatives.len() + usize::from(self.single.is_some());
        (count, Some(count))
    }
}
impl ExactSizeIterator for VariantCaseIter<'_> {}

impl ResolvedMatchPattern {
    /// Same case domain as variant_cases, without allocating a temporary Vec.
    pub(crate) fn case_iter(&self) -> Option<VariantCaseIter<'_>> {
        match self {
            Self::Variant { case, .. } => Some(VariantCaseIter {
                single: Some(case),
                alternatives: [].iter(),
            }),
            Self::Or(alternatives)
                if alternatives.iter().all(|pattern| {
                    matches!(pattern,
                    Self::Variant { fields, .. } if fields.is_empty())
                }) =>
            {
                Some(VariantCaseIter {
                    single: None,
                    alternatives: alternatives.iter(),
                })
            }
            _ => None,
        }
    }
    /// The cases a variant-match arm tests, in authored order: the one case
    /// of a case pattern, or every alternative of an or-pattern over
    /// payload-free cases. `None` for every other pattern.
    pub fn variant_cases(&self) -> Option<Vec<&DeclarationId>> {
        match self {
            Self::Variant { case, .. } => Some(vec![case]),
            Self::Or(alternatives) => alternatives
                .iter()
                .map(|alternative| match alternative {
                    Self::Variant { case, fields, .. } if fields.is_empty() => Some(case),
                    _ => None,
                })
                .collect(),
            Self::Record { .. } | Self::Wildcard | Self::Literal(_) | Self::Binding(_) => None,
        }
    }
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
    /// Authenticated expression-statement boundary. The scalar value is
    /// evaluated and discarded; this declares no binding or storage owner.
    /// Lexical completion is independently checked against the current target.
    /// An ordinary Let initializer never acquires this boundary implicitly.
    CopyDiscard { value: ResolvedExpr, span: Span },
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
    LexicalWhile {
        condition: Box<ResolvedExpr>,
        body: Box<ResolvedExpr>,
        latch: super::lexical_exit::LoopLatchKind,
        span: Span,
    },
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
            Self::Let { value, .. }
            | Self::Assign { value, .. }
            | Self::CopyDiscard { value, .. } => value,
            Self::Unsafe { body, .. } => body,
            Self::While { .. } | Self::LexicalWhile { .. } => {
                panic!("while statements expose condition and body children")
            }
        }
    }

    /// Mutable access to the statement's evaluated expression.
    pub fn value_mut(&mut self) -> &mut ResolvedExpr {
        match self {
            Self::Let { value, .. }
            | Self::Assign { value, .. }
            | Self::CopyDiscard { value, .. } => value,
            Self::Unsafe { body, .. } => body,
            Self::While { .. } | Self::LexicalWhile { .. } => {
                panic!("while statements expose condition and body children")
            }
        }
    }

    /// The statement's target or declared binding. Only `let` and assignment
    /// statements carry one.
    pub fn binding(&self) -> &ResolvedBinding {
        match self {
            Self::Let { binding, .. } | Self::Assign { binding, .. } => binding,
            Self::Unsafe { .. }
            | Self::While { .. }
            | Self::LexicalWhile { .. }
            | Self::CopyDiscard { .. } => {
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
            Self::Let { .. }
            | Self::Assign { .. }
            | Self::Unsafe { .. }
            | Self::CopyDiscard { .. } => 1,
            Self::While { .. } | Self::LexicalWhile { .. } => 2,
        }
    }

    /// One directly nested evaluated expression in left-to-right order.
    pub fn child(&self, index: usize) -> Option<&ResolvedExpr> {
        match self {
            Self::Let { value, .. }
            | Self::Assign { value, .. }
            | Self::CopyDiscard { value, .. } => (index == 0).then_some(value),
            Self::Unsafe { body, .. } => (index == 0).then_some(body.as_ref()),
            Self::While {
                condition, body, ..
            }
            | Self::LexicalWhile {
                condition, body, ..
            } => [condition.as_ref(), body.as_ref()].get(index).copied(),
        }
    }
}

#[cfg(test)]
mod borrowed_variant_case_tests {
    use super::*;
    fn case(id: &str) -> ResolvedMatchPattern {
        ResolvedMatchPattern::Variant {
            variant: DeclarationId::new("variant"),
            case: DeclarationId::new(id),
            fields: Vec::new(),
        }
    }
    #[test]
    fn borrowed_case_domain_matches_allocating_oracle_order_duplicates_and_refusal() {
        for pattern in [
            case("some"),
            ResolvedMatchPattern::Or(vec![case("none"), case("some"), case("none")]),
            ResolvedMatchPattern::Or(Vec::new()),
            ResolvedMatchPattern::Or(vec![case("none"), ResolvedMatchPattern::Wildcard]),
            ResolvedMatchPattern::Wildcard,
        ] {
            let oracle = pattern.variant_cases();
            let (answer, allocation) =
                crate::test_allocation_observer::measure(|| match (pattern.case_iter(), &oracle) {
                    (Some(mut actual), Some(expected)) => {
                        let count = expected.len();
                        assert_eq!(actual.len(), count);
                        for (index, expected) in expected.iter().enumerate() {
                            assert_eq!(actual.len(), count - index);
                            assert!(std::ptr::eq(actual.next().unwrap(), *expected));
                        }
                        assert_eq!(actual.len(), 0);
                        assert!(actual.next().is_none());
                        true
                    }
                    (None, None) => true,
                    _ => false,
                });
            assert!(answer);
            assert!(!allocation.invalid && allocation.allocated == 0 && allocation.live == 0);
        }
    }
}
