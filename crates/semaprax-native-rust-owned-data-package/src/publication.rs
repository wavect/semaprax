use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax_native_rust_interop_platform as platform;

use super::{HostTarget, PackageError, MAX_ARCHIVE_BYTES, MAX_PROVIDER_BYTES};

static STAGE_NONCE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct HeldTools {
    clang: platform::HeldTool,
    archiver: platform::HeldTool,
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
        Ok(Self { clang, archiver })
    }
}

fn absolute_environment_path(name: &str) -> Result<PathBuf, PackageError> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(PackageError::tool)
}

pub(crate) fn preflight_output(output: &Path) -> Result<(), PackageError> {
    if !output.is_absolute() {
        return Err(PackageError::publication());
    }
    let parent_path = output.parent().ok_or_else(PackageError::publication)?;
    let output_name = output.file_name().ok_or_else(PackageError::publication)?;
    let parent = platform::hold_directory(parent_path).map_err(|_| PackageError::publication())?;
    let probe =
        platform::prepare_child_name(output_name).map_err(|_| PackageError::publication())?;
    if !platform::child_absent_prepared(&parent, &probe).map_err(|_| PackageError::publication())? {
        return Err(PackageError::publication());
    }
    platform::recheck_directory(&parent).map_err(|_| PackageError::publication())
}

pub(crate) fn build_archive(
    provider: &[u8],
    target: HostTarget,
    output: &Path,
    tools: &HeldTools,
) -> Result<Vec<u8>, PackageError> {
    if !output.is_absolute() {
        return Err(PackageError::publication());
    }
    let root = output.parent().ok_or_else(PackageError::publication)?;
    let parent = platform::hold_directory(root).map_err(|_| PackageError::publication())?;
    let name = format!(
        ".semaprax-owned-data-provider-{}-{}",
        std::process::id(),
        STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let prepared =
        platform::prepare_stage_name(OsStr::new(&name)).map_err(|_| PackageError::publication())?;
    let path = root.join(&name);
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
    let directory = platform::create_directory_new_prepared(&parent, &prepared, 0o700)
        .map_err(|_| PackageError::publication())?;
    let mut inventory = inventory;
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
        let mut compile_arena = platform::materialize_process_arena(
            platform::prepare_process_arena_plan(1).map_err(|_| PackageError::publication())?,
        )
        .map_err(|_| PackageError::publication())?;
        let compile = platform::prepare_c_compile_invocation(
            target.triple(),
            OsStr::new("provider.c"),
            2,
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
        let mut arena = platform::materialize_process_arena(
            platform::prepare_process_arena_plan(1).map_err(|_| PackageError::publication())?,
        )
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
        .map_err(|_| PackageError::publication())?;
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
    let cleanup =
        platform::discard_owned_stage_prepared(&parent, &directory, &prepared, &inventory);
    match (result, cleanup) {
        (Ok(bytes), Ok(())) => Ok(bytes),
        (Err(error), _) => Err(error),
        (Ok(_), Err(_)) => Err(PackageError::publication()),
    }
}

pub(crate) fn publish_package(
    output: &Path,
    files: [(&str, &[u8]); 7],
) -> Result<(), PackageError> {
    if !output.is_absolute() {
        return Err(PackageError::publication());
    }
    let parent_path = output.parent().ok_or_else(PackageError::publication)?;
    let output_name = output.file_name().ok_or_else(PackageError::publication)?;
    let parent = platform::hold_directory(parent_path).map_err(|_| PackageError::publication())?;
    let probe =
        platform::prepare_child_name(output_name).map_err(|_| PackageError::publication())?;
    if !platform::child_absent_prepared(&parent, &probe).map_err(|_| PackageError::publication())? {
        return Err(PackageError::publication());
    }
    let stage_name_text = format!(
        ".semaprax-owned-data-sdk-{}-{}",
        std::process::id(),
        STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let stage_name = platform::prepare_stage_name(OsStr::new(&stage_name_text))
        .map_err(|_| PackageError::publication())?;
    let stage_path = parent_path.join(&stage_name_text);
    let names = files.map(|(name, _)| OsStr::new(name));
    let inventory =
        platform::prepare_discard_inventory(names).map_err(|_| PackageError::publication())?;
    let stage = platform::create_directory_new_prepared(&parent, &stage_name, 0o700)
        .map_err(|_| PackageError::publication())?;
    let mut inventory = inventory;
    let result = (|| {
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
        inventory
            .settle_for_publish()
            .map_err(|_| PackageError::publication())?;
        let mut publish = platform::prepare_publish_directory(output_name)
            .map_err(|_| PackageError::publication())?;
        platform::publish_directory_new_prepared(
            &mut publish,
            &parent,
            &stage,
            &stage_name,
            output_name,
        )
        .map_err(|_| PackageError::publication())
    })();
    if result.is_err() {
        let _ = platform::discard_owned_stage_prepared(&parent, &stage, &stage_name, &inventory);
    }
    result
}

pub(crate) fn verify_published(
    output: &Path,
    files: [(&str, &[u8]); 7],
) -> Result<(), PackageError> {
    let directory = platform::hold_directory(output).map_err(|_| PackageError::publication())?;
    let held = files
        .iter()
        .map(|(name, bytes)| hold_matching(&directory, name, bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let names = files.map(|(name, _)| OsStr::new(name));
    let mut scan = platform::prepare_inventory_entries_exact(names, 7)
        .map_err(|_| PackageError::publication())?;
    platform::inventory_entries_exact_prepared(
        &mut scan,
        &directory,
        [
            &held[0], &held[1], &held[2], &held[3], &held[4], &held[5], &held[6],
        ],
        [],
    )
    .map_err(|_| PackageError::publication())?;
    platform::recheck_directory(&directory).map_err(|_| PackageError::publication())
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
