//! Full independent source custody, outside all allocation/budget observers.
use sha2::{Digest, Sha256};
#[test]
fn complete_old_receiver_and_full_source_views_remain_bound() {
    const ROOT: &str = "src/cleanup_plan/replay/factored/canonical_shared.rs";
    let inputs = [
        (ROOT, include_str!("../canonical_shared.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/canonical-intern-arena-854-next-20261016/src__cleanup_plan__replay__factored__canonical_shared.rs.before.txt"))),
        ("src/cleanup_plan/replay/factored.rs", include_str!("../../factored.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/canonical-intern-arena-854-next-20261016/src__cleanup_plan__replay__factored.rs.before.txt"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_weighted.rs", include_str!("inline_weighted.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/canonical-intern-arena-854-next-20261016/src__cleanup_plan__replay__factored__canonical_shared__inline_weighted.rs.before.txt"))),
    ];
    for (path, current, old) in inputs {
        assert_eq!(
            crate::source_reflows_merge::arena854_previous(path, current),
            old
        );
    }
    let old = inputs[0].2;
    let begin = old.find("// Invoked only after preflight").unwrap();
    let end = old.find("\n#[cfg(test)]\nmod tests;").unwrap();
    let copy = include_str!("before_arena.rs");
    assert!(copy.contains(&old[begin..end]));
    assert!(copy.contains("states: BTreeMap<SharedWeighted, usize>"));
    assert!(copy.contains("ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>"));
    assert!(!copy.contains("intern_arena"));
    let owners: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/canonical-intern-arena-854-joined-20261016/new-owners.json"
    )))
    .unwrap();
    let bodies = [
        include_str!("intern_arena.rs"),
        include_str!("before_arena.rs"),
        include_str!("arena_tests.rs"),
        include_str!("arena_source.rs"),
    ];
    assert_eq!(owners.as_array().unwrap().len(), bodies.len());
    for (row, body) in owners.as_array().unwrap().iter().zip(bodies) {
        let authenticated =
            crate::source_reflows_merge::arena854_input(row["source"].as_str().unwrap(), body);
        let body = authenticated.as_str();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(body.as_bytes()))
            ),
            row["sha256"].as_str().unwrap()
        );
    }
}
