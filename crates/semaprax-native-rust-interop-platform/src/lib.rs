//! Safe held-handle facade for private Native Rust Interop bundle builds.

#![forbid(unsafe_code)]

mod doctor;
pub use doctor::{doctor_version_probe, DoctorProbeError};
mod host_target;
#[doc(hidden)]
pub use host_target::{current_native_host_target, NativeHostTarget};

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub use semaprax_native_rust_interop_platform_sys::{CreateDirectoryNewFailure, Error};
pub const SDK_ARCHIVE_MAX_BYTES: u64 =
    semaprax_native_rust_interop_platform_sys::SDK_ARCHIVE_MAX_BYTES;

pub struct HeldDirectory(semaprax_native_rust_interop_platform_sys::Directory);
pub struct HeldRegularFile(semaprax_native_rust_interop_platform_sys::RegularFile);
pub struct HeldExecutable(semaprax_native_rust_interop_platform_sys::Executable);
pub struct HeldTool {
    executable: HeldExecutable,
    path: String,
}
pub struct HeldRustcDiscovery(semaprax_native_rust_interop_platform_sys::RustcDiscovery);
pub struct HeldDirectRustc(semaprax_native_rust_interop_platform_sys::DirectRustc);

pub struct PreparedStageName(semaprax_native_rust_interop_platform_sys::PreparedRelativeNameArena);
pub struct PreparedChildName(semaprax_native_rust_interop_platform_sys::PreparedRelativeName);
pub struct PreparedVersionInvocation(
    semaprax_native_rust_interop_platform_sys::PreparedVersionInvocation,
);
pub struct PreparedSysrootInvocation(
    semaprax_native_rust_interop_platform_sys::PreparedSysrootInvocation,
);
pub struct PreparedRustcVersionInvocation(
    semaprax_native_rust_interop_platform_sys::PreparedRustcVersionInvocation,
);
pub struct PreparedProcessArenaPlan(
    semaprax_native_rust_interop_platform_sys::PreparedProcessArenaPlan,
);
pub struct PreparedProcessArena(semaprax_native_rust_interop_platform_sys::PreparedProcessArena);
pub struct PreparedToolResolver(semaprax_native_rust_interop_platform_sys::PreparedToolResolver);
pub struct PreparedCCompileInvocation(
    semaprax_native_rust_interop_platform_sys::PreparedCCompileInvocation,
);
pub struct PreparedRustCompileInvocation(
    semaprax_native_rust_interop_platform_sys::PreparedRustCompileInvocation,
);
pub struct PreparedLinkInvocation(
    semaprax_native_rust_interop_platform_sys::PreparedLinkInvocation,
);
pub struct PreparedArchiveInvocation(
    semaprax_native_rust_interop_platform_sys::PreparedArchiveInvocation,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveToolFailurePhase {
    Platform,
    Preflight,
    ScratchCreation,
    Process,
    ScratchCleanup,
    ArchiverRecheck,
    WorkingDirectoryRecheck,
    InputRecheck,
    ProcessOutput,
    OutputHold,
    ExactArchive,
    LaunchPathRecheck,
    OutputRecheck,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveToolSettlement {
    Settled,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveToolFailure {
    pub error: Error,
    pub phase: ArchiveToolFailurePhase,
    pub settlement: ArchiveToolSettlement,
}
pub struct PreparedRunInvocation(semaprax_native_rust_interop_platform_sys::PreparedRunInvocation);
pub struct PreparedLinkOrCopy {
    native: semaprax_native_rust_interop_platform_sys::PreparedLinkOrCopy,
    source_name: &'static str,
    destination_name: &'static str,
    source_index: usize,
    destination_index: usize,
}
pub struct PreparedInventoryExact<const N: usize>(
    semaprax_native_rust_interop_platform_sys::PreparedInventoryExact<N>,
);
pub struct PreparedInventoryEntriesExact<const N: usize>(
    semaprax_native_rust_interop_platform_sys::PreparedInventoryEntriesExact<N>,
);
pub struct PreparedPublishDirectory(
    semaprax_native_rust_interop_platform_sys::PreparedPublishDirectory,
);

pub struct PreparedDiscardInventory<const N: usize> {
    names: [&'static OsStr; N],
    native: semaprax_native_rust_interop_platform_sys::PreparedDiscardNames<N>,
    files: [Option<HeldRegularFile>; N],
    settled: [Option<semaprax_native_rust_interop_platform_sys::SettledRegularFile>; N],
    attached: usize,
    #[cfg(debug_assertions)]
    failure_after_delete: Option<usize>,
}

pub fn prepare_stage_name(name: &OsStr) -> Result<PreparedStageName, Error> {
    let maximum = name.as_encoded_bytes().len();
    let mut arena = prepare_stage_name_arena(maximum)?;
    arena.set(name)?;
    Ok(arena)
}

pub fn prepare_child_name(name: &OsStr) -> Result<PreparedChildName, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_relative_name(name).map(PreparedChildName)
}

pub fn child_absent_prepared(
    directory: &HeldDirectory,
    name: &PreparedChildName,
) -> Result<bool, Error> {
    semaprax_native_rust_interop_platform_sys::child_absent_prepared(&directory.0, &name.0)
}

pub fn same_child_directory_prepared(
    parent: &HeldDirectory,
    name: &PreparedChildName,
    child: &HeldDirectory,
) -> Result<bool, Error> {
    semaprax_native_rust_interop_platform_sys::same_child_directory_prepared(
        &parent.0, &name.0, &child.0,
    )
}

pub fn prepare_stage_name_arena(maximum: usize) -> Result<PreparedStageName, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_relative_name_arena(maximum)
        .map(PreparedStageName)
}

impl PreparedStageName {
    pub fn set(&mut self, name: &OsStr) -> Result<(), Error> {
        semaprax_native_rust_interop_platform_sys::set_relative_name_arena(&mut self.0, name)
    }

    pub fn capacity(&self) -> usize {
        semaprax_native_rust_interop_platform_sys::relative_name_arena_capacity(&self.0)
    }
}

pub fn prepare_discard_inventory<const N: usize>(
    names: [&'static OsStr; N],
) -> Result<PreparedDiscardInventory<N>, Error> {
    let native = semaprax_native_rust_interop_platform_sys::prepare_discard_names(names)?;
    Ok(PreparedDiscardInventory {
        names,
        native,
        files: [const { None }; N],
        settled: [const { None }; N],
        attached: 0,
        #[cfg(debug_assertions)]
        failure_after_delete: None,
    })
}

pub fn prepare_discard_inventory_bounded<const N: usize>(
    names: [&'static OsStr; N],
    maximum_native_bytes: usize,
) -> Result<PreparedDiscardInventory<N>, Error> {
    let inventory = prepare_discard_inventory(names)?;
    if prepared_discard_inventory_owned_capacity(&inventory) > maximum_native_bytes {
        return Err(Error::OutputLimit);
    }
    Ok(inventory)
}

pub fn prepared_discard_inventory_owned_capacity<const N: usize>(
    inventory: &PreparedDiscardInventory<N>,
) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_discard_names_owned_capacity(
        &inventory.native,
    )
}

impl<const N: usize> PreparedDiscardInventory<N> {
    fn planned_slot(&self, name: &str) -> Result<usize, Error> {
        self.names
            .iter()
            .position(|candidate| *candidate == OsStr::new(name))
            .ok_or(Error::Invalid)
    }

    pub fn validate_next(&self, name: &str) -> Result<usize, Error> {
        let index = self.attached;
        if index >= N
            || self.names[index] != OsStr::new(name)
            || self.files[index].is_some()
            || self.settled[index].is_some()
        {
            return Err(Error::Invalid);
        }
        Ok(index)
    }

    pub fn validate_slot(&self, name: &str) -> Result<usize, Error> {
        self.names[..self.attached]
            .iter()
            .position(|candidate| *candidate == OsStr::new(name))
            .filter(|index| self.files[*index].is_some())
            .ok_or(Error::Invalid)
    }

    pub fn attach(&mut self, name: &str, file: HeldRegularFile) -> Result<(), Error> {
        let index = self.validate_next(name)?;
        self.files[index] = Some(file);
        self.attached += 1;
        Ok(())
    }

    pub fn file(&self, name: &str) -> Result<&HeldRegularFile, Error> {
        self.names[..self.attached]
            .iter()
            .position(|candidate| *candidate == OsStr::new(name))
            .and_then(|index| self.files[index].as_ref())
            .ok_or(Error::Changed)
    }

    pub fn recheck(&self, names: &[&str]) -> Result<(), Error> {
        for name in names {
            recheck_regular_file(self.file(name)?)?;
        }
        Ok(())
    }

    pub fn attached(&self) -> usize {
        self.attached
    }

    pub fn settle_for_publish(&mut self) -> Result<(), Error> {
        if self.attached != N
            || self.files.iter().any(Option::is_none)
            || self.settled.iter().any(Option::is_some)
        {
            return Err(Error::Invalid);
        }
        for file in self.files.iter().flatten() {
            recheck_regular_file(file)?;
        }
        for index in 0..N {
            let file = self.files[index].take().expect("validated attached file");
            self.settled[index] = Some(
                semaprax_native_rust_interop_platform_sys::settle_regular_file_for_publish(file.0),
            );
        }
        Ok(())
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn inject_discard_failure_after_delete(&mut self, deleted: Option<usize>) {
        self.failure_after_delete = deleted;
    }
}

pub fn prepare_link_or_copy<const S: usize, const D: usize>(
    source: &PreparedDiscardInventory<S>,
    source_name: &'static str,
    destination: &PreparedDiscardInventory<D>,
    destination_name: &'static str,
) -> Result<PreparedLinkOrCopy, Error> {
    let source_index = source.planned_slot(source_name)?;
    let destination_index = destination.planned_slot(destination_name)?;
    let native = semaprax_native_rust_interop_platform_sys::prepare_link_or_copy(
        &destination.native,
        destination_index,
    )?;
    Ok(PreparedLinkOrCopy {
        native,
        source_name,
        destination_name,
        source_index,
        destination_index,
    })
}

pub fn link_or_copy_required_capacity<const S: usize, const D: usize>(
    source: &PreparedDiscardInventory<S>,
    source_name: &str,
    destination: &PreparedDiscardInventory<D>,
    destination_name: &str,
) -> Result<usize, Error> {
    let _ = source.planned_slot(source_name)?;
    let destination_index = destination.planned_slot(destination_name)?;
    semaprax_native_rust_interop_platform_sys::link_or_copy_required_capacity(
        &destination.native,
        destination_index,
    )
}

pub fn prepared_link_or_copy_owned_capacity(prepared: &PreparedLinkOrCopy) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_link_or_copy_owned_capacity(
        &prepared.native,
    )
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn inject_link_or_copy_failure_before_authentication(prepared: &mut PreparedLinkOrCopy) {
    semaprax_native_rust_interop_platform_sys::inject_link_or_copy_failure_before_authentication(
        &mut prepared.native,
    );
}

pub fn link_or_copy_new_prepared<const S: usize, const D: usize>(
    prepared: PreparedLinkOrCopy,
    source: &PreparedDiscardInventory<S>,
    destination_directory: &HeldDirectory,
    destination: &mut PreparedDiscardInventory<D>,
    source_bytes: &[u8],
) -> Result<(), Error> {
    let source_index = source.validate_slot(prepared.source_name)?;
    let destination_index = destination.validate_next(prepared.destination_name)?;
    if source_index != prepared.source_index || destination_index != prepared.destination_index {
        return Err(Error::Invalid);
    }
    let source_file = source.files[source_index].as_ref().ok_or(Error::Invalid)?;
    let destination_file = semaprax_native_rust_interop_platform_sys::link_or_copy_new_prepared(
        prepared.native,
        &source_file.0,
        &destination_directory.0,
        &destination.native,
        destination_index,
        source_bytes,
    )?;
    destination.files[destination_index] = Some(HeldRegularFile(destination_file));
    destination.attached += 1;
    Ok(())
}

pub fn inventory_exact_required_capacity<const N: usize>(
    inventory: &PreparedDiscardInventory<N>,
) -> Result<usize, Error> {
    semaprax_native_rust_interop_platform_sys::inventory_exact_required_capacity(&inventory.native)
}

pub fn prepare_inventory_exact<const N: usize>(
    inventory: &PreparedDiscardInventory<N>,
) -> Result<PreparedInventoryExact<N>, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_inventory_exact(&inventory.native)
        .map(PreparedInventoryExact)
}

pub fn prepared_inventory_exact_owned_capacity<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_inventory_exact_owned_capacity(&prepared.0)
}

pub fn prepared_inventory_exact_remaining<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> u8 {
    semaprax_native_rust_interop_platform_sys::prepared_inventory_exact_remaining(&prepared.0)
}

pub fn inventory_exact_prepared<const N: usize>(
    prepared: &mut PreparedInventoryExact<N>,
    directory: &HeldDirectory,
    inventory: &PreparedDiscardInventory<N>,
) -> Result<(), Error> {
    if inventory.attached != N || inventory.files.iter().any(Option::is_none) {
        return Err(Error::Invalid);
    }
    let files = std::array::from_fn(|index| inventory.files[index].as_ref().map(|file| &file.0));
    semaprax_native_rust_interop_platform_sys::inventory_exact_prepared(
        &mut prepared.0,
        &directory.0,
        &inventory.native,
        files,
    )
}

/// Prepares a one-use exact mixed inventory. Names are ordered with all
/// regular files first and all child directories second.
pub fn prepare_inventory_entries_exact<const N: usize>(
    names: [&OsStr; N],
    file_count: usize,
) -> Result<PreparedInventoryEntriesExact<N>, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_inventory_entries_exact(names, file_count)
        .map(PreparedInventoryEntriesExact)
}

pub fn prepared_inventory_entries_exact_owned_capacity<const N: usize>(
    prepared: &PreparedInventoryEntriesExact<N>,
) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_inventory_entries_exact_owned_capacity(
        &prepared.0,
    )
}

