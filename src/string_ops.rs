//! Compiler-owned string operation intrinsics.
//!
//! Three prelude-style intrinsic functions from the first wave, four from
//! the second bounded wave, and two numeric-text conversions are admitted wherever owned `string` values are
//! already admitted. They carry reserved stable identities in the
//! compiler-owned `core.string.*` family so a call site resolves to an
//! ordinary monomorphic [`crate::hir::ResolvedExprKind::Call`] node; no
//! source declaration, prelude contract byte, or graph schema version
//! participates, so programs that never name the operations keep
//! byte-identical projections.
//!
//! First wave (gated as one helper/import group):
//!
//! - `string_len(s: string) -> i64` borrows its operand for a read.
//! - `string_concat(a: string, b: string) -> string` consumes both operands
//!   by move and returns one new owned string.
//! - `string_is_empty(s: string) -> bool` borrows its operand for a read.
//!
//! Second wave (breadth v2, gated as its own helper/import group so first
//! wave programs keep their exact committed bytes):
//!
//! - `string_starts_with(s: string, prefix: string) -> bool` borrows both.
//! - `string_contains(s: string, needle: string) -> bool` borrows both.
//! - `string_len_chars(s: string) -> i64` counts Unicode scalar values,
//!   borrowing its operand for a read.
//! - `string_from_char(c: char) -> string` consumes nothing and returns one
//!   new owned string holding the scalar value's UTF-8 encoding.
//!
//! Numeric text wave (gated separately to preserve all earlier target bytes):
//!
//! - `string_from_i64(value: i64) -> string` renders canonical decimal text.
//! - `string_from_usize(value: usize) -> string` renders canonical decimal text.
//!
//! Text Toolkit v1 (`docs/TEXT-TOOLKIT-V1.md`, gated as its own backend group
//! so every earlier program keeps its exact bytes):
//!
//! - `string_slice(s: string, start: i64, end: i64) -> string` copies a byte
//!   range that must lie on UTF-8 character boundaries.
//! - `string_find(s: string, needle: string, from: i64) -> i64` returns the
//!   first byte offset at or after `from`, or -1.
//! - `string_to_i64(s: string) -> Option<i64>` parses strict decimal text.
//! - `string_trim(s: string) -> string` strips ASCII whitespace at both ends.
//! - `string_byte_at(s: string, index: i64) -> i64` reads one byte value.
//! - `file_read_text(path: borrow str) -> string` reads one bounded UTF-8 file
//!   under the explicit `fs.read` effect. It is the only effectful member of
//!   this family; its identity lives in the `core.host.*` namespace.
//!
//! Out-of-range offsets, non-boundary slices, and invalid UTF-8 select the
//! checked `semaprax.text.v1` status; file failures select the existing
//! `semaprax.filesystem.v1` codes.

use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{OwnershipMode, ResolvedParam, ResolvedType, ValueId};

pub(crate) const LEN_NAME: &str = "string_len";
pub(crate) const CONCAT_NAME: &str = "string_concat";
pub(crate) const IS_EMPTY_NAME: &str = "string_is_empty";
pub(crate) const STARTS_WITH_NAME: &str = "string_starts_with";
pub(crate) const CONTAINS_NAME: &str = "string_contains";
pub(crate) const LEN_CHARS_NAME: &str = "string_len_chars";
pub(crate) const FROM_CHAR_NAME: &str = "string_from_char";
pub(crate) const FROM_I64_NAME: &str = "string_from_i64";
pub(crate) const FROM_USIZE_NAME: &str = "string_from_usize";
pub(crate) const SLICE_NAME: &str = "string_slice";
pub(crate) const FIND_NAME: &str = "string_find";
pub(crate) const TO_I64_NAME: &str = "string_to_i64";
pub(crate) const TRIM_NAME: &str = "string_trim";
pub(crate) const BYTE_AT_NAME: &str = "string_byte_at";
pub(crate) const FILE_READ_TEXT_NAME: &str = "file_read_text";

