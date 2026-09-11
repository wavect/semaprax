//! Public Generic Type Grammar v1: a versioned, target-neutral spelling for
//! the types a public generic ownership surface could ever name, plus the
//! explicit template and ordered argument identities derived from it.
//!
//! This is gate PG-1 and PG-2 of the
//! [Public Generic Ownership milestone](../docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).
//! It is a read-only projection of already-checked HIR. It admits no syntax,
//! defines no descriptor, carrier, or calling convention, generates nothing,
//! executes nothing, and grants no authority. Public generic ownership remains
//! unsupported and unpublished; a term produced here is not a public ABI.
//!
//! Three properties are the whole point of the grammar being its own artifact:
//!
//! - **Target-neutral.** A term carries no C type, Wasm type, size, alignment,
//!   layout, or host width. It names semantic types only, so two backends
//!   cannot disagree about what a term meant.
//! - **Injective.** Every declaration identity is length-prefixed in bytes, so
//!   an identity containing `<`, `>`, `,`, `:`, or `@` can never be confused
//!   with grammar punctuation, and two distinct types can never render alike.
//! - **Identity-bearing, not name-bearing.** Template and instance digests are
//!   computed over persistent declaration identities, declared arity, and
//!   ordered parameter positions. A display rename leaves every digest
//!   unchanged; permuting, omitting, duplicating, or substituting an argument
//!   changes the instance digest.
//!
//! The admitted vocabulary is deliberately narrower than the language: the
//! eight Copy scalars, direct `Bytes`, and fully concrete authored `record`
//! instances. Everything else — type parameters, `string`, `str`,
//! `Slice<u8>`, inline byte arrays, `unit`, function types, compiler-owned
//! nominals, classes, variants, and resources — rejects with one closed
//! reason. Widening the vocabulary is a new grammar version, never a silent
//! admission.

use std::collections::BTreeMap;

use sha2::{Digest as _, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{
    ResolvedFieldDeclaration, ResolvedProgram, ResolvedType, ResolvedTypeDeclaration,
    ResolvedTypeDeclarationKind,
};

/// The versioned grammar schema. A different vocabulary is a different schema.
pub const PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA: &str = "semaprax.public-generic-type-grammar.v1";

const TERM_DOMAIN: &[u8] = b"semaprax.public-generic-type-grammar.v1.term\0";
const TEMPLATE_DOMAIN: &[u8] = b"semaprax.public-generic-type-grammar.v1.template\0";
const INSTANCE_DOMAIN: &[u8] = b"semaprax.public-generic-type-grammar.v1.instance\0";

/// A canonical term may not exceed this many bytes, before or after parsing.
pub const MAX_TERM_BYTES: usize = 64 * 1024;
/// Nominal record nesting bound, matching the nested owned-record work limits.
pub const MAX_RECORD_DEPTH: usize = 64;
/// Transitive owned (`Bytes`) leaf bound per instance.
pub const MAX_OWNED_LEAVES: usize = 256;
/// Visited type node bound per projection.
pub const MAX_VISITED_NODES: usize = 4096;
/// Declared type parameters admitted on one template.
pub const MAX_TEMPLATE_ARITY: usize = 16;

/// A type outside the admitted grammar vocabulary.
pub const REJECTED_TYPE: &str = "SPX-PG101";
/// A grammar bound was reached. Terms are never truncated or repaired.
pub const GRAMMAR_CAPACITY: &str = "SPX-PG102";
/// Submitted bytes are not a canonical term of this grammar.
pub const MALFORMED_TERM: &str = "SPX-PG103";
/// Submitted bytes parse but do not equal the independently recomputed term.
pub const TERM_REPLAY_MISMATCH: &str = "SPX-PG104";

/// Why a checked type is outside the grammar. Closed: a new reason is a new
/// grammar version, and no reason means "partly admitted".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejection {
    /// An unsubstituted type parameter. A public surface names no parameters.
    TypeParameter,
    /// `string`: an owned heap string, reserved for the String profiles.
    OwnedString,
    /// `str`: a borrowed view rooted in the current invocation.
    BorrowedStr,
    /// `Slice<u8>`: a borrowed byte view, not an owned data type.
    BorrowedByteView,
    /// `unit`: no value to own or transfer.
    Unit,
    /// Inline `[u8; N]` Copy storage.
    InlineByteArray,
    /// A function type.
    FunctionType,
    /// A compiler-owned nominal such as `Option`, `Result`, `Vec`, or `Box`.
    CompilerOwnedNominal,
    /// An authored class, variant, or resource declaration.
    UnadmittedNominalKind,
    /// The nominal's declaration is absent from the checked program.
    MissingDeclaration,
    /// Two declarations share one identity in the checked program.
    AmbiguousDeclaration,
    /// The argument count does not equal the declared arity.
    ArityMismatch,
}

