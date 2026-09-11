//! Generated foreign consumers of public generic *metadata*, and the canonical
//! byte format they read.
//!
//! This serves gates PG-5 and PG-6 of the
//! [Public Generic Ownership milestone](../docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md)
//! for the grammar half of each. Before any foreign toolchain can *call* a
//! public generic export, four of them have to agree, byte for byte, on what
//! the type grammar says — and refuse anything else. That is what this
//! generates: a Rust, TypeScript/Wasm, C, and C++ consumer that each parse the
//! canonical metadata of a candidate surface strictly, validate every type
//! field against the grammar, compare the whole document against their own
//! embedded expectation, and fail closed on forged, stale, reordered,
//! truncated, or merely sloppy bytes.
//!
//! What this is *not*: a calling convention. A generated consumer here never
//! receives a SEMAPRAX value, never allocates one, never frees one, and never
//! links against anything. Its generated type declarations exist so a consumer
//! author can see the substituted field tree in their own language; they define
//! no layout, no ownership transfer, and no ABI. Consumers that actually call a
//! public generic export need a versioned descriptor and carrier that do not
//! exist, so PG-5 and PG-6 remain open for that half.
//!
//! Generation is deterministic and authority-free: it reads checked facts,
//! returns source text, and touches no file, process, or network.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::diagnostic::Diagnostic;
use crate::public_generic_surface::CandidateSurface;
use crate::public_generic_type as grammar;

mod c;
mod cxx;
mod rust;
/// The generated *calling* consumer (issue #156): a real Rust crate that
/// verifies an exact descriptor/provider pairing, transfers one owned input
/// record across the native adapter (issue #154) exactly once, calls, and
/// decodes an independently validated result. Distinct from every consumer
/// generated above, which never allocates, transfers, or calls anything.
pub mod rust_calling;
mod typescript;

/// The canonical metadata byte format every generated consumer reads.
pub const CONSUMER_METADATA_SCHEMA: &str = "semaprax.public-generic-consumer-metadata.v1";
/// The magic prefix of that format.
pub const CONSUMER_METADATA_MAGIC: &str = "spxpgcm1;";

/// Submitted metadata is not canonical, or a generated consumer refused it.
pub const METADATA_REFUSED: &str = "SPX-PG401";
/// A metadata bound was reached. Metadata is never truncated or repaired.
pub const METADATA_CAPACITY: &str = "SPX-PG402";

/// Canonical metadata bytes per surface.
pub const MAX_METADATA_BYTES: usize = 1024 * 1024;
/// Records per metadata document.
pub const MAX_METADATA_RECORDS: usize = 8192;
/// Fields per metadata record.
pub const MAX_RECORD_FIELDS: usize = 8;

/// Why a metadata document is refused. Closed: every generated consumer uses
/// exactly these three spellings, so a refusal means the same thing in four
/// languages.
///
/// There is deliberately no separate "non-canonical" reason. The format admits
/// no optional whitespace, no alternate spelling, and no leading zero in a
/// count, so a strict parse that consumes the whole document already implies
/// the bytes are their own canonical rendering — a reason no input could
/// produce would not be a closed vocabulary, it would be decoration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// The bytes are not a well-formed document of the format.
    Malformed,
    /// A type field is not a canonical term of the type grammar.
    Term,
    /// The bytes are well-formed but are not the expected ones.
    Mismatch,
}

impl Refusal {
    /// The closed wire spelling, shared with every generated consumer.
    pub const fn text(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::Term => "term",
            Self::Mismatch => "mismatch",
        }
    }

    /// Every refusal spelling, for a gate that has to cover all of them.
    pub const ALL: [Self; 3] = [Self::Malformed, Self::Term, Self::Mismatch];

    fn diagnostic(self) -> Diagnostic {
        Diagnostic::io(
            METADATA_REFUSED,
            format!("{CONSUMER_METADATA_SCHEMA} refused: {}", self.text()),
        )
    }
}

fn capacity(subject: &str) -> Diagnostic {
    Diagnostic::io(
        METADATA_CAPACITY,
        format!("{CONSUMER_METADATA_SCHEMA} exceeded its {subject}"),
    )
}

