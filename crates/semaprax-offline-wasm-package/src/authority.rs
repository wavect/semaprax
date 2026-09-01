use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_build::OfflinePackageBuild;
use semaprax::package_build_v2::LinkedOfflinePackageBuild;
use semaprax::package_resolution_snapshot::ResolutionSnapshot;
use semaprax_native_rust_interop_platform as platform;

use crate::{
    lock_snapshot::{INPUT_FILE, LOCK_FILE, RESOLUTION_FILE},
    CleanupStatus, CompilerReplayFailure, PublicationError, PublicationVisibility, EVIDENCE_FILE,
    MANIFEST_FILE, MODULE_FILE, PP_CHANGED, PP_EXISTS, PP_INVALID, PP_PUBLISHED_CHANGED,
    PP_STAGE_EXHAUSTED,
};

const BUILD_INVENTORY: [&str; 3] = [MODULE_FILE, EVIDENCE_FILE, MANIFEST_FILE];
const LOCK_SNAPSHOT_INVENTORY: [&str; 3] = [INPUT_FILE, RESOLUTION_FILE, LOCK_FILE];
const MAX_STAGE_ATTEMPTS: usize = 64;
static STAGE_SERIAL: AtomicU64 = AtomicU64::new(0);

pub(crate) fn publish_verified<V>(
    output: &Path,
    build: &OfflinePackageBuild,
    verifier: &mut impl FnMut(&OfflinePackageBuild) -> Result<V, CompilerReplayFailure>,
) -> Result<(), PublicationError> {
    let files = ArtifactFiles::from_v1(build);
    let mut replay = || verifier(build);
    let mut observer = NoopObserver;
    publish_artifacts(output, files, &mut replay, &mut observer)
}

pub(crate) fn publish_linked_verified<V>(
    output: &Path,
    build: &LinkedOfflinePackageBuild,
    verifier: &mut impl FnMut(&LinkedOfflinePackageBuild) -> Result<V, CompilerReplayFailure>,
) -> Result<(), PublicationError> {
    let files = ArtifactFiles::from_v2(build);
    let mut replay = || verifier(build);
    let mut observer = NoopObserver;
    publish_artifacts(output, files, &mut replay, &mut observer)
}

pub(crate) fn publish_lock_snapshot_verified<V>(
    output: &Path,
    snapshot: &ResolutionSnapshot,
    verifier: &mut impl FnMut() -> Result<V, CompilerReplayFailure>,
) -> Result<(), PublicationError> {
    let files = ArtifactFiles::from_lock_snapshot(snapshot);
    let mut observer = NoopObserver;
    publish_artifacts(output, files, verifier, &mut observer)
}

#[cfg(test)]
fn publish_observed<V>(
    output: &Path,
    build: &OfflinePackageBuild,
    verifier: &mut impl FnMut(&OfflinePackageBuild) -> Result<V, CompilerReplayFailure>,
    observer: &mut impl Observer,
) -> Result<(), PublicationError> {
    let files = ArtifactFiles::from_v1(build);
    let mut replay = || verifier(build);
    publish_artifacts(output, files, &mut replay, observer)
}

