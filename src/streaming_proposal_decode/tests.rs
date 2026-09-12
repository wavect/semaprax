use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::agent_interaction_schema::compile_agent_interaction_schema;
use crate::diagnostic::quote_json;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// One record with a nested record (`Outer`/`Inner`, exercising a
/// multi-byte-UTF-8-bearing text field) and one variant (`Choice`, exercising
/// variant-tag refusal), all in one checked module so both compiled schemas
/// below are derived from one real, checker-verified source file rather
/// than a hand-rolled grammar.
const FIXTURE: &str = r#"
module test.streaming_proposal_decode;

@id("inner.type")
record Inner {
    @id("inner.note")
    note: string,
}

@id("outer.type")
record Outer {
    @id("outer.id")
    id: i64,
    @id("outer.active")
    active: bool,
    @id("outer.inner")
    inner: Inner,
}

@id("choice.type")
variant Choice {
    @id("choice.yes")
    Yes,
    @id("choice.no")
    No,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn write_temp(label: &str) -> PathBuf {
    // Tests run concurrently on multiple threads of the same process, so
    // pid+nanos alone is not guaranteed unique (clock resolution can be
    // coarser than the thread-scheduling interval); an atomic counter makes
    // collisions impossible regardless of timing.
    let unique = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-streaming-proposal-decode-{label}-{}-{}-{unique}.spx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, FIXTURE).unwrap();
    path
}

fn compile_outer() -> CompiledInteractionSchema {
    let path = write_temp("outer");
    let compiled = compile_agent_interaction_schema(&path, "outer.type")
        .expect("outer.type derivation succeeds");
    std::fs::remove_file(&path).ok();
    compiled
}

fn compile_choice() -> CompiledInteractionSchema {
    let path = write_temp("choice");
    let compiled = compile_agent_interaction_schema(&path, "choice.type")
        .expect("choice.type derivation succeeds");
    std::fs::remove_file(&path).ok();
    compiled
}

/// Builds one canonical `Outer` document, in the exact zero-whitespace,
/// declaration-ordered form the compiled decoder's own canonical rendering
/// produces, so a syntactically legal mutation still exercises real
/// semantic admission rather than being rejected first by an accidental
/// formatting slip.
fn outer_document(schema: &CompiledInteractionSchema, id: &str, active: &str, note: &str) -> String {
    let inner = format!("{{\"fields\":{{\"inner.note\":{}}}}}", quote_json(note));
    let value = format!(
        "{{\"fields\":{{\"outer.id\":{},\"outer.active\":{active},\"outer.inner\":{inner}}}}}",
        quote_json(id),
    );
    format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"outer.type\",\"schema_digest\":{},\"value\":{value}}}\n",
        quote_json(schema.schema().digest()),
    )
}

fn choice_document(schema: &CompiledInteractionSchema, case: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"choice.type\",\"schema_digest\":{},\"value\":{{\"case\":{},\"fields\":{{}}}}}}\n",
        quote_json(schema.schema().digest()),
        quote_json(case),
    )
}

/// Drives a decoder through `chunks` in order, then declares end of stream
/// with [`ProposalStreamDecoder::finish`] unless an earlier chunk already
/// produced a terminal `Refused` outcome. `push` alone never yields
/// `Accepted` (see its doc comment): every helper below that expects a
/// possible `Accepted` result must route through this, not call `push`
/// directly and stop.
fn drive(schema: &CompiledInteractionSchema, chunks: &[&[u8]]) -> PushOutcome {
    let mut decoder = ProposalStreamDecoder::new(schema);
    for chunk in chunks {
        let outcome = decoder.push(chunk);
        if !matches!(outcome, PushOutcome::Incomplete) {
            return outcome;
        }
    }
    decoder.finish()
}

fn push_all_at_once(schema: &CompiledInteractionSchema, bytes: &[u8]) -> PushOutcome {
    drive(schema, &[bytes])
}

fn push_one_byte_at_a_time(schema: &CompiledInteractionSchema, bytes: &[u8]) -> PushOutcome {
    let chunks: Vec<&[u8]> = bytes.iter().map(std::slice::from_ref).collect();
    drive(schema, &chunks)
}