/// One canonical metadata document: an ordered sequence of records, each an
/// ordered sequence of length-framed fields whose first field is the record
/// kind.
///
/// Every field is length-framed in bytes, so a declaration identity may
/// contain the format's own punctuation — or a newline — without a parser ever
/// having to guess where a field ends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsumerMetadata {
    records: Vec<Vec<String>>,
}

impl ConsumerMetadata {
    /// Project one candidate surface into metadata.
    ///
    /// Record order is exactly the surface's own byte order: exports by
    /// declaration identity, instances by canonical term, then by index. The
    /// surface digest is the last record, so a stale document differs from a
    /// fresh one even when every other fact agrees.
    pub fn of(surface: &CandidateSurface) -> Result<Self, Diagnostic> {
        let mut records = Vec::new();
        for (export, entry) in surface.entries() {
            records.push(vec![
                "E".to_owned(),
                export.clone(),
                entry.parameters.len().to_string(),
            ]);
            for parameter in &entry.parameters {
                records.push(vec![
                    "P".to_owned(),
                    export.clone(),
                    parameter.index.to_string(),
                    parameter.value.kind.to_owned(),
                    parameter.ownership.to_owned(),
                    parameter.value.term.clone(),
                ]);
            }
            records.push(vec![
                "R".to_owned(),
                export.clone(),
                entry.result.kind.to_owned(),
                entry.result.term.clone(),
            ]);
        }
        for (term, facts) in surface.instances() {
            records.push(vec![
                "I".to_owned(),
                term.clone(),
                facts.template.declaration.clone(),
                facts.template.arity.to_string(),
            ]);
            for argument in &facts.arguments {
                records.push(vec![
                    "A".to_owned(),
                    term.clone(),
                    argument.index.to_string(),
                    argument.term.clone(),
                ]);
            }
            for field in &facts.fields {
                records.push(vec![
                    "F".to_owned(),
                    term.clone(),
                    field.index.to_string(),
                    field.id.clone(),
                    field.term.clone(),
                ]);
            }
            for (index, leaf) in facts.owned_leaves.iter().enumerate() {
                records.push(vec![
                    "L".to_owned(),
                    term.clone(),
                    index.to_string(),
                    leaf.clone(),
                ]);
            }
        }
        records.push(vec!["D".to_owned(), surface.digest().to_owned()]);
        let metadata = Self { records };
        metadata.check_bounds()?;
        Ok(metadata)
    }

    fn check_bounds(&self) -> Result<(), Diagnostic> {
        if self.records.len() > MAX_METADATA_RECORDS {
            return Err(capacity("record limit"));
        }
        if self
            .records
            .iter()
            .any(|record| record.len() > MAX_RECORD_FIELDS || record.is_empty())
        {
            return Err(capacity("record field limit"));
        }
        if self.render().len() > MAX_METADATA_BYTES {
            return Err(capacity("canonical byte limit"));
        }
        Ok(())
    }

    /// The canonical bytes.
    pub fn render(&self) -> String {
        let mut output = String::from(CONSUMER_METADATA_MAGIC);
        for record in &self.records {
            let _ = write!(output, "|{};", record.len());
            for field in record {
                let _ = write!(output, "{}:{field};", field.len());
            }
        }
        output
    }

    /// The records, each with its kind as the first field.
    pub fn records(&self) -> &[Vec<String>] {
        &self.records
    }

