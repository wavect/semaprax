//! Complete physical-input successors for the immutable CI and Source36 views.
//! This test-only projection owns no compiler authority and grants no admission.
use sha2::{Digest, Sha256};
fn project(rows: &str, source: &str, current: &str) -> String {
    let rows: serde_json::Value = serde_json::from_str(rows).expect("fixed merge projections");
    let matching: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["source"] == source)
        .collect();
    assert!(matching.len() <= 1, "unique merge source view");
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"]
    );
    let mut result = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(
            result.matches(new).count(),
            1,
            "complete unique merge inverse"
        );
        result = result.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(result.as_bytes()))
        ),
        row["original_sha256"]
    );
    result
}
// Exact post-build name/visibility repair, before every protected older domain.
// Unlisted sources remain unchanged; listed inputs must match their full new SHA.
fn before_build_fix(source: &str, current: &str) -> String {
    project(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/compiler-receiving-fix-7800-20261015/predecessor-projections.json")), source, current)
}
pub(crate) fn ours(source: &str, current: &str) -> String {
    let staged = project(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-composition-f813-held-20261015/pre-ours.json")), source, current);
    let current = staged.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source36-main88884-merge-20261015/ours-projections.json"
        )),
        source,
        current,
    )
}
pub(crate) fn main(source: &str, current: &str) -> String {
    let predecessor = assembled_previous(source, current);
    let current = predecessor.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-composition-f813-held-20261015/main-projections.json"
        )),
        source,
        current,
    )
}

pub(crate) fn held(source: &str, current: &str) -> String {
    let predecessor = assembled_previous(source, current);
    let current = predecessor.as_str();
    project(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-composition-f813-held-20261015/held-projections.json")), source, current)
}

pub(crate) fn weighted(path: &str, current: &str) -> String {
    if path == "../../../../../../docs/ARCHITECTURE.md" {
        return assembled_previous("docs/ARCHITECTURE.md", current);
    }
    if path != "../epsilon_alias/tests.rs" { return current.to_owned(); }
    project(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/source-composition-f813-held-20261015/weighted-projections.json")), "src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias/tests.rs", current)
}

// NEXT842 source transport precedes every immutable runtime domain.
fn inline_weighted_previous(source: &str, current: &str) -> String {
    project(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/inline-weighted-successor-842/source-inverses.json")), source, current)
}

// Separate authenticated #843 predecessor; older runtime manifests stay immutable.
pub(crate) fn closure_previous(source: &str, current: &str) -> String {
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/held-closure-receiver-843/runtime-projections.json"
        )),
        source,
        current,
    )
}

// Each isolated branch projector remains strict. Authenticate the actual final
// input separately into its named branch before comparing complete base bytes.
pub(crate) fn assembly_view(stage: &str, source: &str, current: &str) -> String {
    let data = match stage {
        "9d" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/final-842-843-source-assembly-20261015/view-9d.json")),
        "842" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/final-842-843-source-assembly-20261015/view-842.json")),
        "843" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/final-842-843-source-assembly-20261015/view-843.json")),
        _ => panic!("unknown immutable assembly source domain"),
    };
    project(data, source, current)
}
fn assembled_previous(source: &str, current: &str) -> String {
    let repaired = assembly_view("9d", source, current);
    let repaired = before_build_fix(source, &repaired);
    let inline = assembly_view("842", source, current);
    let inline = inline_weighted_previous(source, &inline);
    let closure = assembly_view("843", source, current);
    let closure = closure_previous(source, &closure);
    assert_eq!(repaired, inline, "independent branch sources must reach the same complete7800 input");
    assert_eq!(repaired, closure, "held branch source must reach the same complete7800 input");
    repaired
}