impl Rejection {
    /// The closed wire spelling. Stable across releases of this grammar.
    pub const fn reason(self) -> &'static str {
        match self {
            Self::TypeParameter => "type_parameter",
            Self::OwnedString => "owned_string",
            Self::BorrowedStr => "borrowed_str",
            Self::BorrowedByteView => "borrowed_byte_view",
            Self::Unit => "unit",
            Self::InlineByteArray => "inline_byte_array",
            Self::FunctionType => "function_type",
            Self::CompilerOwnedNominal => "compiler_owned_nominal",
            Self::UnadmittedNominalKind => "unadmitted_nominal_kind",
            Self::MissingDeclaration => "missing_declaration",
            Self::AmbiguousDeclaration => "ambiguous_declaration",
            Self::ArityMismatch => "arity_mismatch",
        }
    }

    fn diagnostic(self) -> Diagnostic {
        Diagnostic::io(
            REJECTED_TYPE,
            format!(
                "{PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA} does not admit this type: {}",
                self.reason()
            ),
        )
    }
}

/// The eight Copy scalars the grammar admits, with their canonical spellings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrammarScalar {
    I64,
    I32,
    U8,
    Usize,
    Char,
    F32,
    F64,
    Bool,
}

impl GrammarScalar {
    /// The canonical term token.
    pub const fn text(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::I32 => "i32",
            Self::U8 => "u8",
            Self::Usize => "usize",
            Self::Char => "char",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Bool => "bool",
        }
    }

    const fn from_text(text: &str) -> Option<Self> {
        Some(match text.as_bytes() {
            b"i64" => Self::I64,
            b"i32" => Self::I32,
            b"u8" => Self::U8,
            b"usize" => Self::Usize,
            b"char" => Self::Char,
            b"f32" => Self::F32,
            b"f64" => Self::F64,
            b"bool" => Self::Bool,
            _ => return None,
        })
    }
}

/// A parsed canonical term. Parsing is total over the grammar and refuses
/// everything else; it never repairs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GrammarTerm {
    Scalar(GrammarScalar),
    /// Direct uniquely owned immutable bytes.
    Bytes,
    /// A fully concrete authored record instance with ordered arguments.
    Instance {
        declaration: String,
        arguments: Vec<GrammarTerm>,
    },
}

impl GrammarTerm {
    /// Render back to canonical bytes. `parse_term(render(term)) == term`.
    pub fn render(&self) -> String {
        let mut output = String::new();
        self.write(&mut output);
        output
    }

    fn write(&self, output: &mut String) {
        match self {
            Self::Scalar(scalar) => output.push_str(scalar.text()),
            Self::Bytes => output.push_str("bytes"),
            Self::Instance {
                declaration,
                arguments,
            } => {
                write_identity(output, declaration);
                output.push('<');
                for (index, argument) in arguments.iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    argument.write(output);
                }
                output.push('>');
            }
        }
    }
}

/// One declared type parameter position of a template. The position, not the
/// name, is the identity: `name` is presentation only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateParameter {
    pub owner: String,
    pub index: u32,
    pub name: String,
}

/// A template's explicit identity: its persistent declaration identity, its
/// declared arity, and its ordered parameter positions. Display names are
/// carried but never hashed, so a rename cannot change a template identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateIdentity {
    pub declaration: String,
    pub name: String,
    pub arity: usize,
    pub parameters: Vec<TemplateParameter>,
    pub digest: String,
}

/// One ordered concrete argument of an instance, bound to the exact parameter
/// position it substitutes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentFact {
    pub index: u32,
    pub parameter_owner: String,
    pub parameter_index: u32,
    pub term: String,
    pub digest: String,
}

/// One field of the instance after exact owner-and-index substitution, in
/// declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldFact {
    pub index: u32,
    pub id: String,
    pub name: String,
    pub term: String,
    pub digest: String,
}

