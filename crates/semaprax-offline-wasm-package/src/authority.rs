use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_build::OfflinePackageBuild;
use semaprax_native_rust_interop_platform as platform;

use crate::{
    CleanupStatus, CompilerReplayFailure, PublicationError, PublicationVisibility, EVIDENCE_FILE,
    MANIFEST_FILE, MODULE_FILE, PP_CHANGED, PP_EXISTS, PP_INVALID, PP_PUBLISHED_CHANGED,
    PP_STAGE_EXHAUSTED,
};

const INVENTORY: [&str; 3] = [MODULE_FILE, EVIDENCE_FILE, MANIFEST_FILE];
const MAX_STAGE_ATTEMPTS: usize = 64;
static STAGE_SERIAL: AtomicU64 = AtomicU64::new(0);

pub(crate) fn publish_verified<V>(
    output: &Path,
    build: &OfflinePackageBuild,
    verifier: &mut impl FnMut(&OfflinePackageBuild) -> Result<V, CompilerReplayFailure>,
) -> Result<(), PublicationError> {
    let mut observer = NoopObserver;
    publish_observed(output, build, verifier, &mut observer)
}

fn publish_observed<V>(
    output: &Path,
    build: &OfflinePackageBuild,
    verifier: &mut impl FnMut(&OfflinePackageBuild) -> Result<V, CompilerReplayFailure>,
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

    let names = INVENTORY.map(OsStr::new);
    let inventory = platform::prepare_discard_inventory(names)
        .map_err(|_| PublicationError::plain(PP_INVALID, "fixed inventory is invalid"))?;
    let exact = platform::prepare_inventory_exact(&inventory)
        .map_err(|_| changed("prepare exact staged inventory"))?;
    let post = platform::prepare_inventory_entries_exact(names, INVENTORY.len())
        .map_err(|_| changed("prepare exact published inventory"))?;
    let publish = platform::prepare_publish_directory(&output_name)
        .map_err(|_| PublicationError::plain(PP_INVALID, "publication leaf is invalid"))?;

    let (stage_name, stage_path, stage) = allocate_stage(&parent, &parent_path)?;
    let mut staged = StagedPublication {
        parent,
        parent_path,
        output_path: output.to_path_buf(),
        output_name,
        stage,
        stage_name,
        stage_path,
        inventory,
        exact,
        post,
        publish,
        publication_attempted: false,
        published: false,
    };

    if let Err(primary) = staged.write_and_authenticate(build, observer) {
        return Err(staged.fail_before_attempt(primary));
    }
    observer.at(PublishPoint::BeforeSecondReplay, &staged.paths());
    if let Err(failure) = verifier(build) {
        return Err(staged.fail_before_attempt(PublicationError::replay(failure)));
    }
    if let Err(primary) = staged.authenticate_files(build) {
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
    staged.authenticate_published(build)
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
    stage: platform::HeldDirectory,
    stage_name: platform::PreparedStageName,
    stage_path: PathBuf,
    inventory: platform::PreparedDiscardInventory<3>,
    exact: platform::PreparedInventoryExact<3>,
    post: platform::PreparedInventoryEntriesExact<3>,
    publish: platform::PreparedPublishDirectory,
    publication_attempted: bool,
    published: bool,
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
        build: &OfflinePackageBuild,
        observer: &mut impl Observer,
    ) -> Result<(), PublicationError> {
        require_path_binding(
            &self.parent,
            &self.parent_path,
            "output parent path changed",
        )?;
        require_path_binding(&self.stage, &self.stage_path, "staging path changed")?;
        for (index, (name, bytes)) in files(build).into_iter().enumerate() {
            observer.at(PublishPoint::BeforeWrite(index), &self.paths());
            platform::write_file_new_prepared(&self.stage, &mut self.inventory, name, bytes, 0o600)
                .map_err(|_| changed("write create-new staged artifact"))?;
        }
        self.authenticate_files(build)?;
        platform::inventory_exact_prepared(&mut self.exact, &self.stage, &self.inventory)
            .map_err(|_| changed("authenticate exact staged inventory"))
    }

    fn authenticate_files(&self, build: &OfflinePackageBuild) -> Result<(), PublicationError> {
        let mut scratch = [0_u8; platform::FILE_COMPARE_SCRATCH_BYTES];
        for (name, expected) in files(build) {
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
        for name in INVENTORY {
            platform::recheck_regular_file(
                self.inventory
                    .file(name)
                    .map_err(|_| changed("recover staged artifact before settle"))?,
            )
            .map_err(|_| changed("recheck staged artifact before settle"))?;
        }
        Ok(())
    }

    fn authenticate_published(
        &mut self,
        build: &OfflinePackageBuild,
    ) -> Result<(), PublicationError> {
        debug_assert!(self.publication_attempted && self.published);
        if !platform::same_directory_path(&self.stage, &self.output_path)
            .map_err(|_| published_changed("reopen published output"))?
        {
            return Err(published_changed("published output path identity changed"));
        }
        let module = hold_matching(&self.stage, MODULE_FILE, build.module_wasm.as_slice())?;
        let evidence = hold_matching(&self.stage, EVIDENCE_FILE, build.evidence_json.as_bytes())?;
        let manifest = hold_matching(&self.stage, MANIFEST_FILE, build.manifest_json.as_bytes())?;
        platform::inventory_entries_exact_prepared(
            &mut self.post,
            &self.stage,
            [&module, &evidence, &manifest],
            [],
        )
        .map_err(|_| published_changed("authenticate exact published inventory"))?;
        platform::recheck_directory(&self.parent)
            .map_err(|_| published_changed("recheck published parent"))?;
        platform::recheck_directory(&self.stage)
            .map_err(|_| published_changed("recheck published directory"))
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

fn files(build: &OfflinePackageBuild) -> [(&'static str, &[u8]); 3] {
    [
        (MODULE_FILE, build.module_wasm.as_slice()),
        (EVIDENCE_FILE, build.evidence_json.as_bytes()),
        (MANIFEST_FILE, build.manifest_json.as_bytes()),
    ]
}

fn hold_matching(
    directory: &platform::HeldDirectory,
    name: &str,
    expected: &[u8],
) -> Result<platform::HeldRegularFile, PublicationError> {
    let file = platform::hold_regular_file(directory, OsStr::new(name))
        .map_err(|_| published_changed("hold published artifact"))?;
    let mut scratch = [0_u8; platform::FILE_COMPARE_SCRATCH_BYTES];
    if !platform::compare_exact(&file, expected, &mut scratch)
        .map_err(|_| published_changed("authenticate published artifact bytes"))?
    {
        return Err(published_changed("authenticate published artifact bytes"));
    }
    platform::recheck_regular_file(&file)
        .map_err(|_| published_changed("recheck published artifact identity"))?;
    Ok(file)
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
