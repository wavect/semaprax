//! Compact Semantic Projection v1 (issue #201): a deterministic
//! dictionary/index-based re-encoding of an already-selected semantic view
//! ([`crate::graph::to_json`] or [`crate::graph::agent_context_v2_json`])
//! into a smaller textual or binary wire form, independently and losslessly
//! replayable to the exact same selected view.
//!
//! # What already existed before this module
//!
//! SEMAPRAX already produces bounded, deterministic JSON projections of
//! program meaning: the full program graph ([`crate::graph::to_json`]), a
//! byte/node-bounded call closure around one seed
//! ([`crate::graph::agent_context_v2_json`], issue's "Agent Context v2"),
//! and a goal-aware multi-seed token-budgeted bundle over that same engine
//! ([`crate::semantic_task_context`], issue #197). Every one of those
//! already owns its own selection, closure, and budget rules; this module
//! calls them unchanged and never re-derives, relaxes, or duplicates any of
//! that selection logic. This module also does not invent a new
//! model-tokenizer accounting unit -- see `semantic_task_context` for that
//! (out of scope here; this module measures wire bytes only).
//!
//! # What this module adds
//!
//! A second, purely mechanical encoding stage applied *after* selection:
//!
//! 1. **A stable-ID/string dictionary ordered by canonical (raw) byte
//!    order**, built by scanning the selected view's JSON text for its
//!    string literals (which is where every repeated stable ID, type name,
//!    and field-shaped string lives) and deduplicating them. The ordering
//!    is a property of the *set* of distinct literals, never of the order
//!    they first appear in the source, or of what other literals happen to
//!    share the envelope -- the same literal can sit at a different index
//!    in two different envelopes and still decode correctly in both
//!    (`tests::dictionary_index_for_the_same_literal_differs_across_envelopes_and_decode_is_unaffected`).
//! 2. **A body token stream** referencing that dictionary by numeric index
//!    for each string-literal occurrence, with everything between literals
//!    (JSON's own punctuation, numbers, and keywords) carried as raw bytes.
//!    The index is local compression bookkeeping only, never a persistent
//!    identity -- nothing in this module's public surface exposes "resolve
//!    an index," only "reconstruct the whole selected view," and the
//!    dictionary/body are private fields precisely so nothing downstream
//!    can be tempted to treat an index as an identity.
//! 3. **Two independent wire forms** over the identical in-memory
//!    structure: [`CompactProjection::to_text`]/[`decode_text`], a
//!    compact ASCII form meant to be pasted into a model prompt (a
//!    length-framed header, one dictionary entry per line, and an inline
//!    `~index~`-marked body stream with no per-token framing), and
//!    [`CompactProjection::to_binary`]/[`decode_binary`], a fixed-width
//!    length-framed binary form meant for transport/cache. Both decoders
//!    share one validation and reconstruction path
//!    ([`finish_decode`]) so a bound or an integrity rule can never drift
//!    between the two forms.
//! 4. **A source digest carried in the envelope and re-verified after every
//!    decode.** [`decode_text`] and [`decode_binary`] never return a
//!    structurally well-formed but semantically wrong reconstruction: after
//!    parsing, both reconstruct the selected view from the dictionary and
//!    body, recompute its digest, and compare it to the digest the envelope
//!    itself claims. A tampered wire value that still parses cleanly (one
//!    content byte flipped without changing any length or count) is
//!    refused at this step, never silently accepted with different content
//!    than it started from (`tests::tampered_wire_content_fails_digest_verification`,
//!    `tests::tampered_text_body_content_fails_digest_verification`).
//! 5. **A migration/version refusal.** An unrecognized `format_version` is
//!    refused outright; this module never attempts a heuristic decode of an
//!    unknown wire version.
//!
//! # Profiles implemented here
//!
//! [`ProjectionSource::FullGraph`] wraps [`crate::graph::to_json`] (root
//! `"*"`, profile `"full-graph"`); [`ProjectionSource::AgentContextV2`]
//! wraps [`crate::graph::agent_context_v2_json`] for one seed symbol
//! (profile `"agent-context-v2"`, root the seed symbol). The wire format
//! itself does not restrict `profile` to this list -- a future profile can
//! be added by teaching [`encode_profile`] one more `ProjectionSource`
//! variant without any wire-format change -- but only these two are
//! implemented and tested here.
//!
//! # Deliberately out of scope here
//!
//! Issue #201 asks for a much larger surface: a "task context" profile
//! wired to issue #197's goal-aware compiler, an "API surface" profile, a
//! "candidate diff" profile, an "Agent definition" profile, byte/token
//! benchmarks across multiple tokenizer *versions*, and CLI/MCP exposure.
//! None of that is in this module. This is one narrow, honestly-scoped
//! slice: the dictionary/index encoding, canonical ordering, dual wire
//! form, and lossless-with-detection replay bullets of #201's
//! implementation sequence, applied to the two selected-view profiles this
//! module already had unchanged engines for -- matching the precedent
//! `semantic_task_context` and `semantic_embedding` set for shipping one
//! narrow, evidenced slice of a large issue rather than an unverifiable
//! broader claim.
//!
//! # Honesty bar
//!
//! "Lossless" here means exactly this: [`decode_text`] and
//! [`decode_binary`], given bytes produced by [`CompactProjection::to_text`]
//! or [`CompactProjection::to_binary`] from an [`encode_profile`] result,
//! reconstruct the selected view's JSON text byte-for-byte identical to
//! what [`crate::graph::to_json`] or [`crate::graph::agent_context_v2_json`]
//! returned directly -- proved in this module's tests by comparing against
//! an independently obtained value from those functions, not against a
//! round-tripped copy of itself. This module drops no field, comment, or
//! byte of the selected view: every byte is either a dictionary entry or a
//! raw body token, and reconstruction is their concatenation in original
//! order. It claims a materially smaller wire size only where measured
//! (see `docs/COMPACT-SEMANTIC-PROJECTION-V1.md` for the exact numbers on
//! this repository's own committed examples), not as a universal property.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::digest_hex::LowerHex;
use crate::graph::{self, AgentContextV2Options};

