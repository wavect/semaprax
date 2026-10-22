//! Closed whole-input successors for the original boxed-state source readers.
//! Restore only the omitted compact-result change and later catalog pins; all
//! original positional inverses and complete predecessor assertions still run.
use sha2::{Digest, Sha256};
#[path = "../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
fn authenticate(path: &str, supplied: &str, compiled: &str, length: usize, sha: &str) {
    assert_eq!(supplied, compiled, "complete live boxed reader input");
    assert_eq!(compiled.len(), length);
    assert_eq!(digest(compiled), sha);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        reader::read(&root.join(path), length, sha, 65536).unwrap(),
        compiled
    );
}
fn replace(input: &mut String, new: &str, old: &str, count: usize) {
    assert_eq!(
        input.matches(new).count(),
        count,
        "complete exact boxed inverse"
    );
    *input = input.replace(new, old);
}
pub(in crate::source_reflows_merge) fn input(source: &str, current: &str) -> String {
    if source == "docs/AGENT-SKILL-BUNDLE-V1.json" {
        return bundle(current);
    }
    let normalized = if source == "src/cleanup_plan/replay/path_join/boxed_states.rs" {
        authenticate(
            source,
            current,
            include_str!("../cleanup_plan/replay/path_join/boxed_states.rs"),
            7277,
            "1cb644157ccf8246f6a81011d0dd099a64f1809cffae15bb3e993c1d64eb93c5",
        );
        let mut before = current.to_owned();
        replace(
            &mut before,
            "use crate::cleanup_plan::replay::physical_budget::ByteFailure;\n",
            "",
            1,
        );
        replace(
            &mut before,
            concat!(
                "            // At most one positive-payment failure reaches conversion. Pay its\n",
                "            // actual/formal compact values and Diagnostic construction/return\n",
                "            // once, before any entry; sticky failure forbids reuse.\n",
                "            + 2 * size_of::<ByteFailure>()\n",
                "            + 2 * size_of::<Diagnostic>()\n"
            ),
            "",
            1,
        );
        replace(
            &mut before,
            "+ size_of::<Result<(), ByteFailure>>()",
            "+ size_of::<Result<(), Diagnostic>>()",
            1,
        );
        replace(
            &mut before,
            "budget.reserve_compact_bytes(",
            "budget.reserve_bytes(",
            4,
        );
        replace(
            &mut before,
            "Some(error.into_diagnostic())",
            "Some(error)",
            4,
        );
        assert_eq!(before.len(), 6796);
        assert_eq!(
            digest(&before),
            "9e736102f9b39a731db6ab6ae3d17280eb04e5fa7df913c86cc503ec1771566f"
        );
        before
    } else {
        current.to_owned()
    };
    super::super::compile_fixture_input(source, &normalized)
}
fn bundle(current: &str) -> String {
    authenticate(
        "docs/AGENT-SKILL-BUNDLE-V1.json",
        current,
        include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json"),
        5544,
        "11d24f97cac3095b4995eb789108f3d2f0588ec71351cf9a8f64f92a669b25b2",
    );
    let receipt = include_str!("../../docs/evidence/boxed-path-states-v1/catalog-receipt.json");
    authenticate(
        "docs/evidence/boxed-path-states-v1/catalog-receipt.json",
        receipt,
        receipt,
        receipt.len(),
        "bb2d89942ce7b5ad759a787bcb1a0c5a0f1def93075fb70966089d30d1fdc279",
    );
    let mut before = current.to_owned();
    replace(
        &mut before,
        "sha256:232db0c55ab98a09264d0cd816d6e0c01cbf162cbfa8290138adbe14ac5c71c7",
        "sha256:023c0d3ea1823196a2f5d44e3b10a862df939f834e98d7a164b3245af340a73c",
        1,
    );
    replace(
        &mut before,
        "sha256:b181f78350e5b2294341e84f136d9e07abb65f4d9b74b4d89a45a3560ba7bb10",
        "sha256:479c7de1c209061d5706e405ffea92358aef690d02a536dd1f9162c13c977e85",
        1,
    );
    replace(&mut before, "\"code_count\":1517", "\"code_count\":1513", 1);
    assert_eq!(
        digest(&before),
        "bb1d335ef40cff2cf81d08ddcf0943b168d04721ee1e7de1b3d809d3b9600d2f"
    );
    before
}