pub(crate) const LEN_ID: &str = "core.string.len";
pub(crate) const CONCAT_ID: &str = "core.string.concat";
pub(crate) const IS_EMPTY_ID: &str = "core.string.is_empty";
pub(crate) const STARTS_WITH_ID: &str = "core.string.starts_with";
pub(crate) const CONTAINS_ID: &str = "core.string.contains";
pub(crate) const LEN_CHARS_ID: &str = "core.string.len_chars";
pub(crate) const FROM_CHAR_ID: &str = "core.string.from_char";
pub(crate) const FROM_I64_ID: &str = "core.string.from_i64";
pub(crate) const FROM_USIZE_ID: &str = "core.string.from_usize";
pub(crate) const SLICE_ID: &str = "core.string.slice";
pub(crate) const FIND_ID: &str = "core.string.find";
pub(crate) const TO_I64_ID: &str = "core.string.to_i64";
pub(crate) const TRIM_ID: &str = "core.string.trim";
pub(crate) const BYTE_AT_ID: &str = "core.string.byte_at";
pub(crate) const FILE_READ_TEXT_ID: &str = "core.host.file-read-text";

/// The checked status domain of Text Toolkit v1.
pub(crate) const TEXT_STATUS_DOMAIN: &str = "semaprax.text.v1";
/// A byte offset or index lies outside the string (or `start > end`).
pub(crate) const TEXT_OUT_OF_RANGE_CODE: u32 = 1;
/// A slice bound splits a multi-byte UTF-8 character.
pub(crate) const TEXT_NOT_CHAR_BOUNDARY_CODE: u32 = 2;
/// File bytes are not valid UTF-8.
pub(crate) const TEXT_INVALID_UTF8_CODE: u32 = 3;
/// The effect `file_read_text` requires.
pub(crate) const FILE_READ_TEXT_EFFECT: &str = "fs.read";
/// The bounded file size `file_read_text` admits, equal to the Filesystem
/// I/O per-file maximum.
pub(crate) const MAX_FILE_TEXT_BYTES: u64 = 65_536;

/// One admitted string operation intrinsic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StringOp {
    /// Borrowed byte-length read.
    Len,
    /// Consuming concatenation of two owned strings.
    Concat,
    /// Borrowed emptiness read.
    IsEmpty,
    /// Borrowed prefix test over both operands.
    StartsWith,
    /// Borrowed substring test over both operands.
    Contains,
    /// Borrowed Unicode scalar-value count.
    LenChars,
    /// Allocation of one owned string from a copied scalar value.
    FromChar,
    /// Canonical decimal text for one copied signed integer.
    FromI64,
    /// Canonical decimal text for one copied portable size value.
    FromUsize,
    /// Checked byte-range copy on UTF-8 boundaries.
    Slice,
    /// Borrowed forward substring search from a checked offset.
    Find,
    /// Strict decimal parse into `Option<i64>`.
    ToI64,
    /// Copy without leading and trailing ASCII whitespace.
    Trim,
    /// Checked single byte read.
    ByteAt,
    /// Bounded UTF-8 file read under `fs.read`.
    FileReadText,
}

impl StringOp {
    /// The first three waves. Frozen Project candidate schemas enumerate
    /// exactly these operations, so Text Toolkit v1 is listed separately.
    pub(crate) const ALL: [Self; 9] = [
        Self::Len,
        Self::Concat,
        Self::IsEmpty,
        Self::StartsWith,
        Self::Contains,
        Self::LenChars,
        Self::FromChar,
        Self::FromI64,
        Self::FromUsize,
    ];

