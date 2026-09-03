use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: std::path::PathBuf,
    patch: std::path::PathBuf,
    evidence: std::path::PathBuf,
    managed_source: std::path::PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-workspace-evidence-unit-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut child_patches = Vec::new();
        for (index, stem) in ["alpha", "beta"].into_iter().enumerate() {
            let logical = format!("{stem}.spx");
            let source = crate::format::canonical(
                    &crate::parse(
                        &format!(
                            "module evidence.{stem}; @id(\"evidence.{stem}.helper\") fn helper()->i64{{{index}}} @id(\"evidence.{stem}.main\") fn main()->i64{{helper()}}"
                        ),
                        Path::new(&logical),
                    )
                    .unwrap(),
                );
            std::fs::write(root.join(&logical), &source).unwrap();
            let revision =
                crate::graph::revision(&crate::parse(&source, Path::new(&logical)).unwrap());
            child_patches.push(format!(
                "base {revision}\nrename evidence.{stem}.helper to {stem}_renamed\n"
            ));
        }
        let path_set = root.join("paths.json");
        std::fs::write(
                &path_set,
                "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}]}\n",
            )
            .unwrap();
        let base = workspace::initialize(&root, &path_set).unwrap();
        let patch = root.join("change.wspatch");
        std::fs::write(
                &patch,
                format!(
                    "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":\"{base}\",\"files\":[{{\"path\":\"alpha.spx\",\"patch\":{}}},{{\"path\":\"beta.spx\",\"patch\":{}}}]}}\n",
                    serde_json::to_string(&child_patches[0]).unwrap(),
                    serde_json::to_string(&child_patches[1]).unwrap(),
                ),
            )
            .unwrap();
        let evidence = root.join("evidence.json");
        let managed_source = root
            .join(".semaprax-workspace/generations")
            .join(base.strip_prefix("sha256:").unwrap())
            .join("files/alpha.spx");
        Self {
            root,
            patch,
            evidence,
            managed_source,
        }
    }

    fn active(&self) -> std::path::PathBuf {
        self.root.join(".semaprax-workspace/ACTIVE")
    }

    fn revision(&self) -> String {
        workspace::snapshot(&self.root)
            .unwrap()
            .workspace_revision()
            .to_owned()
    }

    fn generation_names(&self) -> Vec<String> {
        directory_names(&self.root.join(".semaprax-workspace/generations"))
    }

    fn staging_names(&self) -> Vec<String> {
        directory_names(&self.root.join(".semaprax-workspace/staging"))
    }
}

