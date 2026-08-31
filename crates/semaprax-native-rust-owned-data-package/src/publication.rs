use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax_native_rust_interop_platform as platform;

use super::{HostTarget, PackageError, MAX_ARCHIVE_BYTES, MAX_PROVIDER_BYTES};

static STAGE_NONCE: AtomicU64 = AtomicU64::new(0);

const fn provider_optimization(target: HostTarget) -> u8 {
    match target {
        // Hosted Windows tool startup consumes most of the fixed 30-second
        // process budget. O2 exhausts that budget in compilation while O0's
        // larger COFF output exhausts the following archive path. O1 keeps
        // both bounded phases inside the unchanged limit; backend O0/O2
        // equivalence is established separately before package promotion.
        HostTarget::X86_64WindowsMsvc => 1,
        _ => 2,
    }
}

pub(crate) struct HeldTools {
    clang: platform::HeldTool,
    archiver: platform::HeldTool,
    include: Option<OsString>,
    libraries: Option<OsString>,
}

/// One invocation-local publication authority. The original parent handle is
/// retained from absence preflight through provider staging, final rename and
/// exact post-publication verification, so an ambient same-path substitution
/// can never redirect a later phase.
pub(crate) struct PublicationAuthority {
    parent_path: PathBuf,
    output: PathBuf,
    output_name: std::ffi::OsString,
    parent: platform::HeldDirectory,
}

impl PublicationAuthority {
    pub(crate) fn new(output: &Path) -> Result<Self, PackageError> {
        if !output.is_absolute() {
            return Err(PackageError::publication());
        }
        let parent_path = output
            .parent()
            .ok_or_else(PackageError::publication)?
            .to_path_buf();
        let output_name = output
            .file_name()
            .ok_or_else(PackageError::publication)?
            .to_os_string();
        let parent =
            platform::hold_directory(&parent_path).map_err(|_| PackageError::publication())?;
        let probe =
            platform::prepare_child_name(&output_name).map_err(|_| PackageError::publication())?;
        if !platform::child_absent_prepared(&parent, &probe)
            .map_err(|_| PackageError::publication())?
        {
            return Err(PackageError::publication());
        }
        platform::recheck_directory(&parent).map_err(|_| PackageError::publication())?;
        Ok(Self {
            parent_path,
            output: output.to_path_buf(),
            output_name,
            parent,
        })
    }

    pub(crate) fn recheck(&self) -> Result<(), PackageError> {
        platform::recheck_directory(&self.parent).map_err(|_| PackageError::publication())
    }
}

impl HeldTools {
    pub(crate) fn from_environment() -> Result<Self, PackageError> {
        let clang_path = absolute_environment_path("CLANG")?;
        let archiver_path = absolute_environment_path("SEMAPRAX_ARCHIVER")?;
        #[cfg(windows)]
        let vctools_path = absolute_environment_path("SEMAPRAX_VCTOOLS")?;
        #[cfg(windows)]
        let vctools = Some(vctools_path.as_path());
        #[cfg(not(windows))]
        let vctools: Option<&Path> = None;
        let clang = platform::hold_prepared_tool(clang_path).map_err(|_| PackageError::tool())?;
        let archiver = platform::hold_configured_archiver(archiver_path, vctools)
            .map_err(|_| PackageError::tool())?;
        #[cfg(windows)]
        let (include, libraries) = (
            Some(required_environment("INCLUDE")?),
            Some(required_environment("LIB")?),
        );
        #[cfg(not(windows))]
        let (include, libraries) = (None, None);
        Ok(Self {
            clang,
            archiver,
            include,
            libraries,
        })
    }

    fn process_environment(&self) -> (Option<&OsStr>, Option<&OsStr>) {
        (self.include.as_deref(), self.libraries.as_deref())
    }
}

fn absolute_environment_path(name: &str) -> Result<PathBuf, PackageError> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(PackageError::tool)
}

#[cfg(windows)]
fn required_environment(name: &str) -> Result<OsString, PackageError> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .ok_or_else(PackageError::tool)
}

