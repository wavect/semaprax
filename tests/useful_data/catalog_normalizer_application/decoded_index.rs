//! Decoded-id fingerprint index evidence. A host decoded-byte oracle,
//! independent of the Semaprax cursors, proves each deliberate collision is
//! unequal and each escaped/raw pair is equal; the frozen Python oracle then
//! decides every response byte. The frozen pairwise batch/record pair stays an
//! independent whole-application reference.
use semaprax::diagnostic::Diagnostic;
use semaprax::hir::ResolvedProgram;
use semaprax::interpreter::{
    evaluate_resolved_owned_data, OwnedDataEvaluationOutcome, OwnedDataValue, MAX_STEPS_LIMIT,
};

use super::{assert_batch_backend_matrix, copy_fixture, fixture, run_oracle_with, ScratchRoot};

/// Mirrors `std.data.json.dec.decoded_fingerprint` over host-decoded bytes.
fn host_fingerprint(decoded: &[u8]) -> usize {
    let hash = decoded
        .iter()
        .fold(0, |hash, byte| (hash * 257 + usize::from(*byte)) % 65521);
    decoded.len() * 65536 + hash
}

/// Decodes one JSON string token with serde_json, never the Semaprax decoder.
fn host_decoded(token: &str) -> Vec<u8> {
    serde_json::from_str::<String>(token)
        .expect("host oracle accepts the id token")
        .into_bytes()
}

fn record(id_token: &str) -> String {
    format!("{{\"id\":{id_token},\"label\":\"l\",\"quantity\":1}}")
}

fn body<T: AsRef<str>>(id_tokens: &[T]) -> Vec<u8> {
    let mut text = id_tokens
        .iter()
        .map(|token| record(token.as_ref()))
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    text.into_bytes()
}

fn assert_collision(left: &str, right: &str) {
    let (left_bytes, right_bytes) = (host_decoded(left), host_decoded(right));
    assert_ne!(left_bytes, right_bytes, "{left} and {right} must differ");
    assert_eq!(
        host_fingerprint(&left_bytes),
        host_fingerprint(&right_bytes),
        "{left} and {right} must be a deliberate fingerprint collision"
    );
}

fn assert_equivalent(left: &str, right: &str) {
    assert_eq!(
        host_decoded(left),
        host_decoded(right),
        "{left} and {right} must decode to the same bytes"
    );
}

struct IndexCase {
    name: &'static str,
    input: Vec<u8>,
    enriched: bool,
    category: Option<&'static str>,
    record_index: Option<u64>,
}

fn case(
    name: &'static str,
    input: Vec<u8>,
    category: Option<&'static str>,
    record_index: Option<u64>,
) -> IndexCase {
    IndexCase {
        name,
        input,
        enriched: false,
        category,
        record_index,
    }
}

/// 256 distinct ids in 128 colliding pairs: each shares a five-byte prefix
/// and ends in the colliding suffixes `BAA`/`ACO`; even prefixes escape `k`.
fn colliding_rows(last: Option<&str>) -> Vec<u8> {
    let owned = (0..128)
        .flat_map(|pair| {
            let lead = if pair % 2 == 0 { "\\u006b" } else { "k" };
            [
                format!("\"{lead}{pair:03}-BAA\""),
                format!("\"k{pair:03}-ACO\""),
            ]
        })
        .collect::<Vec<_>>();
    let mut tokens = owned.iter().map(String::as_str).collect::<Vec<_>>();
    if let Some(last) = last {
        tokens[255] = last;
    }
    assert_eq!(tokens.len(), 256);
    body(&tokens)
}