pub fn inventory_entries_exact_prepared<const N: usize, const F: usize, const D: usize>(
    prepared: &mut PreparedInventoryEntriesExact<N>,
    directory: &HeldDirectory,
    files: [&HeldRegularFile; F],
    directories: [&HeldDirectory; D],
) -> Result<(), Error> {
    let files = files.map(|file| &file.0);
    let directories = directories.map(|child| &child.0);
    semaprax_native_rust_interop_platform_sys::inventory_entries_exact_prepared(
        &mut prepared.0,
        &directory.0,
        files,
        directories,
    )
}

pub fn write_file_new_prepared<const N: usize>(
    directory: &HeldDirectory,
    inventory: &mut PreparedDiscardInventory<N>,
    name: &str,
    bytes: &[u8],
    mode: u32,
) -> Result<(), Error> {
    let index = inventory.validate_next(name)?;
    let file = semaprax_native_rust_interop_platform_sys::write_file_new_prepared(
        &directory.0,
        &inventory.native,
        index,
        bytes,
        mode,
    )?;
    inventory.files[index] = Some(HeldRegularFile(file));
    inventory.attached += 1;
    Ok(())
}

pub fn hold_regular_file_prepared<const N: usize>(
    directory: &HeldDirectory,
    inventory: &PreparedDiscardInventory<N>,
    name: &str,
) -> Result<HeldRegularFile, Error> {
    let index = inventory.validate_slot(name)?;
    let tracked = inventory.files[index].as_ref().ok_or(Error::Invalid)?;
    semaprax_native_rust_interop_platform_sys::hold_regular_file_prepared(
        &directory.0,
        &inventory.native,
        index,
        &tracked.0,
    )
    .map(HeldRegularFile)
}