    /// Text Toolkit v1 operations, outside the frozen [`Self::ALL`] catalog.
    pub(crate) const TEXT_TOOLKIT: [Self; 6] = [
        Self::Slice,
        Self::Find,
        Self::ToI64,
        Self::Trim,
        Self::ByteAt,
        Self::FileReadText,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            StringOp::Len => LEN_NAME,
            StringOp::Concat => CONCAT_NAME,
            StringOp::IsEmpty => IS_EMPTY_NAME,
            StringOp::StartsWith => STARTS_WITH_NAME,
            StringOp::Contains => CONTAINS_NAME,
            StringOp::LenChars => LEN_CHARS_NAME,
            StringOp::FromChar => FROM_CHAR_NAME,
            StringOp::FromI64 => FROM_I64_NAME,
            StringOp::FromUsize => FROM_USIZE_NAME,
            StringOp::Slice => SLICE_NAME,
            StringOp::Find => FIND_NAME,
            StringOp::ToI64 => TO_I64_NAME,
            StringOp::Trim => TRIM_NAME,
            StringOp::ByteAt => BYTE_AT_NAME,
            StringOp::FileReadText => FILE_READ_TEXT_NAME,
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            StringOp::Len => LEN_ID,
            StringOp::Concat => CONCAT_ID,
            StringOp::IsEmpty => IS_EMPTY_ID,
            StringOp::StartsWith => STARTS_WITH_ID,
            StringOp::Contains => CONTAINS_ID,
            StringOp::LenChars => LEN_CHARS_ID,
            StringOp::FromChar => FROM_CHAR_ID,
            StringOp::FromI64 => FROM_I64_ID,
            StringOp::FromUsize => FROM_USIZE_ID,
            StringOp::Slice => SLICE_ID,
            StringOp::Find => FIND_ID,
            StringOp::ToI64 => TO_I64_ID,
            StringOp::Trim => TRIM_ID,
            StringOp::ByteAt => BYTE_AT_ID,
            StringOp::FileReadText => FILE_READ_TEXT_ID,
        }
    }

    pub(crate) fn arity(self) -> usize {
        self.param_names().len()
    }

    /// Source parameter names in left-to-right order; they only label
    /// diagnostics because the operations have no authored declaration.
    pub(crate) fn param_names(self) -> &'static [&'static str] {
        match self {
            StringOp::Len | StringOp::IsEmpty | StringOp::LenChars => &["s"],
            StringOp::FromChar => &["c"],
            StringOp::FromI64 | StringOp::FromUsize => &["value"],
            StringOp::Concat => &["a", "b"],
            StringOp::StartsWith => &["s", "prefix"],
            StringOp::Contains => &["s", "needle"],
            StringOp::Slice => &["s", "start", "end"],
            StringOp::Find => &["s", "needle", "from"],
            StringOp::ToI64 | StringOp::Trim => &["s"],
            StringOp::ByteAt => &["s", "index"],
            StringOp::FileReadText => &["path"],
        }
    }

    /// Resolved parameter types in left-to-right order. Every operation takes
    /// `string` operands except the copied-scalar constructors.
    pub(crate) fn param_types(self) -> &'static [ResolvedType] {
        match self {
            StringOp::Len | StringOp::IsEmpty | StringOp::LenChars => &[ResolvedType::String],
            StringOp::FromChar => &[ResolvedType::Char],
            StringOp::FromI64 => &[ResolvedType::I64],
            StringOp::FromUsize => &[ResolvedType::Usize],
            StringOp::Concat | StringOp::StartsWith | StringOp::Contains => {
                &[ResolvedType::String, ResolvedType::String]
            }
            StringOp::Slice => &[ResolvedType::String, ResolvedType::I64, ResolvedType::I64],
            StringOp::Find => &[
                ResolvedType::String,
                ResolvedType::String,
                ResolvedType::I64,
            ],
            StringOp::ToI64 | StringOp::Trim => &[ResolvedType::String],
            StringOp::ByteAt => &[ResolvedType::String, ResolvedType::I64],
            StringOp::FileReadText => &[ResolvedType::Str],
        }
    }

    pub(crate) fn consumes_arguments(self) -> bool {
        matches!(self, StringOp::Concat)
    }

    /// Whether the operation belongs to the breadth-v2 wave. Its native
    /// helpers and Wasm host imports gate as one separate group so programs
    /// that reach only first-wave operations keep their exact bytes.
    pub(crate) fn is_breadth_v2(self) -> bool {
        matches!(
            self,
            StringOp::StartsWith | StringOp::Contains | StringOp::LenChars | StringOp::FromChar
        )
    }

    /// Numeric-to-text operations form a third optional backend group so
    /// programs using either earlier wave retain byte-identical artifacts.
    pub(crate) fn is_numeric_text(self) -> bool {
        matches!(self, StringOp::FromI64 | StringOp::FromUsize)
    }

    /// Text Toolkit v1 forms a fourth optional backend group.
    pub(crate) fn is_text_toolkit(self) -> bool {
        Self::TEXT_TOOLKIT.contains(&self)
    }

    /// The host effect a call requires, if any. Only `file_read_text` has one.
    pub(crate) fn effect(self) -> Option<&'static str> {
        (self == StringOp::FileReadText).then_some(FILE_READ_TEXT_EFFECT)
    }

    pub(crate) fn return_type(self) -> ResolvedType {
        match self {
            StringOp::Len | StringOp::LenChars | StringOp::Find | StringOp::ByteAt => {
                ResolvedType::I64
            }
            StringOp::Concat
            | StringOp::FromChar
            | StringOp::FromI64
            | StringOp::FromUsize
            | StringOp::Slice
            | StringOp::Trim
            | StringOp::FileReadText => ResolvedType::String,
            StringOp::IsEmpty | StringOp::StartsWith | StringOp::Contains => ResolvedType::Bool,
            StringOp::ToI64 => option_i64(),
        }
    }

    pub(crate) fn ast_return_type(self) -> Type {
        match self {
            StringOp::Len | StringOp::LenChars | StringOp::Find | StringOp::ByteAt => Type::I64,
            StringOp::Concat
            | StringOp::FromChar
            | StringOp::FromI64
            | StringOp::FromUsize
            | StringOp::Slice
            | StringOp::Trim
            | StringOp::FileReadText => Type::String,
            StringOp::IsEmpty | StringOp::StartsWith | StringOp::Contains => Type::Bool,
            StringOp::ToI64 => Type::Named {
                name: "Option".to_owned(),
                arguments: vec![Type::I64],
            },
        }
    }
}

