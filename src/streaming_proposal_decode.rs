//! Streaming structured-output decoding derived from the checked Proposal
//! type ([`docs/STREAMING-PROPOSAL-DECODE-V1.md`](../docs/STREAMING-PROPOSAL-DECODE-V1.md)).
//!
//! Issue #178 asks for a decoder that turns a provider's response bytes,
//! delivered in arbitrarily small chunks, into either one fully typed
//! Proposal or one closed refusal — without ever buffering an unbounded
//! amount of untrusted data first, and without ever mistaking an
//! unterminated prefix for a complete value.
//!
//! This module adds **no** admission rule of its own for what a Proposal
//! document may contain: every semantic acceptance or refusal (unknown or
//! missing fields, wrong variant tags, integer bounds, canonical-rendering
//! equality) is the exact same call to
//! [`agent_interaction_schema::CompiledInteractionSchema::decode`] the
//! whole-document path already uses and already tests. This module only
//! adds the streaming-safety layer around that call: a bounded byte buffer,
//! an incremental UTF-8/JSON-structural scanner that can refuse a
//! malformed or oversized prefix *before* the document completes, and the
//! exact single-newline framing the compiled decoder's canonical documents
//! use. Because the terminal "typed Proposal" case is produced by literally
//! invoking the same function the whole-document caller would invoke, the
//! two paths cannot disagree on what they accept — there is no second,
//! hand-written grammar to drift out of sync with the checked type.
//!
//! `src/live_invocation/` and `src/agent_interaction_schema/` are read-only
//! from this module's perspective: it depends only on the latter's already
//! public [`agent_interaction_schema::CompiledInteractionSchema`] surface
//! and introduces no new source syntax, HIR node, or wire format of its own
//! beyond the existing `semaprax.agent-interaction-value.v1` document this
//! module streams.

use crate::agent_interaction_schema::{CompiledInteractionSchema, DecodedInteractionValue};

/// The maximum number of bytes this decoder will buffer before refusing,
/// independent of whether the document ever completes. Matches the whole-
/// document decoder's own `MAX_DOCUMENT_BYTES` (65536): a streamed document
/// can never legally exceed what the whole-document path accepts, so this
/// bound never refuses anything the compiled decoder would have admitted.
pub const MAX_STREAM_BYTES: usize = 65_536;

/// The maximum admitted JSON container nesting depth (`{`/`[` opens without
/// a matching close). Deliberately generous relative to the compiled
/// schema's own `MAX_DEPTH` (16 semantic levels, each of which costs two
/// JSON object opens plus the top-level envelope): 64 never refuses a
/// document the compiled schema could ever produce, while still refusing a
/// stream that never stops opening containers.
pub const MAX_STREAM_DEPTH: usize = 64;

/// The maximum number of JSON string-literal tokens (field keys, text
/// values, variant tags) admitted across one streamed document. A proxy
/// bound for the "field/case" incremental limit the streaming decoder must
/// enforce without needing its own copy of the compiled type graph: every
/// legal document is already bounded by [`MAX_STREAM_BYTES`], and a minimal
/// string token costs at least two bytes (`""`), so this bound (well under
/// half of [`MAX_STREAM_BYTES`]) is reachable only by a pathological
/// all-empty-string stream, never by a realistic Proposal document.
pub const MAX_STREAM_STRING_TOKENS: u32 = 8_192;

/// One closed, stable streaming-refusal reason. `code` is drawn from a
/// fixed, small vocabulary (see the `STREAM-*` constants below); `message`
/// is caller-facing detail; `at_byte` is the absolute buffer offset the
/// violation was detected at (best-effort: the delegated `STREAM-SEMANTIC`
/// case reports the end of the buffered document, since the compiled
/// decoder does not itself report byte spans).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamRefusal {
    pub code: &'static str,
    pub message: String,
    pub at_byte: usize,
}

impl StreamRefusal {
    fn new(code: &'static str, message: impl Into<String>, at_byte: usize) -> Self {
        Self {
            code,
            message: message.into(),
            at_byte,
        }
    }
}