pub fn transition_regular_file_to_external_read_prepared<const N: usize>(
    directory: &HeldDirectory,
    inventory: &mut PreparedDiscardInventory<N>,
    name: &str,
) -> Result<(), Error> {
    let index = inventory.validate_slot(name)?;
    let tracked = inventory.files[index].as_ref().ok_or(Error::Invalid)?;
    let rebound = semaprax_native_rust_interop_platform_sys::transition_regular_file_to_external_read_prepared(
        &directory.0,
        &inventory.native,
        index,
        &tracked.0,
    )?;
    inventory.files[index] = Some(HeldRegularFile(rebound));
    Ok(())
}

pub struct ToolOutput {
    bytes: Vec<u8>,
}

impl ToolOutput {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn capacity(&self) -> usize {
        self.bytes.capacity()
    }
}

pub fn prepare_version_invocation(
    argument: &str,
    maximum: usize,
) -> Result<PreparedVersionInvocation, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_version_invocation(argument, maximum)
        .map(PreparedVersionInvocation)
}

pub fn prepared_version_owned_capacity(prepared: &PreparedVersionInvocation) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_version_owned_capacity(&prepared.0)
}

pub fn prepare_sysroot_invocation(maximum: usize) -> Result<PreparedSysrootInvocation, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_sysroot_invocation(maximum)
        .map(PreparedSysrootInvocation)
}

