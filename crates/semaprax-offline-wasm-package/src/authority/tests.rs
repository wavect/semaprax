use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.0.join(path)
    }
}

impl AsRef<Path> for TestRoot {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

fn root(label: &str) -> TestRoot {
    let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "semaprax-offline-wasm-publisher-{label}-{}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    TestRoot(path)
}

fn build() -> OfflinePackageBuild {
    OfflinePackageBuild {
        module_wasm: b"\0asm\x01\0\0\0".to_vec(),
        manifest_json: "{\"manifest\":true}".to_owned(),
        evidence_json: "{\"evidence\":true}".to_owned(),
    }
}

fn linked_build() -> LinkedOfflinePackageBuild {
    LinkedOfflinePackageBuild {
        module_wasm: b"\0asm\x01\0\0\0".to_vec(),
        manifest_json: "{\"linked_manifest\":true}".to_owned(),
        evidence_json: "{\"linked_evidence\":true}".to_owned(),
    }
}

fn accept(_: &OfflinePackageBuild) -> Result<(), CompilerReplayFailure> {
    Ok(())
}

#[test]
fn publishes_only_the_exact_three_file_inventory() {
    let root = root("success");
    let output = root.join("package");
    let build = build();
    publish_observed(&output, &build, &mut accept, &mut NoopObserver).unwrap();
    assert_eq!(
        fs::read(output.join(MODULE_FILE)).unwrap(),
        build.module_wasm
    );
    assert_eq!(
        fs::read(output.join(EVIDENCE_FILE)).unwrap(),
        build.evidence_json.as_bytes()
    );
    assert_eq!(
        fs::read(output.join(MANIFEST_FILE)).unwrap(),
        build.manifest_json.as_bytes()
    );
    let mut names = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        [MODULE_FILE, EVIDENCE_FILE, MANIFEST_FILE].map(std::ffi::OsString::from)
    );
}

#[test]
fn existing_output_is_never_replaced() {
    let root = root("exists");
    let output = root.join("package");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("sentinel"), b"foreign").unwrap();
    let error = publish_observed(&output, &build(), &mut accept, &mut NoopObserver).unwrap_err();
    assert_eq!(error.code, PP_EXISTS);
    assert_eq!(error.visibility, PublicationVisibility::NotPublished);
    assert_eq!(fs::read(output.join("sentinel")).unwrap(), b"foreign");
}

#[test]
fn foreign_precreated_inventory_name_is_preserved_on_cleanup_failure() {
    let root = root("foreign-inventory");
    let output = root.join("package");
    let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
        if point == PublishPoint::BeforeWrite(1) {
            fs::write(paths.stage.join(EVIDENCE_FILE), b"foreign").unwrap();
        }
    });
    let error = publish_observed(&output, &build(), &mut accept, &mut observer).unwrap_err();
    assert_eq!(error.code, crate::PP_CLEANUP);
    assert_eq!(error.primary_code, Some(PP_CHANGED));
    assert_eq!(error.cleanup, CleanupStatus::Incomplete);
    let stage = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".semaprax-")
        })
        .unwrap();
    assert_eq!(fs::read(stage.join(EVIDENCE_FILE)).unwrap(), b"foreign");
}

#[test]
fn same_byte_file_substitution_is_rejected_before_visibility() {
    let root = root("substitution");
    let output = root.join("package");
    let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
        if point == PublishPoint::BeforeSecondReplay {
            let file = paths.stage.join(MODULE_FILE);
            let displaced = paths.stage.join("displaced");
            fs::rename(&file, displaced).unwrap();
            fs::write(file, b"\0asm\x01\0\0\0").unwrap();
        }
    });
    let error = publish_observed(&output, &build(), &mut accept, &mut observer).unwrap_err();
    assert_eq!(error.code, crate::PP_CLEANUP);
    assert_eq!(error.primary_code, Some(PP_CHANGED));
    assert!(!output.exists());
}

#[test]
fn foreign_entry_added_during_second_replay_is_never_published() {
    let root = root("second-inventory");
    let output = root.join("package");
    let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
        if point == PublishPoint::BeforeSecondReplay {
            fs::write(paths.stage.join("foreign"), b"foreign").unwrap();
        }
    });
    let error = publish_observed(&output, &build(), &mut accept, &mut observer).unwrap_err();
    assert_eq!(error.code, crate::PP_CLEANUP);
    assert_eq!(error.primary_code, Some(PP_CHANGED));
    assert_eq!(error.cleanup, CleanupStatus::Incomplete);
    assert_eq!(error.visibility, PublicationVisibility::NotPublished);
    assert!(!output.exists());
    let stage = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".semaprax-")
        })
        .unwrap();
    assert_eq!(fs::read(stage.join("foreign")).unwrap(), b"foreign");
}

