//! WIT type projection for one [`AdmittedSubject`](super::classifier::AdmittedSubject).
//!
//! This is a documentation-tracked slice of issue #176 ("Add a WIT and
//! WebAssembly Component Model projection for supported generic resources"),
//! itself gated behind PG-9 of the
//! [Public Generic Ownership milestone](../../docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md),
//! which remains undecided. **Public generic ownership is not supported or
//! published.** This module projects the [Public Generic Type Grammar
//! v1](../../docs/PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) terms already reachable
//! from a [`classifier::classify`](super::classifier::classify) admission
//! into deterministic WIT `record`/`resource` text. It does not emit a
//! Component binary, does not claim a working component (no compiled `.wasm`
//! implements the provider ABI yet — issue #229), does not export a callable
//! function, and does not widen the milestone's standing support decision.
//!
//! ## Scope
//!
//! The type grammar (PG-1/PG-2) admits exactly three term shapes: a Copy
//! scalar, direct owned `Bytes`, and a fully concrete authored record
//! instance. Nothing else — variants, owned/borrowed strings, `Option`,
//! `Result`, borrowed views, or authored resources — is admitted by the
//! grammar in the first place, so nothing else can ever reach this
//! projection; each is refused earlier, by the grammar itself, with its own
//! closed reason. This module therefore has exactly three mapping rows:
//!
//! | Grammar term | WIT projection |
//! | --- | --- |
//! | scalar (`i64`, `i32`, `u8`, `char`, `f32`, `f64`, `bool`) | the matching WIT primitive (`s64`, `s32`, `u8`, `char`, `f32`, `f64`, `bool`) |
//! | scalar (`usize`) | refused: no WIT primitive has a portable pointer width |
//! | `bytes` | `own<`[`OWNED_BYTES_RESOURCE`]`>`, a handle to one shared opaque resource — never `list<u8>`, which would claim by-value component-level copy/GC semantics this compiler's explicit cleanup discipline does not have |
//! | a concrete record instance | a WIT `record` with one field per substituted field, recursively projected |
//!
//! A term the grammar could widen in a future version (a variant, an
//! authored resource, a borrowed view) has no row here and is not silently
//! approximated; it stays a `classifier`/grammar-level refusal until this
//! module gains its own new mapping row and its own new gate.
//!
//! ## Naming
//!
//! Every WIT identifier is `spx-` followed by the lowercase hexadecimal UTF-8
//! bytes of a persistent identity: the record's own canonical grammar term
//! for a record name, and the field's stable declaration id for a field
//! name — the same convention [`crate::project::scalar_wit`] already uses for
//! exported function names. Hex framing of an already-injective identity
//! (the grammar term) is itself injective, so two distinct records can never
//! collide, and no digest, truncation, or normalization step can hide a
//! collision. A display-only rename of a record or field never changes its
//! WIT name, because the grammar term and the stable field id it hexes
//! already exclude every display name.
//!
//! ## Determinism
//!
//! [`project_admitted_subject`] takes its closure from
//! [`AdmittedSubject::record_closure`](super::classifier::AdmittedSubject::record_closure),
//! a `BTreeMap` keyed by canonical term, and visits each field of each record
//! in the record's own declaration order. Nothing here reads a `HashMap`, the
//! process environment, the clock, or randomness, so the same admitted
//! subject renders byte-identical WIT on every call.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::public_generic_type::{
    self as grammar, GrammarScalar, GrammarTerm, InstanceFacts, MAX_RECORD_DEPTH,
};

use super::classifier::AdmittedSubject;

/// The versioned projection schema. A new mapping row is a new schema.
pub const WIT_TYPE_PROJECTION_SCHEMA: &str =
    "semaprax.public-generic-type-grammar.v1.wit-projection.v1";