fn push_split_at(schema: &CompiledInteractionSchema, bytes: &[u8], at: usize) -> PushOutcome {
    drive(schema, &[&bytes[..at], &bytes[at..]])
}

// ---------------------------------------------------------------------
// Criterion: derived from the checked type, not hand-written — prove
// agreement with `CompiledInteractionSchema::decode` rather than assert it.
// ---------------------------------------------------------------------

/// For every case, the streaming decoder's terminal outcome (fed whole, fed
/// one byte at a time, and fed split down the middle) agrees exactly with
/// whatever the whole-document, compiler-derived decoder decides for the
/// identical bytes: an `Accepted` streaming outcome always carries the same
/// decoded value the whole decoder produced, and the streaming decoder
/// never reaches `Accepted` on bytes the whole decoder refuses. This is the
/// property that makes it impossible for the streaming layer to admit a
/// shape the checked Proposal type forbids: the only admission path is the
/// exact same call.
#[test]
fn streaming_outcome_agrees_with_the_whole_document_decoder_for_every_case() {
    let outer = compile_outer();
    let choice = compile_choice();

    let valid_outer = outer_document(&outer, "7", "true", "hi");
    let oversized = "x".repeat(5_000);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("valid outer document", valid_outer.clone().into_bytes()),
        (
            "unknown field",
            valid_outer
                .replace(
                    "\"outer.active\":true",
                    "\"outer.active\":true,\"outer.bogus\":\"1\"",
                )
                .into_bytes(),
        ),
        (
            "missing field",
            valid_outer
                .replace(",\"outer.active\":true", "")
                .into_bytes(),
        ),
        (
            "duplicate field",
            valid_outer
                .replace(
                    "\"outer.id\":\"7\"",
                    "\"outer.id\":\"7\",\"outer.id\":\"7\"",
                )
                .into_bytes(),
        ),
        (
            "wrong scalar type",
            valid_outer.replace("\"outer.active\":true", "\"outer.active\":\"true\"")
                .into_bytes(),
        ),
        (
            "integer out of range",
            valid_outer
                .replace("\"outer.id\":\"7\"", "\"outer.id\":\"99999999999999999999999999\"")
                .into_bytes(),
        ),
        (
            "oversized text field exceeds the checked bound",
            outer_document(&outer, "7", "true", &oversized).into_bytes(),
        ),
        (
            "trailing data after the terminal newline",
            {
                let mut bytes = valid_outer.clone().into_bytes();
                bytes.push(b'{');
                bytes
            },
        ),
        (
            "valid choice tag",
            choice_document(&choice, "choice.yes").into_bytes(),
        ),
        (
            "unknown variant tag",
            choice_document(&choice, "choice.bogus").into_bytes(),
        ),
    ];

    for (label, bytes) in cases {
        let schema = if label.contains("choice") { &choice } else { &outer };
        let whole = schema.decode(&bytes);

        let whole_at_once = push_all_at_once(schema, &bytes);
        let byte_by_byte = push_one_byte_at_a_time(schema, &bytes);
        let midpoint = push_split_at(schema, &bytes, bytes.len() / 2);

        match whole {
            Ok(value) => {
                for (variant_label, outcome) in [
                    ("all-at-once", &whole_at_once),
                    ("one-byte-at-a-time", &byte_by_byte),
                    ("split-at-midpoint", &midpoint),
                ] {
                    match outcome {
                        PushOutcome::Accepted(streamed) => assert_eq!(
                            streamed, &value,
                            "{label} ({variant_label}): streamed value must equal the whole-document decode"
                        ),
                        other => panic!(
                            "{label} ({variant_label}): whole decoder accepted but streaming decoder did not: {other:?}"
                        ),
                    }
                }
            }
            Err(diagnostics) => {
                for (variant_label, outcome) in [
                    ("all-at-once", &whole_at_once),
                    ("one-byte-at-a-time", &byte_by_byte),
                    ("split-at-midpoint", &midpoint),
                ] {
                    assert!(
                        matches!(outcome, PushOutcome::Refused(_)),
                        "{label} ({variant_label}): whole decoder refused ({diagnostics:?}) but streaming decoder did not: {outcome:?}"
                    );
                }
            }
        }
    }
}