/// The complete target-neutral description of one concrete generic instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceFacts {
    pub term: String,
    pub term_digest: String,
    pub template: TemplateIdentity,
    pub arguments: Vec<ArgumentFact>,
    pub fields: Vec<FieldFact>,
    /// Canonical identity paths of every transitive `Bytes` leaf, in
    /// structural order. An instance with no owned leaf is a valid term but
    /// not an ownership surface; that requirement belongs to the surface
    /// admission, not to the grammar.
    pub owned_leaves: Vec<String>,
    pub instance_digest: String,
}

#[derive(Default)]
struct Budget {
    nodes: usize,
    leaves: usize,
}

impl Budget {
    fn visit(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_VISITED_NODES || depth > MAX_RECORD_DEPTH {
            return Err(capacity("type node, nesting or field work limit"));
        }
        Ok(())
    }

    fn leaf(&mut self) -> Result<(), Diagnostic> {
        self.leaves = self.leaves.saturating_add(1);
        if self.leaves > MAX_OWNED_LEAVES {
            return Err(capacity("transitive owned leaf limit"));
        }
        Ok(())
    }
}

fn capacity(subject: &str) -> Diagnostic {
    Diagnostic::io(
        GRAMMAR_CAPACITY,
        format!("{PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA} exceeded its {subject}"),
    )
}

fn malformed(subject: &str) -> Diagnostic {
    Diagnostic::io(
        MALFORMED_TERM,
        format!("not a canonical {PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA} term: {subject}"),
    )
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

/// Length-prefixed identity framing. The prefix counts bytes, so punctuation
/// inside an identity is never grammar punctuation.
fn write_identity(output: &mut String, identity: &str) {
    output.push('@');
    output.push_str(&identity.len().to_string());
    output.push(':');
    output.push_str(identity);
}

fn frame(preimage: &mut Vec<u8>, bytes: &[u8]) {
    preimage.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    preimage.extend_from_slice(bytes);
}

/// The domain-separated digest of canonical term bytes.
pub fn term_digest(term: &str) -> String {
    digest(TERM_DOMAIN, term.as_bytes())
}

fn declarations(program: &ResolvedProgram) -> BTreeMap<&str, &ResolvedTypeDeclaration> {
    let mut index = BTreeMap::new();
    for declaration in &program.types {
        index.entry(declaration.id.as_str()).or_insert(declaration);
    }
    index
}

fn duplicated(program: &ResolvedProgram, declaration: &str) -> bool {
    program
        .types
        .iter()
        .filter(|candidate| candidate.id.as_str() == declaration)
        .count()
        > 1
}

fn record_fields(
    declaration: &ResolvedTypeDeclaration,
) -> Result<&[ResolvedFieldDeclaration], Diagnostic> {
    match &declaration.kind {
        ResolvedTypeDeclarationKind::Record { fields } => Ok(fields),
        _ => Err(Rejection::UnadmittedNominalKind.diagnostic()),
    }
}

/// Classify one checked type into the grammar, or return its closed rejection.
pub fn classify(program: &ResolvedProgram, ty: &ResolvedType) -> Result<GrammarTerm, Diagnostic> {
    let index = declarations(program);
    let mut budget = Budget::default();
    classify_with(program, &index, ty, &mut budget, 0)
}

fn classify_with(
    program: &ResolvedProgram,
    index: &BTreeMap<&str, &ResolvedTypeDeclaration>,
    ty: &ResolvedType,
    budget: &mut Budget,
    depth: usize,
) -> Result<GrammarTerm, Diagnostic> {
    budget.visit(depth)?;
    let rejection = |rejection: Rejection| Err(rejection.diagnostic());
    match ty {
        ResolvedType::I64 => Ok(GrammarTerm::Scalar(GrammarScalar::I64)),
        ResolvedType::I32 => Ok(GrammarTerm::Scalar(GrammarScalar::I32)),
        ResolvedType::U8 => Ok(GrammarTerm::Scalar(GrammarScalar::U8)),
        ResolvedType::Usize => Ok(GrammarTerm::Scalar(GrammarScalar::Usize)),
        ResolvedType::Char => Ok(GrammarTerm::Scalar(GrammarScalar::Char)),
        ResolvedType::F32 => Ok(GrammarTerm::Scalar(GrammarScalar::F32)),
        ResolvedType::F64 => Ok(GrammarTerm::Scalar(GrammarScalar::F64)),
        ResolvedType::Bool => Ok(GrammarTerm::Scalar(GrammarScalar::Bool)),
        ResolvedType::Bytes => Ok(GrammarTerm::Bytes),
        ResolvedType::TypeParameter { .. } => rejection(Rejection::TypeParameter),
        ResolvedType::String => rejection(Rejection::OwnedString),
        ResolvedType::Str => rejection(Rejection::BorrowedStr),
        ResolvedType::SliceU8 => rejection(Rejection::BorrowedByteView),
        ResolvedType::Unit => rejection(Rejection::Unit),
        ResolvedType::ArrayU8(_) => rejection(Rejection::InlineByteArray),
        ResolvedType::Function { .. } => rejection(Rejection::FunctionType),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            let identity = declaration.as_str();
            if crate::prelude::is_compiler_owned_id(identity) {
                return rejection(Rejection::CompilerOwnedNominal);
            }
            if duplicated(program, identity) {
                return rejection(Rejection::AmbiguousDeclaration);
            }
            let Some(found) = index.get(identity).copied() else {
                return rejection(Rejection::MissingDeclaration);
            };
            record_fields(found)?;
            if found.type_parameters.len() != arguments.len() {
                return rejection(Rejection::ArityMismatch);
            }
            if arguments.len() > MAX_TEMPLATE_ARITY {
                return Err(capacity("declared template arity limit"));
            }
            let mut rendered = Vec::with_capacity(arguments.len());
            for argument in arguments {
                rendered.push(classify_with(program, index, argument, budget, depth + 1)?);
            }
            Ok(GrammarTerm::Instance {
                declaration: identity.to_owned(),
                arguments: rendered,
            })
        }
    }
}