pub fn prepare_rustc_version_invocation(
    maximum: usize,
) -> Result<PreparedRustcVersionInvocation, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_rustc_version_invocation(maximum)
        .map(PreparedRustcVersionInvocation)
}

pub fn prepared_sysroot_owned_capacity(prepared: &PreparedSysrootInvocation) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_sysroot_owned_capacity(&prepared.0)
}

pub fn prepared_rustc_version_owned_capacity(prepared: &PreparedRustcVersionInvocation) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_rustc_version_owned_capacity(&prepared.0)
}

pub fn prepare_process_arena_plan(uses: usize) -> Result<PreparedProcessArenaPlan, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_process_arena_plan(uses)
        .map(PreparedProcessArenaPlan)
}

pub fn prepare_process_arena_plan_with_environment(
    uses: usize,
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<PreparedProcessArenaPlan, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_process_arena_plan_with_environment(
        uses, include, libraries,
    )
    .map(PreparedProcessArenaPlan)
}

pub fn prepared_process_arena_plan_capacity(plan: &PreparedProcessArenaPlan) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_process_arena_plan_capacity(&plan.0)
}

pub fn materialize_process_arena(
    plan: PreparedProcessArenaPlan,
) -> Result<PreparedProcessArena, Error> {
    semaprax_native_rust_interop_platform_sys::materialize_process_arena(plan.0)
        .map(PreparedProcessArena)
}

pub fn materialize_process_arena_with_environment(
    plan: PreparedProcessArenaPlan,
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<PreparedProcessArena, Error> {
    semaprax_native_rust_interop_platform_sys::materialize_process_arena_with_environment(
        plan.0, include, libraries,
    )
    .map(PreparedProcessArena)
}

pub fn prepared_process_arena_owned_capacity(prepared: &PreparedProcessArena) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_process_arena_owned_capacity(&prepared.0)
}

pub fn prepared_process_arena_remaining(prepared: &PreparedProcessArena) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_process_arena_remaining(&prepared.0)
}

pub fn prepare_tool_resolver(
    fallback: &str,
    maximum: usize,
) -> Result<PreparedToolResolver, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_tool_resolver(fallback, maximum)
        .map(PreparedToolResolver)
}

pub fn prepared_tool_resolver_owned_capacity(prepared: &PreparedToolResolver) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_tool_resolver_owned_capacity(&prepared.0)
}

pub fn hold_directory(path: &Path) -> Result<HeldDirectory, Error> {
    semaprax_native_rust_interop_platform_sys::hold_directory(path).map(HeldDirectory)
}

pub fn hold_child_directory(parent: &HeldDirectory, name: &OsStr) -> Result<HeldDirectory, Error> {
    semaprax_native_rust_interop_platform_sys::hold_child_directory(&parent.0, name)
        .map(HeldDirectory)
}