/// The compiler-owned `Option<i64>` that `string_to_i64` produces.
pub(crate) fn option_i64() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: crate::hir::DeclarationId::new(crate::prelude::OPTION_ID),
        arguments: vec![ResolvedType::I64],
    }
}

/// Whether `scrutinee` is a direct `string_to_i64` call, the one
/// `Option<i64>` producer the bounded interpreter and the while-body match
/// admission accept.
pub(crate) fn is_to_i64_call_hir(scrutinee: &crate::hir::ResolvedExpr) -> bool {
    matches!(
        &scrutinee.kind,
        crate::hir::ResolvedExprKind::Call { callee, instance: None, type_arguments, args }
            if by_id(callee.as_str()) == Some(StringOp::ToI64)
                && type_arguments.is_empty()
                && args.len() == 1
                && scrutinee.ty == option_i64()
    )
}

/// Resolve a source-level call name to its intrinsic operation.
pub(crate) fn by_name(name: &str) -> Option<StringOp> {
    match name {
        LEN_NAME => Some(StringOp::Len),
        CONCAT_NAME => Some(StringOp::Concat),
        IS_EMPTY_NAME => Some(StringOp::IsEmpty),
        STARTS_WITH_NAME => Some(StringOp::StartsWith),
        CONTAINS_NAME => Some(StringOp::Contains),
        LEN_CHARS_NAME => Some(StringOp::LenChars),
        FROM_CHAR_NAME => Some(StringOp::FromChar),
        FROM_I64_NAME => Some(StringOp::FromI64),
        FROM_USIZE_NAME => Some(StringOp::FromUsize),
        SLICE_NAME => Some(StringOp::Slice),
        FIND_NAME => Some(StringOp::Find),
        TO_I64_NAME => Some(StringOp::ToI64),
        TRIM_NAME => Some(StringOp::Trim),
        BYTE_AT_NAME => Some(StringOp::ByteAt),
        FILE_READ_TEXT_NAME => Some(StringOp::FileReadText),
        _ => None,
    }
}