/// The canonical term of one checked type, bounded by [`MAX_TERM_BYTES`].
pub fn term(program: &ResolvedProgram, ty: &ResolvedType) -> Result<String, Diagnostic> {
    let rendered = classify(program, ty)?.render();
    if rendered.len() > MAX_TERM_BYTES {
        return Err(capacity("canonical term byte limit"));
    }
    Ok(rendered)
}

/// Exact owner-and-index substitution, applied before examining descendants.
fn substitute(
    ty: &ResolvedType,
    owner: &str,
    arguments: &[ResolvedType],
    budget: &mut Budget,
    depth: usize,
) -> Result<ResolvedType, Diagnostic> {
    budget.visit(depth)?;
    Ok(match ty {
        ResolvedType::TypeParameter {
            owner: parameter_owner,
            index,
        } if parameter_owner.as_str() == owner => arguments
            .get(*index as usize)
            .cloned()
            .ok_or_else(|| Rejection::ArityMismatch.diagnostic())?,
        ResolvedType::Nominal {
            declaration,
            arguments: nested,
        } => ResolvedType::Nominal {
            declaration: declaration.clone(),
            arguments: nested
                .iter()
                .map(|item| substitute(item, owner, arguments, budget, depth + 1))
                .collect::<Result<Vec<_>, _>>()?,
        },
        _ => ty.clone(),
    })
}