/// Schema identity of this module's compact wire envelope.
pub const SCHEMA: &str = "semaprax.compact-semantic-projection.v1";
/// Wire format version this build reads and writes. A decoder refuses any
/// other value rather than guessing at its shape.
pub const FORMAT_VERSION: u32 = 1;

/// Largest selected-view JSON text this module will encode.
pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
/// Largest wire value (text or binary) this module will decode.
pub const MAX_ENCODED_BYTES: usize = 16 * 1024 * 1024;
/// Largest number of distinct dictionary entries one envelope may declare.
pub const MAX_DICTIONARY_ENTRIES: usize = 65_536;
/// Largest byte length of one dictionary entry or one raw body token.
pub const MAX_ENTRY_BYTES: usize = 1024 * 1024;
/// Largest number of body tokens one envelope may declare.
pub const MAX_BODY_TOKENS: usize = 1_048_575;
/// Largest byte length of one header string field (`profile`, `root`,
/// `source_revision`, `source_digest`).
pub const MAX_HEADER_FIELD_BYTES: usize = 4096;

const DIGEST_DOMAIN: &[u8] = b"semaprax.compact-semantic-projection.source-digest.v1\0";
const BINARY_MAGIC: &[u8; 8] = b"SPXCPJ\0\0";
const TEXT_MAGIC_PREFIX: &[u8] = b"SPXCPJv";
/// Delimiter the text wire form uses to mark a `~<index>~` dictionary
/// reference inline in the body stream. JSON grammar never places this byte
/// outside a string literal, so [`encode_bytes`] refuses any raw span that
/// contains it rather than risk an ambiguous substitution.
const BODY_REF_MARKER: u8 = b'~';

fn capacity_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z901", message)
}

fn encode_grammar_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z902", message)
}

fn malformed_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z903", message)
}

fn version_error(found: u32) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z904",
        format!(
            "compact semantic projection format version {found} is unsupported; this build reads \
             and writes only version {FORMAT_VERSION} and refuses to heuristically decode any other"
        ),
    )
}

fn dictionary_order_error() -> Diagnostic {
    Diagnostic::io(
        "SPX-Z905",
        "compact semantic projection dictionary is not in strict canonical (ascending byte) \
         order; this rejects both an out-of-order dictionary and a duplicated entry"
            .to_owned(),
    )
}

fn body_ref_out_of_range_error(index: u32, dictionary_len: usize) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z906",
        format!(
            "compact semantic projection body references dictionary index {index}, which is \
             outside the decoded dictionary's {dictionary_len} entries"
        ),
    )
}

fn digest_mismatch_error() -> Diagnostic {
    Diagnostic::io(
        "SPX-Z907",
        "compact semantic projection reconstructed content does not match its declared source \
         digest; refusing rather than returning silently altered content"
            .to_owned(),
    )
}

fn binding_mismatch_error(field: &str, expected: &str, found: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z908",
        format!(
            "compact semantic projection {field} `{found}` does not match the expected `{expected}`"
        ),
    )
}

fn root_not_found_error(symbol: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-Z909",
        format!("compact semantic projection root `{symbol}` does not resolve to a context root"),
    )
}

/// One selected full semantic view this module knows how to compact-encode.
///
/// Each variant calls exactly one existing, unchanged engine function; this
/// module never re-derives selection, closure, or budget rules.
pub enum ProjectionSource<'a> {
    /// [`crate::graph::agent_context_v2_json`] for one seed symbol.
    AgentContextV2 {
        symbol: &'a str,
        options: &'a AgentContextV2Options,
    },
    /// [`crate::graph::to_json`], the whole resolved program graph.
    FullGraph,
}