fn publish_artifacts<V>(
    output: &Path,
    artifacts: ArtifactFiles<'_>,
    verifier: &mut impl FnMut() -> Result<V, CompilerReplayFailure>,
    observer: &mut impl Observer,
) -> Result<(), PublicationError> {
    let parent_path = output
        .parent()
        .ok_or_else(|| PublicationError::plain(PP_INVALID, "publication output has no parent"))?
        .to_path_buf();
    let output_name = output
        .file_name()
        .ok_or_else(|| PublicationError::plain(PP_INVALID, "publication output has no leaf"))?
        .to_os_string();
    let parent =
        platform::hold_directory(&parent_path).map_err(|_| changed("hold output parent"))?;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if !platform::directory_is_current_user_private(&parent)
        .map_err(|_| changed("authenticate private output parent"))?
    {
        return Err(PublicationError::plain(
            PP_INVALID,
            "Unix publication parent must be current-euid-owned with exact mode 0700",
        ));
    }
    require_path_binding(&parent, &parent_path, "output parent path changed")?;
    let output_probe = platform::prepare_child_name(&output_name)
        .map_err(|_| PublicationError::plain(PP_INVALID, "publication leaf is invalid"))?;
    if !platform::child_absent_prepared(&parent, &output_probe)
        .map_err(|_| changed("inspect output leaf"))?
    {
        return Err(PublicationError::plain(
            PP_EXISTS,
            "publication output already exists",
        ));
    }

    let artifact_names = artifacts.names();
    let names = artifact_names.map(OsStr::new);
    let inventory = platform::prepare_discard_inventory(names)
        .map_err(|_| PublicationError::plain(PP_INVALID, "fixed inventory is invalid"))?;
    let exact = platform::prepare_inventory_exact(&inventory)
        .map_err(|_| changed("prepare exact staged inventory"))?;
    let post = platform::prepare_inventory_entries_exact(names, artifact_names.len())
        .map_err(|_| changed("prepare exact published inventory"))?;
    let publish = platform::prepare_publish_directory(&output_name)
        .map_err(|_| PublicationError::plain(PP_INVALID, "publication leaf is invalid"))?;

    let (stage_name, stage_path, stage) = allocate_stage(&parent, &parent_path)?;
    let mut staged = StagedPublication {
        parent,
        parent_path,
        output_path: output.to_path_buf(),
        output_name,
        output_probe,
        stage,
        stage_name,
        stage_path,
        inventory,
        exact,
        post,
        publish,
        publication_attempted: false,
        published: false,
        post_failure: Some(published_changed(
            "published output failed exact post-publication authentication",
        )),
        artifact_names,
    };

    if let Err(primary) = staged.write_and_authenticate(artifacts, observer) {
        return Err(staged.fail_before_attempt(primary));
    }
    observer.at(PublishPoint::BeforeSecondReplay, &staged.paths());
    if let Err(failure) = verifier() {
        return Err(staged.fail_before_attempt(PublicationError::replay(failure)));
    }
    if let Err(primary) = staged.authenticate_files(artifacts) {
        return Err(staged.fail_before_attempt(primary));
    }
    if let Err(primary) = staged.prepare_for_publish(observer) {
        return Err(staged.fail_before_attempt(primary));
    }

    staged.publication_attempted = true;
    observer.at(PublishPoint::BeforePublish, &staged.paths());
    if let Err(error) = platform::publish_directory_new_prepared(
        &mut staged.publish,
        &staged.parent,
        &staged.stage,
        &staged.stage_name,
        &staged.output_name,
    ) {
        let primary = if error == platform::Error::Exists {
            PublicationError::plain(PP_EXISTS, "publication output appeared before rename")
        } else {
            changed("attempt no-replace publication")
        };
        return Err(PublicationError::suppressed_after_attempt(primary));
    }
    staged.published = true;
    observer.at(PublishPoint::AfterPublish, &staged.paths());
    staged.authenticate_published()
}

fn allocate_stage(
    parent: &platform::HeldDirectory,
    parent_path: &Path,
) -> Result<
    (
        platform::PreparedStageName,
        PathBuf,
        platform::HeldDirectory,
    ),
    PublicationError,
> {
    for _ in 0..MAX_STAGE_ATTEMPTS {
        let serial = STAGE_SERIAL.fetch_add(1, Ordering::Relaxed);
        let text = format!(".semaprax-wasm-package-{}-{serial}", std::process::id());
        let prepared = platform::prepare_stage_name(OsStr::new(&text))
            .map_err(|_| PublicationError::plain(PP_INVALID, "stage name is invalid"))?;
        match platform::create_directory_new_prepared_settled(parent, &prepared, 0o700) {
            Ok(stage) => return Ok((prepared, parent_path.join(text), stage)),
            Err(failure)
                if failure.error == platform::Error::Exists && !failure.namespace_created =>
            {
                continue;
            }
            Err(failure) if failure.namespace_created => {
                return Err(PublicationError::unheld_namespace(changed(
                    "create held staging directory",
                )));
            }
            Err(_) => return Err(changed("create held staging directory")),
        }
    }
    Err(PublicationError::plain(
        PP_STAGE_EXHAUSTED,
        "bounded staging-name allocation was exhausted",
    ))
}

