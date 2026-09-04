use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::{graph, hir, parse, patch, patch_evidence, repair, review, target_evidence};
use sha2::{Digest, Sha256};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    directory: PathBuf,
    source: PathBuf,
    patch: PathBuf,
    evidence: PathBuf,
}

impl Fixture {
    fn v1(label: &str) -> Self {
        let source = "module evidence.target_v1;\n@id(\"target.helper\") fn helper()->i64{41}\n@id(\"app.main\") fn main()->i64{helper()+1}\n";
        let patch = format!(
            "base {}\nrename target.helper to answer\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence-v2.spx")).unwrap())
        );
        Self::from_source(label, source, &patch)
    }

    fn new(label: &str) -> Self {
        let source = "module evidence.target_v2;\n@id(\"target.helper\") fn helper()->i64{41}\n@id(\"app.main\") fn main()->i64{helper()+1}\n";
        let patch = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename target.helper to answer\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence-v2.spx")).unwrap())
        );
        Self::from_source(label, source, &patch)
    }

    fn v3(label: &str) -> Self {
        let source = "module evidence.target_rebase;\nfn helper(value:i64)->i64{value+1}\n@id(\"target.caller\") fn caller(value:i64)->i64{helper(value)}\n@id(\"app.main\") fn main()->i64{caller(41)}\n";
        let fixture = Self::from_source(label, source, "");
        let query =
            repair::DiagnosticRepairQuery::assign_function_id("auto:evidence.target_rebase.helper")
                .unwrap();
        let repairs: serde_json::Value =
            serde_json::from_str(&repair::query(&fixture.source, &query).unwrap()).unwrap();
        let preview: serde_json::Value = serde_json::from_str(
            &repair::instantiate(
                &fixture.source,
                repairs["repair"]["id"].as_str().unwrap(),
                &repair::PersistentDeclarationId::new("evidence.target_rebase.helper").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        std::fs::write(&fixture.patch, preview["patch"]["source"].as_str().unwrap()).unwrap();
        fixture
    }

    fn generic_v2(label: &str) -> Self {
        let source = "module evidence.target_generic;\n@id(\"generic.marker\") fn marker<T,U>()->bool{true}\n@id(\"app.main\") fn main()->i64{if marker<i64,bool>() {42}else{0}}\n";
        let program = parse(source, Path::new("generic.spx")).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        let main = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        let body = match &main.body.kind {
            hir::ResolvedExprKind::Block { tail, .. } => tail.as_ref(),
            _ => &main.body,
        };
        let hir::ResolvedExprKind::If { condition, .. } = &body.kind else {
            panic!("main body must be an if")
        };
        let hir::ResolvedExprKind::Call {
            instance: Some(instance),
            ..
        } = &condition.kind
        else {
            panic!("condition must be a materialized call")
        };
        let patch = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 1 from bool to i64\nrequire no-new-effects\n",
            graph::revision(&program), condition.id, instance, condition.id, instance
        );
        Self::from_source(label, source, &patch)
    }

    fn from_source(label: &str, source: &str, patch: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-patch-evidence-v2-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("module.spx");
        let patch_path = directory.join("change.spatch");
        let evidence_path = directory.join("evidence.json");
        std::fs::write(&source_path, source).unwrap();
        std::fs::write(&patch_path, patch).unwrap();
        Self {
            directory,
            source: source_path,
            patch: patch_path,
            evidence: evidence_path,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn sha256(value: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
    )
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

#[test]
fn generate_verify_and_apply_exact_v2() {
    let fixture = Fixture::new("roundtrip");
    let review_before = review::preview(&fixture.source, &fixture.patch).unwrap();
    let capsule = patch_evidence::generate_v2(&fixture.source, &fixture.patch).unwrap();
    assert_eq!(
        capsule,
        patch_evidence::generate_v2(&fixture.source, &fixture.patch).unwrap()
    );
    assert!(capsule.ends_with('\n'));
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(value["schema"], "semaprax.semantic-patch-evidence.v2");
    assert_eq!(value["target_evidence"]["id"], "evidence:1");
    assert_eq!(
        value["target_evidence"]["kind"],
        "semantic_target_evidence_v1"
    );
    assert_eq!(
        value["assessments"]["security_authority"],
        "unchanged_within_admitted_domain"
    );
    assert!(matches!(
        value["assessments"]["target_artifact"].as_str(),
        Some("change_proven" | "unchanged_within_admitted_domain")
    ));
    assert_eq!(value["budget"]["used_evidence_bytes"], capsule.len());
    assert_eq!(
        value["review"]["digest"],
        domain_digest(
            b"semaprax.semantic-patch-evidence.review-digest.v1\0",
            review_before.as_bytes(),
        )
    );
    let target_report = target_evidence::preview(&fixture.source, &fixture.patch).unwrap();
    assert_eq!(
        value["target_evidence"]["digest"],
        domain_digest(
            b"semaprax.semantic-target-evidence.report-digest.v1\0",
            target_report.as_bytes(),
        )
    );
    assert_eq!(
        value["budget"]["used_target_evidence_bytes"],
        target_report.len()
    );
    assert_eq!(
        review::preview(&fixture.source, &fixture.patch).unwrap(),
        review_before
    );
    std::fs::write(&fixture.evidence, &capsule).unwrap();
    let receipt =
        patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence).unwrap();
    let receipt_value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(
        receipt_value["schema"],
        "semaprax.semantic-patch-evidence-verification.v2"
    );
    assert_eq!(receipt_value["result"], "exact_replay");
    let revision =
        patch_evidence::apply_v2(&fixture.source, &fixture.patch, &fixture.evidence).unwrap();
    assert_eq!(revision, value["candidate_revision"].as_str().unwrap());
    assert!(std::fs::read_to_string(&fixture.source)
        .unwrap()
        .contains("fn answer"));
    assert_eq!(sha256(&capsule).len(), 64);
    assert_eq!(sha256(&receipt).len(), 64);
}

#[test]
fn v1_and_receipt_confusion_fail_before_write() {
    let fixture = Fixture::new("confusion");
    let before = std::fs::read(&fixture.source).unwrap();
    let v1 = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    std::fs::write(&fixture.evidence, v1).unwrap();
    assert!(patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence).is_err());
    assert!(patch_evidence::apply_v2(&fixture.source, &fixture.patch, &fixture.evidence).is_err());
    assert_eq!(std::fs::read(&fixture.source).unwrap(), before);
}

#[test]
fn mismatch_fails_before_stage_and_preserves_source() {
    let fixture = Fixture::new("mismatch");
    let before = std::fs::read(&fixture.source).unwrap();
    let mut capsule = patch_evidence::generate_v2(&fixture.source, &fixture.patch).unwrap();
    let position = capsule.find("sha256:").unwrap() + 7;
    capsule.replace_range(position..position + 1, "0");
    std::fs::write(&fixture.evidence, capsule).unwrap();
    assert!(patch_evidence::apply_v2(&fixture.source, &fixture.patch, &fixture.evidence).is_err());
    assert_eq!(std::fs::read(&fixture.source).unwrap(), before);
    assert!(std::fs::read_dir(&fixture.directory)
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("semaprax-stage")));
}

