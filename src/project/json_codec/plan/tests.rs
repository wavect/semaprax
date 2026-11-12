use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
thread_local! {
    static CHECKS: std::cell::Cell<[usize; 3]> = const { std::cell::Cell::new([0; 3]) };
}
pub(super) fn note_check(kind: usize) {
    CHECKS.with(|slot| {
        let mut counts = slot.get();
        counts[kind] += 1;
        slot.set(counts);
    });
}
pub(in crate::project) fn counts() -> [usize; 3] {
    CHECKS.with(|slot| slot.replace([0; 3]))
}
pub(in crate::project) fn fixture(
    changed: bool,
) -> (std::path::PathBuf, std::sync::Arc<ProjectRevision>) {
    let root = std::env::temp_dir().join(format!(
        "starter-candidate-895-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let original = include_str!("../../../../examples/source-command-project/semaprax.toml");
    let manifest = original.replace("sources = [\"app.spx\", \"tests.spx\"]",
        "sources = [\"src/app.spx\", \"src/first.spx\", \"src/second.spx\", \"src/tests.spx\"]")
        .replace("[dependencies]\n", "[dependencies]\nstd.data.json.digits = \"=0.1.0\"\nstd.data.json.query = \"=0.1.0\"\nstd.data.json.scan = \"=0.1.0\"\nstd.data.json.token = \"=0.1.0\"\nstd.data.json.write = \"=0.1.0\"\n");
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
    for (path, text) in [
        (
            "app.spx",
            include_str!("../../../../examples/source-command-project/app.spx"),
        ),
        (
            "tests.spx",
            include_str!("../../../../examples/source-command-project/tests.spx"),
        ),
    ] {
        std::fs::write(root.join("src").join(path), text).unwrap();
    }
    for (path, module, record, value) in [
        (
            "first.spx",
            "schema.first",
            "first.row",
            if changed { 1 } else { 0 },
        ),
        ("second.spx", "schema.second", "second.row", 0),
    ] {
        let text = format!("module {module}; @id(\"{record}\") record Row {{ @id(\"{record}.n\") n: i64, }} @id(\"{record}.anchor\") fn anchor()->i64 {{{value}}}");
        let parsed = crate::parse(&text, path).unwrap();
        let canonical = crate::format::canonical(&parsed);
        std::fs::write(root.join("src").join(path), canonical).unwrap();
    }
    let revision =
        crate::project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
    counts();
    (root, revision)
}
pub(in crate::project) fn entries() -> Vec<JsonCodecPlanEntry> {
    [
        ("src/first.spx", "first.row"),
        ("src/second.spx", "second.row"),
    ]
    .map(|(path, id)| JsonCodecPlanEntry {
        source_path: path.to_owned(),
        record_id: id.to_owned(),
        profile: JsonCodecProfile::FlatScalars,
    })
    .to_vec()
}
#[test]
fn private_replay_preserves_whole_bundle_bytes_and_public_independent_checks() {
    let (root, revision) = fixture(false);
    let entries = entries();
    let old = before::derive_json_codec_plan(&revision, &entries).unwrap();
    before::verify_json_codec_plan(&revision, &entries, &old).unwrap();
    assert_eq!(counts(), [2, 2, 2]);
    let actual = derive_replayed_json_codec_plan(&revision, &entries).unwrap();
    assert_eq!(counts(), [1, 1, 1]);
    assert_eq!(actual, old);
    assert_eq!(
        serde_json::to_vec(&actual).unwrap(),
        serde_json::to_vec(&old).unwrap()
    );
    let public = derive_json_codec_plan(&revision, &entries).unwrap();
    verify_json_codec_plan(&revision, &entries, &public).unwrap();
    assert_eq!(counts(), [2, 2, 2]);
    assert_eq!(public, old);
    let mut reordered = entries.clone();
    reordered.reverse();
    assert_eq!(
        derive_replayed_json_codec_plan(&revision, &reordered).unwrap(),
        old
    );
    assert_eq!(counts(), [1, 1, 1]);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn every_draft_field_and_complete_byte_inventory_must_match_regeneration() {
    let (root, revision) = fixture(false);
    let entries = entries();
    let expected =
        refusal("JSON codec bundle differs from its exact immutable-revision plan replay");
    // Every actual Draft field, every source tuple field, and all nine entry
    // fields have a separate hostile mutation. Hashes never replace full bytes.
    for mutation in 0..23 {
        let mut draft = derive_draft(&revision, &entries).unwrap();
        match mutation {
            0 => draft.authored_project_revision.push('x'),
            1 => draft.manifest.push(' '),
            2 => draft.authored_sources.clear(),
            3 => draft.authored_sources.reverse(),
            4 => draft.authored_sources[0].path.push('x'),
            5 => draft.authored_sources[0].source.push(' '),
            6 => draft.authored_sources[0].sha256.push('x'),
            7 => draft.candidate_sources.clear(),
            8 => draft.candidate_sources.reverse(),
            9 => draft.candidate_sources[0].path.push('x'),
            10 => draft.candidate_sources[0].source.push(' '),
            11 => draft.entries.clear(),
            12 => draft.entries.reverse(),
            13 => draft.entries[0].source_path.push('x'),
            14 => draft.entries[0].record_id.push('x'),
            15 => draft.entries[0].profile.push('x'),
            16 => draft.entries[0].max_string_bytes = Some(1),
            17 => draft.entries[0].max_array_items = Some(1),
            18 => draft.entries[0].authored_sha256.push('x'),
            19 => draft.entries[0].candidate_sha256.push('x'),
            20 => draft.entries[0].generated_declarations.clear(),
            21 => {
                let source = draft
                    .candidate_sources
                    .iter_mut()
                    .find(|s| s.path == draft.entries[0].source_path)
                    .unwrap();
                source.source.push(' ');
                draft.entries[0].candidate_sha256 = digest(&source.source);
            }
            22 => {
                draft.authored_sources[0].source.push(' ');
                draft.authored_sources[0].sha256 = digest(&draft.authored_sources[0].source);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            replay_draft(&revision, &entries, draft).unwrap_err(),
            expected,
            "mutation {mutation}"
        );
        assert_eq!(
            counts(),
            [0, 0, 0],
            "forged draft must not enter candidate build"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn stale_revision_and_mixed_invalid_plan_keep_exact_old_first_diagnostics() {
    let (root, revision) = fixture(false);
    let (changed_root, changed) = fixture(true);
    assert_ne!(revision.project_revision(), changed.project_revision());
    let plan = entries();
    let draft = derive_draft(&revision, &plan).unwrap();
    assert_eq!(
        replay_draft(&changed, &plan, draft).unwrap_err(),
        refusal("JSON codec bundle differs from its exact immutable-revision plan replay")
    );
    assert_eq!(counts(), [0, 0, 0]);
    let mut bad = plan.clone();
    bad[1].record_id = "missing.row".into();
    let mut duplicate = plan.clone();
    duplicate[1].source_path = duplicate[0].source_path.clone();
    for selected in [vec![], bad, duplicate] {
        let old = before::derive_json_codec_plan(&revision, &selected).unwrap_err();
        assert_eq!(
            derive_json_codec_plan(&revision, &selected).unwrap_err(),
            old
        );
        assert_eq!(
            derive_replayed_json_codec_plan(&revision, &selected).unwrap_err(),
            old
        );
        assert_eq!(counts(), [0, 0, 0]);
    }
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(changed_root).unwrap();
}

pub(in crate::project) fn measure_draft_overlap(revision: &ProjectRevision) {
    use crate::test_allocation_observer::{current_metrics, measure_with_capacity};

    let entries = entries();
    let ((first_live, overlap_live, remaining_live), metrics) =
        measure_with_capacity(131_072, || {
            let draft = derive_draft(revision, &entries).unwrap();
            let first_live = current_metrics().live;
            let regenerated = derive_draft(revision, &entries).unwrap();
            assert!(draft == regenerated);
            let overlap_live = current_metrics().live;
            drop(regenerated);
            let remaining_live = current_metrics().live;
            drop(draft);
            (first_live, overlap_live, remaining_live)
        });

    assert_eq!(counts(), [0, 0, 0]);
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(first_live > 0 && overlap_live > first_live);
    assert_eq!(remaining_live, first_live);
    eprintln!(
        "OPT895 draft overlap first={first_live} both={overlap_live} after_replay_drop={remaining_live} {metrics:?}"
    );
}

#[test]
fn draft_layout_invoice_names_all_transport_and_backing_fields() {
    eprintln!(
        "OPT895 typed bytes: Draft={} ResultDraft={} Bundle={} ResultBundle={} source_header={}",
        std::mem::size_of::<Draft>(),
        std::mem::size_of::<Result<Draft, Vec<Diagnostic>>>(),
        std::mem::size_of::<JsonCodecBundle>(),
        std::mem::size_of::<Result<JsonCodecBundle, Vec<Diagnostic>>>(),
        std::mem::size_of::<SemanticWorkspaceSource>()
    );
    // Actual typed carrier, not a guessed word-width or a checked token. Dynamic
    // path/source/receipt backing and overlapping lifetimes are measured by the
    // complete starter observer, never inferred from this fixed header equation.
    assert_eq!(
        std::mem::size_of::<Draft>(),
        2 * std::mem::size_of::<String>()
            + std::mem::size_of::<Vec<JsonCodecBundleSource>>()
            + std::mem::size_of::<Vec<SemanticWorkspaceSource>>()
            + std::mem::size_of::<Vec<JsonCodecEntryReceipt>>()
    );
}
#[test]
fn public_bundle_every_field_still_requires_independent_complete_old_replay() {
    let (root, revision) = fixture(false);
    let entries = entries();
    let bundle = derive_json_codec_plan(&revision, &entries).unwrap();
    counts();
    for mutation in 0..9 {
        let mut forged = bundle.clone();
        match mutation {
            0 => forged.schema.push('x'),
            1 => forged.authored_project_revision.push('x'),
            2 => forged.candidate_project_revision.push('x'),
            3 => forged.candidate_workspace_revision.push('x'),
            4 => forged.manifest.push(' '),
            5 => forged.authored_sources.clear(),
            6 => forged.candidate_sources.clear(),
            7 => forged.entries.clear(),
            8 => {
                forged.candidate_sources[0].source.push(' ');
                forged.candidate_sources[0].sha256 = digest(&forged.candidate_sources[0].source);
            }
            _ => unreachable!(),
        }
        let expected = before::verify_json_codec_plan(&revision, &entries, &forged).unwrap_err();
        let old_counts = counts();
        assert_eq!(
            verify_json_codec_plan(&revision, &entries, &forged).unwrap_err(),
            expected
        );
        assert_eq!(counts(), old_counts);
    }
    std::fs::remove_dir_all(root).unwrap();
}