/// The one document this decoder's own scanner refuses purely for delegated
/// semantic reasons (a legally-shaped, oversized text field) carries the
/// exact underlying diagnostic code in its refusal message, matching
/// `live_bridge::SourceInteractionProposalDecoder`'s own refusal-reason
/// convention: a caller sees which admission rule failed, not a generic tag.
#[test]
fn semantic_refusal_carries_the_underlying_diagnostic_code() {
    let outer = compile_outer();
    let oversized = "x".repeat(5_000);
    let bytes = outer_document(&outer, "7", "true", &oversized).into_bytes();

    let whole_err = outer
        .decode(&bytes)
        .expect_err("an oversized text field must be refused by the whole decoder too");
    let expected_code = whole_err.first().expect("one diagnostic").code;

    match push_all_at_once(&outer, &bytes) {
        PushOutcome::Refused(refusal) => {
            assert_eq!(refusal.code, STREAM_SEMANTIC);
            assert!(
                refusal.message.contains(expected_code),
                "refusal message {:?} must name the underlying diagnostic {expected_code}",
                refusal.message
            );
        }
        other => panic!("expected a semantic refusal, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Criterion: a prefix must never be mistaken for a complete value —
// incomplete and invalid are distinct, never collapsed into one state.
// ---------------------------------------------------------------------

/// Every strict, non-empty prefix of a valid document (short of the whole
/// thing) is `Incomplete`, never `Accepted` and never `Refused`; only the
/// complete byte sequence is `Accepted`. Pairs the positive (legal,
/// complete stream accepted) against the negative (every truncation
/// incomplete, never treated as done).
#[test]
fn every_strict_prefix_is_incomplete_only_the_complete_document_is_accepted() {
    let outer = compile_outer();
    let bytes = outer_document(&outer, "7", "true", "hi").into_bytes();

    for len in 1..bytes.len() {
        let mut decoder = ProposalStreamDecoder::new(&outer);
        let outcome = decoder.push(&bytes[..len]);
        assert!(
            matches!(outcome, PushOutcome::Incomplete),
            "prefix of length {len}/{} must be Incomplete, got {outcome:?}",
            bytes.len()
        );
    }

    let mut decoder = ProposalStreamDecoder::new(&outer);
    assert_eq!(decoder.push(&bytes), PushOutcome::Incomplete);
    match decoder.finish() {
        PushOutcome::Accepted(_) => {}
        other => panic!("the complete document must be Accepted after finish(), got {other:?}"),
    }
}

/// Incomplete carries no code or message at all, so it is structurally
/// incapable of being confused with any closed refusal code: there is no
/// text field on it to inspect. Every declared refusal code, conversely,
/// never mentions "incomplete" — the two states cannot be told apart by
/// accidentally grepping one message for the other's vocabulary.
#[test]
fn incomplete_and_refused_are_never_textually_confusable() {
    let outer = compile_outer();
    let bytes = outer_document(&outer, "7", "true", "hi").into_bytes();

    let mut decoder = ProposalStreamDecoder::new(&outer);
    let incomplete = decoder.push(&bytes[..bytes.len() - 1]);
    assert_eq!(incomplete, PushOutcome::Incomplete);
    // `Incomplete` is a unit variant: there is no `.message`/`.code` to read,
    // which is exactly the guarantee — confirm the type still matches the
    // bare variant (no accidental payload was added to it).
    assert!(matches!(incomplete, PushOutcome::Incomplete));

    let all_codes = [
        STREAM_START,
        STREAM_WHITESPACE,
        STREAM_STRING,
        STREAM_BRACKET,
        STREAM_UTF8,
        STREAM_DEPTH,
        STREAM_TOKENS,
        STREAM_BYTES,
        STREAM_TRAILING,
        STREAM_TRUNCATED,
        STREAM_CANCELLED,
        STREAM_SEMANTIC,
    ];
    for code in all_codes {
        assert!(
            !code.to_lowercase().contains("incomplete"),
            "refusal code {code} must not overlap the incomplete vocabulary"
        );
    }
    // Distinctness of the closed vocabulary itself.
    for (left_index, left) in all_codes.iter().enumerate() {
        for right in &all_codes[left_index + 1..] {
            assert_ne!(left, right, "refusal codes must be pairwise distinct");
        }
    }
}

// ---------------------------------------------------------------------
// Criterion: deterministic — the same bytes delivered at different chunk
// boundaries produce the identical outcome. The single most valuable test.
// ---------------------------------------------------------------------

/// Splits the same total byte sequence at every possible single offset,
/// and also feeds it one byte at a time, asserting every chunking produces
/// the identical final outcome. Run across a valid document, a
/// multi-byte-UTF-8-bearing valid document, and two differently-shaped
/// invalid documents, so determinism is proven for acceptance, UTF-8
/// splitting, and refusal alike — not merely for the happy path.
#[test]
fn identical_bytes_produce_identical_outcomes_regardless_of_chunk_boundaries() {
    let outer = compile_outer();

    let valid = outer_document(&outer, "7", "true", "hi").into_bytes();
    let multibyte_utf8 = outer_document(&outer, "42", "false", "caf\u{e9} \u{1f600} \u{4e2d}").into_bytes();
    let unknown_field = outer_document(&outer, "7", "true", "hi")
        .replace("\"outer.active\":true", "\"outer.active\":true,\"outer.bogus\":\"1\"")
        .into_bytes();
    let bad_utf8 = {
        let mut bytes = outer_document(&outer, "7", "true", "hi").into_bytes();
        // Overwrite one byte inside the "hi" text value with a lone UTF-8
        // continuation byte, which is invalid anywhere it is not preceded
        // by an appropriate leading byte.
        let position = bytes
            .windows(2)
            .position(|window| window == b"hi")
            .expect("the note text is present");
        bytes[position] = 0x80;
        bytes
    };

    for bytes in [valid, multibyte_utf8, unknown_field, bad_utf8] {
        let reference = push_all_at_once(&outer, &bytes);
        assert_eq!(
            reference,
            push_one_byte_at_a_time(&outer, &bytes),
            "one-byte-at-a-time chunking must match the whole-chunk outcome"
        );
        for split in 1..bytes.len() {
            let split_outcome = push_split_at(&outer, &bytes, split);
            assert_eq!(
                reference, split_outcome,
                "splitting at byte {split}/{} must not change the outcome",
                bytes.len()
            );
        }
    }
}

// ---------------------------------------------------------------------
// Criterion: no unbounded buffering — an explicit bound is hit and refused,
// not grown past. Each bound is driven to its exact limit.
// ---------------------------------------------------------------------

/// Exactly [`MAX_STREAM_BYTES`] buffered bytes (an unterminated, still-open
/// string inside an unterminated top-level object — deliberately not a
/// legal Proposal shape, since this test's job is only to prove the
/// streaming layer's own byte bound, not to exercise semantic admission)
/// remains `Incomplete`; the very next byte is refused, never buffered.
#[test]
fn byte_bound_is_enforced_at_its_exact_limit() {
    let outer = compile_outer();
    let mut decoder = ProposalStreamDecoder::new(&outer);

    let mut filler = vec![b'{', b'"'];
    filler.resize(MAX_STREAM_BYTES, b'a');
    assert_eq!(filler.len(), MAX_STREAM_BYTES);

    let outcome = decoder.push(&filler);
    assert_eq!(
        outcome,
        PushOutcome::Incomplete,
        "exactly the byte bound must still be accepted as an in-progress stream"
    );
    assert_eq!(decoder.buffered_len(), MAX_STREAM_BYTES);

    match decoder.push(b"a") {
        PushOutcome::Refused(refusal) => {
            assert_eq!(refusal.code, STREAM_BYTES);
            assert!(refusal.message.contains(&MAX_STREAM_BYTES.to_string()));
        }
        other => panic!("one byte past the bound must be refused, got {other:?}"),
    }
    // The bound must not have grown to admit the extra byte.
    assert_eq!(decoder.buffered_len(), MAX_STREAM_BYTES);
}

/// Exactly [`MAX_STREAM_DEPTH`] levels of open, unclosed containers remains
/// `Incomplete`; the next nested open is refused.
#[test]
fn depth_bound_is_enforced_at_its_exact_limit() {
    let outer = compile_outer();
    let mut decoder = ProposalStreamDecoder::new(&outer);

    // The mandatory leading '{' is depth 1; MAX_STREAM_DEPTH - 1 further
    // opens reach exactly MAX_STREAM_DEPTH.
    let mut bytes = vec![b'{'];
    bytes.extend(std::iter::repeat(b'[').take(MAX_STREAM_DEPTH - 1));
    assert_eq!(
        decoder.push(&bytes),
        PushOutcome::Incomplete,
        "exactly the depth bound must still be an in-progress stream"
    );

    match decoder.push(b"[") {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_DEPTH),
        other => panic!("one level past the depth bound must be refused, got {other:?}"),
    }
}

/// Exactly [`MAX_STREAM_STRING_TOKENS`] complete empty-string tokens remains
/// `Incomplete`; the next string token is refused.
#[test]
fn string_token_bound_is_enforced_at_its_exact_limit() {
    let outer = compile_outer();
    let mut decoder = ProposalStreamDecoder::new(&outer);

    let mut bytes = vec![b'{'];
    for _ in 0..MAX_STREAM_STRING_TOKENS {
        bytes.extend_from_slice(b"\"\"");
    }
    assert_eq!(
        decoder.push(&bytes),
        PushOutcome::Incomplete,
        "exactly the token bound must still be an in-progress stream"
    );

    match decoder.push(b"\"\"") {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_TOKENS),
        other => panic!("one token past the bound must be refused, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Structural refusal vocabulary, each paired with the legal stream that is
// accepted.
// ---------------------------------------------------------------------

#[test]
fn a_document_not_opening_with_a_brace_is_refused_and_a_legal_one_is_accepted() {
    let outer = compile_outer();
    match push_all_at_once(&outer, b"[") {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_START),
        other => panic!("expected STREAM-START, got {other:?}"),
    }
    let valid = outer_document(&outer, "1", "true", "ok").into_bytes();
    assert!(matches!(
        push_all_at_once(&outer, &valid),
        PushOutcome::Accepted(_)
    ));
}

#[test]
fn raw_whitespace_outside_a_string_is_refused_and_the_same_document_without_it_is_accepted() {
    let outer = compile_outer();
    let valid = outer_document(&outer, "1", "true", "ok");
    let with_space = valid.replacen("\"outer.id\"", " \"outer.id\"", 1);
    match push_all_at_once(&outer, with_space.as_bytes()) {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_WHITESPACE),
        other => panic!("expected STREAM-WHITESPACE, got {other:?}"),
    }
    assert!(matches!(
        push_all_at_once(&outer, valid.as_bytes()),
        PushOutcome::Accepted(_)
    ));
}

#[test]
fn an_unescaped_control_byte_in_a_string_is_refused() {
    let outer = compile_outer();
    let mut bytes = outer_document(&outer, "1", "true", "ok").into_bytes();
    let position = bytes
        .windows(2)
        .position(|window| window == b"ok")
        .expect("the note text is present");
    bytes[position] = 0x07; // BEL, a raw control byte, unescaped
    match push_all_at_once(&outer, &bytes) {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_STRING),
        other => panic!("expected STREAM-STRING, got {other:?}"),
    }
}

#[test]
fn an_invalid_unicode_escape_hex_digit_is_refused() {
    let outer = compile_outer();
    let valid = outer_document(&outer, "1", "true", "ok");
    let tampered = valid.replacen("\"ok\"", "\"\\uZZZZ\"", 1);
    match push_all_at_once(&outer, tampered.as_bytes()) {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_STRING),
        other => panic!("expected STREAM-STRING, got {other:?}"),
    }
}

