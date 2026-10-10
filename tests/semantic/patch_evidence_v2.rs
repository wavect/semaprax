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

/// Capsule and receipt known answers over the exact artifact bytes.
///
/// A capsule binds the Target Evidence report by digest, so these answers move
/// whenever that report moves — including when only its two backend target
/// rows move. Triage exactly as
/// `target_evidence::whole_report_sha_kats_cover_patch_v1_v2_v3` documents:
/// diff the reports field by field first, re-pin only when semantic identity
/// (`base_revision`, `candidate_revision`, `source.digest`, `patch.digest`,
/// `graphs.*`) is byte-identical, and fix the computation instead when it is
/// not.
///
/// Re-take history: `c69b09ed` (zeroed generated C temporaries) and
/// `ca1b21af` (emitted C 23638 -> 26671 bytes, Wasm core 193 -> 194 bytes;
/// identity unchanged — issues #80 and #81). Re-taken again for issue #293
/// P2-2, in step with `target_evidence`'s own re-take: only the embedded
/// Wasm core target row moved (194 -> 258 bytes for `kat-v1`/`kat-v2`, 210
/// -> 302 bytes for `kat-v3`), because the legacy scalar-core Wasm emitter
/// gained call-depth admission (`scalar_call_admission`); every identity
/// field was confirmed byte-identical first. Re-taken a third time in the
/// same P2-2 change, in step with `target_evidence`'s own third re-take: a
/// trapped call must not leave the live-frame counter poisoned for later
/// calls on the same instance, so the reset moved out of `main`'s own
/// (possibly recursive) body into a dedicated entry wrapper -- the embedded
/// Wasm core target row grew again, 258 -> 268 bytes for `kat-v1`/`kat-v2`,
/// 302 -> 313 bytes for `kat-v3`; every identity field was confirmed
/// byte-identical again first.
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
        let row = serde_json::json!({
            "report": report,
            "capsule": capsule,
            "receipt": receipt,
        })
        .to_string();
        assert!(
            row.len() <= 1024 * 1024,
            "patch evidence byte audit is bounded"
        );
        println!("SEMAPRAX_PATCH_EVIDENCE_BYTE_AUDIT={row}");
        let previous_report = report.replace("0.259.0", "0.256.0");
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
            "7f678b6a9931d455f1fce03935e494c51de081aa0ecd03a7124985258a432768",
            "0e4ce31db0d91d159e0f89d9bffd18968b1c0631420b80336a96bae0e1660aa0",
            "e1495c29cd7d93fc239f42bef2a11ae8bc95fc07aeca276bdfe5ad0efa0c77a9",
        ]
    );
    assert_eq!(
        previous_receipt_hashes,
        [
            "d5697fd3c8056e80f28e13c51f1d8180161ed049bc532b0c02e63cfe8e89a0cc",
            "49b670bcb568d7e657943c755f8809a4e00fa0fa70f824182a4a5f6ea11fba55",
            "ca412699cda1e5aea87201bbddb6cc66809380bc921ebe7a9384db2378c0449c",
        ]
    );
    assert_eq!(
        capsule_hashes,
        [
            "93089618b1ef9f841f526f10c24fd51cc14e812bebdc5cb93a52d8e6a22e6e79",
            "fa3e33dcf56eb1ad41009912b75d19aba4fdc36a7b646663b3ac7ae17dd2ebb7",
            "d46d1c98d7017904c4727b0cdfeb64a525c210d22023fa77dc7685de50cb3b6f",
        ]
    );
    assert_eq!(
        receipt_hashes,
        [
            "2c70ee808a5276eaa3bb8a4f14102a057b1ba7d229986a0fc9da24e227ae2812",
            "0b42c0b839a2ef7d89c8d46f52679a99438ac39fa1e5e91fd96b9041cdd3608a",
            "d35097a8cb6fc6d26d398d5498e5baec3a8dcb1f7b584d2c900a196186262f0c",
        ]
    );
}