/// One body token: either raw bytes carried verbatim, or a reference to one
/// dictionary entry. The index is local compression bookkeeping only.
#[derive(Clone, Debug, Eq, PartialEq)]
enum BodyToken {
    Raw(Vec<u8>),
    Ref(u32),
}

/// A validated, decoded-or-freshly-encoded compact projection envelope.
///
/// Every value of this type -- whether produced by [`encode_profile`] or
/// returned by [`decode_text`]/[`decode_binary`] -- has already been
/// checked: its dictionary is in strict canonical order, every body
/// reference resolves inside it, and (for a decoded value) its
/// reconstructed content matches its declared digest exactly. There is no
/// way to observe a half-validated value through this module's public API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactProjection {
    profile: String,
    root: String,
    source_revision: String,
    source_digest: String,
    dictionary: Vec<Vec<u8>>,
    body: Vec<BodyToken>,
}

impl CompactProjection {
    #[must_use]
    pub fn profile(&self) -> &str {
        &self.profile
    }

    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    #[must_use]
    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }

    #[must_use]
    pub fn dictionary_len(&self) -> usize {
        self.dictionary.len()
    }

    #[must_use]
    pub fn body_len(&self) -> usize {
        self.body.len()
    }

    /// Reconstruct the exact selected-view bytes this envelope carries.
    ///
    /// Never panics: an out-of-range dictionary reference is reported as
    /// [`SPX-Z906`](body_ref_out_of_range_error) rather than indexed
    /// directly. Every [`CompactProjection`] this module hands back from
    /// [`decode_text`]/[`decode_binary`] has already had this called
    /// successfully once (as part of digest verification), so a second
    /// call against the same value cannot newly fail; it is exposed again
    /// here so a caller can obtain the reconstructed bytes without
    /// re-decoding.
    pub fn reconstructed(&self) -> Result<Vec<u8>, Diagnostic> {
        let mut out = Vec::new();
        for token in &self.body {
            match token {
                BodyToken::Raw(bytes) => out.extend_from_slice(bytes),
                BodyToken::Ref(index) => {
                    let entry = self.dictionary.get(*index as usize).ok_or_else(|| {
                        body_ref_out_of_range_error(*index, self.dictionary.len())
                    })?;
                    out.extend_from_slice(entry);
                }
            }
        }
        Ok(out)
    }

    /// Encode this envelope as the compact textual wire form.
    ///
    /// The dictionary section is one entry per line: a JSON string literal
    /// (this module's own dictionary content) can never contain a raw
    /// newline byte per JSON's own string grammar, so no per-entry length
    /// prefix is needed to delimit entries safely. The body section
    /// substitutes each dictionary reference inline as `~<index>~` and
    /// otherwise emits raw bytes verbatim with no per-span framing at all;
    /// [`encode_bytes`] refuses (at encode time) any source whose raw
    /// (outside-string) content contains the `~` marker byte, so this
    /// substitution is unambiguous by construction rather than by
    /// assumption. This is why the text form is materially smaller than a
    /// length-framed encoding of the same content: real JSON has many short
    /// gaps between string literals (`,`, `:`, `{`), and a fixed per-token
    /// framing cost would otherwise dominate exactly those gaps.
    pub fn to_text(&self) -> String {
        let mut out = Vec::new();
        out.extend_from_slice(TEXT_MAGIC_PREFIX);
        out.extend_from_slice(FORMAT_VERSION.to_string().as_bytes());
        out.push(b'\n');
        write_text_field(&mut out, b"profile", self.profile.as_bytes());
        write_text_field(&mut out, b"root", self.root.as_bytes());
        write_text_field(
            &mut out,
            b"source_revision",
            self.source_revision.as_bytes(),
        );
        write_text_field(&mut out, b"source_digest", self.source_digest.as_bytes());
        out.extend_from_slice(b"dict ");
        out.extend_from_slice(self.dictionary.len().to_string().as_bytes());
        out.push(b'\n');
        for entry in &self.dictionary {
            debug_assert!(
                !entry.contains(&b'\n'),
                "a JSON string literal cannot contain a raw newline byte"
            );
            out.extend_from_slice(entry);
            out.push(b'\n');
        }
        out.extend_from_slice(b"body\n");
        for token in &self.body {
            match token {
                BodyToken::Raw(bytes) => {
                    debug_assert!(
                        !bytes.contains(&BODY_REF_MARKER),
                        "encode_bytes refuses raw content containing the body reference marker"
                    );
                    out.extend_from_slice(bytes);
                }
                BodyToken::Ref(index) => {
                    out.push(BODY_REF_MARKER);
                    out.extend_from_slice(index.to_string().as_bytes());
                    out.push(BODY_REF_MARKER);
                }
            }
        }
        // Every byte written above is ASCII header punctuation, ASCII
        // decimal digits, or bytes copied verbatim from a `String`/`Vec<u8>`
        // that this module itself only ever populates from valid UTF-8
        // (`profile`/`root`/`source_revision`/`source_digest` are `String`;
        // dictionary/body bytes are always a slice of an original selected
        // view, which is UTF-8 JSON text). The whole buffer is valid UTF-8.
        String::from_utf8(out).expect("compact projection text encoding is valid UTF-8")
    }

    /// Encode this envelope as the fixed-width length-framed binary wire
    /// form.
    pub fn to_binary(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(BINARY_MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        write_binary_field(&mut out, self.profile.as_bytes());
        write_binary_field(&mut out, self.root.as_bytes());
        write_binary_field(&mut out, self.source_revision.as_bytes());
        write_binary_field(&mut out, self.source_digest.as_bytes());
        out.extend_from_slice(&(self.dictionary.len() as u32).to_le_bytes());
        for entry in &self.dictionary {
            write_binary_field(&mut out, entry);
        }
        out.extend_from_slice(&(self.body.len() as u32).to_le_bytes());
        for token in &self.body {
            match token {
                BodyToken::Raw(bytes) => {
                    out.push(0);
                    write_binary_field(&mut out, bytes);
                }
                BodyToken::Ref(index) => {
                    out.push(1);
                    out.extend_from_slice(&index.to_le_bytes());
                }
            }
        }
        out
    }
}

