//! HIR type-level data model.
//!
//! Resolved types, type facts, ownership modes, and the declaration
//! nodes (types, interfaces, imports, functions) the resolver emits.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::ast::{MatchMode, ParamMode, Span};
use crate::cleanup::CleanupInventory;
use crate::cleanup_plan::CleanupPlan;
use crate::loan_plan::LoanPlan;

use super::expr_nodes::ResolvedExpr;
use super::ids::{DeclarationId, ExpressionId, FunctionInstanceId, ValueId};
use super::{DeclarationIndex, PlaceProjection};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationKind {
    Resource,
    ResourceDrop,
    Record,
    Field,
    Class,
    Variant,
    VariantCase,
    CaseField,
    Interface,
    Import,
    Function,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityOrigin {
    Explicit,
    Automatic,
    CompilerOwned,
}

impl IdentityOrigin {
    pub fn text(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Automatic => "automatic",
            Self::CompilerOwned => "compiler_owned",
        }
    }

    pub fn is_persistent(self) -> bool {
        matches!(self, Self::Explicit | Self::CompilerOwned)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub id: DeclarationId,
    pub name: String,
    pub kind: DeclarationKind,
    pub identity_origin: IdentityOrigin,
    pub owner: Option<DeclarationId>,
}

/// Authenticated origin class for one non-escaping byte-slice root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ByteSliceRootKind {
    /// A symbolic function-parameter root. Callers substitute the argument's
    /// existing root; only a concrete host entry turns it into external input.
    FunctionParameter,
    OwnedBytes,
    FixedArray,
    BorrowedStr,
    /// The one immutable argument arena owned by the enclosing command
    /// invocation. Every `arg_utf8` view authenticates this same root.
    CommandArguments,
}

/// A symbolic extent deliberately independent of the compiler host's pointer
/// width. External lengths are checked as semantic u64 values at invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ByteSliceExtent {
    Constant(u64),
    ParameterLength,
    ValueLength,
}

/// One authenticated dynamic half-open subrange step. Steps are stored from
/// the original root toward the current view, so nested named ranges form a
/// bounded acyclic derivation chain without inventing a new root identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteSliceRangeStep {
    pub source: ValueId,
    pub producer: ExpressionId,
    pub start: ExpressionId,
    pub end: ExpressionId,
}

/// Exact provenance for a byte view. Legacy views retain a complete symbolic
/// root (`offset = 0`, `length = root length`). The additive projected-field
/// profile retains one stable field-ID projection and its authenticated type;
/// aliases and ranges preserve those facts rather than minting a new root.
/// Host boundaries alone bind external parameter symbols to input storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteSliceProvenance {
    pub root: ValueId,
    /// Exact stable-ID path from `root` to the borrowed storage. Empty retains
    /// the byte-for-byte legacy root provenance carried through Graph v23.
    pub projections: Vec<PlaceProjection>,
    /// Independently resolved type at the end of `projections`.
    pub projected_type: ResolvedType,
    pub root_kind: ByteSliceRootKind,
    pub root_length: ByteSliceExtent,
    pub offset: ByteSliceExtent,
    pub length: ByteSliceExtent,
    /// The authenticated compiler-owned view expression, absent only for a
    /// symbolic external parameter root.
    pub producer: Option<ExpressionId>,
    /// Dynamic range steps relative to each immediately preceding view.
    /// Empty preserves the exact whole-root v17-v19 meaning.
    pub ranges: Vec<ByteSliceRangeStep>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResolvedType {
    Unit,
    I64,
    /// A checked signed 32-bit integer.
    I32,
    /// One Unicode scalar value.
    Char,
    /// One unsigned 8-bit integer value.
    U8,
    /// A target-independent checked unsigned 64-bit semantic integer.
    Usize,
    /// Inline Copy byte storage with exact target-independent length.
    ArrayU8(u32),
    /// IEEE-754 single precision.
    F32,
    /// IEEE-754 double precision.
    F64,
    Bool,
    /// An owned heap UTF-8 string value; never `Copy`.
    String,
    /// Uniquely owned immutable bytes. This needs drop but is not a resource.
    Bytes,
    /// A non-owning UTF-8 view rooted in the current invocation.
    Str,
    /// A non-owning byte view rooted in the current invocation.
    SliceU8,
    TypeParameter {
        owner: DeclarationId,
        index: u32,
    },
    Nominal {
        declaration: DeclarationId,
        arguments: Vec<ResolvedType>,
    },
}