fn directory_names(path: &Path) -> Vec<String> {
    let mut names = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    names
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn child_and_aggregate_node_limit_diagnostics_are_distinct_and_exact() {
    let child = map_review_child_diagnostics(
        vec![Diagnostic::io("SPX-G120", "Impact node budget exhausted")],
        false,
    );
    assert_eq!(child[0].code, "SPX-G161");
    assert_eq!(
        child[0].message,
        "Semantic Workspace Patch Evidence `max_child_impact_nodes` exceeds 1024"
    );

    let aggregate = map_review_child_diagnostics(
        vec![Diagnostic::io("SPX-G120", "Impact node budget exhausted")],
        true,
    );
    assert_eq!(aggregate[0].code, "SPX-G161");
    assert_eq!(
        aggregate[0].message,
        "Semantic Workspace Patch Evidence `max_total_impact_nodes` exceeds 16384"
    );
}

#[test]
fn aggregate_usage_limit_diagnostics_name_each_exact_wire_field() {
    let baseline = AggregateUsage {
        managed_files: 2,
        changed_files: 2,
        base_source_bytes: 0,
        candidate_source_bytes: 0,
        workspace_patch_bytes: 0,
        operations: 0,
        declarations: 0,
        callables: 0,
        call_sites: 0,
        manifest_bytes: 0,
        preview_bytes: 0,
        max_child_impact_depth: 0,
        max_child_impact_nodes: 0,
        total_impact_nodes: 0,
        total_impact_bytes: 0,
        total_review_bytes: 0,
        total_child_evidence_bytes: 0,
        retained_generations: 0,
        staging_attempts: 0,
    };
    macro_rules! assert_limit {
        ($member:ident, $field:literal, $maximum:expr) => {{
            let mut exact = baseline;
            exact.$member = $maximum;
            validate_usage(&exact).unwrap();
            let mut usage = baseline;
            usage.$member = $maximum + 1;
            let diagnostic = usage_limit_error(&usage);
            assert_eq!(diagnostic.code, "SPX-G161");
            assert_eq!(
                diagnostic.message,
                format!(
                    "Semantic Workspace Patch Evidence `{}` exceeds {}",
                    $field, $maximum
                )
            );
        }};
    }
    assert_limit!(managed_files, "max_managed_files", 16);
    assert_limit!(changed_files, "max_changed_files", 16);
    assert_limit!(base_source_bytes, "max_total_base_source_bytes", 16_777_216);
    assert_limit!(
        candidate_source_bytes,
        "max_total_candidate_source_bytes",
        16_777_216
    );
    assert_limit!(
        workspace_patch_bytes,
        "max_workspace_patch_bytes",
        4_194_304
    );
    assert_limit!(operations, "max_operations", 4096);
    assert_limit!(declarations, "max_declarations", 4096);
    assert_limit!(callables, "max_callables", 1024);
    assert_limit!(call_sites, "max_call_sites", 65_536);
    assert_limit!(manifest_bytes, "max_manifest_bytes", 1_048_576);
    assert_limit!(preview_bytes, "max_workspace_preview_bytes", 65_536);
    assert_limit!(max_child_impact_depth, "max_child_impact_depth", 1024);
    assert_limit!(max_child_impact_nodes, "max_child_impact_nodes", 1024);
    assert_limit!(total_impact_nodes, "max_total_impact_nodes", 16_384);
    assert_limit!(total_impact_bytes, "max_total_impact_bytes", 16_777_216);
    assert_limit!(total_review_bytes, "max_total_review_bytes", 33_554_432);
    assert_limit!(
        total_child_evidence_bytes,
        "max_total_child_patch_evidence_bytes",
        1_048_576
    );
    assert_limit!(retained_generations, "max_retained_generations", 32);
    assert_limit!(staging_attempts, "max_staging_attempts", 32);
}

#[test]
fn structural_depth_limit_is_distinct_from_malformed_json() {
    assert!(validate_json_structure("[[[[[[[[0]]]]]]]]").is_ok());
    let depth = validate_json_structure("[[[[[[[[[0]]]]]]]]]").expect_err("depth nine must fail");
    assert_eq!(depth[0].code, "SPX-G161");
    assert_eq!(
        depth[0].message,
        "Semantic Workspace Patch Evidence `max_json_depth` exceeds 8"
    );
    let malformed = validate_json_structure("[[0]").expect_err("unbalanced JSON must fail");
    assert_eq!(malformed[0].code, "SPX-G160");
}

#[test]
fn owned_inputs_and_final_source_recheck_are_route_specific() {
    let fixture = Fixture::new();
    let capsule = generate(&fixture.root, &fixture.patch).unwrap();
    std::fs::write(&fixture.evidence, &capsule).unwrap();

    let displaced_evidence = fixture.root.join("owned-evidence.json");
    let receipt = verify_with_hook(&fixture.root, &fixture.patch, &fixture.evidence, |point| {
        if point == EvidencePoint::AfterEvidenceRead {
            std::fs::rename(&fixture.evidence, &displaced_evidence).unwrap();
            std::fs::write(&fixture.evidence, "not the submitted capsule\n").unwrap();
        }
    })
    .unwrap();
    assert!(receipt.contains("\"result\":\"exact_replay\""));

    let patch_source = std::fs::read_to_string(&fixture.patch).unwrap();
    let displaced_patch = fixture.root.join("owned-change.wspatch");
    let error = generate_with_hook(&fixture.root, &fixture.patch, |point| {
        if point == EvidencePoint::BeforeFinalCheck {
            std::fs::rename(&fixture.patch, &displaced_patch).unwrap();
            std::fs::write(&fixture.patch, &patch_source).unwrap();
        }
    })
    .expect_err("same-byte patch identity replacement must fail the final recheck");
    assert!(matches!(error[0].code, "SPX-I209" | "SPX-G153"));
    std::fs::remove_file(&fixture.patch).unwrap();
    std::fs::rename(&displaced_patch, &fixture.patch).unwrap();

    std::fs::write(&fixture.evidence, &capsule).unwrap();
    let source = std::fs::read_to_string(&fixture.managed_source).unwrap();
    let displaced_source = fixture.root.join("owned-alpha.spx");
    let error = verify_with_hook(&fixture.root, &fixture.patch, &fixture.evidence, |point| {
        if point == EvidencePoint::BeforeFinalCheck {
            std::fs::rename(&fixture.managed_source, &displaced_source).unwrap();
            std::fs::write(&fixture.managed_source, &source).unwrap();
        }
    })
    .expect_err("same-byte managed source replacement must fail the final recheck");
    assert!(matches!(error[0].code, "SPX-I209" | "SPX-G153"));
}

#[test]
fn apply_owns_evidence_and_rechecks_the_owned_patch_before_pivot() {
    let evidence_fixture = Fixture::new();
    let capsule = generate(&evidence_fixture.root, &evidence_fixture.patch).unwrap();
    let candidate = serde_json::from_str::<serde_json::Value>(&capsule).unwrap()
        ["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(&evidence_fixture.evidence, &capsule).unwrap();
    let displaced_evidence = evidence_fixture.root.join("owned-apply-evidence.json");
    let applied = apply_with_hook(
        &evidence_fixture.root,
        &evidence_fixture.patch,
        &evidence_fixture.evidence,
        |point, _, _, _| {
            if point == EvidenceApplyPoint::AfterEvidenceRead {
                std::fs::rename(&evidence_fixture.evidence, &displaced_evidence)?;
                std::fs::write(&evidence_fixture.evidence, "not the owned evidence\n")?;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(applied, candidate);
    assert_eq!(evidence_fixture.revision(), candidate);

    let patch_fixture = Fixture::new();
    let capsule = generate(&patch_fixture.root, &patch_fixture.patch).unwrap();
    std::fs::write(&patch_fixture.evidence, capsule).unwrap();
    let old_revision = patch_fixture.revision();
    let patch_bytes = std::fs::read(&patch_fixture.patch).unwrap();
    let displaced_patch = patch_fixture.root.join("owned-apply-change.wspatch");
    let error = apply_with_hook(
        &patch_fixture.root,
        &patch_fixture.patch,
        &patch_fixture.evidence,
        |point, _, _, _| {
            if point == EvidenceApplyPoint::AfterPatchRead {
                std::fs::rename(&patch_fixture.patch, &displaced_patch)?;
                std::fs::write(&patch_fixture.patch, &patch_bytes)?;
            }
            Ok(())
        },
    )
    .expect_err("same-byte patch replacement must fail before the ACTIVE pivot");
    assert!(matches!(error[0].code, "SPX-I209" | "SPX-G153"));
    assert_eq!(patch_fixture.revision(), old_revision);
}

#[test]
fn replay_boundary_is_no_write_and_shared_pivot_boundaries_are_exact() {
    let replay_fixture = Fixture::new();
    let capsule = generate(&replay_fixture.root, &replay_fixture.patch).unwrap();
    std::fs::write(&replay_fixture.evidence, &capsule).unwrap();
    let active = std::fs::read(replay_fixture.active()).unwrap();
    let generations = replay_fixture.generation_names();
    let staging = replay_fixture.staging_names();
    let error = apply_with_hook(
        &replay_fixture.root,
        &replay_fixture.patch,
        &replay_fixture.evidence,
        |point, _, _, _| {
            if point == EvidenceApplyPoint::AfterReplay {
                return Err(std::io::Error::other("stop after exact replay"));
            }
            Ok(())
        },
    )
    .expect_err("an injected replay-boundary failure must not enter commit");
    assert_eq!(error[0].code, "SPX-G163");
    assert_eq!(std::fs::read(replay_fixture.active()).unwrap(), active);
    assert_eq!(replay_fixture.generation_names(), generations);
    assert_eq!(replay_fixture.staging_names(), staging);

    for (boundary, expected_code, expect_candidate) in [
        (
            workspace::ApplyPoint::BeforeActiveReplace,
            "SPX-I211",
            false,
        ),
        (workspace::ApplyPoint::AfterActiveReplace, "SPX-I212", true),
    ] {
        let fixture = Fixture::new();
        let capsule = generate(&fixture.root, &fixture.patch).unwrap();
        let candidate = serde_json::from_str::<serde_json::Value>(&capsule).unwrap()
            ["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        std::fs::write(&fixture.evidence, &capsule).unwrap();
        let old_revision = fixture.revision();
        let error = apply_with_hook(
            &fixture.root,
            &fixture.patch,
            &fixture.evidence,
            |point, _, _, _| {
                if point == EvidenceApplyPoint::Workspace(boundary) {
                    return Err(std::io::Error::other("injected shared pivot boundary"));
                }
                Ok(())
            },
        )
        .expect_err("the injected shared pivot boundary must fail");
        assert_eq!(error[0].code, expected_code);
        assert_eq!(
            fixture.revision(),
            if expect_candidate {
                candidate
            } else {
                old_revision
            }
        );
    }
}

#[test]
fn snapshot_lock_handoff_precedes_immediate_stale_evidence_reapply() {
    let fixture = Fixture::new();
    let capsule = generate(&fixture.root, &fixture.patch).unwrap();
    let candidate = serde_json::from_str::<serde_json::Value>(&capsule).unwrap()
        ["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(&fixture.evidence, capsule).unwrap();
    assert_eq!(
        apply(&fixture.root, &fixture.patch, &fixture.evidence).unwrap(),
        candidate
    );
    let lock_path = fixture.root.join(".semaprax-workspace/LOCK");

    for _ in 0..64 {
        assert_eq!(fixture.revision(), candidate);
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&lock)
            .expect("snapshot must release shared LOCK before returning");
        fs2::FileExt::unlock(&lock).unwrap();
        let stale = apply(&fixture.root, &fixture.patch, &fixture.evidence)
            .expect_err("an immediate second evidence apply must be stale, never busy");
        assert_eq!(stale[0].code, "SPX-G152");
    }
}

#[test]
fn capsule_and_receipt_self_caps_accept_exact_and_reject_one_less() {
    let fixture = Fixture::new();
    let build = build_owned(&fixture.root, &fixture.patch).unwrap();
    let capsule = render_capsule_bounded(&build.facts).unwrap();
    assert_eq!(
        render_capsule_with_limit(&build.facts, capsule.len()).unwrap(),
        capsule
    );
    let error = render_capsule_with_limit(&build.facts, capsule.len() - 1)
        .expect_err("one byte below the exact capsule must fail");
    assert_eq!(error[0].code, "SPX-G161");
    assert_eq!(
        error[0].message,
        "Semantic Workspace Patch Evidence `max_workspace_evidence_bytes` exceeds 65536"
    );

    let artifact_digest = domain_digest(ARTIFACT_DIGEST_DOMAIN, capsule.as_bytes());
    let receipt = render_receipt_bounded(&build.facts, &artifact_digest, capsule.len()).unwrap();
    assert_eq!(
        render_receipt_with_limit(&build.facts, &artifact_digest, capsule.len(), receipt.len(),)
            .unwrap(),
        receipt
    );
    let error = render_receipt_with_limit(
        &build.facts,
        &artifact_digest,
        capsule.len(),
        receipt.len() - 1,
    )
    .expect_err("one byte below the exact receipt must fail");
    assert_eq!(error[0].code, "SPX-G161");
    assert_eq!(
        error[0].message,
        "Semantic Workspace Patch Evidence `max_workspace_receipt_bytes` exceeds 65536"
    );
    build.recheck().unwrap();
}