/// The WIT package this projection emits. Deliberately its own identity,
/// separate from the frozen `semaprax:project-scalar@1.0.0` package
/// [`crate::project::scalar_wit`] owns: a public generic surface is not
/// admitted, so it cannot reuse that package's frozen identity or claim its
/// standing.
pub const WIT_PACKAGE: &str = "semaprax:public-generic-types@0.1.0";
/// The WIT interface name.
pub const WIT_INTERFACE: &str = "types";
/// The WIT world name.
pub const WIT_WORLD: &str = "public-generic-types-v1";
/// The one shared resource every projected `bytes` leaf is a handle to.
pub const OWNED_BYTES_RESOURCE: &str = "spx-owned-bytes";

/// The complete projected WIT text may not exceed this many bytes.
pub const MAX_WIT_PROJECTION_BYTES: usize = 65_536;

/// A scalar the type grammar admits but this WIT projection cannot: `usize`
/// has no WIT primitive with a portable pointer width.
pub const UNSUPPORTED_SCALAR: &str = "SPX-PGWIT101";
/// A record field names a nested instance term absent from the closure this
/// projection was given. The closure is explicit and caller-supplied, never
/// auto-scanned, so a missing member is a refusal, not a silent expansion.
pub const MISSING_CLOSURE_MEMBER: &str = "SPX-PGWIT102";
/// The closure holds two different [`InstanceFacts`] under the same
/// canonical term, or a record/field name collision was reached despite
/// hex framing (defensive; injectivity makes this unreachable in practice).
pub const CLOSURE_INCONSISTENT: &str = "SPX-PGWIT103";
/// A projection bound was reached. Never truncated or repaired.
pub const PROJECTION_CAPACITY: &str = "SPX-PGWIT104";
/// Submitted WIT text is not this profile's exact canonical rendering.
pub const MALFORMED_WIT: &str = "SPX-PGWIT105";

fn unsupported_scalar(scalar: GrammarScalar) -> Diagnostic {
    Diagnostic::io(
        UNSUPPORTED_SCALAR,
        format!(
            "{WIT_TYPE_PROJECTION_SCHEMA} cannot project `{}`: no WIT primitive has a portable \
             pointer width",
            scalar.text()
        ),
    )
}

fn missing_closure_member(term: &str) -> Diagnostic {
    Diagnostic::io(
        MISSING_CLOSURE_MEMBER,
        format!(
            "{WIT_TYPE_PROJECTION_SCHEMA} needs a record instance absent from the supplied \
             closure: {term}"
        ),
    )
}

fn closure_inconsistent(term: &str) -> Diagnostic {
    Diagnostic::io(
        CLOSURE_INCONSISTENT,
        format!(
            "{WIT_TYPE_PROJECTION_SCHEMA} was given two different instance facts for one \
             canonical term: {term}"
        ),
    )
}

fn capacity(subject: &str) -> Diagnostic {
    Diagnostic::io(
        PROJECTION_CAPACITY,
        format!("{WIT_TYPE_PROJECTION_SCHEMA} exceeded its {subject}"),
    )
}

fn malformed(subject: &str) -> Diagnostic {
    Diagnostic::io(
        MALFORMED_WIT,
        format!("not this profile's canonical {WIT_TYPE_PROJECTION_SCHEMA} rendering: {subject}"),
    )
}

/// `spx-` followed by the lowercase hex bytes of `identity`. Injective: two
/// distinct byte strings hex to two distinct names, so no display name,
/// punctuation, or length can make two identities collide.
fn legal_name(identity: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut name = String::with_capacity(4 + identity.len() * 2);
    name.push_str("spx-");
    for byte in identity.bytes() {
        name.push(HEX[(byte >> 4) as usize] as char);
        name.push(HEX[(byte & 0x0f) as usize] as char);
    }
    name
}

/// One field of one projected WIT record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WitFieldV1 {
    /// The exact persistent field declaration id this field was derived
    /// from, retained as metadata (never itself the WIT identifier).
    pub source_field_id: String,
    /// `spx-<hex(source_field_id)>`.
    pub name: String,
    /// The field's WIT type, exactly as rendered: a primitive name, an
    /// `own<...>` resource handle, or a record name.
    pub type_text: String,
}

