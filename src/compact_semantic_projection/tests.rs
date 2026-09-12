use std::fs;
use std::path::Path;

use super::*;
use crate::ast::Program;
use crate::graph::{self, AgentContextDirection, AgentContextFilter, AgentContextV2Options};

fn program(source: &str) -> Program {
    crate::parse(source, "fixture.spx").expect("fixture parses")
}

fn example(name: &str) -> Program {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples").join(name);
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("committed example {} reads: {error}", path.display()));
    crate::parse(&source, name).unwrap_or_else(|error| panic!("committed example {name} parses: {error:?}"))
}

fn forward_options(depth: usize) -> AgentContextV2Options {
    AgentContextV2Options::new(
        depth,
        64 * 1024,
        256,
        AgentContextFilter::ALL,
        AgentContextDirection::Forward,
    )
    .expect("options are in bounds")
}

// --- Round-trip: the actual lossless claim -------------------------------
//
// Each test below independently obtains the original selected-view text a
// *second* time from the plain existing engine function, and asserts the
// decoded/reconstructed bytes equal *that* value -- never a value derived
// from the compact encoding itself. This is the defect this task calls out
// by name: comparing an encoding's output to itself proves nothing.

#[test]
fn round_trip_agent_context_v2_binary_matches_the_original_engine_output_exactly() {
    let program = example("banking_ledger.spx");
    let options = forward_options(2);
    let symbol = "app.main";

    // Independently obtained: calls the plain existing engine directly.
    let original =
        graph::agent_context_v2_json(&program, symbol, &options).unwrap().expect("seed resolves");

    let projection = encode_profile(
        &program,
        ProjectionSource::AgentContextV2 {
            symbol,
            options: &options,
        },
    )
    .expect("encode succeeds");

    let wire = projection.to_binary();
    // The compact wire form must be a genuinely different byte sequence
    // from the original JSON text, not a disguised passthrough.
    assert_ne!(wire.as_slice(), original.as_bytes());

    let decoded = decode_binary(&wire).expect("decode succeeds");
    let reconstructed = decoded.reconstructed().expect("reconstruction succeeds");
    assert_eq!(
        String::from_utf8(reconstructed).unwrap(),
        original,
        "decoded binary form must reproduce the independently obtained original exactly"
    );
}

#[test]
fn round_trip_full_graph_text_matches_the_original_engine_output_exactly() {
    let program = example("banking_ledger.spx");

    let original = graph::to_json(&program).expect("graph resolves");

    let projection = encode_profile(&program, ProjectionSource::FullGraph).expect("encode succeeds");
    let text = projection.to_text();
    assert_ne!(text, original);

    let decoded = decode_text(&text).expect("decode succeeds");
    let reconstructed = decoded.reconstructed().expect("reconstruction succeeds");
    assert_eq!(
        String::from_utf8(reconstructed).unwrap(),
        original,
        "decoded text form must reproduce the independently obtained original exactly"
    );
}

#[test]
fn compact_wire_forms_are_materially_smaller_on_a_committed_example() {
    let program = example("http_app_routing.spx");
    let original = graph::to_json(&program).expect("graph resolves");
    let projection = encode_profile(&program, ProjectionSource::FullGraph).expect("encode succeeds");

    let binary_len = projection.to_binary().len();
    let text_len = projection.to_text().len();

    assert!(
        binary_len < original.len(),
        "binary wire ({binary_len} bytes) must be smaller than the original graph JSON \
         ({} bytes) on this repeated-identifier-heavy committed example",
        original.len()
    );
    assert!(
        text_len < original.len(),
        "text wire ({text_len} bytes) must be smaller than the original graph JSON \
         ({} bytes) on this repeated-identifier-heavy committed example",
        original.len()
    );
    // Sanity: the dictionary actually captured repetition (more body
    // references than distinct dictionary entries), otherwise this would
    // not be a meaningful compaction claim.
    assert!(projection.dictionary_len() < projection.body_len());
}