fn write_text_field(out: &mut Vec<u8>, name: &[u8], value: &[u8]) {
    out.extend_from_slice(name);
    out.push(b' ');
    out.extend_from_slice(value.len().to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(value);
    out.push(b'\n');
}

fn write_binary_field(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value);
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!("sha256:{:x}", LowerHex(hasher.finalize()))
}

enum Segment<'a> {
    Raw(&'a [u8]),
    /// Includes the surrounding `"` bytes.
    Literal(&'a [u8]),
}

/// Split JSON text into alternating raw and quoted-string-literal spans.
///
/// This is not a JSON parser: it does not validate JSON grammar outside
/// string boundaries. It only needs to find exactly where each string
/// literal starts and ends (respecting backslash escapes) so that literal
/// can become one dictionary entry.
fn tokenize_json_literals(bytes: &[u8]) -> Result<Vec<Segment<'_>>, Diagnostic> {
    let mut segments = Vec::new();
    let mut index = 0usize;
    let mut raw_start = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            if index > raw_start {
                segments.push(Segment::Raw(&bytes[raw_start..index]));
            }
            let literal_start = index;
            index += 1;
            loop {
                if index >= bytes.len() {
                    return Err(encode_grammar_error(
                        "compact projection source ends inside an unterminated string literal"
                            .to_owned(),
                    ));
                }
                match bytes[index] {
                    b'\\' => index += 2,
                    b'"' => {
                        index += 1;
                        break;
                    }
                    _ => index += 1,
                }
            }
            segments.push(Segment::Literal(&bytes[literal_start..index]));
            raw_start = index;
        } else {
            index += 1;
        }
    }
    if raw_start < bytes.len() {
        segments.push(Segment::Raw(&bytes[raw_start..]));
    }
    Ok(segments)
}