#[test]
fn second_replay_failure_discards_the_exact_stage_and_keeps_the_primary() {
    let root = root("second-replay");
    let output = root.join("package");
    let mut verifier = |_: &OfflinePackageBuild| {
        Err::<(), _>(CompilerReplayFailure {
            code: "SPX-PB507",
            message: "adversarial replay mismatch".to_owned(),
        })
    };
    let error = publish_observed(&output, &build(), &mut verifier, &mut NoopObserver).unwrap_err();
    assert_eq!(error.code, crate::PP_REPLAY);
    assert_eq!(error.compiler_code, Some("SPX-PB507"));
    assert_eq!(error.cleanup, CleanupStatus::Settled);
    assert_eq!(error.visibility, PublicationVisibility::NotPublished);
    assert!(!output.exists());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
}

#[test]
fn output_race_at_publish_is_fail_stop_and_preserves_foreign_bytes() {
    let root = root("output-race");
    let output = root.join("package");
    let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
        if point == PublishPoint::BeforePublish {
            fs::create_dir(paths.output).unwrap();
            fs::write(paths.output.join("sentinel"), b"foreign").unwrap();
        }
    });
    let error = publish_observed(&output, &build(), &mut accept, &mut observer).unwrap_err();
    assert_eq!(error.code, PP_EXISTS);
    assert_eq!(
        error.cleanup,
        CleanupStatus::SuppressedAfterPublicationAttempt
    );
    assert_eq!(error.visibility, PublicationVisibility::NotPublished);
    assert_eq!(fs::read(output.join("sentinel")).unwrap(), b"foreign");
    assert!(fs::read_dir(&root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".semaprax-")));
}

#[test]
fn post_publish_mutation_is_reported_visible_and_never_discarded() {
    let root = root("post-publish");
    let output = root.join("package");
    let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
        if point == PublishPoint::AfterPublish {
            fs::write(paths.output.join("foreign"), b"foreign").unwrap();
        }
    });
    let error = publish_observed(&output, &build(), &mut accept, &mut observer).unwrap_err();
    assert_eq!(error.code, PP_PUBLISHED_CHANGED);
    assert_eq!(error.visibility, PublicationVisibility::Published);
    assert_eq!(error.cleanup, CleanupStatus::NotNeeded);
    assert_eq!(fs::read(output.join("foreign")).unwrap(), b"foreign");
    assert_eq!(
        fs::read(output.join(MODULE_FILE)).unwrap(),
        build().module_wasm
    );
}

#[test]
fn post_publish_output_path_substitution_is_visible_failure() {
    let root = root("post-publish-path-substitution");
    let output = root.join("package");
    let displaced = root.join("displaced");
    let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
        if point == PublishPoint::AfterPublish {
            fs::rename(paths.output, &displaced).unwrap();
            fs::create_dir(paths.output).unwrap();
            fs::write(paths.output.join("sentinel"), b"foreign").unwrap();
        }
    });
    let error = publish_observed(&output, &build(), &mut accept, &mut observer).unwrap_err();
    assert_eq!(error.code, PP_PUBLISHED_CHANGED);
    assert_eq!(error.visibility, PublicationVisibility::Published);
    assert_eq!(error.cleanup, CleanupStatus::NotNeeded);
    assert_eq!(fs::read(output.join("sentinel")).unwrap(), b"foreign");
    assert_eq!(
        fs::read(displaced.join(MODULE_FILE)).unwrap(),
        build().module_wasm
    );
}

struct ClosureObserver<F>(F);

#[test]
fn shared_artifact_adapter_preserves_v1_file_names_order_and_bytes() {
    let build = build();
    assert_eq!(
        ArtifactFiles::from_v1(&build).files(),
        [
            (MODULE_FILE, build.module_wasm.as_slice()),
            (EVIDENCE_FILE, build.evidence_json.as_bytes()),
            (MANIFEST_FILE, build.manifest_json.as_bytes()),
        ]
    );
}