/// One projected WIT `record`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WitRecordV1 {
    /// The exact canonical grammar term this record was derived from.
    pub source_term: String,
    /// `spx-<hex(source_term)>`.
    pub name: String,
    pub fields: Vec<WitFieldV1>,
}

/// The complete deterministic WIT type projection of one admitted subject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WitTypeProjectionV1 {
    pub schema: &'static str,
    pub package: &'static str,
    pub interface: &'static str,
    pub world: &'static str,
    /// Whether any projected field is a `bytes` leaf, and therefore whether
    /// [`OWNED_BYTES_RESOURCE`] is declared.
    pub uses_owned_bytes_resource: bool,
    /// Every projected record, in first-discovery postorder: a record's
    /// dependencies are always emitted before the record itself.
    pub records: Vec<WitRecordV1>,
    /// The projected input record's WIT name.
    pub input_type: String,
    /// The projected result record's WIT name.
    pub result_type: String,
    /// The exact rendered WIT text.
    pub wit: String,
}

struct Projector<'a> {
    closure: BTreeMap<&'a str, &'a InstanceFacts>,
    emitted: BTreeSet<String>,
    records: Vec<WitRecordV1>,
    uses_owned_bytes_resource: bool,
}

impl<'a> Projector<'a> {
    fn field_type(&mut self, term_text: &str, depth: usize) -> Result<String, Diagnostic> {
        if depth > MAX_RECORD_DEPTH {
            return Err(capacity("record nesting depth"));
        }
        let parsed = grammar::parse_term(term_text)?;
        Ok(match parsed {
            GrammarTerm::Scalar(scalar) => match scalar {
                GrammarScalar::I64 => "s64".to_owned(),
                GrammarScalar::I32 => "s32".to_owned(),
                GrammarScalar::U8 => "u8".to_owned(),
                GrammarScalar::Char => "char".to_owned(),
                GrammarScalar::F32 => "f32".to_owned(),
                GrammarScalar::F64 => "f64".to_owned(),
                GrammarScalar::Bool => "bool".to_owned(),
                GrammarScalar::Usize => return Err(unsupported_scalar(scalar)),
            },
            GrammarTerm::Bytes => {
                self.uses_owned_bytes_resource = true;
                format!("own<{OWNED_BYTES_RESOURCE}>")
            }
            GrammarTerm::Instance { .. } => {
                self.emit_record(term_text, depth + 1)?;
                legal_name(term_text)
            }
        })
    }

    fn emit_record(&mut self, term: &str, depth: usize) -> Result<(), Diagnostic> {
        let name = legal_name(term);
        if self.emitted.contains(&name) {
            return Ok(());
        }
        if depth > MAX_RECORD_DEPTH {
            return Err(capacity("record nesting depth"));
        }
        let facts = *self
            .closure
            .get(term)
            .ok_or_else(|| missing_closure_member(term))?;
        if facts.term != term {
            return Err(closure_inconsistent(term));
        }
        // Mark as emitted before recursing so a cyclic reference (which the
        // grammar's own arity/finite-instance construction should already
        // make unreachable) fails as a missing dependency instead of
        // recursing forever.
        self.emitted.insert(name.clone());
        let mut fields = Vec::with_capacity(facts.fields.len());
        for field in &facts.fields {
            let type_text = self.field_type(&field.term, depth + 1)?;
            fields.push(WitFieldV1 {
                source_field_id: field.id.clone(),
                name: legal_name(&field.id),
                type_text,
            });
        }
        self.records.push(WitRecordV1 {
            source_term: term.to_owned(),
            name,
            fields,
        });
        Ok(())
    }
}