#[test]
fn typed_target_overlay_distinguishes_projection_changes_from_identity_rebase() {
    let rename = Fixture::new("overlay-rename");
    let rename_capsule: serde_json::Value =
        serde_json::from_str(&patch_evidence::generate_v2(&rename.source, &rename.patch).unwrap())
            .unwrap();
    assert_eq!(
        rename_capsule["assessments"]["target_artifact"],
        "unchanged_within_admitted_domain"
    );

    let generic = Fixture::generic_v2("overlay-generic");
    let generic_capsule: serde_json::Value = serde_json::from_str(
        &patch_evidence::generate_v2(&generic.source, &generic.patch).unwrap(),
    )
    .unwrap();
    assert_eq!(
        generic_capsule["assessments"]["target_artifact"],
        "change_proven"
    );

    let rebase = Fixture::v3("overlay-rebase");
    let rebase_capsule: serde_json::Value =
        serde_json::from_str(&patch_evidence::generate_v2(&rebase.source, &rebase.patch).unwrap())
            .unwrap();
    assert_eq!(
        rebase_capsule["target_evidence"]["kind"],
        "semantic_target_evidence_v1"
    );
    assert_eq!(
        rebase_capsule["assessments"]["target_artifact"],
        "change_proven"
    );
    let rebase_target: serde_json::Value =
        serde_json::from_str(&target_evidence::preview(&rebase.source, &rebase.patch).unwrap())
            .unwrap();
    assert_eq!(rebase_target["targets"][0]["classification"], "changed");
    assert_ne!(
        rebase_target["targets"][0]["base_digest"],
        rebase_target["targets"][0]["candidate_digest"]
    );
}