fn index_cases() -> Vec<IndexCase> {
    let prefix61 = "x".repeat(61);
    let long_left = format!("\"{prefix61}BAA\"");
    let long_right = format!("\"{prefix61}ACO\"");
    let prefix63 = "x".repeat(63);
    let late_a = format!("\"{prefix63}a\"");
    let late_b = format!("\"{prefix63}b\"");
    let late_escaped_a = format!("\"{prefix63}\\u0061\"");
    let escaped64 = format!("\"{}\"", "\\u00e9".repeat(32));
    let raw64 = format!("\"{}\"", "\u{e9}".repeat(32));
    let escaped65 = format!("\"{}a\"", "\\u00e9".repeat(32));
    let first_escaped = "\"\\u006b000-BAA\"";
    let first_raw = "\"k000-BAA\"";

    assert_collision("\"BAA\"", "\"ACO\"");
    assert_collision(&long_left, &long_right);
    assert_eq!(host_decoded(&long_left).len(), 64);
    assert_collision("\"\u{c1}\u{410}\"", "\"\u{c0}\u{49e}\"");
    assert_collision("\"\\u00c1\\u0410\"", "\"\u{c0}\u{49e}\"");
    assert_collision("\"\\u0000CO\"", "\"\\u0001AA\"");
    assert_collision("\"k000-BAA\"", "\"k000-ACO\"");
    assert_equivalent("\"\\u0042AA\"", "\"BAA\"");
    assert_equivalent("\"\\u00c1\\u0410\"", "\"\u{c1}\u{410}\"");
    assert_equivalent("\"\\uD83D\\uDE00\"", "\"\u{1f600}\"");
    assert_equivalent("\"\\ud83d\\ude00\"", "\"\\uD83D\\uDE00\"");
    assert_equivalent("\"\\u0000\\u0043O\"", "\"\\u0000CO\"");
    assert_equivalent(&late_escaped_a, &late_a);
    assert_equivalent(&escaped64, &raw64);
    assert_equivalent(first_escaped, first_raw);
    assert_eq!(host_decoded(&escaped64).len(), 64);
    assert_eq!(host_decoded(&escaped65).len(), 65);
    assert_ne!(host_decoded("\"\\u0000\""), host_decoded("\"\\u0080\""));
    assert_ne!(host_decoded("\"a\\u0000\""), host_decoded("\"a\""));

    let malformed = b"{\"id\":\"a\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"b\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"a\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"c\"\n".to_vec();
    let malformed_first = b"{\"id\":\"a\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"b\"\n{\"id\":\"a\",\"label\":\"l\",\"quantity\":1}\n".to_vec();
    let schema_first = b"{\"id\":\"a\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"b\",\"label\":\"l\"}\n{\"id\":\"a\",\"label\":\"l\",\"quantity\":1}\n".to_vec();
    let overflow_first = b"{\"id\":\"a\",\"label\":\"l\",\"quantity\":9223372036854775807}\n{\"id\":\"b\",\"label\":\"l\",\"quantity\":1}\n{\"id\":\"a\",\"label\":\"l\",\"quantity\":1}\n".to_vec();

    let mut cases = vec![
        case(
            "ascii-collision-distinct",
            body(&["\"BAA\"", "\"ACO\""]),
            None,
            None,
        ),
        case(
            "prefix-64-collision-distinct",
            body(&[&long_left, &long_right]),
            None,
            None,
        ),
        case(
            "raw-nonascii-collision-distinct",
            body(&["\"\u{c1}\u{410}\"", "\"\u{c0}\u{49e}\""]),
            None,
            None,
        ),
        case(
            "escaped-nonascii-collision-distinct",
            body(&["\"\\u00c1\\u0410\"", "\"\u{c0}\u{49e}\""]),
            None,
            None,
        ),
        case(
            "nul-collision-distinct",
            body(&["\"\\u0000CO\"", "\"\\u0001AA\""]),
            None,
            None,
        ),
        case(
            "nul-and-nonascii-distinct",
            body(&[
                "\"\\u0000\"",
                "\"\\u0080\"",
                "\"a\\u0000\"",
                "\"a\"",
                "\"\\u0000a\"",
            ]),
            None,
            None,
        ),
        case(
            "collision-then-escaped-ascii-duplicate",
            body(&["\"ACO\"", "\"BAA\"", "\"\\u0042AA\""]),
            Some("duplicate_id"),
            Some(2),
        ),
        case(
            "collision-then-escaped-nonascii-duplicate",
            body(&[
                "\"\u{c0}\u{49e}\"",
                "\"\u{c1}\u{410}\"",
                "\"\\u00c1\\u0410\"",
            ]),
            Some("duplicate_id"),
            Some(2),
        ),
        case(
            "surrogate-equals-raw",
            body(&["\"\\uD83D\\uDE00\"", "\"\u{1f600}\""]),
            Some("duplicate_id"),
            Some(1),
        ),
        case(
            "surrogate-case-equivalent",
            body(&["\"x\"", "\"\\ud83d\\ude00\"", "\"\\uD83D\\uDE00\""]),
            Some("duplicate_id"),
            Some(2),
        ),
        case(
            "nul-escape-duplicate",
            body(&["\"\\u0001AA\"", "\"\\u0000CO\"", "\"\\u0000\\u0043O\""]),
            Some("duplicate_id"),
            Some(2),
        ),
        case(
            "common-prefix-late-escape-duplicate",
            body(&[&late_a, &late_b, &late_escaped_a]),
            Some("duplicate_id"),
            Some(2),
        ),
        case(
            "exact-64-escaped-equals-raw",
            body(&[&escaped64, &raw64]),
            Some("duplicate_id"),
            Some(1),
        ),
        case(
            "escaped-65-rejected-before-index",
            body(&[&escaped64, &escaped65]),
            Some("oversized_input"),
            Some(1),
        ),
        case(
            "early-duplicate-precedes-later-malformed",
            malformed,
            Some("duplicate_id"),
            Some(2),
        ),
        case(
            "malformed-precedes-later-duplicate",
            malformed_first,
            Some("malformed_json"),
            Some(1),
        ),
        case(
            "schema-precedes-later-duplicate",
            schema_first,
            Some("schema"),
            Some(1),
        ),
        case(
            "overflow-precedes-later-duplicate",
            overflow_first,
            Some("overflow"),
            Some(1),
        ),
        case(
            "rows-256-colliding-distinct",
            colliding_rows(None),
            None,
            None,
        ),
        case(
            "rows-256-last-raw-duplicates-escaped-first",
            colliding_rows(Some(first_raw)),
            Some("duplicate_id"),
            Some(255),
        ),
    ];
    let mut enriched = case(
        "enriched-nonascii-collision-distinct",
        body(&["\"\\u00c1\\u0410\"", "\"\u{c0}\u{49e}\""]),
        None,
        None,
    );
    enriched.enriched = true;
    cases.push(enriched);
    cases
}