/// Project every record instance reachable from `subject` into deterministic
/// WIT text.
///
/// The closure comes from
/// [`AdmittedSubject::record_closure`](super::classifier::AdmittedSubject::record_closure)
/// — the same trusted, already-checked closure the Public Generic Boundary
/// Profile v1 classifier computes for candidate-delta and settlement use — so
/// this projection defines no parallel notion of "everything a signature
/// reaches."
pub fn project_admitted_subject(
    subject: &AdmittedSubject,
) -> Result<WitTypeProjectionV1, Diagnostic> {
    let mut projector = Projector {
        closure: subject
            .record_closure()
            .iter()
            .map(|(term, facts)| (term.as_str(), facts))
            .collect(),
        emitted: BTreeSet::new(),
        records: Vec::new(),
        uses_owned_bytes_resource: false,
    };
    // `record_closure` is a `BTreeMap`, so this iterates in canonical term
    // order: fixed across every call, independent of hashing or insertion
    // order.
    let closure_terms: Vec<&str> = projector.closure.keys().copied().collect();
    for term in closure_terms {
        projector.emit_record(term, 1)?;
    }
    let input_type = legal_name(&subject.input().term);
    let result_type = legal_name(&subject.result().term);
    if !projector.emitted.contains(&input_type) || !projector.emitted.contains(&result_type) {
        // The classifier guarantees the input and result are themselves
        // members of their own closure; this is a defensive, never-expected
        // consistency check, not a reachable product path.
        return Err(closure_inconsistent(&subject.input().term));
    }
    let wit = render(&projector.records, projector.uses_owned_bytes_resource);
    if wit.len() > MAX_WIT_PROJECTION_BYTES {
        return Err(capacity("WIT projection byte limit"));
    }
    Ok(WitTypeProjectionV1 {
        schema: WIT_TYPE_PROJECTION_SCHEMA,
        package: WIT_PACKAGE,
        interface: WIT_INTERFACE,
        world: WIT_WORLD,
        uses_owned_bytes_resource: projector.uses_owned_bytes_resource,
        records: projector.records,
        input_type,
        result_type,
        wit,
    })
}

fn render(records: &[WitRecordV1], uses_owned_bytes_resource: bool) -> String {
    let mut output = format!("package {WIT_PACKAGE};\n\ninterface {WIT_INTERFACE} {{\n");
    if uses_owned_bytes_resource {
        output.push_str("  resource ");
        output.push_str(OWNED_BYTES_RESOURCE);
        output.push_str(";\n");
    }
    for record in records {
        output.push_str("  record ");
        output.push_str(&record.name);
        output.push_str(" {\n");
        for field in &record.fields {
            output.push_str("    ");
            output.push_str(&field.name);
            output.push_str(": ");
            output.push_str(&field.type_text);
            output.push_str(",\n");
        }
        output.push_str("  }\n");
    }
    output.push_str("}\n\nworld ");
    output.push_str(WIT_WORLD);
    output.push_str(" {\n  export ");
    output.push_str(WIT_INTERFACE);
    output.push_str(";\n}\n");
    output
}

/// One record parsed back out of projected WIT text: its name and its
/// ordered `(field name, field type text)` pairs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedWitRecord {
    pub name: String,
    pub fields: Vec<(String, String)>,
}

/// The complete structure independently parsed back out of one rendering of
/// [`render`]. This is a bounded profile parser for exactly this module's own
/// output shape, matching the "independent bounded parser accepts only that
/// profile" convention the private WIT harness already uses; it is not a
/// general WIT-text parser and is not cross-checked against an external WIT
/// parser or `wit-parser`/`wasmparser` crate (adding one is out of this
/// module's lease).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedWitProjection {
    pub package: String,
    pub interface: String,
    pub world: String,
    pub uses_owned_bytes_resource: bool,
    pub records: Vec<ParsedWitRecord>,
}

/// Scan a run of legal WIT kebab-identifier bytes (`[a-z0-9-]`) from the
/// front of `input`. Never crosses a delimiter it does not recognize: unlike
/// an unbounded `str::find`, this cannot walk past a corrupted separator into
/// a later, unrelated line and mistake it for the one being parsed.
fn take_identifier(input: &str) -> Result<(&str, &str), Diagnostic> {
    let end = input
        .find(|byte: char| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == '-'))
        .ok_or_else(|| malformed("identifier runs to end of input"))?;
    if end == 0 {
        return Err(malformed("expected an identifier"));
    }
    Ok((&input[..end], &input[end..]))
}