    /// Parse canonical bytes, and require every type field to be a canonical
    /// term of the grammar.
    ///
    /// Strict: exact byte lengths, decimal counts without a leading zero, a
    /// `;` after every field, a `|` before every record, and nothing after the
    /// last one. Nothing is repaired, and a successful parse round-trips to the
    /// exact submitted bytes.
    pub fn parse(bytes: &str) -> Result<Self, Refusal> {
        if bytes.len() > MAX_METADATA_BYTES {
            return Err(Refusal::Malformed);
        }
        let mut rest = bytes
            .strip_prefix(CONSUMER_METADATA_MAGIC)
            .ok_or(Refusal::Malformed)?;
        let mut records = Vec::new();
        while !rest.is_empty() {
            rest = rest.strip_prefix('|').ok_or(Refusal::Malformed)?;
            let (count, tail) = decimal(rest, ';')?;
            if count == 0 || count > MAX_RECORD_FIELDS {
                return Err(Refusal::Malformed);
            }
            rest = tail;
            let mut record = Vec::with_capacity(count);
            for _ in 0..count {
                let (length, tail) = decimal(rest, ':')?;
                if length > rest.len() {
                    return Err(Refusal::Malformed);
                }
                rest = tail;
                if rest.len() < length || !rest.is_char_boundary(length) {
                    return Err(Refusal::Malformed);
                }
                let (field, tail) = rest.split_at(length);
                if field.contains('\0') {
                    return Err(Refusal::Malformed);
                }
                rest = tail.strip_prefix(';').ok_or(Refusal::Malformed)?;
                record.push(field.to_owned());
            }
            records.push(record);
            if records.len() > MAX_METADATA_RECORDS {
                return Err(Refusal::Malformed);
            }
        }
        if records.is_empty() {
            return Err(Refusal::Malformed);
        }
        let metadata = Self { records };
        debug_assert_eq!(metadata.render(), bytes, "a strict parse must round-trip");
        metadata.check_terms()?;
        Ok(metadata)
    }

    /// Every field that carries a grammar term must be a canonical term.
    fn check_terms(&self) -> Result<(), Refusal> {
        for record in &self.records {
            let term = match (record[0].as_str(), record.len()) {
                ("P", 6) if record[3] == "data" => Some(&record[5]),
                ("R", 4) if record[2] == "data" => Some(&record[3]),
                ("I", 4) => Some(&record[1]),
                ("A", 4) => Some(&record[3]),
                ("F", 5) => Some(&record[4]),
                _ => None,
            };
            if let Some(term) = term {
                let parsed = grammar::parse_term(term).map_err(|_| Refusal::Term)?;
                if parsed.render() != *term {
                    return Err(Refusal::Term);
                }
            }
        }
        Ok(())
    }

    /// The reference implementation of what every generated consumer does:
    /// parse strictly, require every type field to be a canonical grammar
    /// term, then require equality with these expected bytes.
    pub fn accepts(&self, submitted: &str) -> Result<(), Refusal> {
        let parsed = Self::parse(submitted)?;
        if parsed != *self {
            return Err(Refusal::Mismatch);
        }
        Ok(())
    }

    /// The same check as a diagnostic-returning call, for Rust-side gates.
    pub fn verify(&self, submitted: &str) -> Result<(), Diagnostic> {
        self.accepts(submitted)
            .map_err(|refusal| refusal.diagnostic())
    }

    /// Every reachable instance term, in canonical order, with its ordered
    /// substituted fields. This is what the emitters declare types from.
    fn instances(&self) -> Vec<(String, Vec<(String, String)>)> {
        let mut fields: BTreeMap<&str, Vec<(String, String)>> = BTreeMap::new();
        let mut order = Vec::new();
        for record in &self.records {
            match record[0].as_str() {
                "I" => {
                    order.push(record[1].clone());
                    fields.entry(&record[1]).or_default();
                }
                "F" => fields
                    .entry(&record[1])
                    .or_default()
                    .push((record[3].clone(), record[4].clone())),
                _ => {}
            }
        }
        order
            .into_iter()
            .map(|term| {
                let members = fields.get(term.as_str()).cloned().unwrap_or_default();
                (term, members)
            })
            .collect()
    }
}

fn decimal(text: &str, terminator: char) -> Result<(usize, &str), Refusal> {
    let end = text.find(terminator).ok_or(Refusal::Malformed)?;
    let digits = &text[..end];
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Refusal::Malformed);
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return Err(Refusal::Malformed);
    }
    let value = digits.parse::<usize>().map_err(|_| Refusal::Malformed)?;
    Ok((value, &text[end + terminator.len_utf8()..]))
}

/// The foreign languages a consumer is generated for.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConsumerLanguage {
    Rust,
    /// TypeScript declarations plus the ES module a Wasm host would load.
    TypeScript,
    C,
    Cxx,
}

impl ConsumerLanguage {
    /// Every generated language, in a deterministic order.
    pub const ALL: [Self; 4] = [Self::Rust, Self::TypeScript, Self::C, Self::Cxx];