/// Resolve a resolved-callee identity to its intrinsic operation.
pub(crate) fn by_id(id: &str) -> Option<StringOp> {
    match id {
        LEN_ID => Some(StringOp::Len),
        CONCAT_ID => Some(StringOp::Concat),
        IS_EMPTY_ID => Some(StringOp::IsEmpty),
        STARTS_WITH_ID => Some(StringOp::StartsWith),
        CONTAINS_ID => Some(StringOp::Contains),
        LEN_CHARS_ID => Some(StringOp::LenChars),
        FROM_CHAR_ID => Some(StringOp::FromChar),
        FROM_I64_ID => Some(StringOp::FromI64),
        FROM_USIZE_ID => Some(StringOp::FromUsize),
        SLICE_ID => Some(StringOp::Slice),
        FIND_ID => Some(StringOp::Find),
        TO_I64_ID => Some(StringOp::ToI64),
        TRIM_ID => Some(StringOp::Trim),
        BYTE_AT_ID => Some(StringOp::ByteAt),
        FILE_READ_TEXT_ID => Some(StringOp::FileReadText),
        _ => None,
    }
}

/// Synthetic HIR parameters for one operation: consuming arguments carry
/// `Own` ownership exactly like an ordinary declared `string` parameter,
/// borrowed arguments accept every argument ownership without a transfer,
/// and copied scalar arguments use the ordinary `Value` mode of their kind.
pub(crate) fn resolved_params(op: StringOp) -> Vec<ResolvedParam> {
    let consumption = if op.consumes_arguments() {
        OwnershipMode::Own
    } else {
        OwnershipMode::Borrow
    };
    op.param_names()
        .iter()
        .zip(op.param_types())
        .enumerate()
        .map(|(index, (name, ty))| ResolvedParam {
            id: ValueId::intrinsic_parameter(op.id(), index),
            name: (*name).to_owned(),
            ownership: if matches!(
                ty,
                ResolvedType::Char | ResolvedType::I64 | ResolvedType::Usize
            ) {
                OwnershipMode::Value
            } else {
                consumption
            },
            ty: ty.clone(),
            span: Span::default(),
        })
        .collect()
}

/// Synthetic AST parameters for source verification. Consuming arguments use
/// the established `own` transfer mode; borrowed arguments and copied scalars
/// use the plain value mode that never marks its sources moved.
pub(crate) fn ast_params(op: StringOp) -> Vec<Param> {
    op.param_names()
        .iter()
        .zip(op.param_types())
        .map(|(name, ty)| Param {
            name: (*name).to_owned(),
            mode: if *ty == ResolvedType::Str {
                ParamMode::Borrow
            } else if matches!(
                ty,
                ResolvedType::Char | ResolvedType::I64 | ResolvedType::Usize
            ) || !op.consumes_arguments()
            {
                ParamMode::Value
            } else {
                ParamMode::Own
            },
            ty: match ty {
                ResolvedType::Char => Type::Char,
                ResolvedType::I64 => Type::I64,
                ResolvedType::Usize => Type::Usize,
                ResolvedType::Str => Type::Str,
                _ => Type::String,
            },
            span: Span::default(),
        })
        .collect()
}

/// Owned String Loops v1 same-owner append: `text = string_concat(text, …)`,
/// where the assignment target and the first operand are the same whole
/// `let mut` binding. The call consumes the current generation as its first
/// staged argument and the assignment publishes the next one, so exactly one
/// generation of the owner is live and no release happens at the assignment.
pub(crate) fn is_same_owner_concat_source(value: &crate::ast::Expr, name: &str, ty: &Type) -> bool {
    *ty == Type::String && is_same_owner_concat_shape(value, name)
}

/// The syntactic half of [`is_same_owner_concat_source`], without the binding
/// type.
pub(crate) fn is_same_owner_concat_shape(value: &crate::ast::Expr, name: &str) -> bool {
    let crate::ast::ExprKind::Call {
        name: callee,
        type_arguments,
        args,
    } = &value.kind
    else {
        return false;
    };
    by_name(callee) == Some(StringOp::Concat)
        && type_arguments.is_empty()
        && args.len() == 2
        && matches!(&args[0].kind, crate::ast::ExprKind::Var(source) if source == name)
        && !mentions_owner(&args[1], name)
}