struct StagedPublication {
    parent: platform::HeldDirectory,
    parent_path: PathBuf,
    output_path: PathBuf,
    output_name: OsString,
    output_probe: platform::PreparedChildName,
    stage: platform::HeldDirectory,
    stage_name: platform::PreparedStageName,
    stage_path: PathBuf,
    inventory: platform::PreparedDiscardInventory<3>,
    exact: platform::PreparedInventoryExact<3>,
    post: platform::PreparedInventoryEntriesExact<3>,
    publish: platform::PreparedPublishDirectory,
    publication_attempted: bool,
    published: bool,
    post_failure: Option<PublicationError>,
    artifact_names: [&'static str; 3],
}

impl StagedPublication {
    fn paths(&self) -> ObservedPaths<'_> {
        ObservedPaths {
            parent: &self.parent_path,
            stage: &self.stage_path,
            output: &self.output_path,
        }
    }

    fn write_and_authenticate(
        &mut self,
        artifacts: ArtifactFiles<'_>,
        observer: &mut impl Observer,
    ) -> Result<(), PublicationError> {
        require_path_binding(
            &self.parent,
            &self.parent_path,
            "output parent path changed",
        )?;
        require_path_binding(&self.stage, &self.stage_path, "staging path changed")?;
        for (index, (name, bytes)) in artifacts.files().into_iter().enumerate() {
            observer.at(PublishPoint::BeforeWrite(index), &self.paths());
            platform::write_file_new_prepared(&self.stage, &mut self.inventory, name, bytes, 0o600)
                .map_err(|_| changed("write create-new staged artifact"))?;
        }
        self.authenticate_files(artifacts)?;
        platform::inventory_exact_prepared(&mut self.exact, &self.stage, &self.inventory)
            .map_err(|_| changed("authenticate exact staged inventory"))
    }

    fn authenticate_files(&self, artifacts: ArtifactFiles<'_>) -> Result<(), PublicationError> {
        let mut scratch = [0_u8; platform::FILE_COMPARE_SCRATCH_BYTES];
        for (name, expected) in artifacts.files() {
            let held = self
                .inventory
                .file(name)
                .map_err(|_| changed("recover held staged artifact"))?;
            if !platform::compare_exact(held, expected, &mut scratch)
                .map_err(|_| changed("authenticate staged artifact bytes"))?
            {
                return Err(changed("authenticate staged artifact bytes"));
            }
            platform::recheck_regular_file(held)
                .map_err(|_| changed("recheck staged artifact identity"))?;
        }
        Ok(())
    }

    fn prepare_for_publish(
        &mut self,
        observer: &mut impl Observer,
    ) -> Result<(), PublicationError> {
        observer.at(PublishPoint::BeforeSettle, &self.paths());
        require_path_binding(
            &self.parent,
            &self.parent_path,
            "output parent path changed",
        )?;
        require_path_binding(&self.stage, &self.stage_path, "staging path changed")?;
        self.authenticate_files_for_settle()?;
        platform::inventory_exact_prepared(&mut self.exact, &self.stage, &self.inventory)
            .map_err(|_| changed("reauthenticate exact staged inventory before settle"))?;
        self.inventory
            .settle_for_publish()
            .map_err(|_| changed("settle staged artifact handles"))
    }

    fn authenticate_files_for_settle(&self) -> Result<(), PublicationError> {
        for name in self.artifact_names {
            platform::recheck_regular_file(
                self.inventory
                    .file(name)
                    .map_err(|_| changed("recover staged artifact before settle"))?,
            )
            .map_err(|_| changed("recheck staged artifact before settle"))?;
        }
        Ok(())
    }

    fn authenticate_published(&mut self) -> Result<(), PublicationError> {
        debug_assert!(self.publication_attempted && self.published);
        if !matches!(
            platform::same_child_directory_prepared(&self.parent, &self.output_probe, &self.stage,),
            Ok(true)
        ) {
            return Err(self.take_post_failure());
        }
        let files = [
            platform::hold_settled_regular_file_prepared(
                &self.stage,
                &self.inventory,
                self.artifact_names[0],
            )
            .map_err(|_| self.take_post_failure())?,
            platform::hold_settled_regular_file_prepared(
                &self.stage,
                &self.inventory,
                self.artifact_names[1],
            )
            .map_err(|_| self.take_post_failure())?,
            platform::hold_settled_regular_file_prepared(
                &self.stage,
                &self.inventory,
                self.artifact_names[2],
            )
            .map_err(|_| self.take_post_failure())?,
        ];
        if platform::inventory_entries_exact_prepared(
            &mut self.post,
            &self.stage,
            [&files[0], &files[1], &files[2]],
            [],
        )
        .is_err()
            || platform::recheck_directory(&self.parent).is_err()
            || platform::recheck_directory(&self.stage).is_err()
        {
            return Err(self.take_post_failure());
        }
        Ok(())
    }