    /// The closed wire spelling.
    pub const fn text(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::C => "c",
            Self::Cxx => "cxx",
        }
    }

    /// The file names a generated consumer occupies, in emission order.
    pub const fn file_names(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["consumer.rs"],
            Self::TypeScript => &["consumer.mjs", "consumer.d.ts"],
            Self::C => &["consumer.c"],
            Self::Cxx => &["consumer.cpp"],
        }
    }

    /// The grammar scalar spelling in this language's declarations. These are
    /// declaration types for reading metadata, not an ABI mapping.
    fn scalar(self, scalar: grammar::GrammarScalar) -> &'static str {
        use grammar::GrammarScalar as S;
        match (self, scalar) {
            (Self::Rust, S::I64) => "i64",
            (Self::Rust, S::I32) => "i32",
            (Self::Rust, S::U8) => "u8",
            (Self::Rust, S::Usize) => "u64",
            (Self::Rust, S::Char) => "char",
            (Self::Rust, S::F32) => "f32",
            (Self::Rust, S::F64) => "f64",
            (Self::Rust, S::Bool) => "bool",
            (Self::TypeScript, S::I64 | S::Usize) => "bigint",
            (Self::TypeScript, S::I32 | S::U8 | S::F32 | S::F64) => "number",
            (Self::TypeScript, S::Char) => "string",
            (Self::TypeScript, S::Bool) => "boolean",
            (Self::C, S::I64) => "int64_t",
            (Self::C, S::I32) => "int32_t",
            (Self::C, S::U8) => "uint8_t",
            (Self::C, S::Usize) => "uint64_t",
            (Self::C, S::Char) => "uint32_t",
            (Self::C, S::F32) => "float",
            (Self::C, S::F64) => "double",
            (Self::C, S::Bool) => "bool",
            (Self::Cxx, S::I64) => "std::int64_t",
            (Self::Cxx, S::I32) => "std::int32_t",
            (Self::Cxx, S::U8) => "std::uint8_t",
            (Self::Cxx, S::Usize) => "std::uint64_t",
            (Self::Cxx, S::Char) => "char32_t",
            (Self::Cxx, S::F32) => "float",
            (Self::Cxx, S::F64) => "double",
            (Self::Cxx, S::Bool) => "bool",
        }
    }

    /// The owned-bytes declaration spelling.
    const fn bytes(self) -> &'static str {
        match self {
            Self::Rust => "Vec<u8>",
            Self::TypeScript => "Uint8Array",
            Self::C => "struct spx_pg_bytes",
            Self::Cxx => "std::vector<std::uint8_t>",
        }
    }
}

/// One generated consumer: named files of deterministic source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedConsumer {
    language: ConsumerLanguage,
    files: Vec<(String, String)>,
    metadata: String,
}

impl GeneratedConsumer {
    /// The language this consumer is written in.
    pub fn language(&self) -> ConsumerLanguage {
        self.language
    }

    /// The generated files, in emission order.
    pub fn files(&self) -> &[(String, String)] {
        &self.files
    }

    /// The canonical metadata bytes the consumer embeds and expects.
    pub fn metadata(&self) -> &str {
        &self.metadata
    }
}

/// Generate one consumer for one candidate surface.
///
/// Deterministic: the same surface and language always produce byte-identical
/// files. No file is written, no process is started, and no authority is used
/// or granted.
pub fn generate(
    surface: &CandidateSurface,
    language: ConsumerLanguage,
) -> Result<GeneratedConsumer, Diagnostic> {
    let metadata = ConsumerMetadata::of(surface)?;
    let rendered = metadata.render();
    // The generator must never emit metadata its own reference reader would
    // refuse; a consumer built on refused bytes could never be correct.
    metadata.verify(&rendered)?;
    let declarations = declarations(&metadata, language);
    let files = match language {
        ConsumerLanguage::Rust => rust::emit(&rendered, &declarations),
        ConsumerLanguage::TypeScript => typescript::emit(&rendered, &declarations),
        ConsumerLanguage::C => c::emit(&rendered, &declarations),
        ConsumerLanguage::Cxx => cxx::emit(&rendered, &declarations),
    };
    Ok(GeneratedConsumer {
        language,
        files,
        metadata: rendered,
    })
}

