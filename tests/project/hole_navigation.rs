//! Compact typed-hole navigation evidence, authored and intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft,
    ProjectCandidateDraftArchive,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-hole-navigation-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn draft(&self) -> ProjectCandidateDraft {
        let base = with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
                .map(Arc::new)
        })
        .unwrap();
        let expression = selected(&base, "calculator.is-negative", false, "value < 0");
        let contract = selected(&base, "calculator.divide", true, "right != 0");
        let draft = ProjectCandidateDraft::open(base).unwrap();
        let draft = draft
            .with_body_hole(draft.draft_digest(), "calculator.add", "body")
            .unwrap();
        let draft = draft
            .with_expression_hole(
                draft.draft_digest(),
                "calculator.is-negative",
                &expression,
                "expression",
            )
            .unwrap();
        draft
            .with_contract_expression_hole(
                draft.draft_digest(),
                "calculator.divide",
                &contract,
                "contract",
            )
            .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn selected(base: &ProjectCandidate, target: &str, contract: bool, snippet: &str) -> String {
    let catalog: Value = serde_json::from_str(
        &if contract {
            base.contract_expression_catalog(target)
        } else {
            base.expression_catalog(target)
        }
        .unwrap(),
    )
    .unwrap();
    let source = base
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/core.spx")
        .unwrap()
        .source();
    let rows = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let span = &row["source_span"];
            row["replaceable"] == true
                && source.get(
                    span["start"].as_u64().unwrap() as usize
                        ..span["end"].as_u64().unwrap() as usize,
                ) == Some(snippet)
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    rows[0]["expression_id"].as_str().unwrap().to_owned()
}
fn summary(draft: &ProjectCandidateDraft, hole: &str) -> Value {
    serde_json::from_str(&draft.hole_summary(draft.draft_digest(), hole).unwrap()).unwrap()
}
fn reference(summary: &Value, name: &str) -> String {
    summary["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|facet| facet["facet"] == name)
        .unwrap()["reference"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("expected rejection");
    assert!(
        errors.iter().any(|error| error.code == expected),
        "{errors:?}"
    );
}
fn hash(domain: &[u8], text: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}
fn pages(draft: &ProjectCandidateDraft, hole: &str, summary: &Value, facet: &str) -> Vec<Value> {
    let reference = reference(summary, facet);
    let mut offset = 0;
    let mut all = Vec::new();
    loop {
        let text = draft
            .hole_page(draft.draft_digest(), hole, &reference, offset, 1)
            .unwrap();
        assert!(text.len() <= 65536);
        let page: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(page["schema"], "semaprax.project-hole-page.v1");
        assert_eq!(page["draft_revision"], draft.draft_digest());
        assert_eq!(page["hole_id"], hole);
        assert_eq!(page["context_revision"], summary["context_revision"]);
        assert_eq!(page["reference"], reference);
        assert_eq!(page["facet"], facet);
        assert_eq!(page["offset"], offset);
        assert_eq!(page["source_authority"], false);
        let items = page["items"].as_array().unwrap();
        assert!(items.len() <= 1);
        all.extend(items.iter().cloned());
        match page["next_offset"].as_u64() {
            Some(next) => {
                assert!(!items.is_empty());
                assert_eq!(next as usize, offset + items.len());
                offset = next as usize;
            }
            None => {
                assert!(page["next_offset"].is_null());
                assert_eq!(page["total"], all.len());
                break;
            }
        }
    }
    all
}

#[test]
fn all_three_hole_kinds_page_exact_existing_context_facts_without_changing_full_context() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let draft = fixture.draft();
    let before = draft.to_json().to_owned();
    for hole in ["body", "expression", "contract"] {
        let full = draft.hole_context(draft.draft_digest(), hole).unwrap();
        let context: Value = serde_json::from_str(&full).unwrap();
        let compact = summary(&draft, hole);
        assert_eq!(summary(&draft, hole), compact);
        assert_eq!(compact["schema"], "semaprax.project-hole-summary.v1");
        assert_eq!(compact["context_schema"], context["schema"]);
        assert_eq!(
            compact["context_revision"],
            hash(b"semaprax.project-hole-context.v1\0", &full)
        );
        for key in [
            "hole_id",
            "hole_handle",
            "target",
            "last_valid_revision",
            "expected_type_id",
            "intent_kind",
        ] {
            assert_eq!(compact[key], context[key], "{hole} {key}");
        }
        assert_eq!(compact["expected_ownership"], context["expected_ownership"]);
        assert_eq!(compact["draft_revision"], draft.draft_digest());
        assert_eq!(compact["source_authority"], false);
        assert_eq!(compact["materializable"], false);
        assert_eq!(compact["validation"], "pending_fill_full_source_replay");
        assert_eq!(
            compact["evidence_class"],
            "descriptive_context_not_candidate_validation"
        );
        assert_eq!(compact["full_context_method"], "hole/query");
        let facets = compact["facets"].as_array().unwrap();
        assert_eq!(
            facets
                .iter()
                .map(|facet| facet["facet"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["scope", "calls", "obligations", "constructors"]
        );
        let mut references = std::collections::BTreeSet::new();
        for facet in facets {
            assert!(references.insert(facet["reference"].as_str().unwrap()));
        }
        let scope = pages(&draft, hole, &compact, "scope");
        let original = context["scope"].as_array().unwrap();
        assert_eq!(scope.len(), original.len());
        for (actual, expected) in scope.iter().zip(original) {
            assert_eq!(actual["name"], expected["name"]);
            assert_eq!(actual["ownership"], expected["ownership"]);
            if hole == "body" {
                assert_eq!(actual["id"], expected["id"]);
                assert_eq!(actual["type_id"], expected["type_id"]);
                assert!(actual["mutable"].is_null());
            } else {
                assert_eq!(actual["id"], expected["value_id"]);
                assert_eq!(actual["type_id"], expected["type"]);
                assert_eq!(actual["mutable"], expected["mutable"]);
            }
        }
        for (facet, key) in [
            ("calls", "accessible_calls"),
            ("obligations", "obligations"),
            ("constructors", "constructor_kinds"),
        ] {
            let items = pages(&draft, hole, &compact, facet);
            assert_eq!(items, *context[key].as_array().unwrap());
            assert_eq!(
                facets.iter().find(|row| row["facet"] == facet).unwrap()["count"],
                items.len()
            );
        }
        for key in [
            "allowed",
            "forbidden",
            "module_permits",
            "enclosing_declared_effects",
        ] {
            assert_eq!(compact["effect_policy"][key], context["effect_policy"][key]);
        }
        assert_eq!(compact["effect_policy"]["allowed"], json!([]));
        if hole == "contract" {
            assert_eq!(
                compact["effect_policy"]["forbidden"],
                "all_effects_in_contract_predicates"
            );
            assert_eq!(
                compact["effect_policy"]["enclosing_declared_effects"],
                json!([])
            );
        } else {
            assert_eq!(
                compact["effect_policy"]["forbidden"],
                "all_undeclared_effects"
            );
            assert!(compact["effect_policy"]["enclosing_declared_effects"].is_null());
        }
        assert_eq!(
            draft.hole_context(draft.draft_digest(), hole).unwrap(),
            full
        );
    }
    assert_eq!(draft.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
    code(draft.complete(draft.draft_digest()), "SPX-G232");
}

#[test]
fn references_bind_draft_hole_context_and_facet_and_bounds_leave_parent_unchanged() {
    let fixture = Fixture::new();
    let draft = fixture.draft();
    let before = draft.to_json().to_owned();
    let old = summary(&draft, "expression");
    let old_ref = reference(&old, "scope");
    code(
        draft.hole_page(draft.draft_digest(), "contract", &old_ref, 0, 1),
        "SPX-G232",
    );
    code(
        draft.hole_page(
            draft.draft_digest(),
            "expression",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            0,
            1,
        ),
        "SPX-G232",
    );
    for (offset, limit) in [(0, 0), (0, 65), (16385, 1)] {
        code(
            draft.hole_page(draft.draft_digest(), "expression", &old_ref, offset, limit),
            "SPX-G230",
        );
    }
    let filled = draft
        .fill_hole(
            draft.draft_digest(),
            "body",
            &json!({"kind":"i64","value":7}),
        )
        .unwrap();
    let next = summary(&filled, "expression");
    assert_ne!(next["context_revision"], old["context_revision"]);
    assert_ne!(reference(&next, "scope"), old_ref);
    code(
        filled.hole_summary(draft.draft_digest(), "expression"),
        "SPX-G232",
    );
    code(
        filled.hole_page(filled.draft_digest(), "expression", &old_ref, 0, 1),
        "SPX-G232",
    );
    assert_eq!(summary(&draft, "expression"), old);
    assert_eq!(draft.to_json(), before);
}

#[test]
fn compact_references_survive_exact_source_archive_replay_after_origin_is_removed() {
    let fixture = Fixture::new();
    let draft = fixture.draft();
    let compact = summary(&draft, "contract");
    let expected = pages(&draft, "contract", &compact, "calls");
    let archive = ProjectCandidateDraftArchive::prepare(&draft, draft.draft_digest()).unwrap();
    std::fs::remove_dir_all(&fixture.0).unwrap();
    let restored = ProjectCandidateDraftArchive::restore(
        archive.to_json().as_bytes(),
        archive.archive_digest(),
        archive.draft_digest(),
    )
    .unwrap();
    assert_eq!(summary(&restored, "contract"), compact);
    assert_eq!(pages(&restored, "contract", &compact, "calls"), expected);
    code(restored.complete(restored.draft_digest()), "SPX-G232");
    assert!(!fixture.0.exists());
}