#[test]
fn sealed_lock_snapshot_adapter_has_only_the_frozen_inventory() {
    let snapshot = ResolutionSnapshot {
        input_json: "input".to_owned(),
        resolution_evidence_json: "resolution".to_owned(),
        lock_json: "lock".to_owned(),
    };
    assert_eq!(
        ArtifactFiles::from_lock_snapshot(&snapshot).files(),
        [
            (INPUT_FILE, b"input".as_slice()),
            (RESOLUTION_FILE, b"resolution".as_slice()),
            (LOCK_FILE, b"lock".as_slice()),
        ]
    );
}

#[test]
fn lock_snapshot_held_replay_disagreement_settles_without_visibility() {
    let root = root("lock-snapshot-replay");
    let output = root.join("snapshot");
    let snapshot = ResolutionSnapshot {
        input_json: "input".to_owned(),
        resolution_evidence_json: "resolution".to_owned(),
        lock_json: "lock".to_owned(),
    };
    let mut replay = || -> Result<(), CompilerReplayFailure> {
        Err(CompilerReplayFailure {
            code: "SPX-PK505",
            message: "injected exact snapshot disagreement".to_owned(),
        })
    };
    let mut observer = NoopObserver;
    let error = publish_artifacts(
        &output,
        ArtifactFiles::from_lock_snapshot(&snapshot),
        &mut replay,
        &mut observer,
    )
    .unwrap_err();
    assert_eq!(error.code, crate::PP_REPLAY);
    assert_eq!(error.compiler_code, Some("SPX-PK505"));
    assert_eq!(error.visibility, PublicationVisibility::NotPublished);
    assert_eq!(error.cleanup, CleanupStatus::Settled);
    assert!(!output.exists());
}

#[test]
fn lock_snapshot_same_byte_staged_substitution_fails_before_visibility() {
    let root = root("lock-snapshot-substitution");
    let output = root.join("snapshot");
    let snapshot = ResolutionSnapshot {
        input_json: "input".to_owned(),
        resolution_evidence_json: "resolution".to_owned(),
        lock_json: "lock".to_owned(),
    };
    let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
        if point == PublishPoint::BeforeSecondReplay {
            let file = paths.stage.join(INPUT_FILE);
            fs::rename(&file, paths.stage.join("displaced")).unwrap();
            fs::write(file, b"input").unwrap();
        }
    });
    let mut replay = || Ok(());
    let error = publish_artifacts(
        &output,
        ArtifactFiles::from_lock_snapshot(&snapshot),
        &mut replay,
        &mut observer,
    )
    .unwrap_err();
    assert_eq!(error.code, crate::PP_CLEANUP);
    assert_eq!(error.primary_code, Some(PP_CHANGED));
    assert_eq!(error.visibility, PublicationVisibility::NotPublished);
    assert!(!output.exists());
}

#[test]
fn lock_snapshot_rejects_every_pre_effect_write_and_settle_mutation() {
    for write_index in 0..3 {
        let root = root(&format!("lock-snapshot-write-{write_index}"));
        let output = root.join("snapshot");
        let snapshot = ResolutionSnapshot {
            input_json: "input".to_owned(),
            resolution_evidence_json: "resolution".to_owned(),
            lock_json: "lock".to_owned(),
        };
        let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
            if point == PublishPoint::BeforeWrite(write_index) {
                fs::write(paths.stage.join("foreign"), b"foreign").unwrap();
            }
        });
        let mut replay = || Ok(());
        let error = publish_artifacts(
            &output,
            ArtifactFiles::from_lock_snapshot(&snapshot),
            &mut replay,
            &mut observer,
        )
        .unwrap_err();
        assert_eq!(error.code, crate::PP_CLEANUP);
        assert_eq!(error.primary_code, Some(PP_CHANGED));
        assert_eq!(error.cleanup, CleanupStatus::Incomplete);
        assert!(!output.exists());
    }

    let root = root("lock-snapshot-settle");
    let output = root.join("snapshot");
    let snapshot = ResolutionSnapshot {
        input_json: "input".to_owned(),
        resolution_evidence_json: "resolution".to_owned(),
        lock_json: "lock".to_owned(),
    };
    let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
        if point == PublishPoint::BeforeSettle {
            fs::write(paths.stage.join("foreign"), b"foreign").unwrap();
        }
    });
    let mut replay = || Ok(());
    let error = publish_artifacts(
        &output,
        ArtifactFiles::from_lock_snapshot(&snapshot),
        &mut replay,
        &mut observer,
    )
    .unwrap_err();
    assert_eq!(error.code, crate::PP_CLEANUP);
    assert_eq!(error.primary_code, Some(PP_CHANGED));
    assert_eq!(error.cleanup, CleanupStatus::Incomplete);
    assert!(!output.exists());
}