#[test]
fn evidence_v2_apply_matches_patch_for_v1_v2_v3_and_second_apply_is_stale() {
    for (label, make_fixture) in [
        ("v1", Fixture::v1 as fn(&str) -> Fixture),
        ("v2", Fixture::new as fn(&str) -> Fixture),
        ("v3", Fixture::v3 as fn(&str) -> Fixture),
    ] {
        let evidence_fixture = make_fixture(&format!("parity-evidence-{label}"));
        let patch_fixture = Fixture::from_source(
            &format!("parity-patch-{label}"),
            &std::fs::read_to_string(&evidence_fixture.source).unwrap(),
            &std::fs::read_to_string(&evidence_fixture.patch).unwrap(),
        );
        let capsule =
            patch_evidence::generate_v2(&evidence_fixture.source, &evidence_fixture.patch).unwrap();
        std::fs::write(&evidence_fixture.evidence, capsule).unwrap();
        let evidence_revision = patch_evidence::apply_v2(
            &evidence_fixture.source,
            &evidence_fixture.patch,
            &evidence_fixture.evidence,
        )
        .unwrap();
        let patch_revision = patch::apply(&patch_fixture.source, &patch_fixture.patch).unwrap();
        assert_eq!(evidence_revision, patch_revision, "{label}");
        assert_eq!(
            std::fs::read(&evidence_fixture.source).unwrap(),
            std::fs::read(&patch_fixture.source).unwrap(),
            "{label}"
        );

        let already_applied = std::fs::read(&evidence_fixture.source).unwrap();
        let error = patch_evidence::apply_v2(
            &evidence_fixture.source,
            &evidence_fixture.patch,
            &evidence_fixture.evidence,
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G409", "{label}");
        assert_eq!(
            std::fs::read(&evidence_fixture.source).unwrap(),
            already_applied
        );
    }
}

#[test]
fn v2_strict_parser_and_independent_replay_reject_hostile_capsules() {
    let fixture = Fixture::new("hostile");
    let capsule = patch_evidence::generate_v2(&fixture.source, &fixture.patch).unwrap();
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    let first_digest = value["source"]["digest"].as_str().unwrap();
    let mutated_digest = format!(
        "{}{}",
        &first_digest[..first_digest.len() - 1],
        if first_digest.ends_with('0') {
            "1"
        } else {
            "0"
        }
    );
    let ordered_prefix = format!(
        "{{\"schema\":\"semaprax.semantic-patch-evidence.v2\",\"source_graph_schema\":{},",
        serde_json::to_string(value["source_graph_schema"].as_str().unwrap()).unwrap()
    );
    let reordered_prefix = format!(
        "{{\"source_graph_schema\":{},\"schema\":\"semaprax.semantic-patch-evidence.v2\",",
        serde_json::to_string(value["source_graph_schema"].as_str().unwrap()).unwrap()
    );
    let canonical_mismatches = [
        capsule.replacen(first_digest, &mutated_digest, 1),
        capsule.replacen(
            value["candidate_revision"].as_str().unwrap(),
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            1,
        ),
        capsule.replacen(
            value["target_evidence"]["digest"].as_str().unwrap(),
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            1,
        ),
    ];
    for (index, hostile) in canonical_mismatches.into_iter().enumerate() {
        std::fs::write(&fixture.evidence, hostile).unwrap();
        let error = patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence)
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G132", "binding mutation {index}");
    }

    let malformed = [
        format!("\u{feff}{capsule}"),
        format!(" {capsule}"),
        capsule.replace('\n', "\r\n"),
        format!("{capsule}\n"),
        capsule.replacen(
            "{\"schema\":\"semaprax.semantic-patch-evidence.v2\",",
            "{",
            1,
        ),
        capsule.replacen("{\"schema\":", "{\"extra\":0,\"schema\":", 1),
        capsule.replacen(
            "{\"schema\":",
            "{\"schema\":\"semaprax.semantic-patch-evidence.v2\",\"schema\":",
            1,
        ),
        capsule.replacen(
            "no_project_test_discovery_or_execution",
            "project_tests_executed",
            1,
        ),
        capsule.replacen(
            "\"max_source_bytes\":16777216",
            "\"max_source_bytes\":16777215",
            1,
        ),
        capsule.replacen(
            "\"security_authority\":\"unchanged_within_admitted_domain\"",
            "\"security_authority\":\"approved\"",
            1,
        ),
        capsule.replacen(&ordered_prefix, &reordered_prefix, 1),
        format!("{}0{}\n", "[".repeat(9), "]".repeat(9)),
    ];
    for (index, hostile) in malformed.into_iter().enumerate() {
        std::fs::write(&fixture.evidence, hostile).unwrap();
        let error = patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence)
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G130", "format mutation {index}");
    }

    std::fs::write(&fixture.evidence, [0xff, 0xfe]).unwrap();
    assert_eq!(
        patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err()
            [0]
        .code,
        "SPX-G130"
    );

    std::fs::write(&fixture.evidence, &capsule).unwrap();
    let receipt =
        patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence).unwrap();
    std::fs::write(&fixture.evidence, receipt).unwrap();
    assert_eq!(
        patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err()
            [0]
        .code,
        "SPX-G130"
    );

    let foreign_source = "module evidence.hostile_foreign;\n@id(\"foreign.helper\") fn helper()->i64{2}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let foreign_patch = format!(
        "schema semaprax.semantic-patch.v2\nbase {}\nrename foreign.helper to changed\nrequire no-new-effects\n",
        graph::revision(&parse(foreign_source, "foreign.spx").unwrap())
    );
    let foreign = Fixture::from_source("hostile-foreign", foreign_source, &foreign_patch);
    let rehashed_foreign = patch_evidence::generate_v2(&foreign.source, &foreign.patch).unwrap();
    std::fs::write(&fixture.evidence, rehashed_foreign).unwrap();
    assert_eq!(
        patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err()
            [0]
        .code,
        "SPX-G132"
    );
}