/// One generated type declaration: the instance term it describes, the host
/// identifier derived from that term, and its ordered members.
pub(crate) struct Declaration {
    pub(crate) term: String,
    pub(crate) identifier: String,
    pub(crate) members: Vec<(String, String)>,
}

fn declarations(metadata: &ConsumerMetadata, language: ConsumerLanguage) -> Vec<Declaration> {
    let instances = metadata.instances();
    let names = instances
        .iter()
        .map(|(term, _)| (term.clone(), identifier(term)))
        .collect::<BTreeMap<_, _>>();
    let members = instances
        .iter()
        .map(|(term, members)| (term.as_str(), members))
        .collect::<BTreeMap<_, _>>();

    // A struct member of an incomplete type is an error in C and C++, so a
    // nested instance has to be declared before the instance that holds it.
    // The instance graph is acyclic, so one post-order walk over the canonical
    // order is both a valid topological order and deterministic.
    let mut emitted = Vec::new();
    let mut seen = BTreeMap::new();
    for (term, _) in &instances {
        push_declaration(term, &members, &names, language, &mut seen, &mut emitted);
    }
    emitted
}

fn push_declaration(
    term: &str,
    members: &BTreeMap<&str, &Vec<(String, String)>>,
    names: &BTreeMap<String, String>,
    language: ConsumerLanguage,
    seen: &mut BTreeMap<String, ()>,
    emitted: &mut Vec<Declaration>,
) {
    if seen.insert(term.to_owned(), ()).is_some() {
        return;
    }
    let fields = members.get(term).copied().cloned().unwrap_or_default();
    for (_, member_term) in &fields {
        if names.contains_key(member_term) {
            push_declaration(member_term, members, names, language, seen, emitted);
        }
    }
    emitted.push(Declaration {
        term: term.to_owned(),
        identifier: identifier(term),
        members: fields
            .iter()
            .enumerate()
            .map(|(index, (id, member_term))| {
                (
                    format!("member_{index}_{}", identifier(id)),
                    member_type(member_term, language, names),
                )
            })
            .collect(),
    });
}

/// One generated file's fixed template.
///
/// The line endings are normalized because a checkout may deliver these with
/// CRLF — `.gitattributes` pins `.txt` to LF, but a generator that only works
/// because of a checkout setting is not a deterministic generator. Without
/// this, every placeholder whose match includes its newline silently fails to
/// substitute and the generated file keeps the literal placeholder.
fn template(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// One numeric byte literal per generated language: twelve bytes a line,
/// indented, so a diff of two generated consumers is readable.
fn byte_literal(metadata: &str, indent: &str) -> String {
    let mut output = String::new();
    for (index, byte) in metadata.bytes().enumerate() {
        if index % 12 == 0 {
            if index > 0 {
                output.push('\n');
            }
            output.push_str(indent);
        } else {
            output.push(' ');
        }
        let _ = write!(output, "0x{byte:02x},");
    }
    output.push('\n');
    output
}

/// Host identifiers are derived from bytes, not from display names: the
/// lowercase hex of the exact term or identity. Injective, stable under a
/// rename, and valid in all four languages.
///
/// `pub(crate)` so [`rust_calling`] reuses this exact scheme for the calling
/// consumer's field names, instead of restating an independent one.
pub(crate) fn identifier(value: &str) -> String {
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn member_type(term: &str, language: ConsumerLanguage, names: &BTreeMap<String, String>) -> String {
    if let Some(name) = names.get(term) {
        return match language {
            ConsumerLanguage::Rust | ConsumerLanguage::TypeScript => format!("SpxPg{name}"),
            ConsumerLanguage::C => format!("struct spx_pg_{name}"),
            ConsumerLanguage::Cxx => format!("SpxPg{name}"),
        };
    }
    match grammar::parse_term(term) {
        Ok(grammar::GrammarTerm::Scalar(scalar)) => language.scalar(scalar).to_owned(),
        Ok(grammar::GrammarTerm::Bytes) => language.bytes().to_owned(),
        // An instance term with no declaration record cannot occur in metadata
        // this module produced, and a consumer is never generated from
        // metadata the reference reader refused.
        _ => language.bytes().to_owned(),
    }
}

#[cfg(test)]
mod tests;