/// Runs every decoded-index case through one already-authenticated project
/// program, so no additional capacity-sensitive graph is constructed.
pub(super) fn assert_decoded_index_cases(program: &ResolvedProgram) -> Result<(), Vec<Diagnostic>> {
    for case in index_cases() {
        let expected = run_oracle_with(&case.input, case.enriched);
        let response: serde_json::Value = serde_json::from_slice(&expected).unwrap();
        match case.category {
            Some(category) => {
                assert_eq!(
                    response["status"], "error",
                    "oracle status for {}",
                    case.name
                );
                assert_eq!(
                    response["category"], category,
                    "oracle category for {}",
                    case.name
                );
                assert_eq!(
                    response["record_index"].as_u64(),
                    case.record_index,
                    "oracle record index for {}",
                    case.name
                );
            }
            None => assert_eq!(response["status"], "ok", "oracle status for {}", case.name),
        }
        let function = if case.enriched {
            "catalog_normalizer.app.normalize-enriched"
        } else {
            "catalog_normalizer.app.normalize"
        };
        let actual = evaluate_resolved_owned_data(program, function, &case.input, MAX_STEPS_LIMIT)?;
        assert_eq!(
            actual.outcome,
            OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(expected)),
            "{function} disagreed with the independent oracle for decoded-index case {}",
            case.name
        );
    }
    Ok(())
}

#[test]
fn decoded_index_cases_are_deliberate_under_the_host_decoded_byte_oracle() {
    let cases = index_cases();
    assert_eq!(cases.len(), 21, "decoded-index case inventory drifted");
    for case in &cases {
        assert!(
            case.input.len() <= 65_536,
            "{} exceeds the request bound",
            case.name
        );
    }
}

#[test]
fn frozen_pairwise_reference_preserves_the_complete_normalizer_oracle() {
    let scratch = ScratchRoot::new();
    copy_fixture(&fixture(), scratch.path());
    std::fs::write(
        scratch.path().join("src/batch.spx"),
        include_str!("../catalog_normalizer_batch_pairwise.txt"),
    )
    .unwrap();
    std::fs::write(
        scratch.path().join("src/record.spx"),
        include_str!("../catalog_normalizer_record_pairwise.txt"),
    )
    .unwrap();
    super::append_legacy_summary_adapter(scratch.path());
    // The same published36/source14/max160M, decoded-index, backend and fault
    // observers run unchanged against the pre-index pairwise reference.
    assert_batch_backend_matrix(scratch.path());
}

#[path = "decoded_index/backends.rs"]
mod backends;