#[test]
fn round_trip_is_deterministic_across_repeated_encodes() {
    let program = example("http_app_routing.spx");
    let a = encode_profile(&program, ProjectionSource::FullGraph).expect("encode succeeds");
    let b = encode_profile(&program, ProjectionSource::FullGraph).expect("encode succeeds");
    assert_eq!(a.to_binary(), b.to_binary());
    assert_eq!(a.to_text(), b.to_text());
}

// --- Negative control: a genuinely corrupted encoding must fail to round-trip

#[test]
fn tampered_wire_content_fails_digest_verification() {
    let program = program(FIXTURE);
    let projection = encode_profile(&program, ProjectionSource::FullGraph).expect("encode succeeds");
    let mut wire = projection.to_binary();

    // Flip one byte well past the fixed-width header (inside the
    // dictionary/body content), keeping the wire the same length so every
    // length/count field still parses structurally -- the corruption must
    // be caught by the content digest, not by a length mismatch.
    let flip_at = wire.len() - 5;
    wire[flip_at] ^= 0xFF;

    let error = decode_binary(&wire).expect_err("tampered content must not round-trip");
    assert_eq!(error.code, "SPX-Z907", "must report digest mismatch specifically: {error:?}");
}

#[test]
fn tampered_text_body_content_fails_digest_verification() {
    let program = program(FIXTURE);
    let projection = encode_profile(&program, ProjectionSource::FullGraph).expect("encode succeeds");
    let text = projection.to_text();

    // Flip the very first byte of the body stream. It is not a `~`
    // reference marker in this fixture (the graph JSON always opens with a
    // raw `{`), so this changes reconstructed content without touching any
    // length/count field, delimiter, or dictionary content.
    let content_at = text.find("body\n").expect("a body section is present") + "body\n".len();
    assert_ne!(
        text.as_bytes()[content_at], b'~',
        "this test needs the first body byte to be raw, not a reference marker"
    );

    let mut bytes = text.into_bytes();
    bytes[content_at] = if bytes[content_at] == b'X' { b'Y' } else { b'X' };
    let text = String::from_utf8(bytes).expect("swapping one ASCII byte stays valid UTF-8");

    let error = decode_text(&text).expect_err("tampered body content must not round-trip");
    assert_eq!(error.code, "SPX-Z907", "must report digest mismatch specifically: {error:?}");
}

// --- Dictionary indices are compression bookkeeping, never identity ------

#[test]
fn dictionary_index_for_the_same_literal_differs_across_envelopes_and_decode_is_unaffected() {
    // In `source_a`, `"app.same_id"` is the only literal, so it is
    // dictionary index 0.
    let source_a = "\"app.same_id\":1".as_bytes();
    // In `source_b`, two literals that sort before it push it to index 1.
    let source_b = "{\"aaa\":\"app.same_id\",\"zzz\":1}".as_bytes();

    let projection_a = encode_bytes(source_a, "test", "root-a", "rev").unwrap();
    let projection_b = encode_bytes(source_b, "test", "root-b", "rev").unwrap();

    assert_eq!(projection_a.dictionary_len(), 1);
    assert_eq!(projection_b.dictionary_len(), 3);

    let decoded_a = decode_binary(&projection_a.to_binary()).unwrap();
    let decoded_b = decode_binary(&projection_b.to_binary()).unwrap();
    assert_eq!(decoded_a.reconstructed().unwrap(), source_a);
    assert_eq!(decoded_b.reconstructed().unwrap(), source_b);
    // The literal is present, correctly reconstructed, in both -- despite
    // occupying a different index in each envelope's own dictionary. No
    // public accessor on `CompactProjection` exposes "resolve by index";
    // the only operation is "reconstruct the whole selected view."
}

// --- Hostile input: length, count, index, duplicate, order, version, root

const FIXTURE: &str = "module test.compact;