    fn take_post_failure(&mut self) -> PublicationError {
        self.post_failure.take().unwrap_or(PublicationError {
            code: PP_PUBLISHED_CHANGED,
            message: String::new(),
            compiler_code: None,
            primary_code: None,
            visibility: PublicationVisibility::Published,
            cleanup: CleanupStatus::NotNeeded,
        })
    }

    fn fail_before_attempt(&self, mut primary: PublicationError) -> PublicationError {
        debug_assert!(!self.publication_attempted && !self.published);
        match platform::discard_owned_stage_prepared(
            &self.parent,
            &self.stage,
            &self.stage_name,
            &self.inventory,
        ) {
            Ok(()) => {
                primary.cleanup = CleanupStatus::Settled;
                primary
            }
            Err(_) => PublicationError::cleanup_incomplete(primary),
        }
    }
}

#[derive(Clone, Copy)]
enum ArtifactFiles<'a> {
    Build {
        module: &'a [u8],
        evidence: &'a [u8],
        manifest: &'a [u8],
    },
    LockSnapshot {
        input: &'a [u8],
        resolution: &'a [u8],
        lock: &'a [u8],
    },
}

impl<'a> ArtifactFiles<'a> {
    fn from_v1(build: &'a OfflinePackageBuild) -> Self {
        Self::Build {
            module: &build.module_wasm,
            evidence: build.evidence_json.as_bytes(),
            manifest: build.manifest_json.as_bytes(),
        }
    }

    fn from_v2(build: &'a LinkedOfflinePackageBuild) -> Self {
        Self::Build {
            module: &build.module_wasm,
            evidence: build.evidence_json.as_bytes(),
            manifest: build.manifest_json.as_bytes(),
        }
    }

    fn from_lock_snapshot(snapshot: &'a ResolutionSnapshot) -> Self {
        Self::LockSnapshot {
            input: snapshot.input_json.as_bytes(),
            resolution: snapshot.resolution_evidence_json.as_bytes(),
            lock: snapshot.lock_json.as_bytes(),
        }
    }

    const fn names(self) -> [&'static str; 3] {
        match self {
            Self::Build { .. } => BUILD_INVENTORY,
            Self::LockSnapshot { .. } => LOCK_SNAPSHOT_INVENTORY,
        }
    }

    fn files(self) -> [(&'static str, &'a [u8]); 3] {
        match self {
            Self::Build {
                module,
                evidence,
                manifest,
            } => [
                (MODULE_FILE, module),
                (EVIDENCE_FILE, evidence),
                (MANIFEST_FILE, manifest),
            ],
            Self::LockSnapshot {
                input,
                resolution,
                lock,
            } => [
                (INPUT_FILE, input),
                (RESOLUTION_FILE, resolution),
                (LOCK_FILE, lock),
            ],
        }
    }
}

fn require_path_binding(
    held: &platform::HeldDirectory,
    path: &Path,
    message: &'static str,
) -> Result<(), PublicationError> {
    if platform::same_directory_path(held, path).map_err(|_| changed(message))? {
        Ok(())
    } else {
        Err(changed(message))
    }
}

fn changed(action: &'static str) -> PublicationError {
    PublicationError::plain(
        PP_CHANGED,
        format!("held authority changed while trying to {action}"),
    )
}

fn published_changed(action: &'static str) -> PublicationError {
    PublicationError {
        code: PP_PUBLISHED_CHANGED,
        message: format!("published package changed while trying to {action}"),
        compiler_code: None,
        primary_code: None,
        visibility: PublicationVisibility::Published,
        cleanup: CleanupStatus::NotNeeded,
    }
}

#[derive(Clone, Copy)]
struct ObservedPaths<'a> {
    parent: &'a Path,
    stage: &'a Path,
    output: &'a Path,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublishPoint {
    BeforeWrite(usize),
    BeforeSecondReplay,
    BeforeSettle,
    BeforePublish,
    AfterPublish,
}

trait Observer {
    fn at(&mut self, point: PublishPoint, paths: &ObservedPaths<'_>);
}

struct NoopObserver;

impl Observer for NoopObserver {
    fn at(&mut self, _point: PublishPoint, paths: &ObservedPaths<'_>) {
        let _ = (paths.parent, paths.stage, paths.output);
    }
}

#[cfg(test)]
mod tests;