impl ResolvedType {
    /// Canonical ownership classification shared by the resolver, cleanup
    /// builder, hostile validator, and backends. Unique ownership is not the
    /// same fact as containing an opaque resource.
    pub fn is_uniquely_owned(&self) -> bool {
        matches!(self, Self::String | Self::Bytes)
    }
    pub fn is_compiler_byte_option(&self) -> bool {
        matches!(
            self,
            Self::Nominal {
                declaration,
                arguments,
            } if declaration.as_str() == crate::prelude::OPTION_ID
                && arguments.as_slice() == [ResolvedType::U8]
        )
    }

    pub fn nominal_id(&self) -> Option<&DeclarationId> {
        match self {
            Self::Nominal { declaration, .. } => Some(declaration),
            Self::Unit
            | Self::I64
            | Self::I32
            | Self::Char
            | Self::U8
            | Self::Usize
            | Self::ArrayU8(_)
            | Self::F32
            | Self::F64
            | Self::Bool
            | Self::String
            | Self::Bytes
            | Self::Str
            | Self::SliceU8
            | Self::TypeParameter { .. } => None,
        }
    }

    /// A name-independent key suitable as an input to future layout hashing.
    pub fn identity_key(&self) -> String {
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Finish(&'a DeclarationId, usize),
        }
        let mut frames = vec![Frame::Enter(self)];
        let mut keys = Vec::<String>::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(ty) => match ty {
                    Self::Unit => keys.push("unit".to_owned()),
                    Self::I64 => keys.push("i64".to_owned()),
                    Self::I32 => keys.push("i32".to_owned()),
                    Self::Char => keys.push("char".to_owned()),
                    Self::U8 => keys.push("u8".to_owned()),
                    Self::Usize => keys.push("usize".to_owned()),
                    Self::ArrayU8(length) => keys.push(format!("array:u8:{length}")),
                    Self::F32 => keys.push("f32".to_owned()),
                    Self::F64 => keys.push("f64".to_owned()),
                    Self::Bool => keys.push("bool".to_owned()),
                    Self::String => keys.push("string".to_owned()),
                    Self::Bytes => keys.push("bytes".to_owned()),
                    Self::Str => keys.push("str".to_owned()),
                    Self::SliceU8 => keys.push("slice-u8".to_owned()),
                    Self::TypeParameter { owner, index } => keys.push(format!(
                        "parameter:{}:{}:{index}",
                        owner.as_str().len(),
                        owner
                    )),
                    Self::Nominal {
                        declaration,
                        arguments,
                    } => {
                        frames.push(Frame::Finish(declaration, arguments.len()));
                        frames.extend(arguments.iter().rev().map(Frame::Enter));
                    }
                },
                Frame::Finish(declaration, count) => {
                    let split = keys
                        .len()
                        .checked_sub(count)
                        .expect("type-key traversal has one result per argument");
                    let mut encoded = crate::bounded_output::CappedString::new();
                    for key in keys.drain(split..) {
                        write!(encoded, "{}:{key}", key.len())
                            .expect("writing to a string cannot fail");
                    }
                    keys.push(format!(
                        "nominal:{}:{}:{}:{}",
                        declaration.as_str().len(),
                        declaration,
                        count,
                        encoded.into_string()
                    ));
                }
            }
        }
        keys.pop().expect("a type always produces an identity key")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeFacts {
    pub copy: bool,
    pub contains_resource: bool,
    pub sized: bool,
    pub needs_drop: bool,
    pub layout_key: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipMode {
    Value,
    Own,
    Borrow,
    Shared,
}

/// The authenticated ownership spelling attached to a resolved match.
///
/// This tranche preserves source meaning only. Ownership-changing behavior is
/// admitted separately after source and HIR verification can prove it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedMatchMode {
    Value,
    Own,
    Borrow,
}

