//! Exact source-stage inverse for the complete historical fixture controls.
use sha2::{Digest, Sha256};
pub(super) fn before(current: &str) -> String {
    let compiled = include_str!("tests.rs");
    assert_eq!(
        current, compiled,
        "only the complete current controls are admitted"
    );
    let physical = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/source_reflows_merge/runtime_json/tests.rs"),
    )
    .unwrap();
    assert_eq!(
        current, physical,
        "fresh physical source agrees with compiled source"
    );
    let edits: Vec<(usize, String, String)> = serde_json::from_str(include_str!(
        "../../../docs/evidence/runtime-history-20261021/tests-inverse.json"
    ))
    .unwrap();
    let mut original = current.to_owned();
    for (at, after, prior) in edits.into_iter().rev() {
        assert_eq!(&original[at..at + after.len()], after);
        original.replace_range(at..at + after.len(), &prior);
    }
    assert_eq!(original.len(), 169740);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "072667a0bf8d06eeb7cb635014a707b3adbb9e43845a289b9fcfdb688f678f2d"
    );
    original
}