#[test]
fn v2_cli_arity_and_output_are_exact() {
    let fixture = Fixture::new("cli");
    let capsule = patch_evidence::generate_v2(&fixture.source, &fixture.patch).unwrap();
    let generated = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["patch-evidence-v2"])
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .output()
        .unwrap();
    assert!(generated.status.success());
    assert_eq!(generated.stdout, capsule.as_bytes());
    std::fs::write(&fixture.evidence, &capsule).unwrap();

    let receipt =
        patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence).unwrap();
    let verified = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["verify-patch-evidence-v2"])
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .arg(&fixture.evidence)
        .output()
        .unwrap();
    assert!(verified.status.success());
    assert_eq!(verified.stdout, receipt.as_bytes());

    let capsule_value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    let applied = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["patch-with-evidence-v2"])
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .arg(&fixture.evidence)
        .output()
        .unwrap();
    assert!(applied.status.success());
    assert_eq!(
        String::from_utf8(applied.stdout).unwrap(),
        format!(
            "applied semantic patch with exact evidence replay; graph is now {}\n",
            capsule_value["candidate_revision"].as_str().unwrap()
        )
    );

    let arity = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("patch-evidence-v2")
        .output()
        .unwrap();
    assert_eq!(arity.status.code(), Some(2));
}