/// The first operand consumes the owner, so a second operand that also names
/// it (`string_concat(text, text)`) would read a moved value: that is not the
/// admitted reopen and must take the ordinary ownership diagnostic. A `Var`
/// prints as `Var("name")` while string-literal quotes print escaped, so the
/// structured debug form cannot confuse a literal with a reference.
fn mentions_owner(expression: &crate::ast::Expr, name: &str) -> bool {
    format!("{:?}", expression.kind).contains(&format!("Var({name:?})"))
}

/// Resolved-HIR twin of [`is_same_owner_concat_source`]. Hostile HIR that
/// never passed through source text must re-derive the identical fact.
pub(crate) fn is_same_owner_concat_hir(value: &crate::hir::ResolvedExpr, owner: &ValueId) -> bool {
    matches!(
        &value.kind,
        crate::hir::ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
            if by_id(callee.as_str()) == Some(StringOp::Concat)
                && type_arguments.is_empty()
                && args.len() == 2
                && value.ty == ResolvedType::String
                && matches!(
                    &args[0].kind,
                    crate::hir::ResolvedExprKind::Place(place)
                        if &place.root == owner && place.projections.is_empty()
                )
                // As in the source twin: a second operand that reads the
                // consumed owner is not the admitted reopen.
                && !format!("{:?}", args[1]).contains(&format!("{owner:?}"))
    )
}

/// Whether a source call touches an owned `string`: every compiler-owned
/// String operation (producers allocate; readers read a cloned operand), or a
/// declared function with a `string` parameter or result.
pub(crate) fn source_call_uses_string(name: &str, uses_string: &dyn Fn(&str) -> bool) -> bool {
    by_name(name).is_some() || uses_string(name)
}

/// Owned String Loops v1 keeps every `while` condition free of owned String
/// values: a condition re-evaluates outside the per-iteration body region, so
/// an owned temporary there (a literal, a produced String, or the clone an
/// owning String read allocates) would have no per-iteration release point.
/// Return the span of the first string literal or String-touching call in
/// source order. Forms the while admission scan rejects anyway are not entered.
pub(crate) fn owned_string_in_condition(
    condition: &crate::ast::Expr,
    uses_string: &dyn Fn(&str) -> bool,
) -> Option<Span> {
    use crate::ast::{ExprKind, Statement};
    let mut pending = vec![condition];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ExprKind::String(_) => return Some(expression.span),
            ExprKind::Call { name, args, .. } => {
                if source_call_uses_string(name, uses_string) {
                    return Some(expression.span);
                }
                pending.extend(args.iter().rev());
            }
            ExprKind::Unary { value, .. } => pending.push(value),
            ExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(else_branch);
                pending.push(then_branch);
                pending.push(condition);
            }
            ExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements.iter().rev() {
                    if matches!(statement, Statement::Let { .. } | Statement::Assign { .. }) {
                        pending.extend(
                            (0..statement.child_count())
                                .rev()
                                .filter_map(|index| statement.child(index)),
                        );
                    }
                }
            }
            ExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.extend(arms.iter().rev().map(|arm| &arm.value));
                pending.push(scrutinee);
            }
            ExprKind::Yield { request } => pending.push(request),
            _ => {}
        }
    }
    None
}

/// The stable refusal for [`owned_string_in_condition`].
pub(crate) const OWNED_STRING_CONDITION_MESSAGE: &str = "string values are not admitted in while conditions; compute a scalar such as `string_len(text)` in the loop body and test that";