#[test]
fn a_mismatched_closing_bracket_is_refused() {
    let outer = compile_outer();
    let valid = outer_document(&outer, "1", "true", "ok");
    let tampered = valid.replacen("{\"fields\"", "[\"fields\"", 1);
    match push_all_at_once(&outer, tampered.as_bytes()) {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_BRACKET),
        other => panic!("expected STREAM-BRACKET, got {other:?}"),
    }
}

#[test]
fn invalid_utf8_is_refused_distinctly_from_structural_or_semantic_refusals() {
    let outer = compile_outer();
    let mut bytes = outer_document(&outer, "1", "true", "ok").into_bytes();
    let position = bytes
        .windows(2)
        .position(|window| window == b"ok")
        .expect("the note text is present");
    bytes[position] = 0xFF; // never a valid UTF-8 leading byte
    match push_all_at_once(&outer, &bytes) {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_UTF8),
        other => panic!("expected STREAM-UTF8, got {other:?}"),
    }
}

#[test]
fn data_after_the_terminal_newline_is_refused_as_trailing() {
    let outer = compile_outer();
    let mut bytes = outer_document(&outer, "1", "true", "ok").into_bytes();
    bytes.push(b'x');
    match push_all_at_once(&outer, &bytes) {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_TRAILING),
        other => panic!("expected STREAM-TRAILING, got {other:?}"),
    }
}