/// Encode arbitrary selected-view JSON bytes under an explicit
/// `profile`/`root`/`source_revision` binding.
///
/// This is the core building block [`encode_profile`] uses once it has
/// called the real engine; it is exposed directly so this module's own
/// tests can exercise the dictionary/body encoder against synthetic byte
/// strings, including malformed ones, without needing a full program parse
/// for every case.
pub fn encode_bytes(
    source_json: &[u8],
    profile: &str,
    root: &str,
    source_revision: &str,
) -> Result<CompactProjection, Diagnostic> {
    if source_json.len() > MAX_SOURCE_BYTES {
        return Err(capacity_error(format!(
            "compact projection source is {} bytes, exceeding {MAX_SOURCE_BYTES}",
            source_json.len()
        )));
    }
    for (field, value) in [
        ("profile", profile),
        ("root", root),
        ("source_revision", source_revision),
    ] {
        if value.len() > MAX_HEADER_FIELD_BYTES {
            return Err(capacity_error(format!(
                "compact projection {field} is {} bytes, exceeding {MAX_HEADER_FIELD_BYTES}",
                value.len()
            )));
        }
    }

    let segments = tokenize_json_literals(source_json)?;

    let mut distinct: BTreeSet<&[u8]> = BTreeSet::new();
    for segment in &segments {
        if let Segment::Literal(literal) = segment {
            distinct.insert(literal);
        }
    }
    if distinct.len() > MAX_DICTIONARY_ENTRIES {
        return Err(capacity_error(format!(
            "compact projection has {} distinct dictionary entries, exceeding {MAX_DICTIONARY_ENTRIES}",
            distinct.len()
        )));
    }
    for literal in &distinct {
        if literal.len() > MAX_ENTRY_BYTES {
            return Err(capacity_error(format!(
                "compact projection dictionary entry is {} bytes, exceeding {MAX_ENTRY_BYTES}",
                literal.len()
            )));
        }
    }
    let dictionary: Vec<Vec<u8>> = distinct.iter().map(|literal| literal.to_vec()).collect();

    let mut body = Vec::new();
    for segment in segments {
        match segment {
            Segment::Raw(bytes) => {
                if bytes.is_empty() {
                    continue;
                }
                if bytes.len() > MAX_ENTRY_BYTES {
                    return Err(capacity_error(format!(
                        "compact projection raw body span is {} bytes, exceeding {MAX_ENTRY_BYTES}",
                        bytes.len()
                    )));
                }
                if bytes.contains(&BODY_REF_MARKER) {
                    // Never expected from real JSON (the marker only ever
                    // appears inside a string literal there, which is
                    // already routed to the dictionary, never through this
                    // arm), but checked explicitly so a future profile
                    // source with different raw-span content fails closed
                    // here rather than producing an ambiguous text form.
                    return Err(encode_grammar_error(format!(
                        "compact projection raw body span contains the text form's reserved `{}` \
                         marker byte",
                        BODY_REF_MARKER as char
                    )));
                }
                body.push(BodyToken::Raw(bytes.to_vec()));
            }
            Segment::Literal(literal) => {
                let index = dictionary
                    .binary_search_by(|probe| probe.as_slice().cmp(literal))
                    .expect("every literal was inserted into the dictionary it is searched in");
                body.push(BodyToken::Ref(index as u32));
            }
        }
    }
    if body.len() > MAX_BODY_TOKENS {
        return Err(capacity_error(format!(
            "compact projection has {} body tokens, exceeding {MAX_BODY_TOKENS}",
            body.len()
        )));
    }

    let source_digest = digest_bytes(source_json);
    Ok(CompactProjection {
        profile: profile.to_owned(),
        root: root.to_owned(),
        source_revision: source_revision.to_owned(),
        source_digest,
        dictionary,
        body,
    })
}

/// Compile one [`ProjectionSource`] and compact-encode its exact JSON bytes.
///
/// Calls the named existing engine function unchanged, then delegates to
/// [`encode_bytes`]. Never opens a file, spawns a process, or contacts a
/// network; takes only an already-parsed `&Program`.
pub fn encode_profile(
    program: &Program,
    source: ProjectionSource<'_>,
) -> Result<CompactProjection, Vec<Diagnostic>> {
    let (profile, root, json) = match source {
        ProjectionSource::AgentContextV2 { symbol, options } => {
            let json = graph::agent_context_v2_json(program, symbol, options)?
                .ok_or_else(|| vec![root_not_found_error(symbol)])?;
            ("agent-context-v2".to_owned(), symbol.to_owned(), json)
        }
        ProjectionSource::FullGraph => {
            let json = graph::to_json(program)?;
            ("full-graph".to_owned(), "*".to_owned(), json)
        }
    };
    let source_revision = graph::revision(program);
    encode_bytes(json.as_bytes(), &profile, &root, &source_revision)
        .map_err(|diagnostic| vec![diagnostic])
}

fn validate_dictionary_order(dictionary: &[Vec<u8>]) -> Result<(), Diagnostic> {
    for window in dictionary.windows(2) {
        if window[0] >= window[1] {
            return Err(dictionary_order_error());
        }
    }
    Ok(())
}

