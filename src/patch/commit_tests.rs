
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

const SOURCE: &str = r#"module patch.commit;

@id("helper.answer")
fn answer() -> i64
{
    42
}

@id("app.main")
fn main() -> i64
{
    answer()
}
"#;

const CONCURRENT_SOURCE: &str = r#"module patch.commit;

@id("helper.answer")
fn answer() -> i64
{
    41
}

@id("app.main")
fn main() -> i64
{
    answer()
}
"#;

const V3_SOURCE: &str =
    "module patch.v3_commit;\nfn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
const V3_CONCURRENT_SOURCE: &str =
    "module patch.v3_commit;\nfn helper()->i64{2}\n@id(\"app.main\") fn main()->i64{helper()}\n";

fn fixture(label: &str) -> (PathBuf, PathBuf) {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-patch-commit-{}-{label}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("rename.spatch");
    let revision = graph::revision(&parse(SOURCE, &source_path).unwrap());
    std::fs::write(&source_path, SOURCE).unwrap();
    std::fs::write(
        &patch_path,
        format!("base {revision}\nrename helper.answer to computed\n"),
    )
    .unwrap();
    (source_path, patch_path)
}

fn v3_fixture(label: &str) -> (PathBuf, PathBuf) {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-patch-v3-commit-{}-{label}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("repair.spatch");
    std::fs::write(&source_path, V3_SOURCE).unwrap();
    let request =
        crate::repair::DiagnosticRepairQuery::assign_function_id("auto:patch.v3_commit.helper")
            .unwrap();
    let report = crate::repair::query(&source_path, &request).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    let preview = crate::repair::instantiate(
        &source_path,
        report["repair"]["id"].as_str().unwrap(),
        &crate::repair::PersistentDeclarationId::new("patch.v3_commit.helper").unwrap(),
    )
    .unwrap();
    let preview: serde_json::Value = serde_json::from_str(&preview).unwrap();
    std::fs::write(&patch_path, preview["patch"]["source"].as_str().unwrap()).unwrap();
    (source_path, patch_path)
}

fn owned_byte_fixture(label: &str) -> (PathBuf, Vec<u8>) {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-patch-owned-bytes-{}-{label}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let revision = graph::revision(&parse(SOURCE, &source_path).unwrap());
    std::fs::write(&source_path, SOURCE).unwrap();
    (
        source_path,
        format!("base {revision}\nrename helper.answer to computed\n").into_bytes(),
    )
}

fn assert_owned_artifacts_removed(source_path: &Path) {
    let canonical = std::fs::canonicalize(source_path).unwrap();
    assert!(!sibling_path(&canonical, ".semaprax-patch.lock")
        .unwrap()
        .exists());
    for index in 0..STAGING_ATTEMPTS {
        assert!(
            !sibling_path(&canonical, &format!(".semaprax-stage.{index}.tmp"))
                .unwrap()
                .exists()
        );
    }
}