/// Describe one concrete generic instance: its canonical term, its template
/// identity, its ordered argument identities, its substituted field inventory,
/// and its transitive owned leaves.
///
/// Every fact is re-derived from the checked program. Nothing is read back
/// from a previously emitted artifact.
pub fn describe(program: &ResolvedProgram, ty: &ResolvedType) -> Result<InstanceFacts, Diagnostic> {
    let index = declarations(program);
    let mut budget = Budget::default();
    let parsed = classify_with(program, &index, ty, &mut budget, 0)?;
    let GrammarTerm::Instance { .. } = &parsed else {
        return Err(Rejection::UnadmittedNominalKind.diagnostic());
    };
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(Rejection::UnadmittedNominalKind.diagnostic());
    };
    let rendered = parsed.render();
    if rendered.len() > MAX_TERM_BYTES {
        return Err(capacity("canonical term byte limit"));
    }
    let found = index
        .get(declaration.as_str())
        .copied()
        .ok_or_else(|| Rejection::MissingDeclaration.diagnostic())?;

    let template = template_identity(found);
    let mut argument_facts = Vec::with_capacity(arguments.len());
    for (position, argument) in arguments.iter().enumerate() {
        let argument_term = classify_with(program, &index, argument, &mut budget, 1)?.render();
        argument_facts.push(ArgumentFact {
            index: position as u32,
            parameter_owner: declaration.as_str().to_owned(),
            parameter_index: position as u32,
            digest: term_digest(&argument_term),
            term: argument_term,
        });
    }

    let mut fields = Vec::new();
    let mut owned_leaves = Vec::new();
    for field in record_fields(found)? {
        let concrete = substitute(&field.ty, declaration.as_str(), arguments, &mut budget, 1)?;
        let field_term = classify_with(program, &index, &concrete, &mut budget, 1)?.render();
        let mut path = String::new();
        write_identity(&mut path, field.id.as_str());
        collect_owned_leaves(&index, &concrete, &path, &mut owned_leaves, &mut budget, 1)?;
        fields.push(FieldFact {
            index: field.index,
            id: field.id.as_str().to_owned(),
            name: field.name.clone(),
            digest: term_digest(&field_term),
            term: field_term,
        });
    }

    let mut preimage = Vec::new();
    frame(&mut preimage, template.digest.as_bytes());
    frame(&mut preimage, rendered.as_bytes());
    preimage.extend_from_slice(&(argument_facts.len() as u64).to_le_bytes());
    for argument in &argument_facts {
        preimage.extend_from_slice(&argument.index.to_le_bytes());
        frame(&mut preimage, argument.parameter_owner.as_bytes());
        preimage.extend_from_slice(&argument.parameter_index.to_le_bytes());
        frame(&mut preimage, argument.digest.as_bytes());
    }
    preimage.extend_from_slice(&(fields.len() as u64).to_le_bytes());
    for field in &fields {
        preimage.extend_from_slice(&field.index.to_le_bytes());
        frame(&mut preimage, field.id.as_bytes());
        frame(&mut preimage, field.digest.as_bytes());
    }
    preimage.extend_from_slice(&(owned_leaves.len() as u64).to_le_bytes());
    for leaf in &owned_leaves {
        frame(&mut preimage, leaf.as_bytes());
    }

    Ok(InstanceFacts {
        term_digest: term_digest(&rendered),
        term: rendered,
        template,
        arguments: argument_facts,
        fields,
        owned_leaves,
        instance_digest: digest(INSTANCE_DOMAIN, &preimage),
    })
}

fn template_identity(declaration: &ResolvedTypeDeclaration) -> TemplateIdentity {
    let parameters = declaration
        .type_parameters
        .iter()
        .map(|parameter| TemplateParameter {
            owner: declaration.id.as_str().to_owned(),
            index: parameter.index,
            name: parameter.name.clone(),
        })
        .collect::<Vec<_>>();
    let mut preimage = Vec::new();
    frame(&mut preimage, declaration.id.as_str().as_bytes());
    preimage.extend_from_slice(&(parameters.len() as u64).to_le_bytes());
    for parameter in &parameters {
        frame(&mut preimage, parameter.owner.as_bytes());
        preimage.extend_from_slice(&parameter.index.to_le_bytes());
    }
    TemplateIdentity {
        declaration: declaration.id.as_str().to_owned(),
        name: declaration.name.clone(),
        arity: parameters.len(),
        digest: digest(TEMPLATE_DOMAIN, &preimage),
        parameters,
    }
}