/// The document opened with a byte other than `{`.
pub const STREAM_START: &str = "STREAM-START";
/// A raw whitespace or control byte appeared outside a string, other than
/// the exact single terminating newline.
pub const STREAM_WHITESPACE: &str = "STREAM-WHITESPACE";
/// A string literal contained an unescaped control byte, an unknown escape
/// character, or an invalid `\u` hex digit.
pub const STREAM_STRING: &str = "STREAM-STRING";
/// A closing `}`/`]` appeared with no matching open, or did not match the
/// innermost open container's kind.
pub const STREAM_BRACKET: &str = "STREAM-BRACKET";
/// The buffered bytes are not valid UTF-8.
pub const STREAM_UTF8: &str = "STREAM-UTF8";
/// Container nesting exceeded [`MAX_STREAM_DEPTH`].
pub const STREAM_DEPTH: &str = "STREAM-DEPTH";
/// String-literal token count exceeded [`MAX_STREAM_STRING_TOKENS`].
pub const STREAM_TOKENS: &str = "STREAM-TOKENS";
/// Buffered byte count exceeded [`MAX_STREAM_BYTES`].
pub const STREAM_BYTES: &str = "STREAM-BYTES";
/// A byte arrived after the document's terminal newline, or the top-level
/// value closed without being followed by exactly one newline.
pub const STREAM_TRAILING: &str = "STREAM-TRAILING";
/// The caller declared end of input before the document reached its
/// terminal newline.
pub const STREAM_TRUNCATED: &str = "STREAM-TRUNCATED";
/// The caller explicitly cancelled the stream.
pub const STREAM_CANCELLED: &str = "STREAM-CANCELLED";
/// The complete, syntactically well-formed document was rejected by
/// [`CompiledInteractionSchema::decode`] itself. `message` carries that
/// diagnostic's own stable code and text, so a caller sees exactly which
/// admission rule failed, never a generic "malformed" tag.
pub const STREAM_SEMANTIC: &str = "STREAM-SEMANTIC";