pub(crate) fn build_archive(
    provider: &[u8],
    target: HostTarget,
    authority: &PublicationAuthority,
    tools: &HeldTools,
) -> Result<Vec<u8>, PackageError> {
    authority.recheck()?;
    let name = format!(
        ".semaprax-owned-data-provider-{}-{}",
        std::process::id(),
        STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let prepared =
        platform::prepare_stage_name(OsStr::new(&name)).map_err(|_| PackageError::publication())?;
    let path = authority.parent_path.join(&name);
    let object_name = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let internal_archive_name = if cfg!(windows) {
        "semaprax_native_rust_sdk.lib"
    } else {
        "libsemaprax_native_rust_sdk.a"
    };
    let inventory = platform::prepare_discard_inventory([
        OsStr::new("provider.c"),
        OsStr::new(object_name),
        OsStr::new(internal_archive_name),
    ])
    .map_err(|_| PackageError::publication())?;
    let directory = platform::create_directory_new_prepared(&authority.parent, &prepared, 0o700)
        .map_err(|_| PackageError::publication())?;
    let mut inventory = inventory;
    let mut archive_settlement_uncertain = false;
    let result = (|| {
        if !platform::same_directory_path(&directory, &path)
            .map_err(|_| PackageError::publication())?
        {
            return Err(PackageError::publication());
        }
        platform::write_file_new_prepared(
            &directory,
            &mut inventory,
            "provider.c",
            provider,
            0o600,
        )
        .map_err(|_| PackageError::publication())?;
        platform::transition_regular_file_to_external_read_prepared(
            &directory,
            &mut inventory,
            "provider.c",
        )
        .map_err(|_| PackageError::publication())?;
        let (include, libraries) = tools.process_environment();
        let compile_plan =
            platform::prepare_process_arena_plan_with_environment(1, include, libraries)
                .map_err(|_| PackageError::publication())?;
        let mut compile_arena =
            platform::materialize_process_arena_with_environment(compile_plan, include, libraries)
                .map_err(|_| PackageError::publication())?;
        let compile = platform::prepare_c_compile_invocation(
            target.triple(),
            OsStr::new("provider.c"),
            provider_optimization(target),
            false,
            MAX_PROVIDER_BYTES,
        )
        .map_err(|_| PackageError::publication())?;
        let object = platform::compile_c_tool_prepared(
            &tools.clang,
            &directory,
            compile,
            &mut compile_arena,
        )
        .map_err(|_| PackageError::publication())?
        .into_bytes();
        platform::write_file_new_prepared(&directory, &mut inventory, object_name, &object, 0o600)
            .map_err(|_| PackageError::publication())?;
        #[cfg(windows)]
        platform::transition_regular_file_to_external_read_prepared(
            &directory,
            &mut inventory,
            object_name,
        )
        .map_err(|_| PackageError::publication())?;
        let archive_plan =
            platform::prepare_process_arena_plan_with_environment(1, include, libraries)
                .map_err(|_| PackageError::publication())?;
        let mut arena =
            platform::materialize_process_arena_with_environment(archive_plan, include, libraries)
                .map_err(|_| PackageError::publication())?;
        let invocation = platform::prepare_archive_invocation(
            OsStr::new(object_name),
            OsStr::new(internal_archive_name),
        )
        .map_err(|_| PackageError::publication())?;
        let archive = platform::archive_tool_prepared(
            &tools.archiver,
            &directory,
            inventory
                .file(object_name)
                .map_err(|_| PackageError::publication())?,
            invocation,
            &mut arena,
        )
        .map_err(|failure| archive_failure(failure, &mut archive_settlement_uncertain))?;
        inventory
            .attach(internal_archive_name, archive)
            .map_err(|_| PackageError::publication())?;
        platform::read_exact(
            inventory
                .file(internal_archive_name)
                .map_err(|_| PackageError::publication())?,
            MAX_ARCHIVE_BYTES,
        )
        .map_err(|_| PackageError::publication())
    })();
    finish_archive_stage(
        result,
        archive_settlement_uncertain,
        || {
            platform::discard_owned_stage_prepared(
                &authority.parent,
                &directory,
                &prepared,
                &inventory,
            )
        },
        || authority.recheck(),
    )
}

fn archive_failure(
    failure: platform::ArchiveToolFailure,
    settlement_uncertain: &mut bool,
) -> PackageError {
    *settlement_uncertain |= matches!(
        failure.settlement,
        platform::ArchiveToolSettlement::Uncertain
    );
    PackageError::publication()
}

fn finish_archive_stage(
    result: Result<Vec<u8>, PackageError>,
    settlement_uncertain: bool,
    cleanup: impl FnOnce() -> Result<(), platform::Error>,
    recheck: impl FnOnce() -> Result<(), PackageError>,
) -> Result<Vec<u8>, PackageError> {
    if settlement_uncertain {
        // No authenticated archive was returned. Do not even attempt exact
        // inventory discard: an uncertain archive effect leaves its stage for
        // caller reconciliation. Dropping held handles grants no later action.
        return Err(result.err().unwrap_or_else(PackageError::publication));
    }
    match (result, cleanup()) {
        (Ok(bytes), Ok(())) => {
            recheck()?;
            Ok(bytes)
        }
        (Err(error), _) => Err(error),
        (Ok(_), Err(_)) => Err(PackageError::publication()),
    }
}

pub(crate) fn publish_package(
    authority: &PublicationAuthority,
    files: [(&'static str, &[u8]); 7],
) -> Result<platform::HeldDirectory, PackageError> {
    publish_package_inner(
        authority,
        files,
        #[cfg(test)]
        |_, _| Ok(()),
    )
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationPoint {
    BeforeSettlement,
    AfterSettlement,
    AfterRename,
}

fn publish_package_inner(
    authority: &PublicationAuthority,
    files: [(&'static str, &[u8]); 7],
    #[cfg(test)] mut observe: impl FnMut(PublicationPoint, &Path) -> Result<(), PackageError>,
) -> Result<platform::HeldDirectory, PackageError> {
    authority.recheck()?;
    let probe = platform::prepare_child_name(&authority.output_name)
        .map_err(|_| PackageError::publication())?;
    if !platform::child_absent_prepared(&authority.parent, &probe)
        .map_err(|_| PackageError::publication())?
    {
        return Err(PackageError::publication());
    }
    let stage_name_text = format!(
        ".semaprax-owned-data-sdk-{}-{}",
        std::process::id(),
        STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let stage_name = platform::prepare_stage_name(OsStr::new(&stage_name_text))
        .map_err(|_| PackageError::publication())?;
    let stage_path = authority.parent_path.join(&stage_name_text);
    let names = files.map(|(name, _)| OsStr::new(name));
    let inventory =
        platform::prepare_discard_inventory(names).map_err(|_| PackageError::publication())?;
    let stage = platform::create_directory_new_prepared(&authority.parent, &stage_name, 0o700)
        .map_err(|_| PackageError::publication())?;
    let mut inventory = inventory;
    let preparation = (|| {
        if !platform::same_directory_path(&stage, &stage_path)
            .map_err(|_| PackageError::publication())?
        {
            return Err(PackageError::publication());
        }
        for (name, bytes) in files {
            platform::write_file_new_prepared(&stage, &mut inventory, name, bytes, 0o600)
                .map_err(|_| PackageError::publication())?;
        }
        let mut scan = platform::prepare_inventory_exact(&inventory)
            .map_err(|_| PackageError::publication())?;
        platform::inventory_exact_prepared(&mut scan, &stage, &inventory)
            .map_err(|_| PackageError::publication())?;
        platform::recheck_directory(&stage).map_err(|_| PackageError::publication())?;
        #[cfg(test)]
        observe(PublicationPoint::BeforeSettlement, &stage_path)?;
        Ok(())
    })();
    if let Err(error) = preparation {
        let _ = platform::discard_owned_stage_prepared(
            &authority.parent,
            &stage,
            &stage_name,
            &inventory,
        );
        return Err(error);
    }

    // This is the consuming publication boundary. From here onward even a
    // failed settlement or rename retains the stage for reconciliation. In
    // particular, a published tree moved back to its old staging name must
    // never regain deletion authority through still-authenticated file facts.
    inventory
        .settle_for_publish()
        .map_err(|_| PackageError::publication())?;
    #[cfg(test)]
    observe(PublicationPoint::AfterSettlement, &stage_path)?;
    let mut publish = platform::prepare_publish_directory(&authority.output_name)
        .map_err(|_| PackageError::publication())?;
    platform::publish_directory_new_prepared(
        &mut publish,
        &authority.parent,
        &stage,
        &stage_name,
        &authority.output_name,
    )
    .map_err(|_| PackageError::publication())?;
    #[cfg(test)]
    observe(PublicationPoint::AfterRename, &stage_path)?;
    if !platform::same_directory_path(&stage, &authority.output)
        .map_err(|_| PackageError::publication())?
    {
        return Err(PackageError::publication());
    }
    authority.recheck()?;
    Ok(stage)
}

pub(crate) fn verify_published(
    authority: &PublicationAuthority,
    directory: &platform::HeldDirectory,
    files: [(&'static str, &[u8]); 7],
) -> Result<(), PackageError> {
    authority.recheck()?;
    if !platform::same_directory_path(directory, &authority.output)
        .map_err(|_| PackageError::publication())?
    {
        return Err(PackageError::publication());
    }
    let held = files
        .iter()
        .map(|(name, bytes)| hold_matching(directory, name, bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let names = files.map(|(name, _)| OsStr::new(name));
    let mut scan = platform::prepare_inventory_entries_exact(names, 7)
        .map_err(|_| PackageError::publication())?;
    platform::inventory_entries_exact_prepared(
        &mut scan,
        directory,
        [
            &held[0], &held[1], &held[2], &held[3], &held[4], &held[5], &held[6],
        ],
        [],
    )
    .map_err(|_| PackageError::publication())?;
    platform::recheck_directory(directory).map_err(|_| PackageError::publication())?;
    authority.recheck()
}

fn hold_matching(
    directory: &platform::HeldDirectory,
    name: &str,
    expected: &[u8],
) -> Result<platform::HeldRegularFile, PackageError> {
    let file = platform::hold_regular_file(directory, OsStr::new(name))
        .map_err(|_| PackageError::publication())?;
    let bytes =
        platform::read_exact(&file, MAX_ARCHIVE_BYTES).map_err(|_| PackageError::publication())?;
    if bytes != expected {
        return Err(PackageError::publication());
    }
    platform::recheck_regular_file(&file).map_err(|_| PackageError::publication())?;
    Ok(file)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod publish_tests;