/// Shared validation and integrity check for a value parsed off either wire
/// form: canonical dictionary order, then reconstruction (which itself
/// bounds-checks every body reference), then a digest comparison against
/// what the envelope declares.
fn finish_decode(projection: CompactProjection) -> Result<CompactProjection, Diagnostic> {
    validate_dictionary_order(&projection.dictionary)?;
    let reconstructed = projection.reconstructed()?;
    let actual_digest = digest_bytes(&reconstructed);
    if actual_digest != projection.source_digest {
        return Err(digest_mismatch_error());
    }
    Ok(projection)
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], Diagnostic> {
        if count > self.remaining() {
            return Err(malformed_error(
                "compact semantic projection wire is truncated".to_owned(),
            ));
        }
        let slice = &self.bytes[self.offset..self.offset + count];
        self.offset += count;
        Ok(slice)
    }

    fn peek_byte(&self) -> Result<u8, Diagnostic> {
        self.bytes.get(self.offset).copied().ok_or_else(|| {
            malformed_error("compact semantic projection wire is truncated".to_owned())
        })
    }

    fn expect_bytes(&mut self, literal: &[u8]) -> Result<(), Diagnostic> {
        let found = self.take(literal.len())?;
        if found != literal {
            return Err(malformed_error(
                "compact semantic projection wire does not start with its expected magic header"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn finish(self) -> Result<(), Diagnostic> {
        if self.remaining() != 0 {
            return Err(malformed_error(
                "compact semantic projection wire has trailing bytes after its declared content"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

// --- Binary wire form ------------------------------------------------------

impl ByteCursor<'_> {
    fn read_u8(&mut self) -> Result<u8, Diagnostic> {
        Ok(self.take(1)?[0])
    }

    fn read_u32(&mut self) -> Result<u32, Diagnostic> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_binary_field(&mut self, max: usize) -> Result<&[u8], Diagnostic> {
        let len = self.read_u32()? as usize;
        if len > max {
            return Err(capacity_error(format!(
                "compact semantic projection wire field is {len} bytes, exceeding {max}"
            )));
        }
        self.take(len)
    }
}

fn utf8_field(bytes: &[u8], field: &str) -> Result<String, Diagnostic> {
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        malformed_error(format!(
            "compact semantic projection {field} field is not valid UTF-8"
        ))
    })
}

/// Decode the fixed-width binary wire form written by
/// [`CompactProjection::to_binary`].
///
/// Every declared count or length is checked against its policy bound (and,
/// for byte spans, against the bytes actually remaining) before it is used
/// to slice or allocate, so a hostile huge count backed by a short buffer is
/// refused rather than attempted. See the module docs for the full
/// integrity check this runs after parsing.
pub fn decode_binary(bytes: &[u8]) -> Result<CompactProjection, Diagnostic> {
    if bytes.len() > MAX_ENCODED_BYTES {
        return Err(capacity_error(format!(
            "compact semantic projection wire is {} bytes, exceeding {MAX_ENCODED_BYTES}",
            bytes.len()
        )));
    }
    let mut cursor = ByteCursor::new(bytes);
    cursor.expect_bytes(BINARY_MAGIC)?;
    let version = cursor.read_u32()?;
    if version != FORMAT_VERSION {
        return Err(version_error(version));
    }
    let profile = utf8_field(cursor.read_binary_field(MAX_HEADER_FIELD_BYTES)?, "profile")?;
    let root = utf8_field(cursor.read_binary_field(MAX_HEADER_FIELD_BYTES)?, "root")?;
    let source_revision = utf8_field(
        cursor.read_binary_field(MAX_HEADER_FIELD_BYTES)?,
        "source_revision",
    )?;
    let source_digest = utf8_field(
        cursor.read_binary_field(MAX_HEADER_FIELD_BYTES)?,
        "source_digest",
    )?;

    let dictionary_count = cursor.read_u32()? as usize;
    if dictionary_count > MAX_DICTIONARY_ENTRIES {
        return Err(capacity_error(format!(
            "compact semantic projection declares {dictionary_count} dictionary entries, exceeding \
             {MAX_DICTIONARY_ENTRIES}"
        )));
    }
    let mut dictionary = Vec::with_capacity(dictionary_count);
    for _ in 0..dictionary_count {
        dictionary.push(cursor.read_binary_field(MAX_ENTRY_BYTES)?.to_vec());
    }

    let body_count = cursor.read_u32()? as usize;
    if body_count > MAX_BODY_TOKENS {
        return Err(capacity_error(format!(
            "compact semantic projection declares {body_count} body tokens, exceeding {MAX_BODY_TOKENS}"
        )));
    }
    let mut body = Vec::with_capacity(body_count);
    for _ in 0..body_count {
        match cursor.read_u8()? {
            0 => body.push(BodyToken::Raw(
                cursor.read_binary_field(MAX_ENTRY_BYTES)?.to_vec(),
            )),
            1 => body.push(BodyToken::Ref(cursor.read_u32()?)),
            other => {
                return Err(malformed_error(format!(
                    "compact semantic projection body token tag {other} is unrecognized"
                )))
            }
        }
    }
    cursor.finish()?;

    finish_decode(CompactProjection {
        profile,
        root,
        source_revision,
        source_digest,
        dictionary,
        body,
    })
}

// --- Text wire form ---------------------------------------------------------

impl ByteCursor<'_> {
    /// Read ASCII decimal digits up to (not including) `terminator`, then
    /// consume the terminator. Refuses more than `max_digits` digits before
    /// ever attempting to parse, so an absurdly long digit run is rejected
    /// cheaply rather than building an enormous string first.
    fn read_decimal(&mut self, terminator: u8, max_digits: usize) -> Result<usize, Diagnostic> {
        let start = self.offset;
        loop {
            let byte = self.peek_byte()?;
            if byte == terminator {
                break;
            }
            if !byte.is_ascii_digit() {
                return Err(malformed_error(
                    "compact semantic projection text wire expected an ASCII decimal digit"
                        .to_owned(),
                ));
            }
            if self.offset - start >= max_digits {
                return Err(malformed_error(
                    "compact semantic projection text wire has an unreasonably long decimal field"
                        .to_owned(),
                ));
            }
            self.offset += 1;
        }
        let digits = &self.bytes[start..self.offset];
        self.offset += 1; // consume terminator
        if digits.is_empty() {
            return Err(malformed_error(
                "compact semantic projection text wire has an empty decimal field".to_owned(),
            ));
        }
        let text = std::str::from_utf8(digits)
            .expect("ASCII digits are valid UTF-8")
            .parse::<usize>()
            .map_err(|_| {
                malformed_error(
                    "compact semantic projection text wire decimal field does not fit usize"
                        .to_owned(),
                )
            })?;
        Ok(text)
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), Diagnostic> {
        self.expect_bytes(&[expected])
    }

    /// Read one `<name> <len> <bytes>\n` header line.
    fn read_text_field(&mut self, name: &[u8], max: usize) -> Result<&[u8], Diagnostic> {
        self.expect_bytes(name)?;
        self.expect_byte(b' ')?;
        let len = self.read_decimal(b' ', 10)?;
        if len > max {
            return Err(capacity_error(format!(
                "compact semantic projection text wire field is {len} bytes, exceeding {max}"
            )));
        }
        let bytes = self.take(len)?;
        self.expect_byte(b'\n')?;
        Ok(bytes)
    }

    /// Read one dictionary entry line (no length prefix: a JSON string
    /// literal is guaranteed newline-free), bounded so a hostile
    /// unterminated line is refused rather than scanned without limit.
    fn read_dictionary_line(&mut self, max: usize) -> Result<&[u8], Diagnostic> {
        let start = self.offset;
        loop {
            if self.offset - start > max {
                return Err(capacity_error(format!(
                    "compact semantic projection dictionary entry exceeds {max} bytes"
                )));
            }
            if self.peek_byte()? == b'\n' {
                break;
            }
            self.offset += 1;
        }
        let line = &self.bytes[start..self.offset];
        self.offset += 1; // consume newline
        Ok(line)
    }

    /// Consume and return every remaining byte.
    fn take_remaining(&mut self) -> &[u8] {
        let slice = &self.bytes[self.offset..];
        self.offset = self.bytes.len();
        slice
    }
}

/// Parse the text form's body stream: raw bytes verbatim, interrupted by
/// `~<index>~` dictionary references. Bounds the number of tokens produced
/// and the length of any one raw span while scanning, so a pathological
/// input (for example, millions of tiny `~0~` references) is refused
/// instead of building an unbounded `Vec`.
fn parse_body_stream(bytes: &[u8]) -> Result<Vec<BodyToken>, Diagnostic> {
    let mut body = Vec::new();
    let push_token = |body: &mut Vec<BodyToken>, token: BodyToken| -> Result<(), Diagnostic> {
        body.push(token);
        if body.len() > MAX_BODY_TOKENS {
            return Err(capacity_error(format!(
                "compact semantic projection body exceeds {MAX_BODY_TOKENS} tokens"
            )));
        }
        Ok(())
    };
    let mut index = 0usize;
    let mut raw_start = 0usize;
    while index < bytes.len() {
        if bytes[index] != BODY_REF_MARKER {
            index += 1;
            continue;
        }
        if index > raw_start {
            let raw = &bytes[raw_start..index];
            if raw.len() > MAX_ENTRY_BYTES {
                return Err(capacity_error(format!(
                    "compact semantic projection raw body span is {} bytes, exceeding {MAX_ENTRY_BYTES}",
                    raw.len()
                )));
            }
            push_token(&mut body, BodyToken::Raw(raw.to_vec()))?;
        }
        let digits_start = index + 1;
        let mut cursor = digits_start;
        while cursor < bytes.len() && bytes[cursor] != BODY_REF_MARKER {
            if !bytes[cursor].is_ascii_digit() {
                return Err(malformed_error(
                    "compact semantic projection text body reference is not decimal".to_owned(),
                ));
            }
            cursor += 1;
            if cursor - digits_start > 10 {
                return Err(malformed_error(
                    "compact semantic projection text body reference has too many digits"
                        .to_owned(),
                ));
            }
        }
        if cursor >= bytes.len() {
            return Err(malformed_error(
                "compact semantic projection text body reference marker is unterminated".to_owned(),
            ));
        }
        if cursor == digits_start {
            return Err(malformed_error(
                "compact semantic projection text body reference is empty".to_owned(),
            ));
        }
        let digits = std::str::from_utf8(&bytes[digits_start..cursor])
            .expect("ASCII digits are valid UTF-8");
        let value: u64 = digits.parse().map_err(|_| {
            malformed_error(
                "compact semantic projection text body reference does not fit u32".to_owned(),
            )
        })?;
        if value > u64::from(u32::MAX) {
            return Err(malformed_error(
                "compact semantic projection text body reference does not fit u32".to_owned(),
            ));
        }
        push_token(&mut body, BodyToken::Ref(value as u32))?;
        index = cursor + 1;
        raw_start = index;
    }
    if raw_start < bytes.len() {
        let raw = &bytes[raw_start..];
        if raw.len() > MAX_ENTRY_BYTES {
            return Err(capacity_error(format!(
                "compact semantic projection raw body span is {} bytes, exceeding {MAX_ENTRY_BYTES}",
                raw.len()
            )));
        }
        push_token(&mut body, BodyToken::Raw(raw.to_vec()))?;
    }
    Ok(body)
}

/// Decode the textual wire form written by
/// [`CompactProjection::to_text`].
///
/// Applies exactly the same policy bounds, index-range check, and
/// post-decode digest verification as [`decode_binary`]; the two share
/// [`finish_decode`], so neither form can drift from the other's integrity
/// guarantee.
pub fn decode_text(text: &str) -> Result<CompactProjection, Diagnostic> {
    let bytes = text.as_bytes();
    if bytes.len() > MAX_ENCODED_BYTES {
        return Err(capacity_error(format!(
            "compact semantic projection wire is {} bytes, exceeding {MAX_ENCODED_BYTES}",
            bytes.len()
        )));
    }
    let mut cursor = ByteCursor::new(bytes);
    cursor.expect_bytes(TEXT_MAGIC_PREFIX)?;
    let version = cursor.read_decimal(b'\n', 10)? as u64;
    if version != u64::from(FORMAT_VERSION) {
        return Err(version_error(version as u32));
    }

    let profile = utf8_field(
        cursor.read_text_field(b"profile", MAX_HEADER_FIELD_BYTES)?,
        "profile",
    )?;
    let root = utf8_field(
        cursor.read_text_field(b"root", MAX_HEADER_FIELD_BYTES)?,
        "root",
    )?;
    let source_revision = utf8_field(
        cursor.read_text_field(b"source_revision", MAX_HEADER_FIELD_BYTES)?,
        "source_revision",
    )?;
    let source_digest = utf8_field(
        cursor.read_text_field(b"source_digest", MAX_HEADER_FIELD_BYTES)?,
        "source_digest",
    )?;

    cursor.expect_bytes(b"dict ")?;
    let dictionary_count = cursor.read_decimal(b'\n', 10)?;
    if dictionary_count > MAX_DICTIONARY_ENTRIES {
        return Err(capacity_error(format!(
            "compact semantic projection declares {dictionary_count} dictionary entries, exceeding \
             {MAX_DICTIONARY_ENTRIES}"
        )));
    }
    let mut dictionary = Vec::with_capacity(dictionary_count);
    for _ in 0..dictionary_count {
        dictionary.push(cursor.read_dictionary_line(MAX_ENTRY_BYTES)?.to_vec());
    }

    cursor.expect_bytes(b"body\n")?;
    let body = parse_body_stream(cursor.take_remaining())?;

    finish_decode(CompactProjection {
        profile,
        root,
        source_revision,
        source_digest,
        dictionary,
        body,
    })
}

/// Decode and additionally bind the result to an exact expected
/// `profile`/`root`/`source_revision`, refusing a structurally valid but
/// differently bound envelope rather than letting a caller mistake one
/// selected view for another.
pub fn decode_binary_and_verify(
    bytes: &[u8],
    expected_profile: &str,
    expected_root: &str,
    expected_source_revision: &str,
) -> Result<CompactProjection, Diagnostic> {
    verify_binding(
        decode_binary(bytes)?,
        expected_profile,
        expected_root,
        expected_source_revision,
    )
}

/// Text-form counterpart of [`decode_binary_and_verify`].
pub fn decode_text_and_verify(
    text: &str,
    expected_profile: &str,
    expected_root: &str,
    expected_source_revision: &str,
) -> Result<CompactProjection, Diagnostic> {
    verify_binding(
        decode_text(text)?,
        expected_profile,
        expected_root,
        expected_source_revision,
    )
}

fn verify_binding(
    projection: CompactProjection,
    expected_profile: &str,
    expected_root: &str,
    expected_source_revision: &str,
) -> Result<CompactProjection, Diagnostic> {
    if projection.profile != expected_profile {
        return Err(binding_mismatch_error(
            "profile",
            expected_profile,
            &projection.profile,
        ));
    }
    if projection.root != expected_root {
        return Err(binding_mismatch_error(
            "root",
            expected_root,
            &projection.root,
        ));
    }
    if projection.source_revision != expected_source_revision {
        return Err(binding_mismatch_error(
            "source_revision",
            expected_source_revision,
            &projection.source_revision,
        ));
    }
    Ok(projection)
}

#[cfg(test)]
mod tests;