pub fn recheck_directory(directory: &HeldDirectory) -> Result<(), Error> {
    semaprax_native_rust_interop_platform_sys::recheck_directory(&directory.0)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn directory_is_current_user_private(directory: &HeldDirectory) -> Result<bool, Error> {
    semaprax_native_rust_interop_platform_sys::directory_is_current_user_private(&directory.0)
}

pub fn same_directory_path(directory: &HeldDirectory, path: &Path) -> Result<bool, Error> {
    semaprax_native_rust_interop_platform_sys::same_directory_path(&directory.0, path)
}

pub fn create_directory_new(
    parent: &HeldDirectory,
    name: &OsStr,
    mode: u32,
) -> Result<HeldDirectory, Error> {
    semaprax_native_rust_interop_platform_sys::create_directory_new(&parent.0, name, mode)
        .map(HeldDirectory)
}

pub fn create_directory_new_prepared(
    parent: &HeldDirectory,
    name: &PreparedStageName,
    mode: u32,
) -> Result<HeldDirectory, Error> {
    semaprax_native_rust_interop_platform_sys::create_directory_new_prepared(
        &parent.0, &name.0, mode,
    )
    .map(HeldDirectory)
}

pub fn create_directory_new_prepared_settled(
    parent: &HeldDirectory,
    name: &PreparedStageName,
    mode: u32,
) -> Result<HeldDirectory, CreateDirectoryNewFailure> {
    semaprax_native_rust_interop_platform_sys::create_directory_new_prepared_settled(
        &parent.0, &name.0, mode,
    )
    .map(HeldDirectory)
}

pub fn write_file_new(
    directory: &HeldDirectory,
    name: &OsStr,
    bytes: &[u8],
    mode: u32,
) -> Result<HeldRegularFile, Error> {
    semaprax_native_rust_interop_platform_sys::write_file_new(&directory.0, name, bytes, mode)
        .map(HeldRegularFile)
}

pub fn hold_regular_file(
    directory: &HeldDirectory,
    name: &OsStr,
) -> Result<HeldRegularFile, Error> {
    semaprax_native_rust_interop_platform_sys::hold_regular_file(&directory.0, name)
        .map(HeldRegularFile)
}

pub fn recheck_regular_file(file: &HeldRegularFile) -> Result<(), Error> {
    semaprax_native_rust_interop_platform_sys::recheck_regular(&file.0)
}

pub fn hold_executable(directory: &HeldDirectory, name: &OsStr) -> Result<HeldExecutable, Error> {
    semaprax_native_rust_interop_platform_sys::hold_executable(&directory.0, name)
        .map(HeldExecutable)
}

pub fn executable_regular_file(executable: &HeldExecutable) -> Result<HeldRegularFile, Error> {
    semaprax_native_rust_interop_platform_sys::executable_regular_file(&executable.0)
        .map(HeldRegularFile)
}

pub fn hold_external_executable(path: &Path) -> Result<HeldExecutable, Error> {
    semaprax_native_rust_interop_platform_sys::hold_external_executable(path).map(HeldExecutable)
}

pub fn read_exact(file: &HeldRegularFile, maximum: usize) -> Result<Vec<u8>, Error> {
    semaprax_native_rust_interop_platform_sys::read_exact(&file.0, maximum)
}

pub const FILE_COMPARE_SCRATCH_BYTES: usize = 8192;

pub fn compare_exact(
    file: &HeldRegularFile,
    expected: &[u8],
    scratch: &mut [u8; FILE_COMPARE_SCRATCH_BYTES],
) -> Result<bool, Error> {
    semaprax_native_rust_interop_platform_sys::compare_exact(&file.0, expected, scratch)
}

pub fn rustc_version(
    executable: &HeldExecutable,
    cwd: &HeldDirectory,
) -> Result<ToolOutput, Error> {
    rustc_version_bounded(executable, cwd, 65_536)
}

pub fn rustc_version_bounded(
    executable: &HeldExecutable,
    cwd: &HeldDirectory,
    maximum: usize,
) -> Result<ToolOutput, Error> {
    semaprax_native_rust_interop_platform_sys::rustc_version(&executable.0, &cwd.0, maximum)
        .map(|bytes| ToolOutput { bytes })
}

pub fn clang_version(
    executable: &HeldExecutable,
    cwd: &HeldDirectory,
) -> Result<ToolOutput, Error> {
    clang_version_bounded(executable, cwd, 65_536)
}

pub fn clang_version_bounded(
    executable: &HeldExecutable,
    cwd: &HeldDirectory,
    maximum: usize,
) -> Result<ToolOutput, Error> {
    semaprax_native_rust_interop_platform_sys::clang_version(&executable.0, &cwd.0, maximum)
        .map(|bytes| ToolOutput { bytes })
}

pub fn hold_configured_tool(variable: &str, fallback: &str) -> Result<HeldTool, Error> {
    if variable.is_empty() || fallback.is_empty() {
        return Err(Error::Invalid);
    }
    let path = if let Some(value) = std::env::var_os(variable) {
        PathBuf::from(value)
    } else {
        let paths = std::env::var_os("PATH").ok_or(Error::Invalid)?;
        std::env::split_paths(&paths)
            .map(|directory| directory.join(fallback))
            .find(|candidate| candidate.is_file())
            .ok_or(Error::Invalid)?
    };
    let path = path.canonicalize().map_err(|_| Error::Invalid)?;
    let executable = hold_external_executable(&path)?;
    let path = path.to_str().ok_or(Error::Invalid)?.to_owned();
    Ok(HeldTool { executable, path })
}

pub fn hold_prepared_tool(path: PathBuf) -> Result<HeldTool, Error> {
    let executable = hold_external_executable(&path)?;
    let path = path.to_str().ok_or(Error::Invalid)?.to_owned();
    Ok(HeldTool { executable, path })
}

/// Holds the one configured archiver admitted by the current-host SDK profile.
///
/// The child is still launched by held executable authority with no ambient
/// environment. Darwin receives only a fixed `TMPDIR` naming an exact
/// caller-stage scratch directory whose inventory is authenticated and
/// discarded before this operation returns. This function deliberately does
/// not provide tool discovery or a generic process surface.
pub fn hold_configured_archiver(
    configured: PathBuf,
    vctools: Option<&Path>,
) -> Result<HeldTool, Error> {
    if !configured.is_absolute() {
        return Err(Error::Invalid);
    }
    #[cfg(target_os = "linux")]
    {
        // Linux distributions commonly expose `ar` through a symlink to a
        // version-suffixed real image. Authenticate the explicit held image,
        // not a cosmetic basename; the frozen rcsD invocation and exact
        // archive replay are the admitted behavior.
        if vctools.is_some() {
            return Err(Error::Invalid);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if vctools.is_some() || configured != Path::new("/usr/bin/libtool") {
            return Err(Error::Invalid);
        }
    }
    #[cfg(all(target_os = "windows", not(target_arch = "x86_64")))]
    {
        let _ = vctools;
        return Err(Error::Unsupported);
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        let vctools = vctools
            .filter(|root| root.is_absolute())
            .ok_or(Error::Invalid)?;
        if configured.strip_prefix(vctools).ok() != Some(Path::new(r"bin\Hostx64\x64\lib.exe")) {
            return Err(Error::Invalid);
        }
    }
    #[cfg(not(all(target_os = "windows", not(target_arch = "x86_64"))))]
    {
        hold_prepared_tool(configured)
    }
}

pub fn resolve_and_hold_tool_prepared(
    prepared: PreparedToolResolver,
    configured: Option<&OsStr>,
    paths: Option<&OsStr>,
) -> Result<HeldTool, Error> {
    let (executable, path) =
        semaprax_native_rust_interop_platform_sys::resolve_and_hold_tool_prepared(
            prepared.0, configured, paths,
        )?;
    Ok(HeldTool {
        executable: HeldExecutable(executable),
        path,
    })
}

pub fn resolve_and_hold_tool_reusing_prepared(
    prepared: PreparedToolResolver,
    configured: Option<&OsStr>,
    paths: Option<&OsStr>,
) -> Result<(HeldTool, PreparedToolResolver), Error> {
    let (executable, path, prepared) =
        semaprax_native_rust_interop_platform_sys::resolve_and_hold_tool_reusing_prepared(
            prepared.0, configured, paths,
        )?;
    Ok((
        HeldTool {
            executable: HeldExecutable(executable),
            path,
        },
        PreparedToolResolver(prepared),
    ))
}

pub fn hold_rustc_discovery_prepared(
    prepared: PreparedToolResolver,
    configured: &OsStr,
) -> Result<HeldRustcDiscovery, Error> {
    semaprax_native_rust_interop_platform_sys::hold_rustc_discovery_prepared(prepared.0, configured)
        .map(HeldRustcDiscovery)
}

pub fn rustc_discovery_output_prepared(
    discovery: &HeldRustcDiscovery,
    cwd: &HeldDirectory,
    prepared: PreparedSysrootInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<ToolOutput, Error> {
    semaprax_native_rust_interop_platform_sys::rustc_discovery_output_prepared(
        &discovery.0,
        &cwd.0,
        prepared.0,
        &mut process_arena.0,
    )
    .map(|bytes| ToolOutput { bytes })
}

pub fn hold_direct_rustc_prepared(
    discovery: HeldRustcDiscovery,
    sysroot_output: &[u8],
) -> Result<HeldDirectRustc, Error> {
    semaprax_native_rust_interop_platform_sys::hold_direct_rustc_prepared(
        discovery.0,
        sysroot_output,
    )
    .map(HeldDirectRustc)
}

pub fn direct_rustc_output_prepared(
    direct: &HeldDirectRustc,
    cwd: &HeldDirectory,
    prepared: PreparedSysrootInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<ToolOutput, Error> {
    semaprax_native_rust_interop_platform_sys::direct_rustc_output_prepared(
        &direct.0,
        &cwd.0,
        prepared.0,
        &mut process_arena.0,
    )
    .map(|bytes| ToolOutput { bytes })
}

pub fn direct_rustc_version_prepared(
    direct: &HeldDirectRustc,
    cwd: &HeldDirectory,
    prepared: PreparedRustcVersionInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<ToolOutput, Error> {
    semaprax_native_rust_interop_platform_sys::direct_rustc_version_prepared(
        &direct.0,
        &cwd.0,
        prepared.0,
        &mut process_arena.0,
    )
    .map(|bytes| ToolOutput { bytes })
}

pub fn direct_rustc_reproduces_sysroot(
    direct: &mut HeldDirectRustc,
    sysroot_output: &[u8],
) -> Result<(), Error> {
    semaprax_native_rust_interop_platform_sys::direct_rustc_reproduces_sysroot(
        &mut direct.0,
        sysroot_output,
    )
}

pub fn tool_path(tool: &HeldTool) -> &str {
    &tool.path
}

pub fn tool_path_capacity(tool: &HeldTool) -> usize {
    tool.path.capacity()
}

pub fn rustc_tool_version_bounded(
    tool: &HeldTool,
    cwd: &HeldDirectory,
    maximum: usize,
) -> Result<ToolOutput, Error> {
    rustc_version_bounded(&tool.executable, cwd, maximum)
}

pub fn clang_tool_version_bounded(
    tool: &HeldTool,
    cwd: &HeldDirectory,
    maximum: usize,
) -> Result<ToolOutput, Error> {
    clang_version_bounded(&tool.executable, cwd, maximum)
}

pub fn tool_version_prepared(
    tool: &HeldTool,
    cwd: &HeldDirectory,
    prepared: PreparedVersionInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<ToolOutput, Error> {
    semaprax_native_rust_interop_platform_sys::version_prepared(
        &tool.executable.0,
        &cwd.0,
        prepared.0,
        &mut process_arena.0,
    )
    .map(|bytes| ToolOutput { bytes })
}

pub fn compile_c_tool_to_stdout_bounded(
    tool: &HeldTool,
    cwd: &HeldDirectory,
    target: &str,
    input: &OsStr,
    optimization: u8,
    sanitizers: bool,
    maximum: usize,
) -> Result<ToolOutput, Error> {
    compile_c_to_stdout_bounded(
        &tool.executable,
        cwd,
        target,
        input,
        optimization,
        sanitizers,
        maximum,
    )
}

pub fn prepare_c_compile_invocation(
    target: &str,
    input: &OsStr,
    optimization: u8,
    sanitizers: bool,
    maximum: usize,
) -> Result<PreparedCCompileInvocation, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_c_compile_invocation(
        target,
        input,
        optimization,
        sanitizers,
        maximum,
    )
    .map(PreparedCCompileInvocation)
}

pub fn prepared_c_compile_owned_capacity(prepared: &PreparedCCompileInvocation) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_c_compile_owned_capacity(&prepared.0)
}

pub fn compile_c_tool_prepared(
    tool: &HeldTool,
    cwd: &HeldDirectory,
    prepared: PreparedCCompileInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<ToolOutput, Error> {
    semaprax_native_rust_interop_platform_sys::compile_c_prepared(
        &tool.executable.0,
        &cwd.0,
        prepared.0,
        &mut process_arena.0,
    )
    .map(|bytes| ToolOutput { bytes })
}

pub fn prepare_rust_compile_invocation(
    target: &str,
    source: &OsStr,
    output: &OsStr,
) -> Result<PreparedRustCompileInvocation, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_rust_compile_invocation(
        target, source, output,
    )
    .map(PreparedRustCompileInvocation)
}

pub fn prepared_rust_compile_owned_capacity(prepared: &PreparedRustCompileInvocation) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_rust_compile_owned_capacity(&prepared.0)
}

pub fn compile_rust_tool_prepared(
    tool: &HeldDirectRustc,
    cwd: &HeldDirectory,
    prepared: PreparedRustCompileInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<HeldRegularFile, Error> {
    semaprax_native_rust_interop_platform_sys::compile_direct_rustc_prepared(
        &tool.0,
        &cwd.0,
        prepared.0,
        &mut process_arena.0,
    )
    .map(HeldRegularFile)
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_link_invocation(
    target: &str,
    linker: Option<&OsStr>,
    vctools: Option<&OsStr>,
    harness: &OsStr,
    c_object: &OsStr,
    rust_archive: &OsStr,
    output: &OsStr,
    sanitizers: bool,
) -> Result<PreparedLinkInvocation, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_link_invocation(
        target,
        linker,
        vctools,
        harness,
        c_object,
        rust_archive,
        output,
        sanitizers,
    )
    .map(PreparedLinkInvocation)
}

pub fn prepared_link_owned_capacity(prepared: &PreparedLinkInvocation) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_link_owned_capacity(&prepared.0)
}

pub fn prepare_archive_invocation(
    input: &OsStr,
    output: &OsStr,
) -> Result<PreparedArchiveInvocation, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_archive_invocation(input, output)
        .map(PreparedArchiveInvocation)
}

pub fn prepared_archive_owned_capacity(prepared: &PreparedArchiveInvocation) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_archive_owned_capacity(&prepared.0)
}

/// Produces one bounded deterministic archive containing exactly the held
/// object. Platform linker-index members have closed names, order, headers,
/// and total size and are bound by the returned archive digest, but their
/// opaque payload semantics are not independently reconstructed here.
pub fn archive_tool_prepared(
    archiver: &HeldTool,
    cwd: &HeldDirectory,
    input: &HeldRegularFile,
    prepared: PreparedArchiveInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<HeldRegularFile, ArchiveToolFailure> {
    // `cwd` is intentionally the caller-owned private nonce run stage. The
    // accepted archive is copied from there into the publication inventory by
    // the existing create-new held-file authority; this operation is not an
    // in-place publication primitive for a shared directory.
    #[cfg(target_os = "macos")]
    let result = semaprax_native_rust_interop_platform_sys::archive_prepared_settled(
        &archiver.executable.0,
        &cwd.0,
        &input.0,
        prepared.0,
        &mut process_arena.0,
    )
    .map_err(|failure| {
        use semaprax_native_rust_interop_platform_sys::DarwinArchiveFailurePhase as Native;
        let phase = match failure.phase {
            Native::Preflight => ArchiveToolFailurePhase::Preflight,
            Native::ScratchCreation => ArchiveToolFailurePhase::ScratchCreation,
            Native::Process => ArchiveToolFailurePhase::Process,
            Native::ScratchCleanup => ArchiveToolFailurePhase::ScratchCleanup,
            Native::ArchiverRecheck => ArchiveToolFailurePhase::ArchiverRecheck,
            Native::WorkingDirectoryRecheck => ArchiveToolFailurePhase::WorkingDirectoryRecheck,
            Native::InputRecheck => ArchiveToolFailurePhase::InputRecheck,
            Native::ProcessOutput => ArchiveToolFailurePhase::ProcessOutput,
            Native::OutputHold => ArchiveToolFailurePhase::OutputHold,
            Native::ExactArchive => ArchiveToolFailurePhase::ExactArchive,
            Native::LaunchPathRecheck => ArchiveToolFailurePhase::LaunchPathRecheck,
            Native::OutputRecheck => ArchiveToolFailurePhase::OutputRecheck,
        };
        ArchiveToolFailure {
            error: failure.error,
            phase,
            settlement: match failure.settlement {
                semaprax_native_rust_interop_platform_sys::DarwinArchiveSettlement::Settled => {
                    ArchiveToolSettlement::Settled
                }
                semaprax_native_rust_interop_platform_sys::DarwinArchiveSettlement::Uncertain => {
                    ArchiveToolSettlement::Uncertain
                }
            },
        }
    });
    #[cfg(not(target_os = "macos"))]
    let result = semaprax_native_rust_interop_platform_sys::archive_prepared(
        &archiver.executable.0,
        &cwd.0,
        &input.0,
        prepared.0,
        &mut process_arena.0,
    )
    .map_err(|error| ArchiveToolFailure {
        error,
        phase: ArchiveToolFailurePhase::Platform,
        // The legacy Linux and Windows sys boundary returns only an error. It
        // does not prove whether the archive process or its attempted cleanup
        // changed the private namespace, so the safe facade must fail-stop.
        settlement: ArchiveToolSettlement::Uncertain,
    });
    result.map(HeldRegularFile)
}

pub fn link_tool_prepared(
    tool: &HeldTool,
    linker: Option<&HeldTool>,
    cwd: &HeldDirectory,
    prepared: PreparedLinkInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<HeldExecutable, Error> {
    semaprax_native_rust_interop_platform_sys::link_prepared(
        &tool.executable.0,
        linker.map(|linker| (&linker.executable.0, linker.path.as_str())),
        &cwd.0,
        prepared.0,
        &mut process_arena.0,
    )
    .map(HeldExecutable)
}

pub fn prepare_run_invocation() -> Result<PreparedRunInvocation, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_run_invocation().map(PreparedRunInvocation)
}

pub fn prepared_run_owned_capacity(prepared: &PreparedRunInvocation) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_run_owned_capacity(&prepared.0)
}

pub fn execute_tool_prepared(
    executable: &HeldExecutable,
    cwd: &HeldDirectory,
    prepared: PreparedRunInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<(), Error> {
    semaprax_native_rust_interop_platform_sys::run_prepared(
        &executable.0,
        &cwd.0,
        prepared.0,
        &mut process_arena.0,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "each held link input remains explicit"
)]
pub fn link_tool_harness(
    tool: &HeldTool,
    linker: Option<&HeldTool>,
    vctools: Option<&OsStr>,
    cwd: &HeldDirectory,
    target: &str,
    harness: &OsStr,
    c_object: &OsStr,
    rust_archive: &OsStr,
    output: &OsStr,
    sanitizers: bool,
) -> Result<HeldExecutable, Error> {
    link_harness(
        &tool.executable,
        linker,
        vctools,
        cwd,
        target,
        harness,
        c_object,
        rust_archive,
        output,
        sanitizers,
    )
}

pub fn compile_c_to_stdout(
    executable: &HeldExecutable,
    cwd: &HeldDirectory,
    target: &str,
    input: &OsStr,
    optimization: u8,
    sanitizers: bool,
) -> Result<ToolOutput, Error> {
    compile_c_to_stdout_bounded(
        executable,
        cwd,
        target,
        input,
        optimization,
        sanitizers,
        33_554_432,
    )
}

pub fn compile_c_to_stdout_bounded(
    executable: &HeldExecutable,
    cwd: &HeldDirectory,
    target: &str,
    input: &OsStr,
    optimization: u8,
    sanitizers: bool,
    maximum: usize,
) -> Result<ToolOutput, Error> {
    semaprax_native_rust_interop_platform_sys::compile_c_to_stdout(
        &executable.0,
        &cwd.0,
        target,
        input,
        optimization,
        sanitizers,
        maximum,
    )
    .map(|bytes| ToolOutput { bytes })
}

pub fn execute_harness(executable: &HeldExecutable, cwd: &HeldDirectory) -> Result<(), Error> {
    semaprax_native_rust_interop_platform_sys::execute_harness(&executable.0, &cwd.0)
}

#[allow(
    clippy::too_many_arguments,
    reason = "each held link input remains explicit"
)]
pub fn link_harness(
    clang: &HeldExecutable,
    linker: Option<&HeldTool>,
    vctools: Option<&OsStr>,
    cwd: &HeldDirectory,
    target: &str,
    harness: &OsStr,
    c_object: &OsStr,
    rust_archive: &OsStr,
    output: &OsStr,
    sanitizers: bool,
) -> Result<HeldExecutable, Error> {
    semaprax_native_rust_interop_platform_sys::link_harness(
        &clang.0,
        linker.map(|linker| (&linker.executable.0, linker.path.as_str())),
        vctools,
        &cwd.0,
        target,
        harness,
        c_object,
        rust_archive,
        output,
        sanitizers,
    )
    .map(HeldExecutable)
}