impl From<MatchMode> for ResolvedMatchMode {
    fn from(mode: MatchMode) -> Self {
        match mode {
            MatchMode::Value => Self::Value,
            MatchMode::Own => Self::Own,
            MatchMode::Borrow => Self::Borrow,
        }
    }
}

pub(crate) fn admitted_owned_byte_prelude_instance(
    declaration: &DeclarationId,
    arguments: &[ResolvedType],
) -> bool {
    matches!(
        (declaration.as_str(), arguments),
        (crate::prelude::OPTION_ID, [ResolvedType::Bytes])
            | (
                crate::prelude::RESULT_ID,
                [ResolvedType::Bytes, ResolvedType::I64 | ResolvedType::Bool]
            )
            | (
                crate::prelude::RESULT_ID,
                [ResolvedType::I64 | ResolvedType::Bool, ResolvedType::Bytes]
            )
    )
}

pub(super) fn resolver_admits_flat_owned_byte_variant(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if admitted_owned_byte_prelude_instance(declaration, arguments) {
        return true;
    }
    if !arguments.is_empty() {
        return false;
    }
    declarations
        .variant_cases(declaration)
        .is_some_and(|cases| {
            cases
                .iter()
                .flat_map(|case| &case.fields)
                .any(|field| field.ty == ResolvedType::Bytes)
                && cases.iter().flat_map(|case| &case.fields).all(|field| {
                    field.ty == ResolvedType::Bytes || is_scalar_resolved_type(&field.ty)
                })
        })
}