#[test]
fn capsule_and_receipt_sha_kats_cover_patch_v1_v2_v3() {
    let fixtures = [
        Fixture::v1("kat-v1"),
        Fixture::new("kat-v2"),
        Fixture::v3("kat-v3"),
    ];
    let mut capsule_hashes = Vec::new();
    let mut receipt_hashes = Vec::new();
    let mut previous_capsule_hashes = Vec::new();
    let mut previous_receipt_hashes = Vec::new();
    for fixture in &fixtures {
        let capsule = patch_evidence::generate_v2(&fixture.source, &fixture.patch).unwrap();
        std::fs::write(&fixture.evidence, &capsule).unwrap();
        let receipt =
            patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence).unwrap();
        capsule_hashes.push(sha256(&capsule));
        receipt_hashes.push(sha256(&receipt));

        // Reconstruct the prior binding to prove that only validator metadata
        // changed; old capsules must still fail replay before source writes.
        let report = target_evidence::preview(&fixture.source, &fixture.patch).unwrap();
        let previous_report = report.replace("0.258.0", "0.256.0");
        let report_domain = b"semaprax.semantic-target-evidence.report-digest.v1\0";
        let artifact_domain = b"semaprax.semantic-patch-evidence.artifact-digest.v2\0";
        let report_digest = domain_digest(report_domain, report.as_bytes());
        let previous_report_digest = domain_digest(report_domain, previous_report.as_bytes());
        let previous_capsule = capsule.replace(&report_digest, &previous_report_digest);
        let previous_receipt = receipt
            .replace(&report_digest, &previous_report_digest)
            .replace(
                &domain_digest(artifact_domain, capsule.as_bytes()),
                &domain_digest(artifact_domain, previous_capsule.as_bytes()),
            );
        previous_capsule_hashes.push(sha256(&previous_capsule));
        previous_receipt_hashes.push(sha256(&previous_receipt));
        let before = std::fs::read(&fixture.source).unwrap();
        std::fs::write(&fixture.evidence, &previous_capsule).unwrap();
        assert_eq!(
            patch_evidence::verify_v2(&fixture.source, &fixture.patch, &fixture.evidence)
                .unwrap_err()[0]
                .code,
            "SPX-G132"
        );
        assert_eq!(
            patch_evidence::apply_v2(&fixture.source, &fixture.patch, &fixture.evidence)
                .unwrap_err()[0]
                .code,
            "SPX-G132"
        );
        assert_eq!(std::fs::read(&fixture.source).unwrap(), before);
        assert!(std::fs::read_dir(&fixture.directory).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("semaprax-stage")
        }));
    }
    assert_eq!(
        previous_capsule_hashes,
        [
            "e6c2d7a0cb1ccf6834e3e9704d74594f1bb40a8a61f428409550d7e7b258102e",
            "99450107acfa10e429b30a5d4fa065f2d05aa79ba3eab926e3fd713eb8021307",
            "1872d6f6b9735bdd3e357bc159412882778b759cd2cbf354fc31af219dc983d0",
        ]
    );
    assert_eq!(
        previous_receipt_hashes,
        [
            "c95182d7f9671a0d44121b587e074608c4ee646502f56db896a9ca019550e083",
            "b983a42518e7dafb906e8329282b1c1473ca4260c22a341ba893642e37ceb042",
            "6a112a452fdbc8321d57328d83348d632b48d29d9afb4bc2639d7f2ef2967ba9",
        ]
    );
    assert_eq!(
        capsule_hashes,
        [
            "50850a876dca41f09746a82887ec0205da2069e42ab306e5ea8b33785931489a",
            "a694c608e1f1ab8b251daa265405aa79780dd33b5cdb6c6091fd3a31d9d98028",
            "0c335a75009c4b121002ea1718ba7380ef9c1b5f55f55299f22b70925dcc6399",
        ]
    );
    assert_eq!(
        receipt_hashes,
        [
            "56f8b4364e97096e20f4a46f9f5d264ce0a378d3628adbae8bdd72b4c2824191",
            "d25edb747cc5f80160243b0d4b7636eed0b9e82595ad1d7fe91ab5bbebce07a2",
            "ac84e603d5e829bc9bcecb47bbf72b1dd76c437b5022dd6396532e266c3bfaf9",
        ]
    );
}