#[test]
fn lock_snapshot_publish_and_post_publish_uncertainty_fail_stop() {
    let root = root("lock-snapshot-publish-race");
    let output = root.join("snapshot");
    let snapshot = ResolutionSnapshot {
        input_json: "input".to_owned(),
        resolution_evidence_json: "resolution".to_owned(),
        lock_json: "lock".to_owned(),
    };
    let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
        if point == PublishPoint::BeforePublish {
            fs::create_dir(paths.output).unwrap();
            fs::write(paths.output.join("sentinel"), b"foreign").unwrap();
        }
    });
    let mut replay = || Ok(());
    let error = publish_artifacts(
        &output,
        ArtifactFiles::from_lock_snapshot(&snapshot),
        &mut replay,
        &mut observer,
    )
    .unwrap_err();
    assert_eq!(error.code, PP_EXISTS);
    assert_eq!(
        error.cleanup,
        CleanupStatus::SuppressedAfterPublicationAttempt
    );
    assert_eq!(fs::read(output.join("sentinel")).unwrap(), b"foreign");

    let root = root("lock-snapshot-post-publish");
    let output = root.join("snapshot");
    let displaced = root.join("displaced");
    let mut observer = ClosureObserver(|point: PublishPoint, paths: &ObservedPaths<'_>| {
        if point == PublishPoint::AfterPublish {
            fs::rename(paths.output, &displaced).unwrap();
            fs::create_dir(paths.output).unwrap();
            fs::write(paths.output.join("sentinel"), b"foreign").unwrap();
        }
    });
    let mut replay = || Ok(());
    let error = publish_artifacts(
        &output,
        ArtifactFiles::from_lock_snapshot(&snapshot),
        &mut replay,
        &mut observer,
    )
    .unwrap_err();
    assert_eq!(error.code, PP_PUBLISHED_CHANGED);
    assert_eq!(error.visibility, PublicationVisibility::Published);
    assert_eq!(error.cleanup, CleanupStatus::NotNeeded);
    assert_eq!(fs::read(output.join("sentinel")).unwrap(), b"foreign");
    assert_eq!(fs::read(displaced.join(INPUT_FILE)).unwrap(), b"input");
}

#[test]
fn linked_adapter_uses_the_same_exact_file_inventory_and_held_second_replay() {
    let root = root("linked-success");
    let output = root.join("package");
    let build = linked_build();
    let mut held_replays = 0;
    publish_linked_verified(&output, &build, &mut |_| {
        held_replays += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(held_replays, 1);
    assert_eq!(
        fs::read(output.join(MODULE_FILE)).unwrap(),
        build.module_wasm
    );
    assert_eq!(
        fs::read(output.join(EVIDENCE_FILE)).unwrap(),
        build.evidence_json.as_bytes()
    );
    assert_eq!(
        fs::read(output.join(MANIFEST_FILE)).unwrap(),
        build.manifest_json.as_bytes()
    );
    let mut names = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        [MODULE_FILE, EVIDENCE_FILE, MANIFEST_FILE].map(std::ffi::OsString::from)
    );
}

#[test]
fn linked_held_replay_failure_settles_before_any_publication() {
    let root = root("linked-replay-failure");
    let output = root.join("package");
    let build = linked_build();
    let mut replay = || -> Result<(), CompilerReplayFailure> {
        Err(CompilerReplayFailure {
            code: "SPX-PB607",
            message: "injected linked replay rejection".to_owned(),
        })
    };
    let mut observer = NoopObserver;
    let error = publish_artifacts(
        &output,
        ArtifactFiles::from_v2(&build),
        &mut replay,
        &mut observer,
    )
    .unwrap_err();
    assert_eq!(error.code, crate::PP_REPLAY);
    assert_eq!(error.compiler_code, Some("SPX-PB607"));
    assert_eq!(error.visibility, PublicationVisibility::NotPublished);
    assert_eq!(error.cleanup, CleanupStatus::Settled);
    assert!(!output.exists());
    assert!(!fs::read_dir(&root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".semaprax-")));
}

impl<F> Observer for ClosureObserver<F>
where
    F: for<'a, 'b> FnMut(PublishPoint, &'a ObservedPaths<'b>),
{
    fn at(&mut self, point: PublishPoint, paths: &ObservedPaths<'_>) {
        (self.0)(point, paths);
    }
}
