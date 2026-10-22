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

/// Complete residual type kept out of the common expression enum's inline
/// layout. The private cache codec deliberately preserves the former Type wire
/// and node/depth accounting; the box owns no additional semantic authority.
#[repr(transparent)]
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
// Like the other public HIR fields, this raw payload carries NO checked
// authority. Independent hir::validate remains mandatory for forged inputs.
// Compiler and repository construction uses resolved_residual_type! below.
pub struct ResolvedResidualType(#[doc(hidden)] pub Box<ResolvedType>);

impl ResolvedResidualType {
    // Actual lexical constructor: full Type box backing; two Option<Self>
    // initializer/receiving carriers; two Self carriers; Box return/field headers;
    // Box::new's Type parameter; the reservation length; and the required-call
    // and zero-owned-argument admission-return bool receivers. No Type is
    // forwarded into a separate constructor function.
    pub(crate) const CONSTRUCTION_BYTES: usize = std::mem::size_of::<ResolvedType>()
        + 2 * std::mem::size_of::<Option<Self>>()
        + 2 * std::mem::size_of::<Self>()
        + 2 * std::mem::size_of::<Box<ResolvedType>>()
        + std::mem::size_of::<ResolvedType>()
        + std::mem::size_of::<usize>()
        + 2 * std::mem::size_of::<bool>();

    /// Fixed admission only: no owned value parameter and no allocation.
    /// The ordinary initial reservation failure remains an unreceived caller
    /// boundary; success prepays the lexical constructor before its arguments.
    #[doc(hidden)]
    pub fn construction_is_admitted() -> bool {
        crate::bounded_output::reserve_active_required(Self::CONSTRUCTION_BYTES)
    }

    /// Move the complete type into its caller's receiving slot without a new
    /// allocation. Iterative disposal releases this box before traversing the
    /// type's existing children, preserving its fixed frame-stack capacity.
    pub fn into_inner(self) -> ResolvedType {
        *self.0
    }
}

/// Construct a raw residual HIR carrier after its exact receiving admission.
/// This macro grants no validation, source, cache, or backend authority. Fallible
/// old Type producers must run before this macro, preserving their original order.
#[macro_export]
macro_rules! resolved_residual_type {
    ($value:expr) => {{
        if $crate::hir::ResolvedResidualType::construction_is_admitted() {
            ::core::option::Option::Some($crate::hir::ResolvedResidualType(::std::boxed::Box::new(
                $value,
            )))
        } else {
            ::core::option::Option::None
        }
    }};
}

impl std::ops::Deref for ResolvedResidualType {
    type Target = ResolvedType;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ResolvedResidualType {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl std::fmt::Debug for ResolvedResidualType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, formatter)
    }
}
impl PartialEq<ResolvedType> for ResolvedResidualType {
    fn eq(&self, other: &ResolvedType) -> bool {
        **self == *other
    }
}
impl PartialEq<ResolvedResidualType> for ResolvedType {
    fn eq(&self, other: &ResolvedResidualType) -> bool {
        *self == **other
    }
}

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
        residual_type: ResolvedResidualType,
    },
    TryOption {
        operand: Box<ResolvedExpr>,
        option: DeclarationId,
        some_case: DeclarationId,
        some_field: DeclarationId,
        none_case: DeclarationId,
        residual_type: ResolvedResidualType,
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
    use super::{DeclarationId, ResolvedMatchPattern};
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

#[cfg(test)]
mod residual_payload_tests {
    use super::{
        DeclarationId, ExpressionId, OwnershipMode, ResolvedExpr, ResolvedExprKind,
        ResolvedResidualType, ResolvedType, Span,
    };
    use crate::bounded_output as output;
    use crate::hir::residual_payload_before as before;
    use crate::test_allocation_observer::measure;
    use std::mem::size_of;

    #[test]
    fn residual_payload_compacts_the_complete_previous_expression_layout() {
        let saved = size_of::<ResolvedType>() - size_of::<Box<ResolvedType>>();
        // These are the complete Try payloads: the operand, residual type and
        // all five declaration identities. Boxing can remove the type's spare
        // enum tags, so the enclosing enum may need a separate tag and padding.
        let previous_payload =
            size_of::<(Box<before::ResolvedExpr>, ResolvedType, [DeclarationId; 5])>();
        let current_payload =
            size_of::<(Box<ResolvedExpr>, ResolvedResidualType, [DeclarationId; 5])>();
        assert_eq!(previous_payload - current_payload, saved);
        assert_eq!(
            size_of::<ResolvedResidualType>(),
            size_of::<Box<ResolvedType>>()
        );
        let alignment = std::mem::align_of::<ResolvedExprKind>();
        let previous_tag = size_of::<before::ResolvedExprKind>() - previous_payload;
        let current_tag = size_of::<ResolvedExprKind>() - current_payload;
        // Try must remain the largest payload. Its only possible extra storage
        // is one aligned discriminant; an unrelated larger variant fails here.
        assert!([0, alignment].contains(&previous_tag));
        assert!([0, alignment].contains(&current_tag));
        let complete_saved = saved + previous_tag - current_tag;
        assert!(complete_saved > 0);
        for (name, previous, current) in [
            (
                "expression kind",
                size_of::<before::ResolvedExprKind>(),
                size_of::<ResolvedExprKind>(),
            ),
            (
                "expression",
                size_of::<before::ResolvedExpr>(),
                size_of::<ResolvedExpr>(),
            ),
            (
                "statement",
                size_of::<before::ResolvedStatement>(),
                size_of::<super::ResolvedStatement>(),
            ),
            (
                "field initializer",
                size_of::<before::ResolvedFieldInitializer>(),
                size_of::<super::ResolvedFieldInitializer>(),
            ),
            (
                "match arm",
                size_of::<before::ResolvedMatchArm>(),
                size_of::<super::ResolvedMatchArm>(),
            ),
            (
                "closure capture",
                size_of::<before::ResolvedClosureCapture>(),
                size_of::<super::ResolvedClosureCapture>(),
            ),
        ] {
            assert_eq!(previous - current, complete_saved, "{name}");
        }
        assert_eq!(
            std::mem::align_of::<before::ResolvedExpr>(),
            std::mem::align_of::<ResolvedExpr>()
        );
    }

    #[test]
    fn residual_payload_zero_exact_short_and_drop_pay_before_allocation() {
        let debit = ResolvedResidualType::CONSTRUCTION_BYTES;
        for limit in [0, debit - 1, debit] {
            let ((constructed, allocation), refused, used) =
                output::with_limit_usage(limit, || {
                    measure(|| {
                        let value = crate::hir::resolved_residual_type!(ResolvedType::Bool);
                        let constructed = value.is_some();
                        if let Some(value) = value {
                            assert_eq!(*value, ResolvedType::Bool);
                            let inner = value.into_inner();
                            assert_eq!(inner, ResolvedType::Bool);
                            drop(inner);
                        }
                        constructed
                    })
                });
            assert_eq!(constructed, limit == debit);
            assert_eq!(refused, limit < debit);
            assert_eq!(used, if constructed { debit } else { 0 });
            let bytes = if constructed {
                size_of::<ResolvedType>()
            } else {
                0
            };
            assert!(!allocation.invalid);
            assert_eq!(allocation.allocated, bytes);
            assert_eq!(allocation.freed, bytes);
            assert_eq!(allocation.live, 0);
            assert_eq!(allocation.reallocations, 0);
        }
        let (_, refused, used, first, _) = output::with_limit_usage_refusal_site(0, || {
            assert!(!output::reserve_active_required(debit + 17));
            assert!(crate::hir::resolved_residual_type!(ResolvedType::Bool).is_none());
        });
        assert!(refused);
        assert_eq!(used, 0);
        assert_eq!(first.unwrap().requested, debit + 17);
    }

    #[test]
    fn lexical_residual_receiving_keeps_old_producer_order_floor_and_restored_sticky_owner() {
        use std::cell::Cell;
        let old_produced = Cell::new(0);
        let new_arguments = Cell::new(0);
        let debit = ResolvedResidualType::CONSTRUCTION_BYTES;
        let ((present, allocation), refused, used, first, _) =
            output::with_limit_usage_refusal_site(debit, || {
                assert!(output::set_active_floor(1));
                // The old fallible producer's result exists at its original
                // point. It is not skipped by new receiving admission.
                old_produced.set(old_produced.get() + 1);
                let ty = ResolvedType::Bool;
                measure(|| {
                    let value = crate::hir::resolved_residual_type!({
                        new_arguments.set(new_arguments.get() + 1);
                        ty
                    });
                    let present = value.is_some();
                    drop(value);
                    present
                })
            });
        assert!(!present && refused);
        assert_eq!(old_produced.get(), 1);
        assert_eq!(new_arguments.get(), 0);
        assert_eq!(used, 0);
        assert_eq!(first.unwrap().requested, debit);
        assert!(!allocation.invalid);
        assert_eq!(allocation.allocated, 0);
        assert_eq!(allocation.live, 0);

        let (_, refused, used, first, _) = output::with_limit_usage_refusal_site(0, || {
            assert!(!output::reserve_active_required(17));
            // A nested child is clamped by its parent's remaining zero bytes;
            // asking for the constructor's debit does not create another payer.
            let arguments = Cell::new(0);
            let ((value, allocation), child_refused, child_used) =
                output::with_limit_usage(debit, || {
                    assert_eq!(output::active_limit(), Some(0));
                    measure(|| {
                        crate::hir::resolved_residual_type!({
                            arguments.set(arguments.get() + 1);
                            ResolvedType::I64
                        })
                    })
                });
            assert!(value.is_none() && child_refused);
            assert_eq!(child_used, 0);
            assert_eq!(arguments.get(), 0);
            assert!(!allocation.invalid);
            assert_eq!(allocation.allocated, 0);
            assert_eq!(allocation.freed, 0);
            assert_eq!(allocation.live, 0);
            assert_eq!(allocation.reallocations, 0);

            // A constructor retains no ambient owner/permit. This existing
            // explicit source scope owns an independent exact-debit payer,
            // then restores the already-sticky old account before receiving.
            let ((), inner_refused, inner_used) = output::source_owner_scope(
                debit,
                || {
                    assert_eq!(output::active_limit(), Some(debit));
                    measure(|| {
                        let value = crate::hir::resolved_residual_type!({
                            arguments.set(arguments.get() + 1);
                            ResolvedType::I64
                        })
                        .unwrap();
                        assert_eq!(*value, ResolvedType::I64);
                        drop(value);
                    })
                },
                |((), allocation), formatting_refused, _source_payer| {
                    assert!(!formatting_refused);
                    assert_eq!(arguments.get(), 1);
                    assert!(!allocation.invalid);
                    assert_eq!(allocation.allocated, size_of::<ResolvedType>());
                    assert_eq!(allocation.freed, size_of::<ResolvedType>());
                    assert_eq!(allocation.live, 0);
                    assert_eq!(allocation.reallocations, 0);
                    assert!(output::active_refused());
                    assert_eq!(output::active_remaining(), Some(0));
                },
            );
            assert!(!inner_refused);
            assert_eq!(inner_used, debit);
            assert!(crate::hir::resolved_residual_type!(ResolvedType::Bool).is_none());
        });
        assert!(refused);
        assert_eq!(used, 0);
        assert_eq!(first.unwrap().requested, 17);
    }

    fn leaf() -> ResolvedExpr {
        ResolvedExpr {
            id: ExpressionId::from_owned("residual.operand".into()),
            ty: ResolvedType::Bool,
            ownership: OwnershipMode::Value,
            kind: ResolvedExprKind::Bool(true),
            span: Span {
                start: 0,
                end: 1,
                line: 1,
                column: 1,
            },
        }
    }
    fn old_leaf() -> before::ResolvedExpr {
        let leaf = leaf();
        before::ResolvedExpr {
            id: leaf.id,
            ty: leaf.ty,
            ownership: leaf.ownership,
            kind: before::ResolvedExprKind::Bool(true),
            span: leaf.span,
        }
    }

    #[test]
    fn residual_payload_keeps_all_result_and_option_fields_debug_clone_and_backing() {
        let ty = ResolvedType::Function {
            parameters: vec![ResolvedType::I64, ResolvedType::Bool],
            result: Box::new(ResolvedType::Bytes),
        };
        let id = DeclarationId::new;
        let old = before::ResolvedExprKind::Try {
            operand: Box::new(old_leaf()),
            result: id("result"),
            ok_case: id("ok"),
            ok_field: id("ok.value"),
            err_case: id("error"),
            err_field: id("error.value"),
            residual_type: ty.clone(),
        };
        let current = ResolvedExprKind::Try {
            operand: Box::new(leaf()),
            result: id("result"),
            ok_case: id("ok"),
            ok_field: id("ok.value"),
            err_case: id("error"),
            err_field: id("error.value"),
            residual_type: {
                let residual = ty.clone();
                crate::hir::resolved_residual_type!(residual).unwrap()
            },
        };
        assert_eq!(format!("{old:#?}"), format!("{current:#?}"));
        let (_, old_allocation) = measure(|| {
            let value = old.clone();
            assert_eq!(value, old);
        });
        let (_, allocation) = measure(|| {
            let value = current.clone();
            assert_eq!(value, current);
        });
        assert!(!old_allocation.invalid && !allocation.invalid);
        assert_eq!(old_allocation.live, 0);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        // Cloning the operand saves its inline node bytes; the rare residual
        // creates its complete, separately owned Type box. Nested backing is
        // still cloned in full by both complete representations.
        assert_eq!(
            allocation.allocated,
            old_allocation.allocated + size_of::<ResolvedType>()
                - (size_of::<before::ResolvedExpr>() - size_of::<ResolvedExpr>())
        );
        let old = before::ResolvedExprKind::TryOption {
            operand: Box::new(old_leaf()),
            option: id("option"),
            some_case: id("some"),
            some_field: id("some.value"),
            none_case: id("none"),
            residual_type: ty.clone(),
        };
        let current = ResolvedExprKind::TryOption {
            operand: Box::new(leaf()),
            option: id("option"),
            some_case: id("some"),
            some_field: id("some.value"),
            none_case: id("none"),
            residual_type: crate::hir::resolved_residual_type!(ty).unwrap(),
        };
        assert_eq!(format!("{old:#?}"), format!("{current:#?}"));
        let mut changed = current.clone();
        let ResolvedExprKind::TryOption { residual_type, .. } = &mut changed else {
            unreachable!()
        };
        **residual_type = ResolvedType::Bool;
        assert_ne!(changed, current);
    }

    #[test]
    fn residual_payload_owned_handoff_preserves_children_without_allocation_or_budget() {
        for sticky in [false, true] {
            // This is the receiver's existing preallocated Type slot. Unboxing
            // cannot require a new disposal frame, reserve or allocation.
            let mut frames = Vec::<ResolvedType>::with_capacity(1);
            let (_, allocation) = measure(|| {
                let residual = ResolvedType::Function {
                    parameters: vec![ResolvedType::I64, ResolvedType::Bool],
                    result: Box::new(ResolvedType::Bytes),
                };
                let wrapped = crate::hir::resolved_residual_type!(residual).unwrap();
                let (_, refused, used, first, _) = output::with_limit_usage_refusal_site(0, || {
                    if sticky {
                        assert!(!output::reserve_active_required(1));
                    }
                    let before = crate::test_allocation_observer::current_metrics();
                    frames.push(wrapped.into_inner());
                    let after = crate::test_allocation_observer::current_metrics();
                    assert_eq!(after.allocated, before.allocated);
                    assert_eq!(after.allocations, before.allocations);
                    assert_eq!(after.freed - before.freed, size_of::<ResolvedType>());
                });
                assert_eq!(refused, sticky);
                assert_eq!(used, 0);
                assert_eq!(first.map(|r| r.requested), sticky.then_some(1));
                assert_eq!(frames.capacity(), 1);
                let ResolvedType::Function { parameters, result } = frames.pop().unwrap() else {
                    panic!("owned residual changed type during handoff")
                };
                assert_eq!(
                    parameters.as_slice(),
                    [ResolvedType::I64, ResolvedType::Bool]
                );
                assert_eq!(*result, ResolvedType::Bytes);
            });
            assert!(frames.is_empty());
            assert!(!allocation.invalid);
            assert_eq!(allocation.allocated, allocation.freed);
            assert_eq!(allocation.live, 0);
        }
    }

    #[test]
    fn residual_payload_cache_keeps_wire_depth_node_and_hostile_input_boundaries() {
        use crate::cache_codec::{decode, encode};
        for depth in [0, 40, 100, 127, 128] {
            let mut ty = ResolvedType::I64;
            for _ in 0..depth {
                ty = ResolvedType::Function {
                    parameters: Vec::new(),
                    result: Box::new(ty),
                };
            }
            let residual = ty.clone();
            let wrapped = crate::hir::resolved_residual_type!(residual).unwrap();
            let old = encode(&ty);
            let current = encode(&wrapped);
            assert_eq!(old, current, "codec depth {depth}");
            if let Ok(bytes) = old {
                let direct = decode::<ResolvedType>(&bytes);
                let boxed = decode::<ResolvedResidualType>(&bytes);
                assert_eq!(direct.is_ok(), boxed.is_ok(), "decode depth {depth}");
                if let (Ok(direct), Ok(boxed)) = (direct, boxed) {
                    assert_eq!(direct, *boxed);
                }
                let mut trailing = bytes.clone();
                trailing.push(0);
                assert!(decode::<ResolvedResidualType>(&trailing).is_err());
                assert!(decode::<ResolvedResidualType>(&bytes[..bytes.len() - 1]).is_err());
            }
        }
    }
}