pub fn publish_directory_required_capacity(name: &OsStr) -> Result<usize, Error> {
    semaprax_native_rust_interop_platform_sys::publish_directory_required_capacity(name)
}

pub fn prepare_publish_directory(name: &OsStr) -> Result<PreparedPublishDirectory, Error> {
    semaprax_native_rust_interop_platform_sys::prepare_publish_directory(name)
        .map(PreparedPublishDirectory)
}

pub fn prepared_publish_directory_owned_capacity(prepared: &PreparedPublishDirectory) -> usize {
    semaprax_native_rust_interop_platform_sys::prepared_publish_directory_owned_capacity(
        &prepared.0,
    )
}

pub fn prepared_publish_directory_remaining(prepared: &PreparedPublishDirectory) -> u8 {
    semaprax_native_rust_interop_platform_sys::prepared_publish_directory_remaining(&prepared.0)
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn inject_publish_directory_failure(
    prepared: &mut PreparedPublishDirectory,
    point: u8,
) -> Result<(), Error> {
    semaprax_native_rust_interop_platform_sys::inject_publish_directory_failure(
        &mut prepared.0,
        point,
    )
}

pub fn publish_directory_new_prepared(
    prepared: &mut PreparedPublishDirectory,
    parent: &HeldDirectory,
    stage: &HeldDirectory,
    stage_name: &PreparedStageName,
    output_name: &OsStr,
) -> Result<(), Error> {
    semaprax_native_rust_interop_platform_sys::publish_directory_new_prepared(
        &mut prepared.0,
        &parent.0,
        &stage.0,
        &stage_name.0,
        output_name,
    )
}

pub fn discard_owned_stage_prepared<const N: usize>(
    parent: &HeldDirectory,
    stage: &HeldDirectory,
    stage_name: &PreparedStageName,
    inventory: &PreparedDiscardInventory<N>,
) -> Result<(), Error> {
    let raw = std::array::from_fn(|index| inventory.files[index].as_ref().map(|file| &file.0));
    let settled = std::array::from_fn(|index| inventory.settled[index].as_ref());
    semaprax_native_rust_interop_platform_sys::discard_owned_stage_prepared(
        &parent.0,
        &stage.0,
        &stage_name.0,
        &inventory.native,
        &raw,
        &settled,
        #[cfg(debug_assertions)]
        inventory.failure_after_delete,
    )
}