/// Scan a run of legal projected field-type bytes: a primitive name, or an
/// `own<resource-name>`/record-name reference, all built only from
/// `[a-z0-9<>-]`. Bounded the same way [`take_identifier`] is, for the same
/// reason: it must not be able to swallow a corrupted separator and continue
/// into later text.
fn take_type_text(input: &str) -> Result<(&str, &str), Diagnostic> {
    let end = input
        .find(|byte: char| {
            !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, '-' | '<' | '>'))
        })
        .ok_or_else(|| malformed("field type runs to end of input"))?;
    if end == 0 {
        return Err(malformed("expected a field type"));
    }
    Ok((&input[..end], &input[end..]))
}

/// Parse text this module's own [`render`] could have produced. Strict:
/// unexpected indentation, punctuation, ordering, or trailing bytes fail
/// closed rather than being skipped or repaired. Every name and type is
/// scanned by its exact legal character set ([`take_identifier`] /
/// [`take_type_text`]) rather than searched for with an unbounded `find`, so
/// a single corrupted delimiter cannot let parsing resynchronize on a later,
/// unrelated line and silently accept the wrong text as the mutated field.
pub fn parse_wit_projection(text: &str) -> Result<ParsedWitProjection, Diagnostic> {
    if text.len() > MAX_WIT_PROJECTION_BYTES {
        return Err(capacity("WIT projection byte limit"));
    }
    let mut rest = text
        .strip_prefix("package ")
        .ok_or_else(|| malformed("missing `package` line"))?;
    let semi = rest
        .find(';')
        .ok_or_else(|| malformed("unterminated package line"))?;
    let package = rest[..semi].to_owned();
    rest = rest[semi + 1..]
        .strip_prefix("\n\ninterface ")
        .ok_or_else(|| malformed("missing interface header"))?;
    let (interface, after) = take_identifier(rest)?;
    let interface = interface.to_owned();
    rest = after
        .strip_prefix(" {\n")
        .ok_or_else(|| malformed("unterminated interface header"))?;

    let mut uses_owned_bytes_resource = false;
    if let Some(after) = rest.strip_prefix(&format!("  resource {OWNED_BYTES_RESOURCE};\n")) {
        uses_owned_bytes_resource = true;
        rest = after;
    }

    let mut records = Vec::new();
    while let Some(after) = rest.strip_prefix("  record ") {
        let (name, after) = take_identifier(after)?;
        let name = name.to_owned();
        rest = after
            .strip_prefix(" {\n")
            .ok_or_else(|| malformed("unterminated record header"))?;
        let mut fields = Vec::new();
        loop {
            if let Some(after) = rest.strip_prefix("  }\n") {
                rest = after;
                break;
            }
            let after = rest
                .strip_prefix("    ")
                .ok_or_else(|| malformed("expected an indented field or closing brace"))?;
            let (field_name, after) = take_identifier(after)?;
            let field_name = field_name.to_owned();
            let after = after
                .strip_prefix(": ")
                .ok_or_else(|| malformed("field missing `: `"))?;
            let (field_type, after) = take_type_text(after)?;
            let field_type = field_type.to_owned();
            rest = after
                .strip_prefix(",\n")
                .ok_or_else(|| malformed("field missing trailing comma"))?;
            fields.push((field_name, field_type));
        }
        records.push(ParsedWitRecord { name, fields });
    }

    rest = rest
        .strip_prefix("}\n\nworld ")
        .ok_or_else(|| malformed("missing world header"))?;
    let (world, after) = take_identifier(rest)?;
    let world = world.to_owned();
    rest = after
        .strip_prefix(" {\n  export ")
        .ok_or_else(|| malformed("unterminated world header"))?;
    let footer = format!("{interface};\n}}\n");
    if rest != footer {
        return Err(malformed("unexpected world export footer"));
    }

    Ok(ParsedWitProjection {
        package,
        interface,
        world,
        uses_owned_bytes_resource,
        records,
    })
}

#[cfg(test)]
mod tests;