fn collect_owned_leaves(
    index: &BTreeMap<&str, &ResolvedTypeDeclaration>,
    ty: &ResolvedType,
    path: &str,
    output: &mut Vec<String>,
    budget: &mut Budget,
    depth: usize,
) -> Result<(), Diagnostic> {
    budget.visit(depth)?;
    match ty {
        ResolvedType::Bytes => {
            budget.leaf()?;
            output.push(path.to_owned());
            Ok(())
        }
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            let found = index
                .get(declaration.as_str())
                .copied()
                .ok_or_else(|| Rejection::MissingDeclaration.diagnostic())?;
            for field in record_fields(found)? {
                let concrete = substitute(
                    &field.ty,
                    declaration.as_str(),
                    arguments,
                    budget,
                    depth + 1,
                )?;
                let mut child = path.to_owned();
                child.push('/');
                write_identity(&mut child, field.id.as_str());
                collect_owned_leaves(index, &concrete, &child, output, budget, depth + 1)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Parse canonical bytes. Strict: no whitespace, no leading zeros in a length
/// prefix, no trailing bytes, no repair.
pub fn parse_term(text: &str) -> Result<GrammarTerm, Diagnostic> {
    if text.len() > MAX_TERM_BYTES {
        return Err(capacity("canonical term byte limit"));
    }
    let mut cursor = Cursor {
        text,
        offset: 0,
        nodes: 0,
    };
    let term = cursor.term(0)?;
    if cursor.offset != text.len() {
        return Err(malformed("trailing bytes after the term"));
    }
    Ok(term)
}

struct Cursor<'a> {
    text: &'a str,
    offset: usize,
    nodes: usize,
}

impl Cursor<'_> {
    fn rest(&self) -> &str {
        &self.text[self.offset..]
    }

    fn term(&mut self, depth: usize) -> Result<GrammarTerm, Diagnostic> {
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_VISITED_NODES || depth > MAX_RECORD_DEPTH {
            return Err(capacity("type node or nesting limit"));
        }
        if self.rest().starts_with('@') {
            return self.instance(depth);
        }
        for token in [
            "bytes", "usize", "bool", "char", "i64", "i32", "f32", "f64", "u8",
        ] {
            if self.rest().starts_with(token) {
                self.offset += token.len();
                return Ok(if token == "bytes" {
                    GrammarTerm::Bytes
                } else {
                    GrammarTerm::Scalar(
                        GrammarScalar::from_text(token)
                            .ok_or_else(|| malformed("unknown scalar token"))?,
                    )
                });
            }
        }
        Err(malformed("expected a scalar, `bytes`, or an instance"))
    }

    fn instance(&mut self, depth: usize) -> Result<GrammarTerm, Diagnostic> {
        self.offset += 1;
        let rest = self.rest();
        let colon = rest
            .find(':')
            .ok_or_else(|| malformed("identity length prefix has no `:`"))?;
        let digits = &rest[..colon];
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(malformed("identity length prefix is not decimal"));
        }
        if digits.len() > 1 && digits.starts_with('0') {
            return Err(malformed("identity length prefix has a leading zero"));
        }
        let length = digits
            .parse::<usize>()
            .map_err(|_| malformed("identity length prefix overflows"))?;
        if length > MAX_TERM_BYTES {
            return Err(capacity("identity byte limit"));
        }
        self.offset += colon + 1;
        let rest = self.rest();
        if rest.len() < length {
            return Err(malformed("identity is shorter than its length prefix"));
        }
        if !rest.is_char_boundary(length) {
            return Err(malformed("identity length splits a UTF-8 sequence"));
        }
        let identity = &rest[..length];
        if identity.contains('\0') {
            return Err(malformed("identity contains a NUL byte"));
        }
        let declaration = identity.to_owned();
        self.offset += length;
        if !self.rest().starts_with('<') {
            return Err(malformed("instance arguments do not open with `<`"));
        }
        self.offset += 1;
        let mut arguments = Vec::new();
        if self.rest().starts_with('>') {
            self.offset += 1;
            return Ok(GrammarTerm::Instance {
                declaration,
                arguments,
            });
        }
        loop {
            arguments.push(self.term(depth + 1)?);
            if arguments.len() > MAX_TEMPLATE_ARITY {
                return Err(capacity("declared template arity limit"));
            }
            if self.rest().starts_with(',') {
                self.offset += 1;
                continue;
            }
            if self.rest().starts_with('>') {
                self.offset += 1;
                return Ok(GrammarTerm::Instance {
                    declaration,
                    arguments,
                });
            }
            return Err(malformed("instance arguments do not close with `>`"));
        }
    }
}

/// Independently recompute the canonical term of `ty` and require the
/// submitted bytes to equal it exactly. Submitted bytes are never treated as
/// source, HIR, identity, or authority.
pub fn verify_term(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    submitted: &str,
) -> Result<(), Diagnostic> {
    let parsed = parse_term(submitted)?;
    let recomputed = term(program, ty)?;
    if parsed.render() != submitted || recomputed != submitted {
        return Err(Diagnostic::io(
            TERM_REPLAY_MISMATCH,
            format!(
                "{PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA} replay mismatch: submitted bytes are not \
                 the independently recomputed canonical term"
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
