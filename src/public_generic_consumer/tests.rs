//! Evidence for the metadata format and the generators themselves. Compiling
//! and running the four generated consumers is a separate gate in the
//! projections harness, because it needs foreign toolchains.

use std::path::Path;

use super::*;
use crate::hir;
use crate::parse;
use crate::public_generic_surface::CandidateSurface;

const SOURCE: &str = r#"
module test.public_generic_consumer;

@id("consumer.leaf")
record Leaf {
    @id("consumer.leaf.head")
    head: Bytes,
    @id("consumer.leaf.tag")
    tag: i64,
}

@id("consumer.pair")
record Pair<T, U> {
    @id("consumer.pair.left")
    left: T,
    @id("consumer.pair.right")
    right: U,
}

@id("consumer.take")
fn take(value: own Pair<Leaf, bool>) -> i64 {
    match own value {
        Pair { left: Leaf { head: payload, tag: tag }, right: present } =>
            if present && tag > 0 && byte_len(bytes_as_slice(payload)) > 0usize { 1 } else { 0 },
    }
}

@id("consumer.make")
fn make(input: borrow Slice<u8>) -> Pair<Leaf, bool> {
    Pair<Leaf, bool> {
        left: Leaf { head: bytes_copy(input), tag: 7 },
        right: byte_len(input) > 0usize,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn surface() -> CandidateSurface {
    let parsed = parse(SOURCE, Path::new("consumer.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    CandidateSurface::derive(
        &program,
        &["consumer.take".to_owned(), "consumer.make".to_owned()],
    )
    .unwrap()
}

fn metadata() -> ConsumerMetadata {
    ConsumerMetadata::of(&surface()).unwrap()
}

/// Every record kind appears, in the surface's own canonical order, and the
/// digest closes the document.
#[test]
fn metadata_carries_every_record_kind_in_canonical_order() {
    let metadata = metadata();
    let kinds = metadata
        .records()
        .iter()
        .map(|record| record[0].as_str())
        .collect::<Vec<_>>();
    for kind in ["E", "P", "R", "I", "A", "F", "L", "D"] {
        assert!(
            kinds.contains(&kind),
            "{kind} record missing from {kinds:?}"
        );
    }
    assert_eq!(kinds.last(), Some(&"D"));
    assert_eq!(
        metadata.records().last().unwrap()[1],
        surface().digest(),
        "the last record binds the surface digest, so a stale document differs"
    );
    let exports = metadata
        .records()
        .iter()
        .filter(|record| record[0] == "E")
        .map(|record| record[1].as_str())
        .collect::<Vec<_>>();
    assert_eq!(exports, vec!["consumer.make", "consumer.take"]);
}

/// The document is length-framed, so it round-trips exactly and the reference
/// reader accepts its own bytes.
#[test]
fn canonical_metadata_round_trips_and_is_accepted() {
    let metadata = metadata();
    let rendered = metadata.render();
    assert!(rendered.starts_with(CONSUMER_METADATA_MAGIC));
    assert_eq!(ConsumerMetadata::parse(&rendered).unwrap(), metadata);
    metadata.accepts(&rendered).unwrap();
    assert_eq!(
        ConsumerMetadata::parse(&rendered).unwrap().render(),
        rendered
    );
}

/// Hostile bytes fail closed, each with the reason that actually applies.
#[test]
fn hostile_metadata_fails_closed_with_a_closed_reason() {
    let metadata = metadata();
    let canonical = metadata.render();

    let mut cases: Vec<(String, Refusal, &str)> = vec![
        (String::new(), Refusal::Malformed, "empty"),
        (
            canonical.replacen("spxpgcm1;", "spxpgcm2;", 1),
            Refusal::Malformed,
            "wrong magic",
        ),
        (
            canonical[..canonical.len() - 1].to_owned(),
            Refusal::Malformed,
            "truncated tail",
        ),
        (
            canonical[..CONSUMER_METADATA_MAGIC.len() + 4].to_owned(),
            Refusal::Malformed,
            "truncated record",
        ),
        (
            format!("{canonical}|3;1:E;1:x;1:0;"),
            Refusal::Mismatch,
            "appended record",
        ),
        (
            canonical.replacen("|3;", "|03;", 1),
            Refusal::Malformed,
            "leading zero field count",
        ),
        (
            canonical.replacen("|3;", "|9;", 1),
            Refusal::Malformed,
            "field count beyond the record",
        ),
        (
            canonical.replacen("|3;", "|0;", 1),
            Refusal::Malformed,
            "zero field count",
        ),
        (
            CONSUMER_METADATA_MAGIC.to_owned(),
            Refusal::Malformed,
            "header only",
        ),
        (
            format!("{canonical}\n"),
            Refusal::Malformed,
            "trailing newline",
        ),
    ];

    // A forged term: the length prefix no longer matches the identity bytes.
    let forged = canonical.replacen("@13:consumer.leaf<>", "@12:consumer.leaf<>", 1);
    assert_ne!(forged, canonical, "the fixture must contain that term");
    cases.push((forged, Refusal::Term, "forged term length"));

    // A different but well-formed document: same shape, other content.
    let other = canonical.replacen("1:E;", "1:Z;", 1);
    cases.push((other, Refusal::Mismatch, "unknown record kind"));

    for (submitted, expected, label) in cases {
        let refusal = metadata
            .accepts(&submitted)
            .expect_err(&format!("{label} must be refused"));
        assert_eq!(refusal, expected, "{label}");
    }
}

/// Reordering records keeps every fact and still fails closed: the document is
/// compared as bytes, not as a set.
#[test]
fn reordered_records_are_refused() {
    let metadata = metadata();
    let canonical = metadata.render();
    let mut records = metadata.records().to_vec();
    records.swap(0, 1);
    let reordered = ConsumerMetadata { records };
    let bytes = reordered.render();
    assert_ne!(bytes, canonical);
    assert_eq!(metadata.accepts(&bytes), Err(Refusal::Mismatch));
    // Still a well-formed document: only the expectation rejects it.
    ConsumerMetadata::parse(&bytes).unwrap();
}

/// Every refusal spelling is reachable, and the spellings are distinct.
#[test]
fn the_refusal_vocabulary_is_closed_and_reachable() {
    let mut spellings = Refusal::ALL.map(Refusal::text).to_vec();
    spellings.sort_unstable();
    let count = spellings.len();
    spellings.dedup();
    assert_eq!(spellings.len(), count);
    assert_eq!(spellings, vec!["malformed", "mismatch", "term"]);

    let metadata = metadata();
    let canonical = metadata.render();
    assert_eq!(metadata.accepts("").unwrap_err(), Refusal::Malformed);
    assert_eq!(
        metadata
            .accepts(&canonical.replacen("@13:consumer.leaf<>", "@12:consumer.leaf<>", 1))
            .unwrap_err(),
        Refusal::Term
    );
    assert_eq!(
        metadata
            .accepts(&canonical.replacen("1:E;", "1:Z;", 1))
            .unwrap_err(),
        Refusal::Mismatch
    );
}

/// Generation is deterministic, complete, and identity-derived: a generated
/// consumer names its types from bytes, not from display names.
#[test]
fn generation_is_deterministic_and_identity_derived() {
    let surface = surface();
    for language in ConsumerLanguage::ALL {
        let first = generate(&surface, language).unwrap();
        let second = generate(&surface, language).unwrap();
        assert_eq!(first, second, "{} is not deterministic", language.text());
        assert_eq!(
            first
                .files()
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            language.file_names().to_vec()
        );
        assert_eq!(
            first.metadata(),
            ConsumerMetadata::of(&surface).unwrap().render()
        );
        for (name, source) in first.files() {
            assert!(!source.is_empty(), "{name} is empty");
            for display_name in ["Pair", "Leaf", "take", "make", "left", "right", "head"] {
                assert!(
                    !source.contains(&format!("SpxPg{display_name}"))
                        && !source.contains(&format!("spx_pg_{display_name}")),
                    "{name} names the declaration {display_name} instead of its identity"
                );
            }
            assert!(
                source.contains("no layout, no ownership transfer, and no ABI"),
                "{name} must carry the nonclaim banner"
            );
        }
    }
}

/// The generated declarations put a nested instance before the instance that
/// holds it, or C and C++ would see an incomplete type.
#[test]
fn nested_declarations_precede_the_instances_that_hold_them() {
    let surface = surface();
    let leaf = identifier("@13:consumer.leaf<>");
    let pair = identifier("@13:consumer.pair<@13:consumer.leaf<>,bool>");
    for language in [ConsumerLanguage::C, ConsumerLanguage::Cxx] {
        let generated = generate(&surface, language).unwrap();
        let (_, source) = &generated.files()[0];
        let leaf_at = source.find(&leaf).expect("the nested record is declared");
        let pair_at = source
            .find(&pair)
            .expect("the holding instance is declared");
        assert!(
            leaf_at < pair_at,
            "{} declares the holder before the nested record",
            language.text()
        );
    }
}

/// The metadata the generator embeds is exactly the metadata the reference
/// reader accepts, so a consumer is never built on bytes its own algorithm
/// would refuse.
#[test]
fn generated_consumers_embed_accepted_metadata() {
    let surface = surface();
    let expected = ConsumerMetadata::of(&surface).unwrap();
    for language in ConsumerLanguage::ALL {
        let generated = generate(&surface, language).unwrap();
        expected.verify(generated.metadata()).unwrap();
        let embedded = generated
            .files()
            .iter()
            .find(|(_, source)| source.contains("0x73, 0x70, 0x78"))
            .expect("the metadata bytes are embedded as a numeric literal");
        assert!(
            embedded.1.matches("0x").count() >= generated.metadata().len(),
            "every metadata byte must be embedded"
        );
    }
}

/// Bounds refuse rather than truncate.
#[test]
fn metadata_bounds_refuse() {
    let oversized = format!("{}|9;", CONSUMER_METADATA_MAGIC);
    assert_eq!(
        ConsumerMetadata::parse(&oversized).unwrap_err(),
        Refusal::Malformed
    );
    let huge = "a".repeat(MAX_METADATA_BYTES + 1);
    assert_eq!(
        ConsumerMetadata::parse(&huge).unwrap_err(),
        Refusal::Malformed
    );
}