@id(\"app.helper\")
fn helper(value: i64) -> i64 { value }

@id(\"app.main\")
fn main() -> i64 { helper(1) }
";

fn small_projection() -> CompactProjection {
    let program = program(FIXTURE);
    encode_profile(&program, ProjectionSource::FullGraph).expect("encode succeeds")
}

#[test]
fn truncated_binary_wire_is_refused() {
    let wire = small_projection().to_binary();
    let truncated = &wire[..wire.len() - 3];
    let error = decode_binary(truncated).expect_err("a truncated wire must be refused");
    assert_eq!(error.code, "SPX-Z903");
}

#[test]
fn truncated_text_wire_is_refused() {
    let text = small_projection().to_text();
    let truncated = &text[..text.len() - 3];
    let error = decode_text(truncated).expect_err("a truncated wire must be refused");
    // Truncating inside the fixed-framed header or a newline-delimited
    // dictionary line is caught structurally (`SPX-Z903`); truncating
    // inside the unframed body stream instead still parses structurally
    // (there is no required terminator on a final raw span) but is then
    // caught by the reconstructed-content digest no longer matching the
    // envelope's declared digest (`SPX-Z907`). Both are genuine refusals of
    // the same truncated input; which one fires depends only on where the
    // cut landed.
    assert!(
        error.code == "SPX-Z903" || error.code == "SPX-Z907",
        "expected a structural or digest refusal, got {error:?}"
    );
}

#[test]
fn trailing_bytes_after_a_complete_binary_wire_are_refused() {
    let mut wire = small_projection().to_binary();
    wire.push(0);
    let error = decode_binary(&wire).expect_err("trailing bytes must be refused");
    assert_eq!(error.code, "SPX-Z903");
}

#[test]
fn hostile_declared_dictionary_count_is_refused_before_allocating() {
    // Hand-build a header claiming far more dictionary entries than the
    // policy bound, backed by almost no actual bytes -- proving the count
    // is checked against its bound (and, independently, against the bytes
    // actually remaining) before any per-entry read is attempted.
    let mut wire = Vec::new();
    wire.extend_from_slice(BINARY_MAGIC);
    wire.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    for field in [b"p".as_slice(), b"r", b"rev", b"dig"] {
        wire.extend_from_slice(&(field.len() as u32).to_le_bytes());
        wire.extend_from_slice(field);
    }
    wire.extend_from_slice(&u32::MAX.to_le_bytes()); // declared dictionary count
    let error = decode_binary(&wire).expect_err("an absurd declared count must be refused");
    assert_eq!(error.code, "SPX-Z901", "expected a capacity refusal: {error:?}");
}

#[test]
fn hostile_out_of_range_body_reference_is_refused() {
    let mut wire = Vec::new();
    wire.extend_from_slice(BINARY_MAGIC);
    wire.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    for field in [b"p".as_slice(), b"r", b"rev", b"dig"] {
        wire.extend_from_slice(&(field.len() as u32).to_le_bytes());
        wire.extend_from_slice(field);
    }
    // One dictionary entry: `"a"`.
    wire.extend_from_slice(&1u32.to_le_bytes());
    wire.extend_from_slice(&3u32.to_le_bytes());
    wire.extend_from_slice(b"\"a\"");
    // One body token: a reference to index 5, which does not exist.
    wire.extend_from_slice(&1u32.to_le_bytes());
    wire.push(1); // Ref tag
    wire.extend_from_slice(&5u32.to_le_bytes());

    let error = decode_binary(&wire).expect_err("an out-of-range index must be refused");
    assert_eq!(error.code, "SPX-Z906");
}

#[test]
fn hostile_duplicate_dictionary_entries_are_refused() {
    let mut wire = Vec::new();
    wire.extend_from_slice(BINARY_MAGIC);
    wire.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    for field in [b"p".as_slice(), b"r", b"rev", b"dig"] {
        wire.extend_from_slice(&(field.len() as u32).to_le_bytes());
        wire.extend_from_slice(field);
    }
    wire.extend_from_slice(&2u32.to_le_bytes()); // two entries
    for _ in 0..2 {
        wire.extend_from_slice(&3u32.to_le_bytes());
        wire.extend_from_slice(b"\"a\"");
    }
    wire.extend_from_slice(&0u32.to_le_bytes()); // no body tokens

    let error = decode_binary(&wire).expect_err("duplicate dictionary entries must be refused");
    assert_eq!(error.code, "SPX-Z905");
}

#[test]
fn hostile_reordered_dictionary_is_refused() {
    let mut wire = Vec::new();
    wire.extend_from_slice(BINARY_MAGIC);
    wire.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    for field in [b"p".as_slice(), b"r", b"rev", b"dig"] {
        wire.extend_from_slice(&(field.len() as u32).to_le_bytes());
        wire.extend_from_slice(field);
    }
    wire.extend_from_slice(&2u32.to_le_bytes());
    // `"b"` before `"a"`: valid distinct entries, invalid order.
    for entry in [b"\"b\"".as_slice(), b"\"a\""] {
        wire.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        wire.extend_from_slice(entry);
    }
    wire.extend_from_slice(&0u32.to_le_bytes());

    let error = decode_binary(&wire).expect_err("an out-of-order dictionary must be refused");
    assert_eq!(error.code, "SPX-Z905");
}

#[test]
fn hostile_unknown_format_version_is_refused_not_heuristically_decoded() {
    let mut wire = small_projection().to_binary();
    // format_version is the 4 bytes immediately after the 8-byte magic.
    wire[8..12].copy_from_slice(&2u32.to_le_bytes());
    let error = decode_binary(&wire).expect_err("an unknown version must be refused");
    assert_eq!(error.code, "SPX-Z904");
    assert!(error.message.contains("version 2"), "{}", error.message);
}

#[test]
fn hostile_text_unknown_format_version_is_refused() {
    let projection = small_projection();
    let text = projection.to_text();
    let replaced = text.replacen("SPXCPJv1\n", "SPXCPJv7\n", 1);
    assert_ne!(text, replaced, "the fixture must contain the version line being replaced");
    let error = decode_text(&replaced).expect_err("an unknown version must be refused");
    assert_eq!(error.code, "SPX-Z904");
}

#[test]
fn hostile_unrecognized_body_tag_is_refused() {
    let mut wire = Vec::new();
    wire.extend_from_slice(BINARY_MAGIC);
    wire.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    for field in [b"p".as_slice(), b"r", b"rev", b"dig"] {
        wire.extend_from_slice(&(field.len() as u32).to_le_bytes());
        wire.extend_from_slice(field);
    }
    wire.extend_from_slice(&0u32.to_le_bytes()); // no dictionary entries
    wire.extend_from_slice(&1u32.to_le_bytes()); // one body token
    wire.push(9); // unrecognized tag
    let error = decode_binary(&wire).expect_err("an unrecognized body tag must be refused");
    assert_eq!(error.code, "SPX-Z903");
}

#[test]
fn field_injected_root_binding_mismatch_is_refused_by_verify_but_not_by_plain_decode() {
    let projection = small_projection();
    assert_eq!(projection.root(), "*", "this test's offset arithmetic assumes a 1-byte root");
    let mut wire = projection.to_binary();

    // Locate the root field's content byte by exact layout, not by
    // scanning for `*` (which could coincidentally appear elsewhere):
    // magic(8) + version(4) + profile length-prefix(4) + profile bytes,
    // then the root field's own length-prefix(4) precedes its content.
    let root_content_at = 8 + 4 + 4 + projection.profile().len() + 4;
    assert_eq!(&wire[root_content_at..root_content_at + 1], b"*");
    wire[root_content_at] = b'!';

    // Plain decode succeeds: the envelope is structurally well-formed and
    // internally consistent (its own digest still matches its own content).
    let decoded = decode_binary(&wire).expect("a well-formed envelope with a different root decodes");
    assert_eq!(decoded.root(), "!");

    // But binding verification against the caller's actual expectation
    // (the real root this projection was built for) must refuse.
    let error = decode_binary_and_verify(&wire, "full-graph", "*", projection.source_revision())
        .expect_err("root binding mismatch must be refused");
    assert_eq!(error.code, "SPX-Z908");
}

#[test]
fn profile_binding_mismatch_is_refused() {
    let projection = small_projection();
    let wire = projection.to_binary();
    let error = decode_binary_and_verify(
        &wire,
        "agent-context-v2",
        projection.root(),
        projection.source_revision(),
    )
    .expect_err("profile binding mismatch must be refused");
    assert_eq!(error.code, "SPX-Z908");
}

#[test]
fn missing_root_symbol_is_refused_at_encode_time() {
    let program = program(FIXTURE);
    let options = forward_options(1);
    let error = encode_profile(
        &program,
        ProjectionSource::AgentContextV2 {
            symbol: "app.does_not_exist",
            options: &options,
        },
    )
    .expect_err("an unresolved seed must be refused");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-Z909");
}

// --- Every declared bound driven to its exact limit: one under, one at, one over

fn distinct_literal_source(count: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for index in 0..count {
        if index > 0 {
            out.push(b',');
        }
        out.extend_from_slice(format!("\"v{index}\"").as_bytes());
    }
    out
}

#[test]
fn max_dictionary_entries_bound_is_enforced_at_its_exact_limit() {
    let under = distinct_literal_source(MAX_DICTIONARY_ENTRIES - 1);
    let at = distinct_literal_source(MAX_DICTIONARY_ENTRIES);
    let over = distinct_literal_source(MAX_DICTIONARY_ENTRIES + 1);

    assert!(encode_bytes(&under, "p", "r", "rev").is_ok());
    assert!(encode_bytes(&at, "p", "r", "rev").is_ok());
    let error = encode_bytes(&over, "p", "r", "rev").expect_err("one over the limit must be refused");
    assert_eq!(error.code, "SPX-Z901");
    assert!(error.message.contains("dictionary entries"), "{}", error.message);
}

/// `occurrences` repetitions of the single literal `"x"`, joined by `,`
/// with no leading/trailing byte, producing exactly `2*occurrences - 1`
/// body tokens (`occurrences` refs and `occurrences - 1` raw separators).
/// `leading_raw` prepends one extra raw byte, adding exactly one more
/// token -- the only lever needed to hit an odd bound at, and an even
/// bound one-under/one-over, without changing the dictionary at all (it
/// always stays a single entry).
fn repeated_literal_source(occurrences: usize, leading_raw: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if leading_raw {
        out.push(b'!');
    }
    for index in 0..occurrences {
        if index > 0 {
            out.push(b',');
        }
        out.extend_from_slice(b"\"x\"");
    }
    out
}

#[test]
fn max_body_tokens_bound_is_enforced_at_its_exact_limit() {
    assert_eq!(MAX_BODY_TOKENS % 2, 1, "the construction below assumes an odd bound");
    let at_occurrences = (MAX_BODY_TOKENS + 1) / 2; // 2*o - 1 == MAX_BODY_TOKENS
    let under_occurrences = at_occurrences - 1; // with leading_raw: 2*o == MAX_BODY_TOKENS - 1
    let over_occurrences = at_occurrences; // with leading_raw: 2*o == MAX_BODY_TOKENS + 1

    let under = repeated_literal_source(under_occurrences, true);
    let at = repeated_literal_source(at_occurrences, false);
    let over = repeated_literal_source(over_occurrences, true);

    let under_projection = encode_bytes(&under, "p", "r", "rev").expect("one under the limit succeeds");
    assert_eq!(under_projection.body_len(), MAX_BODY_TOKENS - 1);
    let at_projection = encode_bytes(&at, "p", "r", "rev").expect("exactly at the limit succeeds");
    assert_eq!(at_projection.body_len(), MAX_BODY_TOKENS);
    let error = encode_bytes(&over, "p", "r", "rev").expect_err("one over the limit must be refused");
    assert_eq!(error.code, "SPX-Z901");
    assert!(error.message.contains("body tokens"), "{}", error.message);
}

fn single_literal_source(inner_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(inner_len + 2);
    out.push(b'"');
    out.resize(out.len() + inner_len, b'a');
    out.push(b'"');
    out
}

#[test]
fn max_entry_bytes_bound_is_enforced_at_its_exact_limit() {
    let under = single_literal_source(MAX_ENTRY_BYTES - 3); // total MAX_ENTRY_BYTES - 1
    let at = single_literal_source(MAX_ENTRY_BYTES - 2); // total MAX_ENTRY_BYTES
    let over = single_literal_source(MAX_ENTRY_BYTES - 1); // total MAX_ENTRY_BYTES + 1

    assert!(encode_bytes(&under, "p", "r", "rev").is_ok());
    assert!(encode_bytes(&at, "p", "r", "rev").is_ok());
    let error = encode_bytes(&over, "p", "r", "rev").expect_err("one over the limit must be refused");
    assert_eq!(error.code, "SPX-Z901");
    assert!(error.message.contains("dictionary entry"), "{}", error.message);
}

#[test]
fn max_header_field_bytes_bound_is_enforced_at_its_exact_limit() {
    let source = b"\"x\"";
    let under = "r".repeat(MAX_HEADER_FIELD_BYTES - 1);
    let at = "r".repeat(MAX_HEADER_FIELD_BYTES);
    let over = "r".repeat(MAX_HEADER_FIELD_BYTES + 1);

    assert!(encode_bytes(source, "p", &under, "rev").is_ok());
    assert!(encode_bytes(source, "p", &at, "rev").is_ok());
    let error = encode_bytes(source, "p", &over, "rev").expect_err("one over the limit must be refused");
    assert_eq!(error.code, "SPX-Z901");
    assert!(error.message.contains("root"), "{}", error.message);
}

/// Bytes of exactly `target_len`, built from same-content string literals
/// (so the dictionary stays at 1-2 entries and body tokens stay in the
/// dozens) so `MAX_SOURCE_BYTES` can be driven to its exact byte limit
/// without also approaching `MAX_ENTRY_BYTES`, `MAX_DICTIONARY_ENTRIES`, or
/// `MAX_BODY_TOKENS`.
fn exact_length_source(target_len: usize) -> Vec<u8> {
    const UNIT: usize = 1024;
    const INNER: usize = UNIT - 2;
    let mut full_units = target_len / UNIT;
    let mut remainder = target_len % UNIT;
    if remainder == 1 {
        full_units -= 1;
        remainder += UNIT;
    }
    let mut out = Vec::with_capacity(target_len);
    for _ in 0..full_units {
        out.push(b'"');
        out.resize(out.len() + INNER, b'a');
        out.push(b'"');
    }
    if remainder > 0 {
        out.push(b'"');
        out.resize(out.len() + (remainder - 2), b'a');
        out.push(b'"');
    }
    assert_eq!(out.len(), target_len, "exact_length_source arithmetic");
    out
}

#[test]
fn max_source_bytes_bound_is_enforced_at_its_exact_limit() {
    let under = exact_length_source(MAX_SOURCE_BYTES - 1);
    let at = exact_length_source(MAX_SOURCE_BYTES);
    let over = exact_length_source(MAX_SOURCE_BYTES + 1);

    assert!(encode_bytes(&under, "p", "r", "rev").is_ok());
    assert!(encode_bytes(&at, "p", "r", "rev").is_ok());
    let error = encode_bytes(&over, "p", "r", "rev").expect_err("one over the limit must be refused");
    assert_eq!(error.code, "SPX-Z901");
    assert!(error.message.contains("compact projection source"), "{}", error.message);
}

#[test]
fn max_encoded_bytes_bound_is_enforced_at_its_exact_limit_ahead_of_structural_parsing() {
    // All-zero buffers: guaranteed not to start with `BINARY_MAGIC`, so
    // whatever error occurs at/under the limit must come from structural
    // parsing (a bad magic header), never from the size gate -- isolating
    // exactly what the size gate itself decides.
    let under = vec![0u8; MAX_ENCODED_BYTES - 1];
    let at = vec![0u8; MAX_ENCODED_BYTES];
    let over = vec![0u8; MAX_ENCODED_BYTES + 1];

    let under_error = decode_binary(&under).expect_err("garbage still fails, but not on size");
    assert_eq!(under_error.code, "SPX-Z903", "must fail on structure, not size: {under_error:?}");
    let at_error = decode_binary(&at).expect_err("garbage still fails, but not on size");
    assert_eq!(at_error.code, "SPX-Z903", "must fail on structure, not size: {at_error:?}");
    let over_error = decode_binary(&over).expect_err("one over the limit must be refused on size");
    assert_eq!(over_error.code, "SPX-Z901", "must fail on the size gate itself: {over_error:?}");
}