/// The result of feeding one chunk (or of [`ProposalStreamDecoder::cancel`]
/// / [`ProposalStreamDecoder::finish`]).
///
/// `Incomplete` and `Refused` are always distinct: a prefix that has not
/// yet violated any rule and has not yet completed is `Incomplete` and
/// carries no code or message at all, so it can never be confused with a
/// genuine refusal by inspecting its text. [`ProposalStreamDecoder::push`]
/// alone never returns `Accepted`: seeing a document's terminal newline
/// means only that the bytes seen *so far* are complete, not that the
/// provider has stopped sending — more bytes in a later chunk would make
/// them trailing garbage. Only [`ProposalStreamDecoder::finish`], the
/// caller's explicit "no more bytes are coming" signal, can produce
/// `Accepted`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PushOutcome {
    /// Valid so far; more bytes are required before a decision is possible.
    Incomplete,
    /// The stream completed and decoded against the bound schema.
    Accepted(DecodedInteractionValue),
    /// The stream is closed: refused for the given stable reason.
    Refused(StreamRefusal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TerminalResult {
    Accepted(DecodedInteractionValue),
    Refused(StreamRefusal),
}

fn terminal_to_outcome(terminal: &TerminalResult) -> PushOutcome {
    match terminal {
        TerminalResult::Accepted(value) => PushOutcome::Accepted(value.clone()),
        TerminalResult::Refused(refusal) => PushOutcome::Refused(refusal.clone()),
    }
}

/// Incremental UTF-8-text-level JSON structural scanner.
///
/// This is not a JSON parser: it does not validate object key/value/comma
/// grammar, number lexical form, or `true`/`false` spelling. It validates
/// only what it needs to bound work and detect the document's terminal
/// byte without buffering unboundedly: container nesting (matched
/// open/close pairs, depth-bounded), string-literal well-formedness
/// (quote/escape/`\u`-hex matching, token-count-bounded), the absence of
/// raw whitespace/control bytes outside strings, and the exact
/// single-trailing-newline framing every canonical
/// `semaprax.agent-interaction-value.v1` document uses. Every rule here is
/// a strict subset of what [`CompiledInteractionSchema::decode`] already
/// requires of a complete document, so this scanner can reject a
/// document *earlier* than the whole-document path would, but it can never
/// accept (i.e. reach the terminal decode call with) a document shape the
/// checked type forbids: full semantic legality is always decided by that
/// exact same call, never by this scanner.
#[derive(Clone, Debug, Default)]
struct Scanner {
    stack: Vec<u8>,
    in_string: bool,
    escaping: bool,
    unicode_remaining: u8,
    started: bool,
    awaiting_terminal_lf: bool,
    terminated: bool,
    string_tokens: u32,
}

impl Scanner {
    fn step(&mut self, byte: u8, at: usize) -> Result<(), StreamRefusal> {
        if self.terminated {
            return Err(StreamRefusal::new(
                STREAM_TRAILING,
                "data followed the document's terminal newline",
                at,
            ));
        }
        if self.awaiting_terminal_lf {
            return if byte == b'\n' {
                self.terminated = true;
                Ok(())
            } else {
                Err(StreamRefusal::new(
                    STREAM_TRAILING,
                    "the top-level value closed without being followed by a terminating newline",
                    at,
                ))
            };
        }
        if !self.started {
            self.started = true;
            if byte != b'{' {
                return Err(StreamRefusal::new(
                    STREAM_START,
                    "a streamed Proposal document must open with '{'",
                    at,
                ));
            }
            self.stack.push(b'}');
            return Ok(());
        }
        if self.in_string {
            return self.step_in_string(byte, at);
        }
        match byte {
            b'"' => {
                self.in_string = true;
                self.string_tokens += 1;
                if self.string_tokens > MAX_STREAM_STRING_TOKENS {
                    return Err(StreamRefusal::new(
                        STREAM_TOKENS,
                        format!(
                            "streamed document exceeded {MAX_STREAM_STRING_TOKENS} string-literal tokens"
                        ),
                        at,
                    ));
                }
            }
            b'{' | b'[' => {
                self.stack.push(if byte == b'{' { b'}' } else { b']' });
                if self.stack.len() > MAX_STREAM_DEPTH {
                    return Err(StreamRefusal::new(
                        STREAM_DEPTH,
                        format!("streamed document exceeded {MAX_STREAM_DEPTH} levels of nesting"),
                        at,
                    ));
                }
            }
            b'}' | b']' => match self.stack.pop() {
                Some(expected) if expected == byte => {
                    if self.stack.is_empty() {
                        self.awaiting_terminal_lf = true;
                    }
                }
                _ => {
                    return Err(StreamRefusal::new(
                        STREAM_BRACKET,
                        "unexpected or mismatched closing bracket",
                        at,
                    ));
                }
            },
            0x00..=0x20 => {
                return Err(StreamRefusal::new(
                    STREAM_WHITESPACE,
                    "a canonical document carries no raw whitespace or control byte outside a string, other than its single terminal newline",
                    at,
                ));
            }
            _ => {}
        }
        Ok(())
    }

    fn step_in_string(&mut self, byte: u8, at: usize) -> Result<(), StreamRefusal> {
        if self.unicode_remaining > 0 {
            if !byte.is_ascii_hexdigit() {
                return Err(StreamRefusal::new(
                    STREAM_STRING,
                    "invalid hex digit in a \\u escape",
                    at,
                ));
            }
            self.unicode_remaining -= 1;
            return Ok(());
        }
        if self.escaping {
            self.escaping = false;
            return match byte {
                b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => Ok(()),
                b'u' => {
                    self.unicode_remaining = 4;
                    Ok(())
                }
                _ => Err(StreamRefusal::new(
                    STREAM_STRING,
                    "unknown escape character",
                    at,
                )),
            };
        }
        match byte {
            b'"' => self.in_string = false,
            b'\\' => self.escaping = true,
            0x00..=0x1F => {
                return Err(StreamRefusal::new(
                    STREAM_STRING,
                    "raw control byte inside a string literal",
                    at,
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

/// A streaming decoder bound to one compiled Proposal grammar.
///
/// Feed provider response bytes to [`push`](Self::push) in any chunking —
/// one byte at a time, all at once, or anything between — then call
/// [`finish`](Self::finish) once no more bytes are coming; the same total
/// byte sequence always produces the same final outcome regardless of how
/// it was chunked. Once a terminal outcome is reached (`Accepted` from
/// `finish`, or any `Refused`), the decoder is closed: further calls return
/// that same stored outcome rather than resuming or silently accepting
/// more input.
pub struct ProposalStreamDecoder<'a> {
    schema: &'a CompiledInteractionSchema,
    buffer: Vec<u8>,
    confirmed_len: usize,
    scan: Scanner,
    terminal: Option<TerminalResult>,
}

impl<'a> ProposalStreamDecoder<'a> {
    #[must_use]
    pub fn new(schema: &'a CompiledInteractionSchema) -> Self {
        Self {
            schema,
            buffer: Vec::new(),
            confirmed_len: 0,
            scan: Scanner::default(),
            terminal: None,
        }
    }

    /// The grammar digest this decoder decodes against, mirroring
    /// `ProposalDecoder::schema_digest`'s meaning for the whole-document
    /// seam so a caller can compare it against a request's expected digest
    /// before streaming a response through this decoder.
    #[must_use]
    pub fn schema_digest(&self) -> &str {
        self.schema.schema().digest()
    }

    /// The number of bytes buffered so far (never more than
    /// [`MAX_STREAM_BYTES`]).
    #[must_use]
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    /// Feeds the next chunk of untrusted provider bytes.
    ///
    /// Never returns `Accepted`: reaching the document's terminal newline
    /// only stops `push` from refusing more input under this document — it
    /// does not yet authorize anything, because a later chunk could still
    /// turn out to carry trailing data after that newline. Call
    /// [`Self::finish`] once the caller knows no more bytes are coming to
    /// get the final `Accepted`/`Refused` decision.
    pub fn push(&mut self, chunk: &[u8]) -> PushOutcome {
        if let Some(terminal) = &self.terminal {
            return terminal_to_outcome(terminal);
        }
        if chunk.is_empty() {
            return PushOutcome::Incomplete;
        }
        if self.buffer.len().saturating_add(chunk.len()) > MAX_STREAM_BYTES {
            return self.finalize(TerminalResult::Refused(StreamRefusal::new(
                STREAM_BYTES,
                format!("streamed document exceeded the {MAX_STREAM_BYTES}-byte bound"),
                self.buffer.len(),
            )));
        }
        self.buffer.extend_from_slice(chunk);

        let tail = &self.buffer[self.confirmed_len..];
        let (confirmed_text, utf8_violation) = match std::str::from_utf8(tail) {
            Ok(text) => (text, false),
            Err(error) => {
                let valid = error.valid_up_to();
                let text =
                    std::str::from_utf8(&tail[..valid]).expect("from_utf8 proved this prefix valid");
                (text, error.error_len().is_some())
            }
        };

        let base = self.confirmed_len;
        for (offset, byte) in confirmed_text.bytes().enumerate() {
            if let Err(refusal) = self.scan.step(byte, base + offset) {
                return self.finalize(TerminalResult::Refused(refusal));
            }
        }
        self.confirmed_len = base + confirmed_text.len();

        if utf8_violation {
            return self.finalize(TerminalResult::Refused(StreamRefusal::new(
                STREAM_UTF8,
                "invalid UTF-8 byte sequence",
                self.confirmed_len,
            )));
        }
        PushOutcome::Incomplete
    }

    /// Explicit cancellation between chunks (e.g. the caller's own deadline
    /// or budget expired). If the stream already reached a terminal
    /// outcome — including a prior successful `Accepted` from
    /// [`Self::finish`] — that outcome is returned unchanged: cancellation
    /// cannot retroactively un-authorize an already-completed decode,
    /// matching this codebase's sticky failure-selection discipline
    /// elsewhere.
    pub fn cancel(&mut self, reason: impl Into<String>) -> PushOutcome {
        if let Some(terminal) = &self.terminal {
            return terminal_to_outcome(terminal);
        }
        self.finalize(TerminalResult::Refused(StreamRefusal::new(
            STREAM_CANCELLED,
            reason.into(),
            self.buffer.len(),
        )))
    }

    /// Declares that no more bytes are coming and produces the final
    /// decision.
    ///
    /// If the document already reached its terminal newline, this decodes
    /// the buffered bytes through the exact same
    /// `CompiledInteractionSchema::decode` call the whole-document path
    /// uses, returning `Accepted` or a delegated `Refused`. Otherwise the
    /// stream is refused as truncated rather than left waiting forever.
    /// Idempotent: once a terminal outcome exists (from this call or an
    /// earlier `push`/`cancel` refusal), further calls return it unchanged.
    pub fn finish(&mut self) -> PushOutcome {
        if let Some(terminal) = &self.terminal {
            return terminal_to_outcome(terminal);
        }
        if self.scan.terminated {
            return self.finalize_from_schema();
        }
        self.finalize(TerminalResult::Refused(StreamRefusal::new(
            STREAM_TRUNCATED,
            "the stream ended before the document reached its terminating newline",
            self.buffer.len(),
        )))
    }

    fn finalize_from_schema(&mut self) -> PushOutcome {
        match self.schema.decode(&self.buffer) {
            Ok(value) => self.finalize(TerminalResult::Accepted(value)),
            Err(diagnostics) => {
                let reason = diagnostics
                    .first()
                    .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                    .unwrap_or_else(|| "decode produced no diagnostic".to_owned());
                self.finalize(TerminalResult::Refused(StreamRefusal::new(
                    STREAM_SEMANTIC,
                    reason,
                    self.buffer.len(),
                )))
            }
        }
    }

    fn finalize(&mut self, terminal: TerminalResult) -> PushOutcome {
        let outcome = terminal_to_outcome(&terminal);
        self.terminal = Some(terminal);
        outcome
    }
}

#[cfg(test)]
mod tests;