/// Owned String Loops v1: every same-owner append
/// `text = string_concat(text, …)` in one function, keyed by its moving
/// first-operand place and mapped to the appended call value. Every other
/// owning String place read allocates a clone; these reads instead move the
/// binding's current generation into the call, so the assignment can publish
/// the next generation into the then-dead binding slot. The cleanup builder,
/// its independent replay, and every lowering derive this exact map.
pub(crate) fn same_owner_concat_appends(
    function: &crate::hir::ResolvedFunction,
) -> std::collections::BTreeMap<crate::hir::ExpressionId, crate::hir::ExpressionId> {
    use crate::hir::{ResolvedExprKind, ResolvedStatement};
    let mut appends = std::collections::BTreeMap::new();
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    binding,
                    field: None,
                    value,
                    ..
                } = statement
                {
                    if let (true, ResolvedExprKind::Call { args, .. }) =
                        (is_same_owner_concat_hir(value, &binding.id), &value.kind)
                    {
                        appends.insert(args[0].id.clone(), value.id.clone());
                    }
                }
            }
        }
        pending.extend(crate::interpreter::trace_child_expressions(expression));
    }
    appends
}

/// The moving first-operand places of [`same_owner_concat_appends`].
pub(crate) fn same_owner_concat_operands(
    function: &crate::hir::ResolvedFunction,
) -> std::collections::BTreeSet<crate::hir::ExpressionId> {
    same_owner_concat_appends(function).into_keys().collect()
}

/// Cleanup order after a same-owner append publishes the next generation:
/// the binding keeps the position its previous generation held when it was
/// staged (`history`), so loop iterations and branch joins see one stable
/// initialization history. Flags initialized meanwhile keep their order after
/// the reserved history.
pub(crate) fn append_publication_order(
    history: &[crate::cleanup::LivenessFlagId],
    live: &[crate::cleanup::LivenessFlagId],
) -> Vec<crate::cleanup::LivenessFlagId> {
    history
        .iter()
        .filter(|flag| live.contains(flag))
        .chain(live.iter().filter(|flag| !history.contains(flag)))
        .copied()
        .collect()
}

/// Text Toolkit v1 is not lowered by any Core Wasm lane. Every lane refuses
/// it with this one stable diagnostic before emitting any operand.
pub(crate) fn text_toolkit_wasm_refusal(op: StringOp) -> crate::diagnostic::Diagnostic {
    crate::diagnostic::Diagnostic::io(
        "SPX-W116",
        format!(
            "Text Toolkit v1 operation `{}` is not lowered to Core Wasm; run it on the reference interpreter or native C11",
            op.name()
        ),
    )
}

/// The shared byte semantics of `string_find`: the first offset at or after
/// `from` where `needle` occurs. An empty needle matches at `from`.
pub(crate) fn find_bytes(text: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > text.len() {
        return None;
    }
    if needle.is_empty() {
        return Some(from);
    }
    text[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| from + offset)
}

/// `string_to_i64`: an optional leading `-`, then one or more ASCII digits
/// and nothing else, within the i64 range. Everything else is `None`.
pub(crate) fn parse_i64(text: &[u8]) -> Option<i64> {
    let (negative, digits) = match text.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, text),
    };
    if digits.is_empty() {
        return None;
    }
    let mut value: i64 = 0;
    for byte in digits {
        if !byte.is_ascii_digit() {
            return None;
        }
        let digit = i64::from(byte - b'0');
        value = value.checked_mul(10)?;
        value = if negative {
            value.checked_sub(digit)?
        } else {
            value.checked_add(digit)?
        };
    }
    Some(value)
}

/// ASCII whitespace for `string_trim`: space, tab, line feed, vertical tab,
/// form feed, and carriage return.
pub(crate) fn is_text_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// The byte range `string_trim` keeps. Whitespace bytes are ASCII, so both
/// ends always lie on character boundaries.
pub(crate) fn trim_range(text: &[u8]) -> std::ops::Range<usize> {
    let start = text
        .iter()
        .position(|byte| !is_text_whitespace(*byte))
        .unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|byte| !is_text_whitespace(*byte))
        .map_or(start, |index| index + 1);
    start..end
}

/// Whether any resolved body or contract calls `op`.
pub(crate) fn program_uses_op(program: &crate::hir::ResolvedProgram, op: StringOp) -> bool {
    let mut pending = Vec::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if let crate::hir::ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if by_id(callee.as_str()) == Some(op) {
                return true;
            }
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}