impl From<ParamMode> for OwnershipMode {
    fn from(mode: ParamMode) -> Self {
        match mode {
            ParamMode::Value => Self::Value,
            ParamMode::Own => Self::Own,
            ParamMode::Borrow => Self::Borrow,
            ParamMode::Shared => Self::Shared,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProgram {
    pub module: String,
    pub permits: Vec<String>,
    pub entrypoint: DeclarationId,
    pub declarations: DeclarationIndex,
    pub types: Vec<ResolvedTypeDeclaration>,
    pub interfaces: Vec<ResolvedInterface>,
    pub function_templates: Vec<ResolvedFunctionTemplate>,
    pub functions: Vec<ResolvedFunction>,
    pub function_instances: Vec<ResolvedFunctionInstance>,
}

/// One real, owned monomorphic function admitted to the private workspace
/// scalar linker.
pub(crate) struct LinkedScalarFunction {
    pub(crate) function: ResolvedFunction,
    pub(crate) origin: IdentityOrigin,
}

/// Phase-A-authenticated declaration identity used only while projecting one
/// exact linked Project closure into an independently validated HIR program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LinkedDeclarationFact {
    pub(crate) kind: DeclarationKind,
    pub(crate) origin: IdentityOrigin,
    pub(crate) owner: Option<DeclarationId>,
}

/// Exact non-function semantic inventory needed by a Project-v8 closure.
pub(crate) struct LinkedOwnedDataParts {
    pub(crate) permits: Vec<String>,
    pub(crate) types: Vec<ResolvedTypeDeclaration>,
    pub(crate) interfaces: Vec<ResolvedInterface>,
    pub(crate) declaration_facts: BTreeMap<DeclarationId, LinkedDeclarationFact>,
    pub(crate) function_templates: Vec<ResolvedFunctionTemplate>,
    pub(crate) function_instances: Vec<ResolvedFunctionInstance>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedNativeRustImportCall {
    pub expression: ExpressionId,
    pub import: DeclarationId,
    pub args: Vec<ResolvedExpr>,
    pub result: ResolvedImportResultKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedHostCommandOperation {
    ArgsLen,
    ArgUtf8,
    StdinRead,
    StderrWrite,
    StdoutAppend,
    StderrAppend,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedHostCommandCall {
    pub expression: ExpressionId,
    pub operation: ResolvedHostCommandOperation,
    pub args: Vec<ResolvedExpr>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeDeclaration {
    pub id: DeclarationId,
    pub name: String,
    pub type_parameters: Vec<ResolvedTypeParameterDeclaration>,
    pub kind: ResolvedTypeDeclarationKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeParameterDeclaration {
    pub name: String,
    pub index: u32,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedTypeDeclarationKind {
    Resource {
        drop: ResolvedResourceDrop,
    },
    Record {
        fields: Vec<ResolvedFieldDeclaration>,
    },
    Class {
        fields: Vec<ResolvedFieldDeclaration>,
        methods: Vec<DeclarationId>,
    },
    Variant {
        cases: Vec<ResolvedVariantCaseDeclaration>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedVariantCaseDeclaration {
    pub id: DeclarationId,
    pub name: String,
    pub index: u32,
    pub fields: Vec<ResolvedFieldDeclaration>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedResourceDrop {
    pub id: DeclarationId,
    pub kind: ResolvedResourceDropKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedResourceDropKind {
    Trivial,
    Imported {
        import: DeclarationId,
        import_key: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterface {
    pub id: DeclarationId,
    pub name: String,
    pub permits: Vec<String>,
    pub imports: Vec<ResolvedImport>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImport {
    pub id: DeclarationId,
    pub name: String,
    pub interface: DeclarationId,
    pub import_key: String,
    pub native_rust: bool,
    pub parameters: Vec<ResolvedImportParameter>,
    pub result: ResolvedImportResult,
    pub effects: Vec<String>,
    pub required_authority: Vec<String>,
    pub failure: ResolvedImportFailure,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImportParameter {
    pub name: String,
    pub ty: ResolvedType,
    pub ownership: OwnershipMode,
    pub consumes_on_failure: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImportResult {
    pub kind: ResolvedImportResultKind,
    pub ownership: OwnershipMode,
    pub producer: &'static str,
    pub out_slot_initialization: &'static str,
    pub ownership_transfer: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedImportResultKind {
    Unit,
    I64,
    Bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedImportFailure {
    Infallible,
    Status {
        domain_id: String,
        normalization: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldDeclaration {
    pub id: DeclarationId,
    pub name: String,
    pub index: u32,
    pub ty: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunction {
    pub id: DeclarationId,
    pub name: String,
    pub params: Vec<ResolvedParam>,
    pub result_id: ValueId,
    pub return_type: ResolvedType,
    pub effects: Vec<String>,
    pub requires: Vec<ResolvedExpr>,
    pub ensures: Vec<ResolvedExpr>,
    pub body: ResolvedExpr,
    pub cleanup: CleanupInventory,
    pub cleanup_plan: CleanupPlan,
    pub loan_plan: LoanPlan,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionInstance {
    pub id: FunctionInstanceId,
    pub template: DeclarationId,
    pub type_arguments: Vec<ResolvedType>,
    pub function: ResolvedFunction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionTemplate {
    pub id: DeclarationId,
    pub name: String,
    pub type_parameters: Vec<ResolvedTypeParameterDeclaration>,
    pub params: Vec<ResolvedParam>,
    pub result_id: ValueId,
    pub return_type: ResolvedType,
    pub effects: Vec<String>,
    pub requires: Vec<ResolvedExpr>,
    pub ensures: Vec<ResolvedExpr>,
    pub body: ResolvedExpr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedParam {
    pub id: ValueId,
    pub name: String,
    pub ownership: OwnershipMode,
    pub ty: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBinding {
    pub id: ValueId,
    pub name: String,
    pub ownership: OwnershipMode,
    pub ty: ResolvedType,
    pub span: Span,
}

/// Explicit Mutation v1 admits exactly the checked Copy scalar value types.
pub(crate) fn is_scalar_resolved_type(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::Char
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
    )
}