#[test]
fn a_top_level_close_not_followed_by_a_newline_is_refused_as_trailing() {
    let outer = compile_outer();
    let valid = outer_document(&outer, "1", "true", "ok");
    let without_newline = format!("{}x", valid.trim_end_matches('\n'));
    match push_all_at_once(&outer, without_newline.as_bytes()) {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_TRAILING),
        other => panic!("expected STREAM-TRAILING, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Cancellation and premature end of stream, and sticky terminal outcomes.
// ---------------------------------------------------------------------

#[test]
fn finish_refuses_a_stream_that_never_reached_its_terminal_newline() {
    let outer = compile_outer();
    let mut decoder = ProposalStreamDecoder::new(&outer);
    let valid = outer_document(&outer, "1", "true", "ok").into_bytes();
    assert_eq!(decoder.push(&valid[..valid.len() - 1]), PushOutcome::Incomplete);

    match decoder.finish() {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_TRUNCATED),
        other => panic!("expected STREAM-TRUNCATED, got {other:?}"),
    }
    // Sticky: calling it again returns the same stored outcome.
    match decoder.finish() {
        PushOutcome::Refused(refusal) => assert_eq!(refusal.code, STREAM_TRUNCATED),
        other => panic!("expected the stored STREAM-TRUNCATED outcome, got {other:?}"),
    }
}

#[test]
fn cancel_refuses_an_in_progress_stream_with_the_given_reason() {
    let outer = compile_outer();
    let mut decoder = ProposalStreamDecoder::new(&outer);
    let valid = outer_document(&outer, "1", "true", "ok").into_bytes();
    assert_eq!(decoder.push(&valid[..3]), PushOutcome::Incomplete);

    match decoder.cancel("caller deadline expired") {
        PushOutcome::Refused(refusal) => {
            assert_eq!(refusal.code, STREAM_CANCELLED);
            assert_eq!(refusal.message, "caller deadline expired");
        }
        other => panic!("expected STREAM-CANCELLED, got {other:?}"),
    }
}

#[test]
fn cancel_after_acceptance_cannot_retroactively_unauthorize_the_decoded_value() {
    let outer = compile_outer();
    let mut decoder = ProposalStreamDecoder::new(&outer);
    let valid = outer_document(&outer, "1", "true", "ok").into_bytes();
    assert_eq!(decoder.push(&valid), PushOutcome::Incomplete);
    let value = match decoder.finish() {
        PushOutcome::Accepted(value) => value,
        other => panic!("expected Accepted, got {other:?}"),
    };

    match decoder.cancel("too late") {
        PushOutcome::Accepted(after_cancel) => assert_eq!(after_cancel, value),
        other => panic!(
            "cancelling after a successful decode must not change the outcome, got {other:?}"
        ),
    }
}

#[test]
fn a_refusal_is_sticky_across_further_pushes() {
    let outer = compile_outer();
    let mut decoder = ProposalStreamDecoder::new(&outer);
    assert!(matches!(
        decoder.push(b"["),
        PushOutcome::Refused(_)
    ));
    let first = decoder.push(b"[");
    let second = decoder.push(b"anything else at all");
    assert_eq!(first, second);
}
