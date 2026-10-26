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
        "a140b4074a80af47f3615831ac01354a851f0789b9f03265526f2d457ffc05a9",
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
    // Restore the entire authenticated 2837 bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:af6e6094624826672ef9c1101fa970d419253f699ff043e9d70cc294056b660b",
        "sha256:cb997dd38feea65c0ba677503350005b9d9d3dee885c6812947cdc364018eb4d",
        1,
    );
    replace(
        &mut before,
        "sha256:bb992492efa1a070766040e0adb0943c032fd051ec132d5444a1e1f478fb5342",
        "sha256:7413596845b7d232f8ff8c7b9c3d9318acd4b7b5324d6deb64d8e0f8b891c86e",
        1,
    );
    assert_eq!(digest(&before), "f4031b6b357499122748e9d0c5de0d3641f45855566ed899ff600347688210c8");
    // Restore the entire authenticated 2837 bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:cb997dd38feea65c0ba677503350005b9d9d3dee885c6812947cdc364018eb4d",
        "sha256:525efa443c2b2a1f97b64debeac080e3a82b092ed510feb416798f29ad58ed48",
        1,
    );
    replace(
        &mut before,
        "sha256:7413596845b7d232f8ff8c7b9c3d9318acd4b7b5324d6deb64d8e0f8b891c86e",
        "sha256:bb7816577dde8feaf180dfa647195c9abd4a1dfac194ddfa55c9b7ae3e22b134",
        1,
    );
    assert_eq!(digest(&before), "77cc2280493407e6f3a1c4247f778cceaf09be14f1421f6f23787f8c4f0dabf7");
    // Restore the entire authenticated 2837 bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:525efa443c2b2a1f97b64debeac080e3a82b092ed510feb416798f29ad58ed48",
        "sha256:d9abead8fcb85521d6ca6183d62f885ba3b85558f81384c960b0db01b948add8",
        1,
    );
    replace(
        &mut before,
        "sha256:bb7816577dde8feaf180dfa647195c9abd4a1dfac194ddfa55c9b7ae3e22b134",
        "sha256:7f4788567710a86259ceb3621963db6ca499ad6ac5323d1b4497f313b52c635b",
        1,
    );
    assert_eq!(digest(&before), "12698588bbf09117e96fe8f8f62f74b1290f37fc7d8d0e6e20dfe54f8bc02c65");
    // Restore the entire authenticated 56fb bundle before its unchanged old stages.
    replace(
        &mut before,
        "sha256:d9abead8fcb85521d6ca6183d62f885ba3b85558f81384c960b0db01b948add8",
        "sha256:e894c9102dfb2fc1b29d48ee64265786dd18ec9491417cea9725aaac3a40d96f",
        1,
    );
    replace(
        &mut before,
        "sha256:7f4788567710a86259ceb3621963db6ca499ad6ac5323d1b4497f313b52c635b",
        "sha256:19cb70ce2a043430d6fd71b60ff203ce2cf26390ed4ca01339c8259efb7d36ed",
        1,
    );
    assert_eq!(digest(&before), "51c95c8f2849633d9abfd66812f57429d8ff9c72b2d9882af01179bdbd9862dc");
    replace(
        &mut before,
        "sha256:19cb70ce2a043430d6fd71b60ff203ce2cf26390ed4ca01339c8259efb7d36ed",
        "sha256:023c0d3ea1823196a2f5d44e3b10a862df939f834e98d7a164b3245af340a73c",
        1,
    );
    replace(
        &mut before,
        "sha256:e894c9102dfb2fc1b29d48ee64265786dd18ec9491417cea9725aaac3a40d96f",
        "sha256:479c7de1c209061d5706e405ffea92358aef690d02a536dd1f9162c13c977e85",
        1,
    );
    replace(&mut before, "\"code_count\":1517", "\"code_count\":1513", 1);
    replace(
        &mut before,
        "sha256:bad33c1869a184361663b0a9f4b5d46a6fb9a52433c94d66289c8456ced75b67",
        "sha256:cb620f216f1fe93bad2c50db7e07448679497e73857a0104c499f3d22238ea3c",
        1,
    );
    assert_eq!(
        digest(&before),
        "bb1d335ef40cff2cf81d08ddcf0943b168d04721ee1e7de1b3d809d3b9600d2f"
    );
    before
}