#[test]
fn concurrent_edit_is_preserved_and_rejected_before_commit() {
    let (source_path, patch_path) = fixture("concurrent-edit");
    let error = apply_with_commit_hook(&source_path, &patch_path, |phase, source, _| {
        if phase == CommitPhase::BeforeFinalCheck {
            std::fs::write(source, CONCURRENT_SOURCE)?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(
        std::fs::read_to_string(&source_path).unwrap(),
        CONCURRENT_SOURCE
    );
    assert_owned_artifacts_removed(&source_path);
}

#[test]
fn same_bytes_with_replaced_source_identity_are_rejected_after_final_check() {
    let (source_path, patch_path) = fixture("concurrent-replacement");
    let backup_path = source_path.with_extension("original.spx");
    let error = apply_with_commit_hook(&source_path, &patch_path, |phase, source, _| {
        if phase == CommitPhase::BeforeRename {
            std::fs::rename(source, &backup_path)?;
            std::fs::write(source, SOURCE)?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), SOURCE);
    assert_eq!(std::fs::read_to_string(&backup_path).unwrap(), SOURCE);
    assert_owned_artifacts_removed(&source_path);
}

#[test]
fn staging_bytes_changed_after_final_check_are_rejected_and_cleaned() {
    let (source_path, patch_path) = fixture("stage-mutation");
    let error = apply_with_commit_hook(&source_path, &patch_path, |phase, _, staging| {
        if phase == CommitPhase::BeforeRename {
            std::fs::write(staging, b"attacker bytes")?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error[0].code, "SPX-I203");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), SOURCE);
    assert_owned_artifacts_removed(&source_path);
}

#[test]
fn v3_concurrent_edit_is_preserved_and_rejected_before_commit() {
    let (source_path, patch_path) = v3_fixture("concurrent-edit");
    let error = apply_with_commit_hook(&source_path, &patch_path, |phase, source, _| {
        if phase == CommitPhase::BeforeFinalCheck {
            std::fs::write(source, V3_CONCURRENT_SOURCE)?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(
        std::fs::read_to_string(&source_path).unwrap(),
        V3_CONCURRENT_SOURCE
    );
    assert_owned_artifacts_removed(&source_path);
}

#[test]
fn v3_staging_mutation_is_rejected_without_source_change() {
    let (source_path, patch_path) = v3_fixture("stage-mutation");
    let error = apply_with_commit_hook(&source_path, &patch_path, |phase, _, staging| {
        if phase == CommitPhase::BeforeRename {
            std::fs::write(staging, b"attacker bytes")?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error[0].code, "SPX-I203");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), V3_SOURCE);
    assert_owned_artifacts_removed(&source_path);
}

#[test]
fn v3_initial_source_read_rejects_more_than_the_repair_bound() {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-patch-v3-commit-{}-initial-bound-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("repair.spatch");
    let oversized = vec![b'x'; crate::repair::MAX_SOURCE_BYTES + 1];
    std::fs::write(&source_path, &oversized).unwrap();
    std::fs::write(
            &patch_path,
            "schema semaprax.semantic-patch.v3\nbase sha256:0\nassign-function-id repair sha256:0 diagnostic SPX-S103 target auto:large.helper name helper to large.helper\n",
        )
        .unwrap();

    let error = apply(&source_path, &patch_path).unwrap_err();
    assert_eq!(error[0].code, "SPX-R101");
    assert_eq!(
        std::fs::metadata(&source_path).unwrap().len(),
        oversized.len() as u64
    );
    assert_owned_artifacts_removed(&source_path);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn v3_final_recheck_bounds_concurrent_same_identity_growth() {
    let (source_path, patch_path) = v3_fixture("final-growth");
    let oversized = vec![b'x'; crate::repair::MAX_SOURCE_BYTES + 1];
    let error = apply_with_commit_hook(&source_path, &patch_path, |phase, source, _| {
        if phase == CommitPhase::BeforeFinalCheck {
            std::fs::write(source, &oversized)?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(
        std::fs::metadata(&source_path).unwrap().len(),
        (crate::repair::MAX_SOURCE_BYTES + 1) as u64
    );
    assert_owned_artifacts_removed(&source_path);
    std::fs::remove_dir_all(source_path.parent().unwrap()).unwrap();
}

#[test]
fn foreign_stage_path_replacement_is_rejected_and_never_deleted() {
    let (source_path, patch_path) = fixture("stage-path-replacement");
    let displaced_owned_stage = source_path.with_extension("owned-stage");
    let error = apply_with_commit_hook(&source_path, &patch_path, |phase, _, staging| {
        if phase == CommitPhase::BeforeRename {
            std::fs::rename(staging, &displaced_owned_stage)?;
            std::fs::write(staging, b"foreign path object")?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error[0].code, "SPX-I203");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), SOURCE);
    assert_eq!(
        std::fs::read_to_string(
            sibling_path(
                &std::fs::canonicalize(&source_path).unwrap(),
                ".semaprax-stage.0.tmp"
            )
            .unwrap()
        )
        .unwrap(),
        "foreign path object"
    );
    assert!(displaced_owned_stage.exists());
    assert!(!sibling_path(
        &std::fs::canonicalize(&source_path).unwrap(),
        ".semaprax-patch.lock"
    )
    .unwrap()
    .exists());
}

#[test]
fn injected_rename_failure_preserves_source_and_cleans_owned_artifacts() {
    let (source_path, patch_path) = fixture("rename-failure");
    let error = apply_with_commit_hook(&source_path, &patch_path, |phase, _, _| {
        if phase == CommitPhase::BeforeRename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected rename rejection",
            ));
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error[0].code, "SPX-I204");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), SOURCE);
    assert_owned_artifacts_removed(&source_path);
}

#[test]
fn prepared_commit_rejects_a_preflight_from_another_snapshot_before_staging() {
    let (source_path, _) = fixture("sealed-prepared-commit");
    let guard = acquire_a0_commit_guard(&source_path).unwrap();
    let authenticated =
        authenticate_a0_source(&guard, Some((crate::review::MAX_SOURCE_BYTES, "SPX-G131")))
            .unwrap();
    let other_source = SOURCE.replace("42", "41");
    let other_revision = graph::revision(&parse(&other_source, &source_path).unwrap());
    let other_patch = format!("base {other_revision}\nrename helper.answer to computed\n");
    let other_preflight =
        preflight_review_owned(other_source, other_patch, source_path.clone(), 1).unwrap();
    let Err(error) = prepare_a0_commit(&authenticated, &other_preflight) else {
        panic!("mismatched preflight must not prepare a commit");
    };
    assert_eq!(error[0].code, "SPX-G133");
    assert!(std::fs::read_dir(source_path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .all(|name| !name.contains(".semaprax-stage.")));
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), SOURCE);
    drop(authenticated);
    drop(guard);
    assert_owned_artifacts_removed(&source_path);
}

#[test]
fn general_patch_preflight_remains_standalone_strict_for_project_modules() {
    let no_main = "module patch.library;\n@id(\"library.value\")\nfn value() -> i64 { 1 }\n";
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-patch-standalone-strict-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("library.spx");
    std::fs::write(&path, no_main).unwrap();
    let revision = graph::revision(&parse(no_main, &path).unwrap());
    let patch = format!("base {revision}\nrename library.value to renamed\n");
    let Err(diagnostics) = prepare_owned_a0_patch_bytes(&path, patch.into_bytes()) else {
        panic!("no-main Project module acquired general A0 authority")
    };
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T105"));
    assert_owned_artifacts_removed(&path);

    let imported = "module patch.library;\nuse function @id(\"provider.value\") from provider.module as provider_value;\n@id(\"library.value\")\nfn value() -> i64 { provider_value() }\n";
    let path = directory.join("imported-library.spx");
    std::fs::write(&path, imported).unwrap();
    let revision = graph::revision(&parse(imported, &path).unwrap());
    let patch = format!("base {revision}\nrename library.value to renamed\n");
    let Err(diagnostics) = prepare_owned_a0_patch_bytes(&path, patch.into_bytes()) else {
        panic!("imported Project module acquired general A0 authority")
    };
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-G172"));
    assert_owned_artifacts_removed(&path);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn owned_patch_bytes_match_public_a0_apply_and_expose_exact_handoff_facts() {
    let (public_source, public_patch) = fixture("owned-byte-parity-public");
    let public_revision = apply(&public_source, &public_patch).unwrap();
    let public_candidate = std::fs::read_to_string(&public_source).unwrap();

    let (owned_source, patch_bytes) = owned_byte_fixture("parity-owned");
    let base_revision = graph::revision(&parse(SOURCE, &owned_source).unwrap());
    let prepared = prepare_owned_a0_patch_bytes(&owned_source, patch_bytes).unwrap();
    assert_eq!(prepared.base_revision(), base_revision);
    assert_eq!(prepared.candidate_revision(), public_revision);
    assert_eq!(prepared.canonical_candidate(), public_candidate);
    let owned_revision = commit_owned_a0(prepared).unwrap();

    assert_eq!(owned_revision, public_revision);
    assert_eq!(
        std::fs::read_to_string(&owned_source).unwrap(),
        public_candidate
    );
    assert_owned_artifacts_removed(&public_source);
    assert_owned_artifacts_removed(&owned_source);
    std::fs::remove_dir_all(public_source.parent().unwrap()).unwrap();
    std::fs::remove_dir_all(owned_source.parent().unwrap()).unwrap();
}

#[test]
fn owned_patch_handoff_rejects_stale_source_before_rename() {
    let (source_path, patch_bytes) = owned_byte_fixture("stale");
    let prepared = prepare_owned_a0_patch_bytes(&source_path, patch_bytes).unwrap();
    std::fs::write(&source_path, CONCURRENT_SOURCE).unwrap();

    let error = commit_owned_a0(prepared).unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(
        std::fs::read_to_string(&source_path).unwrap(),
        CONCURRENT_SOURCE
    );
    assert_owned_artifacts_removed(&source_path);
    std::fs::remove_dir_all(source_path.parent().unwrap()).unwrap();
}

#[test]
fn owned_patch_handoff_is_consumed_once_and_never_materializes_a_proposal() {
    let (source_path, patch_bytes) = owned_byte_fixture("one-use-no-proposal");
    let prepared = prepare_owned_a0_patch_bytes(&source_path, patch_bytes).unwrap();
    let canonical_source = std::fs::canonicalize(&source_path).unwrap();
    let names = std::fs::read_dir(source_path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        names,
        BTreeSet::from([
            "module.spx".to_owned(),
            ".module.spx.semaprax-patch.lock".to_owned(),
        ])
    );
    assert!(names.iter().all(|name| !name.ends_with(".spatch")));

    // The function type freezes the one-use API: the authority is passed
    // by value, not by shared or mutable reference.
    let consume_once: fn(A0OwnedPreparedCommit) -> Result<String, Vec<Diagnostic>> =
        commit_owned_a0;
    consume_once(prepared).unwrap();

    assert!(!sibling_path(&canonical_source, ".semaprax-patch.lock")
        .unwrap()
        .exists());
    assert!(std::fs::read_dir(source_path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .all(|name| name == "module.spx"));
    std::fs::remove_dir_all(source_path.parent().unwrap()).unwrap();
}

#[test]
fn target_preflight_constructs_four_thousand_edits_in_one_source_pass() {
    let mut source = String::from(
            "module patch.many_edits;\n@id(\"target.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{\n",
        );
    for index in 0..4096 {
        source.push_str(&format!("let value{index}=helper();\n"));
    }
    source.push_str("value4095}\n");
    let patch = format!(
        "base {}\nrename target.helper to renamed\n",
        graph::revision(&parse(&source, "many-edits.spx").unwrap())
    );
    let preflight = preflight_target_owned(
        source,
        patch,
        std::path::PathBuf::from("many-edits.spx"),
        crate::review::MAX_OPERATIONS,
        crate::target_evidence::MAX_NATIVE_C11_BYTES,
    )
    .unwrap();
    assert_eq!(
        preflight.canonical_candidate().matches("renamed()").count(),
        4097
    );
}

#[test]
fn typed_v2_selectors_distinguish_colons_and_reject_true_duplicates() {
    let distinct = "schema semaprax.semantic-patch.v2\nbase sha256:0\nrename-member owner a:b member c to first\nrename-member owner a member b:c to second\n";
    let parsed = parse_patch(distinct).unwrap();
    assert_eq!(parsed.operations.len(), 2);

    let duplicate = "schema semaprax.semantic-patch.v2\nbase sha256:0\nrename-member owner a:b member c to first\nrename-member owner a:b member c to second\n";
    let error = parse_patch(duplicate).unwrap_err();
    assert_eq!(error[0].code, "SPX-G106");
}

#[test]
fn workspace_patch_requires_canonical_bytes_and_a_semantic_change() {
    let revision = graph::revision(&parse(SOURCE, "logical/module.spx").unwrap());
    let canonical = format!("base {revision}\nrename helper.answer to computed\n");
    let preflight = preflight_workspace_owned(
        SOURCE.to_owned(),
        canonical.clone(),
        PathBuf::from("logical/module.spx"),
        WorkspacePreflightLimits::new(4096, 16 * 1024 * 1024, 4096, 1024, 65_536),
    )
    .unwrap();
    assert_ne!(preflight.base_revision(), preflight.candidate_revision());

    for hostile in [
        format!("# comment\n{canonical}"),
        canonical.replace("rename ", "rename  "),
        canonical.replace('\n', "\r\n"),
        canonical.trim_end().to_owned(),
        format!("{canonical}\n"),
    ] {
        let error = preflight_workspace_owned(
            SOURCE.to_owned(),
            hostile,
            PathBuf::from("logical/module.spx"),
            WorkspacePreflightLimits::new(4096, 16 * 1024 * 1024, 4096, 1024, 65_536),
        )
        .err()
        .expect("hostile spelling must be rejected");
        assert_eq!(error[0].code, "SPX-G150");
    }

    let no_op = format!("base {revision}\nrename helper.answer to answer\n");
    let error = preflight_workspace_owned(
        SOURCE.to_owned(),
        no_op,
        PathBuf::from("logical/module.spx"),
        WorkspacePreflightLimits::new(4096, 16 * 1024 * 1024, 4096, 1024, 65_536),
    )
    .err()
    .expect("semantic no-op must be rejected");
    assert_eq!(error[0].code, "SPX-G153");
}

#[test]
fn workspace_v1_duplicate_selector_is_rejected_after_typed_canonical_parse() {
    let revision = graph::revision(&parse(SOURCE, "logical/module.spx").unwrap());
    let patch =
        format!("base {revision}\nrename helper.answer to first\nrename helper.answer to second\n");
    let error = preflight_workspace_owned(
        SOURCE.to_owned(),
        patch,
        PathBuf::from("logical/module.spx"),
        WorkspacePreflightLimits::new(4096, 16 * 1024 * 1024, 4096, 1024, 65_536),
    )
    .err()
    .expect("duplicate selector must be rejected");
    assert_eq!(error[0].code, "SPX-G150");
}

#[test]
fn workspace_candidate_canonicalization_is_bounded_and_checked_after_formatting() {
    let revision = graph::revision(&parse(SOURCE, "logical/module.spx").unwrap());
    let patch = format!("base {revision}\nrename helper.answer to computed\n");
    let unrestricted = preflight_workspace_owned_with_formatter_limit(
        SOURCE.to_owned(),
        patch.clone(),
        PathBuf::from("logical/module.spx"),
        WorkspacePreflightLimits::new(4096, 16 * 1024 * 1024, 4096, 1024, 65_536),
        WORKSPACE_FORMATTER_WORK_BYTES,
    )
    .unwrap();
    let exact_candidate_bytes = unrestricted.canonical_candidate().len();

    let exact = preflight_workspace_owned_with_formatter_limit(
        SOURCE.to_owned(),
        patch.clone(),
        PathBuf::from("logical/module.spx"),
        WorkspacePreflightLimits::new(4096, exact_candidate_bytes, 4096, 1024, 65_536),
        WORKSPACE_FORMATTER_WORK_BYTES,
    )
    .unwrap();
    assert_eq!(
        exact.canonical_candidate(),
        unrestricted.canonical_candidate()
    );

    let over = preflight_workspace_owned_with_formatter_limit(
        SOURCE.to_owned(),
        patch.clone(),
        PathBuf::from("logical/module.spx"),
        WorkspacePreflightLimits::new(4096, exact_candidate_bytes - 1, 4096, 1024, 65_536),
        WORKSPACE_FORMATTER_WORK_BYTES,
    )
    .err()
    .expect("one byte below the candidate limit must fail");
    assert_eq!(over[0].code, "SPX-G151");

    let formatter_overflow = preflight_workspace_owned_with_formatter_limit(
        SOURCE.to_owned(),
        patch,
        PathBuf::from("logical/module.spx"),
        WorkspacePreflightLimits::new(4096, 16 * 1024 * 1024, 4096, 1024, 65_536),
        1,
    )
    .err()
    .expect("one byte of formatter work must fail closed");
    assert_eq!(formatter_overflow[0].code, "SPX-G151");
    assert_eq!(
        formatter_overflow[0].message,
        "workspace patch preflight exceeds its bounded formatter-work limit"
    );
}

#[test]
fn workspace_rejects_canonical_expansion_beyond_the_remaining_candidate_budget() {
    let source = "module patch.expand;@id(\"helper.answer\")fn answer()->i64{42}@id(\"app.main\")fn main()->i64{answer()}\n";
    let revision = graph::revision(&parse(source, "logical/expand.spx").unwrap());
    let patch = format!("base {revision}\nrename helper.answer to computed\n");
    let raw_candidate =
        source
            .replacen("fn answer", "fn computed", 1)
            .replacen("{answer()}", "{computed()}", 1);
    let unrestricted = preflight_workspace_owned_with_formatter_limit(
        source.to_owned(),
        patch.clone(),
        PathBuf::from("logical/expand.spx"),
        WorkspacePreflightLimits::new(4096, 16 * 1024 * 1024, 4096, 1024, 65_536),
        WORKSPACE_FORMATTER_WORK_BYTES,
    )
    .unwrap();
    assert!(unrestricted.canonical_candidate().len() > raw_candidate.len());

    let error = preflight_workspace_owned_with_formatter_limit(
        source.to_owned(),
        patch,
        PathBuf::from("logical/expand.spx"),
        WorkspacePreflightLimits::new(4096, raw_candidate.len(), 4096, 1024, 65_536),
        WORKSPACE_FORMATTER_WORK_BYTES,
    )
    .err()
    .expect("canonical expansion beyond the remaining budget must fail");
    assert_eq!(error[0].code, "SPX-G151");
    assert_eq!(
        error[0].message,
        "workspace candidate exceeds the total candidate-source byte limit"
    );
}

#[test]
fn workspace_v3_candidate_is_checked_against_the_remaining_budget() {
    let (source_path, patch_path) = v3_fixture("workspace-candidate-bound");
    let directory = source_path
        .parent()
        .expect("v3 fixture has a parent")
        .to_path_buf();
    let source = std::fs::read_to_string(&source_path).unwrap();
    let patch = std::fs::read_to_string(&patch_path).unwrap();
    let unrestricted = preflight_workspace_owned_with_formatter_limit(
        source.clone(),
        patch.clone(),
        PathBuf::from("logical/v3.spx"),
        WorkspacePreflightLimits::new(4096, 16 * 1024 * 1024, 4096, 1024, 65_536),
        WORKSPACE_FORMATTER_WORK_BYTES,
    )
    .unwrap();
    let exact_candidate_bytes = unrestricted.canonical_candidate().len();

    let exact = preflight_workspace_owned_with_formatter_limit(
        source.clone(),
        patch.clone(),
        PathBuf::from("logical/v3.spx"),
        WorkspacePreflightLimits::new(4096, exact_candidate_bytes, 4096, 1024, 65_536),
        WORKSPACE_FORMATTER_WORK_BYTES,
    )
    .unwrap();
    assert_eq!(
        exact.canonical_candidate(),
        unrestricted.canonical_candidate()
    );

    let error = preflight_workspace_owned_with_formatter_limit(
        source,
        patch,
        PathBuf::from("logical/v3.spx"),
        WorkspacePreflightLimits::new(4096, exact_candidate_bytes - 1, 4096, 1024, 65_536),
        WORKSPACE_FORMATTER_WORK_BYTES,
    )
    .err()
    .expect("v3 candidate beyond the remaining budget must fail");
    assert_eq!(error[0].code, "SPX-G151");
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(windows)]
#[test]
fn windows_held_identity_matches_hardlinks_and_rejects_distinct_files() {
    let (source_path, _) = fixture("windows-held-identity");
    let source_file = File::open(&source_path).unwrap();
    let source_metadata = source_file.metadata().unwrap();
    let source_identity = platform_handle_identity(&source_file, &source_metadata).unwrap();

    let hardlink_path = source_path.with_extension("hardlink.spx");
    std::fs::hard_link(&source_path, &hardlink_path).unwrap();
    let hardlink_metadata = std::fs::symlink_metadata(&hardlink_path).unwrap();
    let hardlink_identity = platform_path_identity(&hardlink_path, &hardlink_metadata).unwrap();
    assert_eq!(source_identity, hardlink_identity);

    let distinct_path = source_path.with_extension("distinct.spx");
    std::fs::write(&distinct_path, SOURCE).unwrap();
    let distinct_metadata = std::fs::symlink_metadata(&distinct_path).unwrap();
    let distinct_identity = platform_path_identity(&distinct_path, &distinct_metadata).unwrap();
    assert_ne!(source_identity, distinct_identity);
}
