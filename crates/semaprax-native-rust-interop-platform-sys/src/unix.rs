//! Unix held-handle filesystem, process, and archive authority.
use super::*;
use sha2::{Digest as _, Sha256};
use std::os::fd::{AsRawFd as _, FromRawFd as _, RawFd};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{FileExt as _, MetadataExt as _};

struct CheckedFd(Option<RawFd>);

impl CheckedFd {
    fn new(fd: RawFd) -> Self {
        Self(Some(fd))
    }

    fn raw(&self) -> RawFd {
        self.0.expect("checked descriptor remains owned")
    }

    fn close(mut self) -> Result<(), Error> {
        let descriptor = self.0.take().expect("checked descriptor remains owned");
        if unsafe { libc::close(descriptor) } == 0 {
            Ok(())
        } else {
            Err(Error::Spawn)
        }
    }

    fn close_injected(self, point: TestClosePoint) -> Result<(), Error> {
        let result = self.close();
        if point.injected() {
            Err(Error::Spawn)
        } else {
            result
        }
    }
}

impl Drop for CheckedFd {
    fn drop(&mut self) {
        if let Some(descriptor) = self.0.take() {
            if unsafe { libc::close(descriptor) } != 0 {
                std::process::abort();
            }
        }
    }
}

#[derive(Clone, Copy)]
enum TestClosePoint {
    Settle,
    SuccessRead,
    ParentWrite,
    ParentNull,
}

impl TestClosePoint {
    fn injected(self) -> bool {
        match self {
            Self::Settle => injected_settlement_failure!(UnixSettleClose),
            Self::SuccessRead => injected_settlement_failure!(UnixSuccessReadClose),
            Self::ParentWrite => injected_settlement_failure!(UnixParentWriteClose),
            Self::ParentNull => injected_settlement_failure!(UnixParentNullClose),
        }
    }
}

pub struct Directory {
    file: File,
    dev: u64,
    ino: u64,
    mode: u32,
    #[cfg(target_os = "macos")]
    generation: u32,
}

pub struct RegularFile {
    file: File,
    dev: u64,
    ino: u64,
    mode: u32,
    len: u64,
    digest: [u8; 32],
    #[cfg(target_os = "macos")]
    generation: u32,
}

pub struct SettledRegularFile(RegularFile);

pub fn settle_regular_file_for_publish(file: RegularFile) -> SettledRegularFile {
    SettledRegularFile(file)
}

pub struct Executable {
    file: RegularFile,
    slice_offset: u64,
    slice_size: u64,
    // Darwin's installed developer tools are hard-linked multicall images.
    // F_GETPATH identifies the vnode, but does not preserve which admitted
    // name selected the tool's behavior (for example libtool vs clang).
    #[cfg(target_os = "macos")]
    launch_path: Option<CString>,
}

pub struct RustcDiscovery {
    executable: Executable,
    resolver: PreparedToolResolver,
}

pub struct DirectRustc {
    executable: Executable,
    sysroot: Directory,
    recheck_resolver: Option<PreparedToolResolver>,
}

pub struct PreparedRelativeName(CString);

pub struct PreparedRelativeNameArena {
    bytes: Vec<u8>,
    maximum: usize,
}

pub struct PreparedVersionInvocation {
    argument: CString,
    output: Vec<u8>,
}

pub struct PreparedSysrootInvocation(PreparedVersionInvocation);
pub struct PreparedRustcVersionInvocation(PreparedVersionInvocation);

pub struct PreparedProcessArena {
    remaining: usize,
}

pub struct PreparedProcessArenaPlan {
    uses: usize,
}

impl Drop for PreparedProcessArena {
    fn drop(&mut self) {}
}

pub struct PreparedToolResolver {
    candidate: Vec<u8>,
    canonical: Vec<u8>,
    display: String,
    fallback: CString,
    maximum: usize,
}

struct PreparedCommand {
    arguments: Vec<CString>,
    output: Vec<u8>,
}

pub struct PreparedCCompileInvocation(PreparedCommand);
pub struct PreparedRustCompileInvocation {
    command: PreparedCommand,
    output_name: PreparedRelativeName,
}
pub struct PreparedLinkInvocation {
    command: PreparedCommand,
    output_name: PreparedRelativeName,
}
pub struct PreparedArchiveInvocation {
    command: PreparedCommand,
    input_name: PreparedRelativeName,
    output_name: PreparedRelativeName,
    #[cfg(target_os = "macos")]
    scratch_name: PreparedRelativeNameArena,
    #[cfg(target_os = "macos")]
    scratch_file: PreparedRelativeName,
    #[cfg(target_os = "macos")]
    scratch_inventory: PreparedDiscardNames<1>,
    #[cfg(target_os = "macos")]
    empty_scratch_inventory: PreparedDiscardNames<0>,
}
pub struct PreparedRunInvocation(PreparedCommand);

#[cfg(target_os = "linux")]
const LINUX_LINKER_ARGUMENT: &str = "--ld-path=/usr/bin/ld";

#[cfg(target_os = "linux")]
const LINUX_RUST_STATICLIB_NATIVE_LIBS: [&str; 7] = [
    "-lgcc_s",
    "-lutil",
    "-lrt",
    "-lpthread",
    "-lm",
    "-ldl",
    "-lc",
];

fn prepare_command(values: &[&str], output_capacity: usize) -> Result<PreparedCommand, Error> {
    let mut arguments = Vec::with_capacity(values.len());
    if arguments.capacity() != values.len() {
        return Err(Error::OutputLimit);
    }
    for value in values {
        arguments.push(argument(value)?);
    }
    let output = Vec::with_capacity(output_capacity);
    if output.capacity() != output_capacity {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedCommand { arguments, output })
}

fn prepared_command_owned_capacity(command: &PreparedCommand) -> usize {
    command
        .arguments
        .capacity()
        .saturating_mul(std::mem::size_of::<CString>())
        .saturating_add(
            command
                .arguments
                .iter()
                .map(|value| value.as_bytes_with_nul().len())
                .sum::<usize>(),
        )
        .saturating_add(command.output.capacity())
}

pub fn prepare_tool_resolver(
    fallback: &str,
    maximum: usize,
) -> Result<PreparedToolResolver, Error> {
    if maximum == 0
        || maximum > 32_768
        || fallback.is_empty()
        || fallback.as_bytes().contains(&b'/')
    {
        return Err(Error::Invalid);
    }
    let candidate = Vec::with_capacity(maximum);
    let canonical = Vec::with_capacity(maximum);
    let display = String::with_capacity(maximum);
    if candidate.capacity() != maximum
        || canonical.capacity() != maximum
        || display.capacity() != maximum
    {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedToolResolver {
        candidate,
        canonical,
        display,
        fallback: CString::new(fallback).map_err(|_| Error::Invalid)?,
        maximum,
    })
}

pub fn prepared_tool_resolver_owned_capacity(prepared: &PreparedToolResolver) -> usize {
    prepared
        .candidate
        .capacity()
        .saturating_add(prepared.canonical.capacity())
        .saturating_add(prepared.display.capacity())
        .saturating_add(prepared.fallback.as_bytes_with_nul().len())
}

pub fn prepare_version_invocation(
    argument: &str,
    maximum: usize,
) -> Result<PreparedVersionInvocation, Error> {
    if maximum > 65_536 {
        return Err(Error::OutputLimit);
    }
    let argument = CString::new(argument).map_err(|_| Error::Invalid)?;
    let output = Vec::with_capacity(maximum);
    if output.capacity() != maximum {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedVersionInvocation { argument, output })
}

pub fn prepare_sysroot_invocation(maximum: usize) -> Result<PreparedSysrootInvocation, Error> {
    prepare_version_invocation("--print=sysroot", maximum).map(PreparedSysrootInvocation)
}

pub fn prepare_rustc_version_invocation(
    maximum: usize,
) -> Result<PreparedRustcVersionInvocation, Error> {
    prepare_version_invocation("-vV", maximum).map(PreparedRustcVersionInvocation)
}

pub fn prepared_sysroot_owned_capacity(prepared: &PreparedSysrootInvocation) -> usize {
    prepared_version_owned_capacity(&prepared.0)
}

pub fn prepared_rustc_version_owned_capacity(prepared: &PreparedRustcVersionInvocation) -> usize {
    prepared_version_owned_capacity(&prepared.0)
}

pub fn prepared_version_owned_capacity(prepared: &PreparedVersionInvocation) -> usize {
    prepared
        .argument
        .as_bytes_with_nul()
        .len()
        .saturating_add(prepared.output.capacity())
}

pub fn prepare_process_arena_plan(uses: usize) -> Result<PreparedProcessArenaPlan, Error> {
    if uses == 0 || uses > 32 {
        return Err(Error::Invalid);
    }
    Ok(PreparedProcessArenaPlan { uses })
}

pub fn prepare_process_arena_plan_with_environment(
    uses: usize,
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<PreparedProcessArenaPlan, Error> {
    if include.is_some() || libraries.is_some() {
        return Err(Error::Invalid);
    }
    prepare_process_arena_plan(uses)
}

pub fn prepared_process_arena_plan_capacity(_: &PreparedProcessArenaPlan) -> usize {
    0
}

pub fn materialize_process_arena(
    plan: PreparedProcessArenaPlan,
) -> Result<PreparedProcessArena, Error> {
    Ok(PreparedProcessArena {
        remaining: plan.uses,
    })
}

pub fn materialize_process_arena_with_environment(
    plan: PreparedProcessArenaPlan,
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<PreparedProcessArena, Error> {
    if include.is_some() || libraries.is_some() {
        return Err(Error::Invalid);
    }
    materialize_process_arena(plan)
}

pub fn prepare_process_arena(uses: usize) -> Result<PreparedProcessArena, Error> {
    materialize_process_arena(prepare_process_arena_plan(uses)?)
}

pub fn prepared_process_arena_owned_capacity(_: &PreparedProcessArena) -> usize {
    0
}

pub fn prepared_process_arena_remaining(prepared: &PreparedProcessArena) -> usize {
    prepared.remaining
}

pub(super) fn consume_process_arena(prepared: &mut PreparedProcessArena) -> Result<(), Error> {
    prepared.remaining = prepared
        .remaining
        .checked_sub(1)
        .ok_or(Error::OutputLimit)?;
    Ok(())
}

pub struct PreparedDiscardNames<const N: usize> {
    names: [Option<PreparedRelativeName>; N],
}

pub struct PreparedLinkOrCopy {
    destination_index: usize,
    destination: PreparedRelativeName,
    #[cfg(debug_assertions)]
    fail_before_authentication: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct PreparedDirectoryIdentity {
    dev: u64,
    ino: u64,
    mode: u32,
    #[cfg(target_os = "macos")]
    generation: u32,
}

pub struct PreparedInventoryExact<const N: usize> {
    names: [Option<PreparedRelativeName>; N],
    bindings: [(usize, usize); N],
    storage: Box<[u64]>,
    directory_identity: Option<PreparedDirectoryIdentity>,
    remaining: u8,
    #[cfg(test)]
    scan_entries: usize,
    #[cfg(test)]
    fail_initial_seek: bool,
    #[cfg(test)]
    fail_reset_seek: bool,
    #[cfg(test)]
    fail_rebound_authentication: bool,
    #[cfg(test)]
    fail_rebound_close: bool,
}

pub struct PreparedInventoryEntriesExact<const N: usize> {
    names: PreparedDiscardNames<N>,
    file_count: usize,
    storage: Box<[u64]>,
    remaining: u8,
}

pub struct PreparedPublishDirectory {
    destination: CString,
    exact_capacity: usize,
    remaining: u8,
    #[cfg(debug_assertions)]
    fail_before_open: bool,
    #[cfg(debug_assertions)]
    fail_information: bool,
    #[cfg(debug_assertions)]
    fail_close: bool,
    #[cfg(debug_assertions)]
    fail_rename: bool,
}

#[cfg(target_os = "linux")]
const INVENTORY_EXACT_ARENA_WORDS: usize = 8192;
#[cfg(target_os = "macos")]
const INVENTORY_EXACT_ARENA_WORDS: usize = 131_072;

fn prepared_name_bindings<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<[(usize, usize); N], Error> {
    let mut bindings = [(0, 0); N];
    for (index, binding) in bindings.iter_mut().enumerate() {
        let name = prepared_discard_name(names, index)?;
        *binding = (name.0.as_ptr() as usize, name.0.as_bytes_with_nul().len());
    }
    Ok(bindings)
}

pub fn inventory_exact_required_capacity<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<usize, Error> {
    let mut total = 0usize;
    for index in 0..N {
        total = total
            .checked_add(
                prepared_discard_name(names, index)?
                    .0
                    .as_bytes_with_nul()
                    .len(),
            )
            .ok_or(Error::OutputLimit)?;
    }
    total
        .checked_add(
            INVENTORY_EXACT_ARENA_WORDS
                .checked_mul(std::mem::size_of::<u64>())
                .ok_or(Error::OutputLimit)?,
        )
        .ok_or(Error::OutputLimit)
}

pub fn prepare_inventory_exact<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<PreparedInventoryExact<N>, Error> {
    let bindings = prepared_name_bindings(names)?;
    let mut copied = [const { None }; N];
    for (index, slot) in copied.iter_mut().enumerate() {
        let source = prepared_discard_name(names, index)?;
        let exact = source.0.as_bytes_with_nul().len();
        let mut bytes = Vec::with_capacity(exact);
        bytes.extend_from_slice(source.0.as_bytes_with_nul());
        if bytes.capacity() != exact {
            return Err(Error::OutputLimit);
        }
        *slot = Some(
            CString::from_vec_with_nul(bytes)
                .map(PreparedRelativeName)
                .map_err(|_| Error::Invalid)?,
        );
    }
    Ok(PreparedInventoryExact {
        names: copied,
        bindings,
        storage: vec![0_u64; INVENTORY_EXACT_ARENA_WORDS].into_boxed_slice(),
        directory_identity: None,
        remaining: 2,
        #[cfg(test)]
        scan_entries: 0,
        #[cfg(test)]
        fail_initial_seek: false,
        #[cfg(test)]
        fail_reset_seek: false,
        #[cfg(test)]
        fail_rebound_authentication: false,
        #[cfg(test)]
        fail_rebound_close: false,
    })
}

pub fn prepared_inventory_exact_owned_capacity<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> usize {
    prepared
        .names
        .iter()
        .filter_map(Option::as_ref)
        .map(|name| name.0.as_bytes_with_nul().len())
        .sum::<usize>()
        .saturating_add(
            prepared
                .storage
                .len()
                .saturating_mul(std::mem::size_of::<u64>()),
        )
}

pub fn prepared_inventory_exact_remaining<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> u8 {
    prepared.remaining
}

pub fn prepare_inventory_entries_exact<const N: usize>(
    names: [&OsStr; N],
    file_count: usize,
) -> Result<PreparedInventoryEntriesExact<N>, Error> {
    if N == 0 || file_count > N {
        return Err(Error::Invalid);
    }
    Ok(PreparedInventoryEntriesExact {
        names: prepare_discard_names(names)?,
        file_count,
        storage: vec![0_u64; INVENTORY_EXACT_ARENA_WORDS].into_boxed_slice(),
        remaining: 1,
    })
}

pub fn prepared_inventory_entries_exact_owned_capacity<const N: usize>(
    prepared: &PreparedInventoryEntriesExact<N>,
) -> usize {
    prepared_discard_names_owned_capacity(&prepared.names).saturating_add(
        prepared
            .storage
            .len()
            .saturating_mul(std::mem::size_of::<u64>()),
    )
}

pub fn publish_directory_required_capacity(name: &OsStr) -> Result<usize, Error> {
    validated_c_name_bytes(name)?
        .len()
        .checked_add(1)
        .ok_or(Error::OutputLimit)
}

pub fn prepare_publish_directory(name: &OsStr) -> Result<PreparedPublishDirectory, Error> {
    let bytes = validated_c_name_bytes(name)?;
    let exact_capacity = bytes.len().checked_add(1).ok_or(Error::OutputLimit)?;
    let mut copied = Vec::with_capacity(exact_capacity);
    copied.extend_from_slice(bytes);
    copied.push(0);
    if copied.capacity() != exact_capacity {
        return Err(Error::OutputLimit);
    }
    let destination = CString::from_vec_with_nul(copied).map_err(|_| Error::Invalid)?;
    Ok(PreparedPublishDirectory {
        destination,
        exact_capacity,
        remaining: 1,
        #[cfg(debug_assertions)]
        fail_before_open: false,
        #[cfg(debug_assertions)]
        fail_information: false,
        #[cfg(debug_assertions)]
        fail_close: false,
        #[cfg(debug_assertions)]
        fail_rename: false,
    })
}

pub fn prepared_publish_directory_owned_capacity(prepared: &PreparedPublishDirectory) -> usize {
    prepared.destination.as_bytes_with_nul().len()
}

pub fn prepared_publish_directory_remaining(prepared: &PreparedPublishDirectory) -> u8 {
    prepared.remaining
}

#[cfg(debug_assertions)]
pub fn inject_publish_directory_failure(
    prepared: &mut PreparedPublishDirectory,
    point: u8,
) -> Result<(), Error> {
    match point {
        1 => prepared.fail_before_open = true,
        2 => prepared.fail_information = true,
        3 => prepared.fail_close = true,
        4 => prepared.fail_rename = true,
        _ => return Err(Error::Invalid),
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn test_inventory_exact_failures<const N: usize>(
    prepared: &mut PreparedInventoryExact<N>,
    initial_seek: bool,
    reset_seek: bool,
    rebound_authentication: bool,
    rebound_close: bool,
) {
    prepared.fail_initial_seek = initial_seek;
    prepared.fail_reset_seek = reset_seek;
    prepared.fail_rebound_authentication = rebound_authentication;
    prepared.fail_rebound_close = rebound_close;
}

#[cfg(test)]
pub(crate) fn test_inventory_exact_scan_entries<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> usize {
    prepared.scan_entries
}

pub fn prepare_link_or_copy<const N: usize>(
    names: &PreparedDiscardNames<N>,
    destination_index: usize,
) -> Result<PreparedLinkOrCopy, Error> {
    let destination = prepared_discard_name(names, destination_index)?;
    let exact = destination.0.as_bytes_with_nul().len();
    let mut bytes = Vec::with_capacity(exact);
    bytes.extend_from_slice(destination.0.as_bytes_with_nul());
    if bytes.capacity() != exact {
        return Err(Error::OutputLimit);
    }
    let destination = CString::from_vec_with_nul(bytes)
        .map(PreparedRelativeName)
        .map_err(|_| Error::Invalid)?;
    Ok(PreparedLinkOrCopy {
        destination_index,
        destination,
        #[cfg(debug_assertions)]
        fail_before_authentication: false,
    })
}

pub fn link_or_copy_required_capacity<const N: usize>(
    names: &PreparedDiscardNames<N>,
    destination_index: usize,
) -> Result<usize, Error> {
    Ok(prepared_discard_name(names, destination_index)?
        .0
        .as_bytes_with_nul()
        .len())
}

pub fn prepared_link_or_copy_owned_capacity(prepared: &PreparedLinkOrCopy) -> usize {
    prepared.destination.0.as_bytes_with_nul().len()
}

#[cfg(debug_assertions)]
pub fn inject_link_or_copy_failure_before_authentication(prepared: &mut PreparedLinkOrCopy) {
    prepared.fail_before_authentication = true;
}

pub fn prepare_relative_name(name: &OsStr) -> Result<PreparedRelativeName, Error> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes == b"."
        || bytes == b".."
        || bytes.contains(&b'/')
        || bytes.contains(&0)
    {
        return Err(Error::Invalid);
    }
    let exact = bytes.len().checked_add(1).ok_or(Error::OutputLimit)?;
    let mut owned = Vec::with_capacity(exact);
    owned.extend_from_slice(bytes);
    owned.push(0);
    if owned.capacity() != exact {
        return Err(Error::OutputLimit);
    }
    CString::from_vec_with_nul(owned)
        .map(PreparedRelativeName)
        .map_err(|_| Error::Invalid)
}

pub fn prepare_relative_name_arena(maximum: usize) -> Result<PreparedRelativeNameArena, Error> {
    let capacity = maximum.checked_add(1).ok_or(Error::OutputLimit)?;
    let bytes = Vec::with_capacity(capacity);
    if bytes.capacity() != capacity {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedRelativeNameArena { bytes, maximum })
}

pub fn set_relative_name_arena(
    arena: &mut PreparedRelativeNameArena,
    name: &OsStr,
) -> Result<(), Error> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > arena.maximum
        || bytes == b"."
        || bytes == b".."
        || bytes.contains(&b'/')
        || bytes.contains(&0)
    {
        return Err(Error::Invalid);
    }
    let capacity = arena.maximum.checked_add(1).ok_or(Error::OutputLimit)?;
    arena.bytes.clear();
    arena.bytes.extend_from_slice(bytes);
    arena.bytes.push(0);
    if arena.bytes.capacity() != capacity {
        return Err(Error::OutputLimit);
    }
    Ok(())
}

pub fn relative_name_arena_capacity(arena: &PreparedRelativeNameArena) -> usize {
    arena.bytes.capacity()
}

fn relative_name_arena_cstr(arena: &PreparedRelativeNameArena) -> Result<&std::ffi::CStr, Error> {
    std::ffi::CStr::from_bytes_with_nul(&arena.bytes).map_err(|_| Error::Invalid)
}

pub fn prepare_discard_names<const N: usize>(
    names: [&OsStr; N],
) -> Result<PreparedDiscardNames<N>, Error> {
    let names = names.map(|name| prepare_relative_name(name).ok());
    if names.iter().any(Option::is_none) {
        return Err(Error::Invalid);
    }
    for left in 0..N {
        for right in 0..left {
            if names[left].as_ref().expect("validated").0.as_bytes()
                == names[right].as_ref().expect("validated").0.as_bytes()
            {
                return Err(Error::Invalid);
            }
        }
    }
    Ok(PreparedDiscardNames { names })
}

pub fn prepared_discard_names_owned_capacity<const N: usize>(
    prepared: &PreparedDiscardNames<N>,
) -> usize {
    prepared
        .names
        .iter()
        .filter_map(Option::as_ref)
        .map(|name| name.0.as_bytes_with_nul().len())
        .sum()
}

fn prepared_discard_name<const N: usize>(
    prepared: &PreparedDiscardNames<N>,
    index: usize,
) -> Result<&PreparedRelativeName, Error> {
    prepared
        .names
        .get(index)
        .and_then(Option::as_ref)
        .ok_or(Error::Invalid)
}

#[cfg(target_os = "macos")]
fn metadata_generation(metadata: &std::fs::Metadata) -> u32 {
    use std::os::macos::fs::MetadataExt as _;
    metadata.st_gen()
}

fn digest_file(file: &File, length: u64) -> Result<[u8; 32], Error> {
    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 8192];
    while offset < length {
        let remaining = usize::try_from((length - offset).min(buffer.len() as u64))
            .map_err(|_| Error::OutputLimit)?;
        let count = file
            .read_at(&mut buffer[..remaining], offset)
            .map_err(|_| Error::Changed)?;
        if count == 0 {
            return Err(Error::Changed);
        }
        hasher.update(&buffer[..count]);
        offset = offset
            .checked_add(u64::try_from(count).map_err(|_| Error::OutputLimit)?)
            .ok_or(Error::OutputLimit)?;
    }
    Ok(hasher.finalize().into())
}

fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(target_os = "macos")]
fn executable_slice(file: &RegularFile) -> Result<(u64, u64), Error> {
    let mut prefix = [0_u8; 32];
    if file
        .file
        .read_at(&mut prefix, 0)
        .map_err(|_| Error::Changed)?
        != prefix.len()
    {
        return Err(Error::Invalid);
    }
    let current_cpu = if cfg!(target_arch = "aarch64") {
        0x0100_000c_u32
    } else if cfg!(target_arch = "x86_64") {
        0x0100_0007_u32
    } else {
        return Err(Error::Unsupported);
    };
    let little = u32::from_le_bytes(prefix[0..4].try_into().map_err(|_| Error::Invalid)?);
    if little == 0xfeed_facf {
        let cpu = u32::from_le_bytes(prefix[4..8].try_into().map_err(|_| Error::Invalid)?);
        let subtype =
            u32::from_le_bytes(prefix[8..12].try_into().map_err(|_| Error::Invalid)?) & 0x00ff_ffff;
        let compatible_subtype = if cfg!(target_arch = "aarch64") {
            matches!(subtype, 0 | 2)
        } else {
            subtype == 3
        };
        let filetype = u32::from_le_bytes(prefix[12..16].try_into().map_err(|_| Error::Invalid)?);
        if cpu != current_cpu || !compatible_subtype || filetype != 2 {
            return Err(Error::Invalid);
        }
        return Ok((0, file.len));
    }
    let magic = u32::from_be_bytes(prefix[0..4].try_into().map_err(|_| Error::Invalid)?);
    let entry_size = match magic {
        0xcafe_babe => 20_usize,
        0xcafe_babf => 32_usize,
        _ => return Err(Error::Invalid),
    };
    let count = usize::try_from(u32::from_be_bytes(
        prefix[4..8].try_into().map_err(|_| Error::Invalid)?,
    ))
    .map_err(|_| Error::Invalid)?;
    if count == 0 || count > 64 {
        return Err(Error::Invalid);
    }
    let table_size = count.checked_mul(entry_size).ok_or(Error::Invalid)?;
    let table_end = 8_usize.checked_add(table_size).ok_or(Error::Invalid)?;
    if u64::try_from(table_end).map_err(|_| Error::Invalid)? > file.len {
        return Err(Error::Invalid);
    }
    let mut table = [0_u8; 64 * 32];
    if file
        .file
        .read_at(&mut table[..table_size], 8)
        .map_err(|_| Error::Changed)?
        != table_size
    {
        return Err(Error::Changed);
    }
    let mut rows = [(0_u32, 0_u32, 0_u64, 0_u64); 64];
    let mut row_count = 0usize;
    for index in 0..count {
        let start = index.checked_mul(entry_size).ok_or(Error::Invalid)?;
        let row = table.get(start..start + entry_size).ok_or(Error::Invalid)?;
        let cpu = u32::from_be_bytes(row[0..4].try_into().map_err(|_| Error::Invalid)?);
        let subtype = u32::from_be_bytes(row[4..8].try_into().map_err(|_| Error::Invalid)?);
        let (offset, size, alignment, reserved) = if entry_size == 20 {
            (
                u64::from(u32::from_be_bytes(
                    row[8..12].try_into().map_err(|_| Error::Invalid)?,
                )),
                u64::from(u32::from_be_bytes(
                    row[12..16].try_into().map_err(|_| Error::Invalid)?,
                )),
                u32::from_be_bytes(row[16..20].try_into().map_err(|_| Error::Invalid)?),
                0,
            )
        } else {
            (
                u64::from_be_bytes(row[8..16].try_into().map_err(|_| Error::Invalid)?),
                u64::from_be_bytes(row[16..24].try_into().map_err(|_| Error::Invalid)?),
                u32::from_be_bytes(row[24..28].try_into().map_err(|_| Error::Invalid)?),
                u32::from_be_bytes(row[28..32].try_into().map_err(|_| Error::Invalid)?),
            )
        };
        let end = offset.checked_add(size).ok_or(Error::Invalid)?;
        if size < 32
            || end > file.len
            || offset < u64::try_from(table_end).map_err(|_| Error::Invalid)?
            || alignment > 63
            || offset % (1_u64 << alignment) != 0
            || reserved != 0
            || rows[..row_count]
                .iter()
                .any(|(_, _, prior_offset, prior_end)| offset < *prior_end && *prior_offset < end)
            || rows[..row_count]
                .iter()
                .any(|(prior_cpu, prior_subtype, _, _)| {
                    *prior_cpu == cpu && *prior_subtype == subtype
                })
        {
            return Err(Error::Invalid);
        }
        rows[row_count] = (cpu, subtype, offset, end);
        row_count += 1;
    }
    let mut selected = None;
    for (cpu, subtype, offset, end) in &rows[..row_count] {
        let masked_subtype = *subtype & 0x00ff_ffff;
        let matches_current = *cpu == current_cpu
            && if cfg!(target_arch = "aarch64") {
                matches!(masked_subtype, 0 | 2)
            } else {
                masked_subtype == 3
            };
        if matches_current
            && selected
                .replace((masked_subtype, *offset, *end - *offset))
                .is_some()
        {
            return Err(Error::Invalid);
        }
    }
    let Some((selected_subtype, offset, size)) = selected else {
        return Err(Error::Invalid);
    };
    let mut header = [0_u8; 16];
    if file
        .file
        .read_at(&mut header, offset)
        .map_err(|_| Error::Changed)?
        != header.len()
    {
        return Err(Error::Changed);
    }
    if u32::from_le_bytes(header[0..4].try_into().map_err(|_| Error::Invalid)?) != 0xfeed_facf
        || u32::from_le_bytes(header[4..8].try_into().map_err(|_| Error::Invalid)?) != current_cpu
        || u32::from_le_bytes(header[12..16].try_into().map_err(|_| Error::Invalid)?) != 2
        || (u32::from_le_bytes(header[8..12].try_into().map_err(|_| Error::Invalid)?) & 0x00ff_ffff)
            != selected_subtype
    {
        return Err(Error::Invalid);
    }
    Ok((offset, size))
}

#[cfg(target_os = "linux")]
fn executable_slice(file: &RegularFile) -> Result<(u64, u64), Error> {
    let mut header = [0_u8; 64];
    if file
        .file
        .read_at(&mut header, 0)
        .map_err(|_| Error::Changed)?
        != header.len()
        || &header[..4] != b"\x7fELF"
        || header[4] != 2
        || header[5] != 1
        || header[6] != 1
        || !matches!(u16::from_le_bytes([header[16], header[17]]), 2 | 3)
        || u32::from_le_bytes(header[20..24].try_into().map_err(|_| Error::Invalid)?) != 1
        || u16::from_le_bytes([header[52], header[53]]) != 64
    {
        return Err(Error::Invalid);
    }
    let machine = u16::from_le_bytes([header[18], header[19]]);
    if (cfg!(target_arch = "x86_64") && machine != 62)
        || (cfg!(target_arch = "aarch64") && machine != 183)
    {
        return Err(Error::Invalid);
    }
    let program_offset = u64::from_le_bytes(header[32..40].try_into().map_err(|_| Error::Invalid)?);
    let entry_size = usize::from(u16::from_le_bytes([header[54], header[55]]));
    let entry_count = usize::from(u16::from_le_bytes([header[56], header[57]]));
    if entry_size != 56 || entry_count == 0 || entry_count > 4096 {
        return Err(Error::Invalid);
    }
    let table_size = entry_size.checked_mul(entry_count).ok_or(Error::Invalid)?;
    let table_end = program_offset
        .checked_add(u64::try_from(table_size).map_err(|_| Error::Invalid)?)
        .ok_or(Error::Invalid)?;
    if table_end > file.len {
        return Err(Error::Invalid);
    }
    let mut table = [0_u8; 56 * 4096];
    if file
        .file
        .read_at(&mut table[..table_size], program_offset)
        .map_err(|_| Error::Changed)?
        != table_size
    {
        return Err(Error::Changed);
    }
    let entry = u64::from_le_bytes(header[24..32].try_into().map_err(|_| Error::Invalid)?);
    let mut executable_load = false;
    for row in table[..table_size].chunks_exact(entry_size) {
        let kind = u32::from_le_bytes(row[0..4].try_into().map_err(|_| Error::Invalid)?);
        let flags = u32::from_le_bytes(row[4..8].try_into().map_err(|_| Error::Invalid)?);
        let offset = u64::from_le_bytes(row[8..16].try_into().map_err(|_| Error::Invalid)?);
        let virtual_address =
            u64::from_le_bytes(row[16..24].try_into().map_err(|_| Error::Invalid)?);
        let file_size = u64::from_le_bytes(row[32..40].try_into().map_err(|_| Error::Invalid)?);
        let memory_size = u64::from_le_bytes(row[40..48].try_into().map_err(|_| Error::Invalid)?);
        let alignment = u64::from_le_bytes(row[48..56].try_into().map_err(|_| Error::Invalid)?);
        if offset
            .checked_add(file_size)
            .is_none_or(|end| end > file.len)
            || virtual_address.checked_add(memory_size).is_none()
            || file_size > memory_size
            || (alignment > 1
                && (!alignment.is_power_of_two()
                    || offset % alignment != virtual_address % alignment))
        {
            return Err(Error::Invalid);
        }
        if kind == 1
            && flags & 1 != 0
            && entry >= virtual_address
            && entry
                < virtual_address
                    .checked_add(memory_size)
                    .ok_or(Error::Invalid)?
        {
            executable_load = true;
        }
    }
    if !executable_load {
        return Err(Error::Invalid);
    }
    Ok((0, file.len))
}

fn validated_c_name_bytes(name: &OsStr) -> Result<&[u8], Error> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes == b"."
        || bytes == b".."
        || bytes.contains(&b'/')
        || bytes.contains(&0)
    {
        return Err(Error::Invalid);
    }
    Ok(bytes)
}

fn c_name(name: &OsStr) -> Result<CString, Error> {
    CString::new(validated_c_name_bytes(name)?).map_err(|_| Error::Invalid)
}

fn wait_child(pid: libc::pid_t, kill_first: bool) -> Result<libc::c_int, Error> {
    if kill_first {
        let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
        let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let mut status = 0;
        let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if waited == pid {
            return Ok(status);
        }
        if waited == 0 {
            if std::time::Instant::now() >= deadline {
                let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
                return Err(Error::Spawn);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::EINTR) => continue,
            // ECHILD is accepted only after the kernel proves that this exact
            // pid is no longer a waitable child. No retry or stronger signal
            // authority is used.
            Some(libc::ECHILD) => return Err(Error::Spawn),
            _ => return Err(Error::Spawn),
        }
    }
}

#[cfg(target_os = "macos")]
fn quiesce_group_before_reap(pid: libc::pid_t) -> Result<(), Error> {
    const MAX_GROUP_MEMBERS: usize = 4096;

    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_listpgrppids(
            pgrpid: libc::pid_t,
            buffer: *mut libc::c_void,
            buffersize: libc::c_int,
        ) -> libc::c_int;
    }

    if unsafe { libc::kill(-pid, libc::SIGKILL) } != 0
        && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    {
        return Err(Error::Spawn);
    }
    let mut members = [0 as libc::pid_t; MAX_GROUP_MEMBERS];
    let member_bytes =
        libc::c_int::try_from(std::mem::size_of_val(&members)).map_err(|_| Error::Spawn)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        unsafe {
            *libc::__error() = 0;
        }
        let required = unsafe { proc_listpgrppids(pid, std::ptr::null_mut(), 0) };
        let required_errno = unsafe { *libc::__error() };
        // proc_listpgrppids returns a PID count, unlike proc_listpids,
        // which returns a byte count. The wrapper reports kernel failure
        // as zero, so errno must be bound independently.
        if (required == 0 && required_errno != 0)
            || required < 0
            || usize::try_from(required).map_err(|_| Error::Spawn)? > MAX_GROUP_MEMBERS
        {
            return Err(Error::Spawn);
        }
        members.fill(0);
        unsafe {
            *libc::__error() = 0;
        }
        let returned = unsafe { proc_listpgrppids(pid, members.as_mut_ptr().cast(), member_bytes) };
        let returned_errno = unsafe { *libc::__error() };
        if (returned == 0 && returned_errno != 0)
            || returned < 0
            || usize::try_from(returned).map_err(|_| Error::Spawn)? > MAX_GROUP_MEMBERS
        {
            return Err(Error::Spawn);
        }
        let count = usize::try_from(returned).map_err(|_| Error::Spawn)?;
        if members[..count].iter().all(|member| *member == pid) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Spawn);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn quiesce_group(pid: libc::pid_t) -> Result<(), Error> {
    let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let result = unsafe { libc::kill(-pid, 0) };
        if result != 0 {
            match std::io::Error::last_os_error().raw_os_error() {
                Some(libc::ESRCH) => return Ok(()),
                Some(libc::EINTR) => continue,
                _ => return Err(Error::Spawn),
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Spawn);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn settle_failed_group(
    pid: libc::pid_t,
    pipe: CheckedFd,
    leader_reaped: bool,
) -> Result<(), Error> {
    let close_failed = pipe.close_injected(TestClosePoint::Settle).is_err();
    #[cfg(target_os = "linux")]
    let mut leader = if leader_reaped {
        let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
        Ok(())
    } else {
        wait_child(pid, true).map(|_| ())
    };
    #[cfg(target_os = "linux")]
    let mut group = quiesce_group(pid);
    #[cfg(target_os = "macos")]
    let mut group = if leader_reaped {
        Ok(())
    } else {
        quiesce_group_before_reap(pid)
    };
    #[cfg(target_os = "macos")]
    let mut leader = if leader_reaped || group.is_err() {
        if leader_reaped {
            Ok(())
        } else {
            Err(Error::Spawn)
        }
    } else {
        wait_child(pid, false).map(|_| ())
    };
    if injected_settlement_failure!(UnixWait) {
        leader = Err(Error::Spawn);
    }
    if injected_settlement_failure!(UnixGroup) {
        group = Err(Error::Spawn);
    }
    if close_failed || leader.is_err() || group.is_err() {
        Err(Error::Spawn)
    } else {
        Ok(())
    }
}

fn must_settle_failed_group(pid: libc::pid_t, pipe: CheckedFd, leader_reaped: bool) {
    if settle_failed_group(pid, pipe, leader_reaped).is_err() {
        std::process::abort();
    }
}

fn drain_and_wait(
    pid: libc::pid_t,
    pipe: CheckedFd,
    stdout_limit: usize,
    mut output: Vec<u8>,
    close_pipe_after_leader: bool,
) -> Result<(Vec<u8>, libc::c_int), Error> {
    if injected_settlement_failure!(UnixDrainFcntl)
        || unsafe { libc::fcntl(pipe.raw(), libc::F_SETFL, libc::O_NONBLOCK) } != 0
    {
        must_settle_failed_group(pid, pipe, false);
        return Err(Error::Spawn);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let fixed_output = output.capacity() != 0 || stdout_limit == 0;
    if (fixed_output && output.capacity() != stdout_limit) || !output.is_empty() {
        must_settle_failed_group(pid, pipe, false);
        return Err(Error::OutputLimit);
    }
    let mut status = None;
    let mut eof = false;
    loop {
        let mut poll_fd = libc::pollfd {
            fd: pipe.raw(),
            events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
            revents: 0,
        };
        let polled = unsafe { libc::poll(&mut poll_fd, 1, 25) };
        if injected_settlement_failure!(UnixPoll)
            || (polled < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR))
        {
            must_settle_failed_group(pid, pipe, status.is_some());
            return Err(Error::Spawn);
        }
        if polled > 0 {
            loop {
                let mut buffer = [0_u8; 8192];
                if injected_settlement_failure!(UnixRead) {
                    must_settle_failed_group(pid, pipe, status.is_some());
                    return Err(Error::Spawn);
                }
                let read =
                    unsafe { libc::read(pipe.raw(), buffer.as_mut_ptr().cast(), buffer.len()) };
                match read.cmp(&0) {
                    std::cmp::Ordering::Greater => {
                        if injected_settlement_failure!(UnixReadConversion) {
                            must_settle_failed_group(pid, pipe, status.is_some());
                            return Err(Error::OutputLimit);
                        }
                        let count = match usize::try_from(read) {
                            Ok(count) => count,
                            Err(_) => {
                                must_settle_failed_group(pid, pipe, status.is_some());
                                return Err(Error::OutputLimit);
                            }
                        };
                        if count > stdout_limit.saturating_sub(output.len()) {
                            must_settle_failed_group(pid, pipe, status.is_some());
                            return Err(Error::OutputLimit);
                        }
                        output.extend_from_slice(&buffer[..count]);
                        if fixed_output && output.capacity() != stdout_limit {
                            must_settle_failed_group(pid, pipe, status.is_some());
                            return Err(Error::OutputLimit);
                        }
                    }
                    std::cmp::Ordering::Equal => {
                        eof = true;
                        break;
                    }
                    std::cmp::Ordering::Less => {
                        match std::io::Error::last_os_error().raw_os_error() {
                            Some(libc::EAGAIN) => break,
                            Some(libc::EINTR) => continue,
                            _ => {
                                must_settle_failed_group(pid, pipe, status.is_some());
                                return Err(Error::Spawn);
                            }
                        }
                    }
                }
            }
        }
        if status.is_none() {
            let mut child_status = 0;
            if injected_settlement_failure!(UnixWaitpid) {
                must_settle_failed_group(pid, pipe, false);
                return Err(Error::Spawn);
            }
            let waited = unsafe { libc::waitpid(pid, &mut child_status, libc::WNOHANG) };
            match waited {
                waited if waited == pid => {
                    status = Some(child_status);
                    if !eof {
                        // A descendant retaining the private pipe is not part of
                        // the admitted tool result. Close the whole private group.
                        let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
                        if close_pipe_after_leader {
                            // Archive stdout is fixed at zero bytes. Once the
                            // leader is reaped and its entire owned group is
                            // quiescent, EOF carries no additional authority:
                            // an unrelated holder must not keep publication
                            // waiting. Drain bytes already committed to the pipe,
                            // then close the read end without waiting for HUP.
                            if quiesce_group(pid).is_err() {
                                std::process::abort();
                            }
                            let result = loop {
                                let mut byte = [0_u8; 1];
                                let read = unsafe {
                                    libc::read(pipe.raw(), byte.as_mut_ptr().cast(), byte.len())
                                };
                                if read > 0 {
                                    break Err(Error::OutputLimit);
                                }
                                if read == 0 {
                                    break Ok((output, child_status));
                                }
                                match std::io::Error::last_os_error().raw_os_error() {
                                    Some(libc::EAGAIN) => break Ok((output, child_status)),
                                    Some(libc::EINTR) => continue,
                                    _ => break Err(Error::Spawn),
                                }
                            };
                            if pipe.close_injected(TestClosePoint::SuccessRead).is_err() {
                                std::process::abort();
                            }
                            return result;
                        }
                    }
                }
                0 => {}
                -1 if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) => {}
                _ => {
                    must_settle_failed_group(pid, pipe, status.is_some());
                    return Err(Error::Spawn);
                }
            }
        }
        if eof {
            if let Some(status) = status {
                // Quiesce every descendant in the private group even if it
                // closed stdout before the leader exited.
                let close_failed = pipe.close_injected(TestClosePoint::SuccessRead).is_err();
                let group = quiesce_group(pid);
                if close_failed || group.is_err() {
                    std::process::abort();
                }
                return Ok((output, status));
            }
        }
        if injected_settlement_failure!(UnixDeadline) || std::time::Instant::now() >= deadline {
            must_settle_failed_group(pid, pipe, status.is_some());
            return Err(Error::Spawn);
        }
    }
}

fn identity(metadata: &std::fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn open_directory_at(parent: RawFd, name: &std::ffi::CStr) -> Result<Directory, Error> {
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::Changed);
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file.metadata().map_err(|_| Error::Changed)?;
    let (dev, ino) = identity(&metadata);
    Ok(Directory {
        file,
        dev,
        ino,
        mode: metadata.mode(),
        #[cfg(target_os = "macos")]
        generation: metadata_generation(&metadata),
    })
}

pub fn hold_directory(path: &Path) -> Result<Directory, Error> {
    use std::path::Component;
    if !path.is_absolute() {
        return Err(Error::Invalid);
    }
    let c_path = CString::new("/").expect("literal");
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::Changed);
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file.metadata().map_err(|_| Error::Changed)?;
    let (dev, ino) = identity(&metadata);
    let mut current = Directory {
        file,
        dev,
        ino,
        mode: metadata.mode(),
        #[cfg(target_os = "macos")]
        generation: metadata_generation(&metadata),
    };
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let name = c_name(name)?;
                current = open_directory_at(current.file.as_raw_fd(), &name)?;
            }
            _ => return Err(Error::Invalid),
        }
    }
    Ok(current)
}

pub fn hold_child_directory(parent: &Directory, name: &OsStr) -> Result<Directory, Error> {
    recheck_directory(parent)?;
    let name = c_name(name)?;
    open_directory_at(parent.file.as_raw_fd(), &name)
}

pub fn recheck_directory(directory: &Directory) -> Result<(), Error> {
    let metadata = directory.file.metadata().map_err(|_| Error::Changed)?;
    if identity(&metadata) != (directory.dev, directory.ino)
        || metadata.mode() != directory.mode
        || !metadata.is_dir()
    {
        return Err(Error::Changed);
    }
    #[cfg(target_os = "macos")]
    if metadata_generation(&metadata) != directory.generation {
        return Err(Error::Changed);
    }
    Ok(())
}

/// Admits a Unix publication root only inside the repository's explicit
/// trusted-root model. Owner/mode checks are necessary but not sufficient:
/// callers separately uphold the documented no-uncooperative-mutation
/// precondition, including ancestor stability and Darwin ACL authority.
pub fn directory_is_current_user_private(directory: &Directory) -> Result<bool, Error> {
    recheck_directory(directory)?;
    let metadata = directory.file.metadata().map_err(|_| Error::Changed)?;
    Ok(metadata.uid() == unsafe { libc::geteuid() } && metadata.mode() & 0o7777 == 0o700)
}

pub fn same_directory_path(directory: &Directory, path: &Path) -> Result<bool, Error> {
    recheck_directory(directory)?;
    let rebound = hold_directory(path)?;
    Ok((rebound.dev, rebound.ino, rebound.mode) == (directory.dev, directory.ino, directory.mode))
}

pub fn create_directory_new(
    parent: &Directory,
    name: &OsStr,
    mode: u32,
) -> Result<Directory, Error> {
    recheck_directory(parent)?;
    let name = c_name(name)?;
    let mode = libc::mode_t::try_from(mode).map_err(|_| Error::Invalid)?;
    let result = unsafe { libc::mkdirat(parent.file.as_raw_fd(), name.as_ptr(), mode) };
    if result != 0 {
        return Err(match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::EEXIST) => Error::Exists,
            _ => Error::Changed,
        });
    }
    open_directory_at(parent.file.as_raw_fd(), &name)
}

pub fn create_directory_new_prepared(
    parent: &Directory,
    name: &PreparedRelativeNameArena,
    mode: u32,
) -> Result<Directory, Error> {
    create_directory_new_prepared_settled(parent, name, mode).map_err(|failure| failure.error)
}

pub fn create_directory_new_prepared_settled(
    parent: &Directory,
    name: &PreparedRelativeNameArena,
    mode: u32,
) -> Result<Directory, CreateDirectoryNewFailure> {
    let settled = |error| CreateDirectoryNewFailure {
        error,
        namespace_created: false,
    };
    recheck_directory(parent).map_err(settled)?;
    let name = relative_name_arena_cstr(name).map_err(settled)?;
    let mode = libc::mode_t::try_from(mode).map_err(|_| settled(Error::Invalid))?;
    let result = unsafe { libc::mkdirat(parent.file.as_raw_fd(), name.as_ptr(), mode) };
    if result != 0 {
        return Err(settled(
            match std::io::Error::last_os_error().raw_os_error() {
                Some(libc::EEXIST) => Error::Exists,
                _ => Error::Changed,
            },
        ));
    }
    #[cfg(target_os = "macos")]
    if archive_scratch_open_failure_injected() {
        return Err(CreateDirectoryNewFailure {
            error: Error::Changed,
            namespace_created: true,
        });
    }
    open_directory_at(parent.file.as_raw_fd(), name).map_err(|error| CreateDirectoryNewFailure {
        error,
        namespace_created: true,
    })
}

pub fn write_file_new(
    directory: &Directory,
    name: &OsStr,
    bytes: &[u8],
    mode: u32,
) -> Result<RegularFile, Error> {
    recheck_directory(directory)?;
    let name = c_name(name)?;
    let fd = unsafe {
        libc::openat(
            directory.file.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            mode,
        )
    };
    if fd < 0 {
        return Err(Error::Exists);
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes).map_err(|_| Error::Changed)?;
    file.sync_data().map_err(|_| Error::Changed)?;
    drop(file);
    hold_regular_file(
        directory,
        OsStr::new(name.to_str().map_err(|_| Error::Invalid)?),
    )
}

pub fn write_file_new_prepared<const N: usize>(
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    index: usize,
    bytes: &[u8],
    mode: u32,
) -> Result<RegularFile, Error> {
    let name = enter_prepared_file_syscalls(prepared_discard_name(names, index))?;
    recheck_directory(directory)?;
    let fd = unsafe {
        libc::openat(
            directory.file.as_raw_fd(),
            name.0.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            mode,
        )
    };
    if fd < 0 {
        return Err(Error::Exists);
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes).map_err(|_| Error::Changed)?;
    file.sync_data().map_err(|_| Error::Changed)?;
    authenticate_regular_file(file)
}

pub fn hold_regular_file(directory: &Directory, name: &OsStr) -> Result<RegularFile, Error> {
    recheck_directory(directory)?;
    let name = prepare_relative_name(name)?;
    hold_regular_file_name_prepared(directory, &name)
}

fn authenticate_regular_file(file: File) -> Result<RegularFile, Error> {
    authenticate_regular_file_bounded(file, u64::MAX)
}

fn authenticate_regular_file_bounded(file: File, maximum: u64) -> Result<RegularFile, Error> {
    let metadata = file.metadata().map_err(|_| Error::Changed)?;
    if !metadata.is_file() {
        return Err(Error::Changed);
    }
    if metadata.len() > maximum {
        return Err(Error::OutputLimit);
    }
    let (dev, ino) = identity(&metadata);
    let digest = digest_file(&file, metadata.len())?;
    Ok(RegularFile {
        file,
        dev,
        ino,
        mode: metadata.mode(),
        len: metadata.len(),
        digest,
        #[cfg(target_os = "macos")]
        generation: metadata_generation(&metadata),
    })
}

fn hold_regular_file_name_prepared(
    directory: &Directory,
    name: &PreparedRelativeName,
) -> Result<RegularFile, Error> {
    let fd = unsafe {
        libc::openat(
            directory.file.as_raw_fd(),
            name.0.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::Changed);
    }
    authenticate_regular_file(unsafe { File::from_raw_fd(fd) })
}

fn hold_regular_file_name_bounded_prepared(
    directory: &Directory,
    name: &PreparedRelativeName,
    maximum: u64,
) -> Result<RegularFile, Error> {
    let fd = unsafe {
        libc::openat(
            directory.file.as_raw_fd(),
            name.0.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::Changed);
    }
    authenticate_regular_file_bounded(unsafe { File::from_raw_fd(fd) }, maximum)
}

#[cfg(test)]
pub(crate) fn test_hold_regular_file_name_bounded(
    directory: &Directory,
    name: &OsStr,
    maximum: u64,
) -> Result<RegularFile, Error> {
    let name = prepare_relative_name(name)?;
    hold_regular_file_name_bounded_prepared(directory, &name, maximum)
}

fn hold_regular_file_cstr(
    directory: &Directory,
    name: &std::ffi::CStr,
) -> Result<RegularFile, Error> {
    let fd = unsafe {
        libc::openat(
            directory.file.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::Changed);
    }
    authenticate_regular_file(unsafe { File::from_raw_fd(fd) })
}

fn hold_executable_cstr(directory: &Directory, name: &std::ffi::CStr) -> Result<Executable, Error> {
    let file = hold_regular_file_cstr(directory, name)?;
    if file.mode & 0o111 == 0 {
        return Err(Error::Invalid);
    }
    let (slice_offset, slice_size) = executable_slice(&file)?;
    Ok(Executable {
        file,
        slice_offset,
        slice_size,
        #[cfg(target_os = "macos")]
        launch_path: None,
    })
}

pub fn hold_regular_file_prepared<const N: usize>(
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    index: usize,
    tracked: &RegularFile,
) -> Result<RegularFile, Error> {
    let name = enter_prepared_file_syscalls(prepared_discard_name(names, index))?;
    recheck_directory(directory)?;
    let rebound = hold_regular_file_name_prepared(directory, name)?;
    if rebound.dev != tracked.dev
        || rebound.ino != tracked.ino
        || rebound.mode != tracked.mode
        || rebound.len != tracked.len
        || rebound.digest != tracked.digest
        || cfg!(target_os = "macos") && {
            #[cfg(target_os = "macos")]
            {
                rebound.generation != tracked.generation
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        }
    {
        return Err(Error::Changed);
    }
    Ok(rebound)
}

pub fn hold_settled_regular_file_prepared<const N: usize>(
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    index: usize,
    tracked: &SettledRegularFile,
) -> Result<RegularFile, Error> {
    hold_regular_file_prepared(directory, names, index, &tracked.0)
}

pub fn transition_regular_file_to_external_read_prepared<const N: usize>(
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    index: usize,
    tracked: &RegularFile,
) -> Result<RegularFile, Error> {
    hold_regular_file_prepared(directory, names, index, tracked)
}

pub fn hold_external_executable(path: &Path) -> Result<Executable, Error> {
    let parent = path.parent().ok_or(Error::Invalid)?;
    let name = path.file_name().ok_or(Error::Invalid)?;
    let directory = hold_directory(parent)?;
    let executable = hold_executable(&directory, name)?;
    #[cfg(target_os = "macos")]
    let executable = {
        let mut executable = executable;
        if path.is_absolute() {
            executable.launch_path =
                Some(CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::Invalid)?);
        }
        executable
    };
    Ok(executable)
}

fn set_tool_candidate(
    prepared: &mut PreparedToolResolver,
    directory: Option<&[u8]>,
    configured: Option<&[u8]>,
) -> Result<(), Error> {
    prepared.candidate.clear();
    if let Some(configured) = configured {
        if configured.is_empty() || configured.contains(&0) {
            return Err(Error::Invalid);
        }
        if configured.len().saturating_add(1) > prepared.maximum {
            return Err(Error::OutputLimit);
        }
        prepared.candidate.extend_from_slice(configured);
    } else {
        let directory = directory.ok_or(Error::Invalid)?;
        let directory = if directory.is_empty() {
            b"."
        } else {
            directory
        };
        let fallback = prepared.fallback.as_bytes();
        let separator = usize::from(!directory.ends_with(b"/"));
        if directory
            .len()
            .checked_add(separator)
            .and_then(|length| length.checked_add(fallback.len()))
            .and_then(|length| length.checked_add(1))
            .is_none_or(|length| length > prepared.maximum)
        {
            return Err(Error::OutputLimit);
        }
        prepared.candidate.extend_from_slice(directory);
        if separator != 0 {
            prepared.candidate.push(b'/');
        }
        prepared.candidate.extend_from_slice(fallback);
    }
    prepared.candidate.push(0);
    if prepared.candidate.capacity() != prepared.maximum {
        return Err(Error::OutputLimit);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn canonical_tool_path(file: &File, output: &mut Vec<u8>, maximum: usize) -> Result<(), Error> {
    let mut link = [0_u8; 64];
    let prefix = b"/proc/self/fd/";
    link[..prefix.len()].copy_from_slice(prefix);
    let mut digits = [0_u8; 20];
    let mut value = u64::try_from(file.as_raw_fd()).map_err(|_| Error::Changed)?;
    let mut count = 0usize;
    loop {
        digits[count] = b'0' + u8::try_from(value % 10).map_err(|_| Error::Changed)?;
        count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for index in 0..count {
        link[prefix.len() + index] = digits[count - index - 1];
    }
    link[prefix.len() + count] = 0;
    output.clear();
    output.resize(maximum, 0);
    let length = unsafe {
        libc::readlink(
            link.as_ptr().cast(),
            output.as_mut_ptr().cast(),
            output.len(),
        )
    };
    if length <= 0 {
        return Err(Error::Changed);
    }
    let length = usize::try_from(length).map_err(|_| Error::Changed)?;
    if length >= maximum {
        return Err(Error::OutputLimit);
    }
    output.truncate(length);
    Ok(())
}

#[cfg(target_os = "macos")]
fn canonical_tool_path(file: &File, output: &mut Vec<u8>, maximum: usize) -> Result<(), Error> {
    output.clear();
    output.resize(maximum, 0);
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, output.as_mut_ptr()) } != 0 {
        return Err(Error::Changed);
    }
    let length = output
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::OutputLimit)?;
    output.truncate(length);
    Ok(())
}

fn hold_tool_candidate(
    prepared: &mut PreparedToolResolver,
    record_display: bool,
) -> Result<Option<Executable>, Error> {
    let candidate = prepared.candidate.as_ptr().cast();
    let fd = unsafe { libc::open(candidate, libc::O_RDONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Ok(None);
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file.metadata().map_err(|_| Error::Changed)?;
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.mode() & 0o111 == 0 {
        return Err(Error::Invalid);
    }
    if record_display {
        canonical_tool_path(&file, &mut prepared.canonical, prepared.maximum)?;
        let canonical = std::str::from_utf8(&prepared.canonical).map_err(|_| Error::Invalid)?;
        prepared.display.clear();
        prepared.display.push_str(canonical);
        if prepared.display.capacity() != prepared.maximum {
            return Err(Error::OutputLimit);
        }
    }
    let (dev, ino) = identity(&metadata);
    let digest = digest_file(&file, metadata.len())?;
    let regular = RegularFile {
        file,
        dev,
        ino,
        mode: metadata.mode(),
        len: metadata.len(),
        digest,
        #[cfg(target_os = "macos")]
        generation: metadata_generation(&metadata),
    };
    let (slice_offset, slice_size) = executable_slice(&regular)?;
    Ok(Some(Executable {
        file: regular,
        slice_offset,
        slice_size,
        #[cfg(target_os = "macos")]
        launch_path: if prepared.candidate.first() == Some(&b'/') {
            Some(unsafe { std::ffi::CStr::from_ptr(candidate) }.to_owned())
        } else {
            None
        },
    }))
}

pub fn resolve_and_hold_tool_prepared(
    prepared: PreparedToolResolver,
    configured: Option<&OsStr>,
    paths: Option<&OsStr>,
) -> Result<(Executable, String), Error> {
    let (executable, path, _) =
        resolve_and_hold_tool_reusing_prepared(prepared, configured, paths)?;
    Ok((executable, path))
}

pub fn resolve_and_hold_tool_reusing_prepared(
    mut prepared: PreparedToolResolver,
    configured: Option<&OsStr>,
    paths: Option<&OsStr>,
) -> Result<(Executable, String, PreparedToolResolver), Error> {
    if let Some(configured) = configured {
        set_tool_candidate(&mut prepared, None, Some(configured.as_bytes()))?;
        let executable = hold_tool_candidate(&mut prepared, true)?.ok_or(Error::Changed)?;
        let path = std::mem::take(&mut prepared.display);
        return Ok((executable, path, prepared));
    }
    let paths = paths.ok_or(Error::Invalid)?.as_bytes();
    for directory in paths.split(|byte| *byte == b':') {
        set_tool_candidate(&mut prepared, Some(directory), None)?;
        if let Some(executable) = hold_tool_candidate(&mut prepared, true)? {
            let path = std::mem::take(&mut prepared.display);
            return Ok((executable, path, prepared));
        }
    }
    Err(Error::Changed)
}

pub fn hold_rustc_discovery_prepared(
    mut prepared: PreparedToolResolver,
    configured: &OsStr,
) -> Result<RustcDiscovery, Error> {
    if !Path::new(configured).is_absolute() {
        return Err(Error::Invalid);
    }
    set_tool_candidate(&mut prepared, None, Some(configured.as_bytes()))?;
    let executable = hold_tool_candidate(&mut prepared, false)?.ok_or(Error::Changed)?;
    Ok(RustcDiscovery {
        executable,
        resolver: prepared,
    })
}

pub fn rustc_discovery_output_prepared(
    discovery: &RustcDiscovery,
    cwd: &Directory,
    prepared: PreparedSysrootInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    version_prepared(&discovery.executable, cwd, prepared.0, process_arena)
}

pub(super) fn one_sysroot_line(output: &[u8]) -> Result<&[u8], Error> {
    let line = output.strip_suffix(b"\n").ok_or(Error::Invalid)?;
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    if line.is_empty()
        || line.contains(&0)
        || line.contains(&b'\n')
        || line.contains(&b'\r')
        || std::str::from_utf8(line).is_err()
    {
        return Err(Error::Invalid);
    }
    Ok(line)
}

fn held_sysroot_from_output(
    prepared: &mut PreparedToolResolver,
    output: &[u8],
) -> Result<Directory, Error> {
    let line = one_sysroot_line(output)?;
    if line
        .len()
        .checked_add(1)
        .is_none_or(|length| length > prepared.maximum)
    {
        return Err(Error::OutputLimit);
    }
    prepared.candidate.clear();
    prepared.candidate.extend_from_slice(line);
    prepared.candidate.push(0);
    if prepared.candidate.capacity() != prepared.maximum {
        return Err(Error::OutputLimit);
    }
    if prepared.candidate.first() != Some(&b'/') {
        return Err(Error::Invalid);
    }
    let root_fd = unsafe {
        libc::open(
            c"/".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if root_fd < 0 {
        return Err(Error::Changed);
    }
    let root_file = unsafe { File::from_raw_fd(root_fd) };
    let root_metadata = root_file.metadata().map_err(|_| Error::Changed)?;
    let (dev, ino) = identity(&root_metadata);
    let mut current = Directory {
        file: root_file,
        dev,
        ino,
        mode: root_metadata.mode(),
        #[cfg(target_os = "macos")]
        generation: metadata_generation(&root_metadata),
    };
    let mut start = 1usize;
    let end_of_line = prepared.candidate.len() - 1;
    while start < end_of_line {
        let end = prepared.candidate[start..end_of_line]
            .iter()
            .position(|byte| *byte == b'/')
            .map_or(end_of_line, |offset| start + offset);
        if end == start
            || prepared.candidate[start..end] == *b"."
            || prepared.candidate[start..end] == *b".."
        {
            return Err(Error::Invalid);
        }
        let saved = prepared.candidate[end];
        prepared.candidate[end] = 0;
        let component = std::ffi::CStr::from_bytes_with_nul(&prepared.candidate[start..=end])
            .map_err(|_| Error::Invalid)?;
        current = open_directory_at(current.file.as_raw_fd(), component)?;
        prepared.candidate[end] = saved;
        start = end.saturating_add(1);
    }
    Ok(current)
}

pub fn hold_direct_rustc_prepared(
    discovery: RustcDiscovery,
    output: &[u8],
) -> Result<DirectRustc, Error> {
    let RustcDiscovery {
        executable,
        mut resolver,
    } = discovery;
    drop(executable);
    let sysroot = held_sysroot_from_output(&mut resolver, output)?;
    let bin = open_directory_at(sysroot.file.as_raw_fd(), c"bin")?;
    let executable = hold_executable_cstr(&bin, c"rustc")?;
    Ok(DirectRustc {
        executable,
        sysroot,
        recheck_resolver: Some(resolver),
    })
}

pub fn direct_rustc_output_prepared(
    direct: &DirectRustc,
    cwd: &Directory,
    prepared: PreparedSysrootInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    version_prepared(&direct.executable, cwd, prepared.0, process_arena)
}

pub fn direct_rustc_version_prepared(
    direct: &DirectRustc,
    cwd: &Directory,
    prepared: PreparedRustcVersionInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    recheck_directory(&direct.sysroot)?;
    version_prepared(&direct.executable, cwd, prepared.0, process_arena)
}

pub fn direct_rustc_reproduces_sysroot(
    direct: &mut DirectRustc,
    output: &[u8],
) -> Result<(), Error> {
    let mut prepared = direct.recheck_resolver.take().ok_or(Error::Invalid)?;
    let rebound = held_sysroot_from_output(&mut prepared, output)?;
    if rebound.dev != direct.sysroot.dev
        || rebound.ino != direct.sysroot.ino
        || rebound.mode != direct.sysroot.mode
        || cfg!(target_os = "macos") && {
            #[cfg(target_os = "macos")]
            {
                rebound.generation != direct.sysroot.generation
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        }
    {
        return Err(Error::Changed);
    }
    recheck_executable(&direct.executable)?;
    recheck_directory(&direct.sysroot)
}

pub fn hold_executable(directory: &Directory, name: &OsStr) -> Result<Executable, Error> {
    let file = hold_regular_file(directory, name)?;
    if file.mode & 0o111 == 0 {
        return Err(Error::Invalid);
    }
    let (slice_offset, slice_size) = executable_slice(&file)?;
    Ok(Executable {
        file,
        slice_offset,
        slice_size,
        #[cfg(target_os = "macos")]
        launch_path: None,
    })
}

pub fn executable_regular_file(executable: &Executable) -> Result<RegularFile, Error> {
    recheck_executable(executable)?;
    Ok(RegularFile {
        file: executable
            .file
            .file
            .try_clone()
            .map_err(|_| Error::Changed)?,
        dev: executable.file.dev,
        ino: executable.file.ino,
        mode: executable.file.mode,
        len: executable.file.len,
        digest: executable.file.digest,
        #[cfg(target_os = "macos")]
        generation: executable.file.generation,
    })
}

fn recheck_executable(executable: &Executable) -> Result<(), Error> {
    recheck_regular(&executable.file)?;
    if executable_slice(&executable.file)? != (executable.slice_offset, executable.slice_size) {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn recheck_executable_launch_path(executable: &Executable) -> Result<(), Error> {
    let Some(path) = executable.launch_path.as_deref() else {
        return Ok(());
    };
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::Changed);
    }
    let rebound = authenticate_regular_file(unsafe { File::from_raw_fd(fd) })?;
    if rebound.dev != executable.file.dev
        || rebound.ino != executable.file.ino
        || rebound.mode != executable.file.mode
        || rebound.len != executable.file.len
        || rebound.digest != executable.file.digest
        || rebound.generation != executable.file.generation
    {
        return Err(Error::Changed);
    }
    Ok(())
}

pub fn recheck_regular(file: &RegularFile) -> Result<(), Error> {
    let metadata = file.file.metadata().map_err(|_| Error::Changed)?;
    if identity(&metadata) != (file.dev, file.ino)
        || metadata.mode() != file.mode
        || metadata.len() != file.len
        || !metadata.is_file()
        || digest_file(&file.file, file.len)? != file.digest
    {
        return Err(Error::Changed);
    }
    #[cfg(target_os = "macos")]
    if metadata_generation(&metadata) != file.generation {
        return Err(Error::Changed);
    }
    Ok(())
}

pub fn read_exact(file: &RegularFile, maximum: usize) -> Result<Vec<u8>, Error> {
    recheck_regular(file)?;
    let length = usize::try_from(file.len).map_err(|_| Error::OutputLimit)?;
    if length > maximum {
        return Err(Error::OutputLimit);
    }
    let mut bytes = vec![0; length];
    let mut offset = 0;
    while offset < length {
        let count = file
            .file
            .read_at(&mut bytes[offset..], offset as u64)
            .map_err(|_| Error::Changed)?;
        if count == 0 {
            return Err(Error::Changed);
        }
        offset += count;
    }
    recheck_regular(file)?;
    Ok(bytes)
}

pub fn compare_exact(
    file: &RegularFile,
    expected: &[u8],
    scratch: &mut [u8; 8192],
) -> Result<bool, Error> {
    recheck_regular(file)?;
    if usize::try_from(file.len).map_err(|_| Error::OutputLimit)? != expected.len() {
        return Ok(false);
    }
    let mut offset = 0usize;
    while offset < expected.len() {
        let chunk = (expected.len() - offset).min(scratch.len());
        let count = file
            .file
            .read_at(
                &mut scratch[..chunk],
                u64::try_from(offset).map_err(|_| Error::OutputLimit)?,
            )
            .map_err(|_| Error::Changed)?;
        if count == 0 || scratch[..count] != expected[offset..offset + count] {
            return Ok(false);
        }
        offset = offset.checked_add(count).ok_or(Error::OutputLimit)?;
    }
    recheck_regular(file)?;
    Ok(true)
}

pub fn link_or_copy_new_prepared<const N: usize>(
    prepared: PreparedLinkOrCopy,
    source: &RegularFile,
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    destination_index: usize,
    source_bytes: &[u8],
) -> Result<RegularFile, Error> {
    if prepared.destination_index != destination_index {
        return Err(Error::Invalid);
    }
    let expected = prepared_discard_name(names, destination_index)?;
    if expected.0.as_bytes() != prepared.destination.0.as_bytes() {
        return Err(Error::Invalid);
    }
    let name = &prepared.destination;
    if usize::try_from(source.len).map_err(|_| Error::OutputLimit)? != source_bytes.len()
        || digest_bytes(source_bytes) != source.digest
    {
        return Err(Error::Changed);
    }
    recheck_regular(source)?;
    recheck_directory(directory)?;
    #[cfg(debug_assertions)]
    let fail_before_authentication = prepared.fail_before_authentication;
    #[cfg(not(debug_assertions))]
    let fail_before_authentication = false;
    #[cfg(target_os = "macos")]
    {
        copy_regular_file_new_prepared(
            source,
            directory,
            name,
            source_bytes,
            fail_before_authentication,
        )
    }
    #[cfg(target_os = "linux")]
    {
        let result = unsafe {
            libc::linkat(
                source.file.as_raw_fd(),
                c"".as_ptr(),
                directory.file.as_raw_fd(),
                name.0.as_ptr(),
                libc::AT_EMPTY_PATH,
            )
        };
        if result == 0 {
            #[cfg(debug_assertions)]
            if fail_before_authentication {
                return Err(Error::Changed);
            }
            let destination = hold_regular_file_name_prepared(directory, name)?;
            if destination.dev != source.dev
                || destination.ino != source.ino
                || destination.mode != source.mode
                || destination.len != source.len
                || destination.digest != source.digest
            {
                return Err(Error::Changed);
            }
            return Ok(destination);
        }
        let errno = std::io::Error::last_os_error()
            .raw_os_error()
            .ok_or(Error::Changed)?;
        if errno == libc::EEXIST {
            return Err(Error::Exists);
        }
        if ![
            libc::EPERM,
            libc::EACCES,
            libc::EOPNOTSUPP,
            libc::ENOSYS,
            libc::EINVAL,
            libc::ENOENT,
        ]
        .contains(&errno)
        {
            return Err(Error::Changed);
        }
        copy_regular_file_new_prepared(
            source,
            directory,
            name,
            source_bytes,
            fail_before_authentication,
        )
    }
}

fn copy_regular_file_new_prepared(
    source: &RegularFile,
    directory: &Directory,
    name: &PreparedRelativeName,
    source_bytes: &[u8],
    fail_before_authentication: bool,
) -> Result<RegularFile, Error> {
    let fd = unsafe {
        libc::openat(
            directory.file.as_raw_fd(),
            name.0.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            source.mode & 0o777,
        )
    };
    if fd < 0 {
        return Err(
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EEXIST) {
                Error::Exists
            } else {
                Error::Changed
            },
        );
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(source_bytes).map_err(|_| Error::Changed)?;
    file.sync_data().map_err(|_| Error::Changed)?;
    #[cfg(not(debug_assertions))]
    let _ = fail_before_authentication;
    #[cfg(debug_assertions)]
    if fail_before_authentication {
        return Err(Error::Changed);
    }
    let destination = authenticate_regular_file(file)?;
    if destination.len != source.len
        || destination.digest != source.digest
        || destination.mode & 0o777 != source.mode & 0o777
    {
        return Err(Error::Changed);
    }
    Ok(destination)
}

fn admit_inventory_entry<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
    files: &[Option<&RegularFile>; N],
    seen: &mut [bool; N],
    count: &mut usize,
    actual: &[u8],
    inode: u64,
) -> Result<(), Error> {
    if actual == b"." || actual == b".." {
        return Ok(());
    }
    let Some(index) = prepared
        .names
        .iter()
        .position(|expected| expected.as_ref().expect("prepared name").0.as_bytes() == actual)
    else {
        return Err(Error::Changed);
    };
    if seen[index] || inode != files[index].expect("attached").ino {
        return Err(Error::Changed);
    }
    seen[index] = true;
    *count = count.checked_add(1).ok_or(Error::OutputLimit)?;
    if *count > N {
        return Err(Error::Changed);
    }
    Ok(())
}

fn prepared_directory_identity(directory: &Directory) -> PreparedDirectoryIdentity {
    PreparedDirectoryIdentity {
        dev: directory.dev,
        ino: directory.ino,
        mode: directory.mode,
        #[cfg(target_os = "macos")]
        generation: directory.generation,
    }
}

struct ObservedRegularIdentity {
    dev: u64,
    ino: u64,
    mode: u32,
    len: u64,
    digest: [u8; 32],
    #[cfg(target_os = "macos")]
    generation: u32,
}

fn same_regular_identity(left: &ObservedRegularIdentity, right: &RegularFile) -> bool {
    let same = left.dev == right.dev
        && left.ino == right.ino
        && left.mode == right.mode
        && left.len == right.len
        && left.digest == right.digest;
    #[cfg(target_os = "macos")]
    {
        same && left.generation == right.generation
    }
    #[cfg(not(target_os = "macos"))]
    {
        same
    }
}

fn must_close_inventory_descriptor(descriptor: RawFd, inject_failure: bool) {
    let failed = unsafe { libc::close(descriptor) } != 0;
    if failed || inject_failure {
        std::process::abort();
    }
}

fn observe_inventory_rebound(
    directory: &Directory,
    name: &PreparedRelativeName,
    fail_authentication: bool,
    fail_close: bool,
) -> Result<ObservedRegularIdentity, Error> {
    let descriptor = unsafe {
        libc::openat(
            directory.file.as_raw_fd(),
            name.0.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        return Err(Error::Changed);
    }
    let file = std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(descriptor) });
    let observed = (|| {
        if fail_authentication {
            return Err(Error::Changed);
        }
        let metadata = file.metadata().map_err(|_| Error::Changed)?;
        if !metadata.is_file() {
            return Err(Error::Changed);
        }
        let (dev, ino) = identity(&metadata);
        Ok(ObservedRegularIdentity {
            dev,
            ino,
            mode: metadata.mode(),
            len: metadata.len(),
            digest: digest_file(&file, metadata.len())?,
            #[cfg(target_os = "macos")]
            generation: metadata_generation(&metadata),
        })
    })();
    must_close_inventory_descriptor(descriptor, fail_close);
    observed
}

#[cfg(target_os = "linux")]
fn parse_linux_inventory_records(
    bytes: &[u8],
    mut admit: impl FnMut(&[u8], u64) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut offset = 0usize;
    while offset < bytes.len() {
        let header_end = offset.checked_add(19).ok_or(Error::Changed)?;
        if header_end > bytes.len() {
            return Err(Error::Changed);
        }
        let inode = unsafe { std::ptr::read_unaligned(bytes.as_ptr().add(offset).cast::<u64>()) };
        let record = usize::from(unsafe {
            std::ptr::read_unaligned(bytes.as_ptr().add(offset + 16).cast::<u16>())
        });
        let next = offset.checked_add(record).ok_or(Error::Changed)?;
        if record < 20
            || record % std::mem::align_of::<u64>() != 0
            || next <= offset
            || next > bytes.len()
        {
            return Err(Error::Changed);
        }
        let name = &bytes[header_end..next];
        let nul = name
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(Error::Changed)?;
        if nul == 0 || inode == 0 {
            return Err(Error::Changed);
        }
        admit(&name[..nul], inode)?;
        offset = next;
    }
    if offset != bytes.len() {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn test_parse_inventory_records(
    bytes: &[u8],
    expected: &[(&[u8], u64)],
) -> Result<(), Error> {
    if expected.len() > 16 {
        return Err(Error::Invalid);
    }
    let mut seen = [false; 16];
    parse_linux_inventory_records(bytes, |name, inode| {
        let Some(index) = expected.iter().position(|(expected_name, expected_inode)| {
            *expected_name == name && *expected_inode == inode
        }) else {
            return Err(Error::Changed);
        };
        if seen[index] {
            return Err(Error::Changed);
        }
        seen[index] = true;
        Ok(())
    })?;
    if seen[..expected.len()].iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn scan_prepared_directory<const N: usize>(
    prepared: &mut PreparedInventoryExact<N>,
    directory: &Directory,
    files: &[Option<&RegularFile>; N],
) -> Result<(), Error> {
    #[cfg(test)]
    {
        prepared.scan_entries = prepared.scan_entries.saturating_add(1);
    }
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut raw_records = 0usize;
    let mut queries = 0usize;
    let mut saw_dot = false;
    let mut saw_dot_dot = false;
    let maximum_records = N.checked_add(2).ok_or(Error::OutputLimit)?;
    let maximum_queries = N.checked_add(3).ok_or(Error::OutputLimit)?;
    let capacity = prepared
        .storage
        .len()
        .checked_mul(std::mem::size_of::<u64>())
        .ok_or(Error::OutputLimit)?;
    let bytes_limit = libc::c_uint::try_from(capacity).map_err(|_| Error::OutputLimit)?;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > maximum_queries {
            return Err(Error::Changed);
        }
        prepared.storage.fill(u64::MAX);
        let read = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                directory.file.as_raw_fd(),
                prepared.storage.as_mut_ptr().cast::<u8>(),
                bytes_limit,
            )
        };
        if read < 0 {
            return Err(Error::Changed);
        }
        let used = usize::try_from(libc::c_uint::try_from(read).map_err(|_| Error::Changed)?)
            .map_err(|_| Error::Changed)?;
        if used == 0 {
            break;
        }
        if used > capacity {
            return Err(Error::Changed);
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(prepared.storage.as_ptr().cast::<u8>(), used) };
        parse_linux_inventory_records(bytes, |name, inode| {
            raw_records = raw_records.checked_add(1).ok_or(Error::OutputLimit)?;
            if raw_records > maximum_records {
                return Err(Error::Changed);
            }
            if name == b"." {
                if saw_dot {
                    return Err(Error::Changed);
                }
                saw_dot = true;
            } else if name == b".." {
                if saw_dot_dot {
                    return Err(Error::Changed);
                }
                saw_dot_dot = true;
            }
            admit_inventory_entry(prepared, files, &mut seen, &mut count, name, inode)
        })?;
    }
    if count != N || seen.iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn parse_darwin_inventory_records(
    bytes: &[u8],
    mut admit: impl FnMut(&[u8], u64) -> Result<(), Error>,
) -> Result<(), Error> {
    let header = std::mem::offset_of!(libc::dirent, d_name);
    let mut offset = 0usize;
    while offset < bytes.len() {
        let header_end = offset.checked_add(header).ok_or(Error::Changed)?;
        if header_end > bytes.len() {
            return Err(Error::Changed);
        }
        let entry = unsafe { bytes.as_ptr().add(offset).cast::<libc::dirent>() };
        let inode = unsafe { std::ptr::addr_of!((*entry).d_ino).read_unaligned() };
        let record = usize::from(unsafe { std::ptr::addr_of!((*entry).d_reclen).read_unaligned() });
        let name_length =
            usize::from(unsafe { std::ptr::addr_of!((*entry).d_namlen).read_unaligned() });
        let name_end = header_end.checked_add(name_length).ok_or(Error::Changed)?;
        let next = offset.checked_add(record).ok_or(Error::Changed)?;
        if record < header + 1
            || !record.is_multiple_of(4)
            || name_length > 1023
            || name_end >= next
            || next <= offset
            || next > bytes.len()
        {
            return Err(Error::Changed);
        }
        let name = &bytes[header_end..name_end];
        if bytes[name_end] != 0 || name.contains(&0) {
            return Err(Error::Changed);
        }
        if inode == 0 {
            offset = next;
            continue;
        }
        if name.is_empty() {
            return Err(Error::Changed);
        }
        admit(name, inode)?;
        offset = next;
    }
    if offset != bytes.len() {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn test_parse_inventory_records(
    bytes: &[u8],
    expected: &[(&[u8], u64)],
) -> Result<(), Error> {
    if expected.len() > 16 {
        return Err(Error::Invalid);
    }
    let mut seen = [false; 16];
    parse_darwin_inventory_records(bytes, |name, inode| {
        let Some(index) = expected.iter().position(|(expected_name, expected_inode)| {
            *expected_name == name && *expected_inode == inode
        }) else {
            return Err(Error::Changed);
        };
        if seen[index] {
            return Err(Error::Changed);
        }
        seen[index] = true;
        Ok(())
    })?;
    if seen[..expected.len()].iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn scan_prepared_directory<const N: usize>(
    prepared: &mut PreparedInventoryExact<N>,
    directory: &Directory,
    files: &[Option<&RegularFile>; N],
) -> Result<(), Error> {
    #[cfg(test)]
    {
        prepared.scan_entries = prepared.scan_entries.saturating_add(1);
    }
    const SYS_GETDIRENTRIES64: libc::c_int = 344;
    const _: () = assert!(std::mem::size_of::<libc::off_t>() == 8);
    const _: () = assert!(std::mem::offset_of!(libc::dirent, d_ino) == 0);
    const _: () = assert!(std::mem::offset_of!(libc::dirent, d_reclen) == 16);
    const _: () = assert!(std::mem::offset_of!(libc::dirent, d_namlen) == 18);
    const _: () = assert!(std::mem::offset_of!(libc::dirent, d_name) == 21);
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut raw_records = 0usize;
    let mut queries = 0usize;
    let mut saw_dot = false;
    let mut saw_dot_dot = false;
    let maximum_records = N.checked_add(2).ok_or(Error::OutputLimit)?;
    let maximum_queries = N.checked_add(3).ok_or(Error::OutputLimit)?;
    let capacity = prepared
        .storage
        .len()
        .checked_mul(std::mem::size_of::<u64>())
        .ok_or(Error::OutputLimit)?;
    let bytes_limit: libc::size_t = capacity;
    let mut base: libc::off_t = 0;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > maximum_queries {
            return Err(Error::Changed);
        }
        prepared.storage.fill(u64::MAX);
        let read = unsafe {
            libc::syscall(
                SYS_GETDIRENTRIES64,
                directory.file.as_raw_fd(),
                prepared.storage.as_mut_ptr().cast::<libc::c_char>(),
                bytes_limit,
                &mut base,
            )
        };
        if read < 0 {
            return Err(Error::Changed);
        }
        let used = usize::try_from(read).map_err(|_| Error::Changed)?;
        if used == 0 {
            break;
        }
        if used > capacity {
            return Err(Error::Changed);
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(prepared.storage.as_ptr().cast::<u8>(), used) };
        parse_darwin_inventory_records(bytes, |name, inode| {
            raw_records = raw_records.checked_add(1).ok_or(Error::OutputLimit)?;
            if raw_records > maximum_records {
                return Err(Error::Changed);
            }
            if name == b"." {
                if saw_dot {
                    return Err(Error::Changed);
                }
                saw_dot = true;
            } else if name == b".." {
                if saw_dot_dot {
                    return Err(Error::Changed);
                }
                saw_dot_dot = true;
            }
            admit_inventory_entry(prepared, files, &mut seen, &mut count, name, inode)
        })?;
    }
    if count != N || seen.iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

fn admit_inventory_typed_entry<const N: usize, const F: usize, const D: usize>(
    prepared: &PreparedInventoryEntriesExact<N>,
    files: &[&RegularFile; F],
    directories: &[&Directory; D],
    seen: &mut [bool; N],
    count: &mut usize,
    actual: &[u8],
    inode: u64,
) -> Result<(), Error> {
    if actual == b"." || actual == b".." {
        return Ok(());
    }
    let Some(index) = prepared
        .names
        .names
        .iter()
        .position(|expected| expected.as_ref().expect("prepared name").0.as_bytes() == actual)
    else {
        return Err(Error::Changed);
    };
    let expected_inode = if index < F {
        files[index].ino
    } else {
        directories[index.checked_sub(F).ok_or(Error::Changed)?].ino
    };
    if seen[index] || inode != expected_inode {
        return Err(Error::Changed);
    }
    seen[index] = true;
    *count = count.checked_add(1).ok_or(Error::OutputLimit)?;
    if *count > N {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn scan_prepared_entries_directory<const N: usize, const F: usize, const D: usize>(
    prepared: &mut PreparedInventoryEntriesExact<N>,
    directory: &Directory,
    files: &[&RegularFile; F],
    directories: &[&Directory; D],
) -> Result<(), Error> {
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut raw_records = 0usize;
    let mut queries = 0usize;
    let maximum_records = N.checked_add(2).ok_or(Error::OutputLimit)?;
    let maximum_queries = N.checked_add(3).ok_or(Error::OutputLimit)?;
    let capacity = prepared
        .storage
        .len()
        .checked_mul(std::mem::size_of::<u64>())
        .ok_or(Error::OutputLimit)?;
    let bytes_limit = libc::c_uint::try_from(capacity).map_err(|_| Error::OutputLimit)?;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > maximum_queries {
            return Err(Error::Changed);
        }
        prepared.storage.fill(u64::MAX);
        let read = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                directory.file.as_raw_fd(),
                prepared.storage.as_mut_ptr().cast::<u8>(),
                bytes_limit,
            )
        };
        if read < 0 {
            return Err(Error::Changed);
        }
        let used = usize::try_from(libc::c_uint::try_from(read).map_err(|_| Error::Changed)?)
            .map_err(|_| Error::Changed)?;
        if used == 0 {
            break;
        }
        if used > capacity {
            return Err(Error::Changed);
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(prepared.storage.as_ptr().cast::<u8>(), used) };
        parse_linux_inventory_records(bytes, |name, inode| {
            raw_records = raw_records.checked_add(1).ok_or(Error::OutputLimit)?;
            if raw_records > maximum_records {
                return Err(Error::Changed);
            }
            admit_inventory_typed_entry(
                prepared,
                files,
                directories,
                &mut seen,
                &mut count,
                name,
                inode,
            )
        })?;
    }
    if count != N || seen.iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn scan_prepared_entries_directory<const N: usize, const F: usize, const D: usize>(
    prepared: &mut PreparedInventoryEntriesExact<N>,
    directory: &Directory,
    files: &[&RegularFile; F],
    directories: &[&Directory; D],
) -> Result<(), Error> {
    const SYS_GETDIRENTRIES64: libc::c_int = 344;
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut raw_records = 0usize;
    let mut queries = 0usize;
    let maximum_records = N.checked_add(2).ok_or(Error::OutputLimit)?;
    let maximum_queries = N.checked_add(3).ok_or(Error::OutputLimit)?;
    let capacity = prepared
        .storage
        .len()
        .checked_mul(std::mem::size_of::<u64>())
        .ok_or(Error::OutputLimit)?;
    let mut base: libc::off_t = 0;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > maximum_queries {
            return Err(Error::Changed);
        }
        prepared.storage.fill(u64::MAX);
        let read = unsafe {
            libc::syscall(
                SYS_GETDIRENTRIES64,
                directory.file.as_raw_fd(),
                prepared.storage.as_mut_ptr().cast::<libc::c_char>(),
                capacity,
                &mut base,
            )
        };
        if read < 0 {
            return Err(Error::Changed);
        }
        let used = usize::try_from(read).map_err(|_| Error::Changed)?;
        if used == 0 {
            break;
        }
        if used > capacity {
            return Err(Error::Changed);
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(prepared.storage.as_ptr().cast::<u8>(), used) };
        parse_darwin_inventory_records(bytes, |name, inode| {
            raw_records = raw_records.checked_add(1).ok_or(Error::OutputLimit)?;
            if raw_records > maximum_records {
                return Err(Error::Changed);
            }
            admit_inventory_typed_entry(
                prepared,
                files,
                directories,
                &mut seen,
                &mut count,
                name,
                inode,
            )
        })?;
    }
    if count != N || seen.iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

pub fn inventory_entries_exact_prepared<const N: usize, const F: usize, const D: usize>(
    prepared: &mut PreparedInventoryEntriesExact<N>,
    directory: &Directory,
    files: [&RegularFile; F],
    directories: [&Directory; D],
) -> Result<(), Error> {
    if prepared.remaining != 1 || prepared.file_count != F || F.checked_add(D) != Some(N) {
        return Err(Error::Invalid);
    }
    prepared.remaining = 0;
    recheck_directory(directory)?;
    for file in files {
        recheck_regular(file)?;
    }
    for child in directories {
        recheck_directory(child)?;
    }
    if unsafe { libc::lseek(directory.file.as_raw_fd(), 0, libc::SEEK_SET) } < 0 {
        return Err(Error::Changed);
    }
    let scan = scan_prepared_entries_directory(prepared, directory, &files, &directories);
    let reset = unsafe { libc::lseek(directory.file.as_raw_fd(), 0, libc::SEEK_SET) };
    if scan.is_err() || reset < 0 {
        return Err(Error::Changed);
    }
    for (index, file) in files.iter().enumerate() {
        recheck_named_regular(
            directory,
            prepared_discard_name(&prepared.names, index)?,
            file,
        )?;
    }
    for (index, child) in directories.iter().enumerate() {
        let name = prepared_discard_name(
            &prepared.names,
            F.checked_add(index).ok_or(Error::OutputLimit)?,
        )?;
        let rebound = open_directory_at(directory.file.as_raw_fd(), &name.0)?;
        if prepared_directory_identity(&rebound) != prepared_directory_identity(child) {
            return Err(Error::Changed);
        }
    }
    recheck_directory(directory)?;
    for file in files {
        recheck_regular(file)?;
    }
    for child in directories {
        recheck_directory(child)?;
    }
    Ok(())
}

pub fn inventory_exact_prepared<const N: usize>(
    prepared: &mut PreparedInventoryExact<N>,
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    files: [Option<&RegularFile>; N],
) -> Result<(), Error> {
    let current_bindings = prepared_name_bindings(names)?;
    if prepared.remaining == 0
        || prepared.bindings != current_bindings
        || files.iter().any(Option::is_none)
    {
        return Err(Error::Invalid);
    }
    let directory_identity = prepared_directory_identity(directory);
    match prepared.directory_identity {
        Some(first) if first != directory_identity => return Err(Error::Changed),
        None => prepared.directory_identity = Some(directory_identity),
        Some(_) => {}
    }
    prepared.remaining -= 1;
    recheck_directory(directory)?;
    for file in files.iter().flatten() {
        recheck_regular(file)?;
    }
    #[cfg(test)]
    let fail_initial_seek = prepared.fail_initial_seek;
    #[cfg(not(test))]
    let fail_initial_seek = false;
    if fail_initial_seek
        || unsafe { libc::lseek(directory.file.as_raw_fd(), 0, libc::SEEK_SET) } < 0
    {
        return Err(Error::Changed);
    }
    let scan = scan_prepared_directory(prepared, directory, &files);
    #[cfg(test)]
    let fail_reset_seek = prepared.fail_reset_seek;
    #[cfg(not(test))]
    let fail_reset_seek = false;
    let reset = if fail_reset_seek {
        -1
    } else {
        unsafe { libc::lseek(directory.file.as_raw_fd(), 0, libc::SEEK_SET) }
    };
    if scan.is_err() || reset < 0 {
        return Err(Error::Changed);
    }
    for (index, tracked) in files.iter().enumerate() {
        #[cfg(test)]
        let (fail_authentication, fail_close) = (
            prepared.fail_rebound_authentication,
            prepared.fail_rebound_close,
        );
        #[cfg(not(test))]
        let (fail_authentication, fail_close) = (false, false);
        let rebound = observe_inventory_rebound(
            directory,
            prepared.names[index].as_ref().expect("prepared name"),
            fail_authentication,
            fail_close,
        )?;
        if !same_regular_identity(&rebound, tracked.expect("attached")) {
            return Err(Error::Changed);
        }
    }
    recheck_directory(directory)?;
    for file in files.iter().flatten() {
        recheck_regular(file)?;
    }
    Ok(())
}

fn observe_publish_rebound(
    parent: &Directory,
    name: &std::ffi::CStr,
    fail_information: bool,
    fail_close: bool,
) -> Result<PreparedDirectoryIdentity, Error> {
    let descriptor = unsafe {
        libc::openat(
            parent.file.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        return Err(Error::Changed);
    }
    let file = std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(descriptor) });
    let observed = (|| {
        if fail_information {
            return Err(Error::Changed);
        }
        let metadata = file.metadata().map_err(|_| Error::Changed)?;
        if !metadata.is_dir() {
            return Err(Error::Changed);
        }
        let (dev, ino) = identity(&metadata);
        Ok(PreparedDirectoryIdentity {
            dev,
            ino,
            mode: metadata.mode(),
            #[cfg(target_os = "macos")]
            generation: metadata_generation(&metadata),
        })
    })();
    let close_failed = unsafe { libc::close(descriptor) } != 0;
    if close_failed || fail_close {
        std::process::abort();
    }
    observed
}

pub fn publish_directory_new_prepared(
    prepared: &mut PreparedPublishDirectory,
    parent: &Directory,
    stage: &Directory,
    stage_name: &PreparedRelativeNameArena,
    output_name: &OsStr,
) -> Result<(), Error> {
    let stage_name = relative_name_arena_cstr(stage_name)?;
    let output_bytes = validated_c_name_bytes(output_name)?;
    if prepared.remaining != 1
        || prepared.exact_capacity != prepared.destination.as_bytes_with_nul().len()
        || prepared.destination.as_bytes() != output_bytes
    {
        return Err(Error::Invalid);
    }
    prepared.remaining = 0;
    recheck_directory(parent)?;
    recheck_directory(stage)?;
    #[cfg(debug_assertions)]
    if prepared.fail_before_open {
        return Err(Error::Changed);
    }
    #[cfg(debug_assertions)]
    let (fail_information, fail_close) = (prepared.fail_information, prepared.fail_close);
    #[cfg(not(debug_assertions))]
    let (fail_information, fail_close) = (false, false);
    if observe_publish_rebound(parent, stage_name, fail_information, fail_close)?
        != prepared_directory_identity(stage)
    {
        return Err(Error::Changed);
    }
    #[cfg(debug_assertions)]
    if prepared.fail_rename {
        return Err(Error::Changed);
    }
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            parent.file.as_raw_fd(),
            stage_name.as_ptr(),
            parent.file.as_raw_fd(),
            prepared.destination.as_ptr(),
            1_u32,
        )
    } as i32;
    #[cfg(target_os = "macos")]
    let result = unsafe {
        unsafe extern "C" {
            fn renameatx_np(
                fromfd: libc::c_int,
                from: *const libc::c_char,
                tofd: libc::c_int,
                to: *const libc::c_char,
                flags: libc::c_uint,
            ) -> libc::c_int;
        }
        renameatx_np(
            parent.file.as_raw_fd(),
            stage_name.as_ptr(),
            parent.file.as_raw_fd(),
            prepared.destination.as_ptr(),
            0x0000_0004,
        )
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let result = -1;
    if result != 0 {
        let errno = std::io::Error::last_os_error().raw_os_error();
        return Err(
            if errno == Some(libc::EEXIST) || errno == Some(libc::ENOTEMPTY) {
                Error::Exists
            } else {
                Error::Changed
            },
        );
    }
    Ok(())
}

pub fn discard_owned_stage_prepared<const N: usize>(
    parent: &Directory,
    stage: &Directory,
    stage_name: &PreparedRelativeNameArena,
    names: &PreparedDiscardNames<N>,
    files: &[Option<&RegularFile>; N],
    settled: &[Option<&SettledRegularFile>; N],
    #[cfg(debug_assertions)] failure_after_delete: Option<usize>,
) -> Result<(), Error> {
    recheck_directory(parent)?;
    recheck_directory(stage)?;
    let stage_name = relative_name_arena_cstr(stage_name)?;
    let rebound = open_directory_at(parent.file.as_raw_fd(), stage_name)?;
    if (rebound.dev, rebound.ino, rebound.mode) != (stage.dev, stage.ino, stage.mode) {
        return Err(Error::Changed);
    }
    let attached = files
        .iter()
        .zip(settled)
        .take_while(|(file, settled)| file.is_some() ^ settled.is_some())
        .count();
    if files[attached..]
        .iter()
        .zip(&settled[attached..])
        .any(|(file, settled)| file.is_some() || settled.is_some())
    {
        return Err(Error::Invalid);
    }

    let duplicate = stage.file.try_clone().map_err(|_| Error::Changed)?;
    let fd = unsafe { libc::dup(duplicate.as_raw_fd()) };
    if fd < 0 {
        return Err(Error::Changed);
    }
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        unsafe { libc::close(fd) };
        return Err(Error::Changed);
    }
    let scan = (|| {
        unsafe { libc::rewinddir(stream) };
        let mut seen = [false; N];
        let mut count = 0usize;
        loop {
            #[cfg(target_os = "linux")]
            unsafe {
                *libc::__errno_location() = 0;
            }
            #[cfg(target_os = "macos")]
            unsafe {
                *libc::__error() = 0;
            }
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                #[cfg(target_os = "linux")]
                let errno = unsafe { *libc::__errno_location() };
                #[cfg(target_os = "macos")]
                let errno = unsafe { *libc::__error() };
                if errno != 0 {
                    return Err(Error::Changed);
                }
                break;
            }
            let actual = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if actual == b"." || actual == b".." {
                continue;
            }
            let Some(index) = names.names[..attached]
                .iter()
                .position(|expected| expected.as_ref().expect("validated").0.as_bytes() == actual)
            else {
                return Err(Error::Changed);
            };
            if seen[index] {
                return Err(Error::Changed);
            }
            seen[index] = true;
            count = count.checked_add(1).ok_or(Error::OutputLimit)?;
        }
        if count != attached || seen[..attached].iter().any(|seen| !seen) {
            return Err(Error::Changed);
        }
        Ok(())
    })();
    unsafe { libc::closedir(stream) };
    scan?;

    for (index, name) in names.names[..attached].iter().enumerate() {
        let file = files[index]
            .or_else(|| settled[index].map(|file| &file.0))
            .expect("attached prefix");
        recheck_regular(file)?;
        let name = name.as_ref().expect("validated");
        let rebound = hold_regular_file_name_prepared(stage, name)?;
        if (
            rebound.dev,
            rebound.ino,
            rebound.mode,
            rebound.len,
            rebound.digest,
        ) != (file.dev, file.ino, file.mode, file.len, file.digest)
        {
            return Err(Error::Changed);
        }
    }
    for (deleted, name) in names.names[..attached].iter().enumerate() {
        #[cfg(not(debug_assertions))]
        let _ = deleted;
        #[cfg(debug_assertions)]
        if failure_after_delete == Some(deleted) {
            return Err(Error::Changed);
        }
        let name = name.as_ref().expect("validated");
        if unsafe { libc::unlinkat(stage.file.as_raw_fd(), name.0.as_ptr(), 0) } != 0 {
            return Err(Error::Changed);
        }
    }
    #[cfg(debug_assertions)]
    if failure_after_delete == Some(attached) {
        return Err(Error::Changed);
    }
    if unsafe {
        libc::unlinkat(
            parent.file.as_raw_fd(),
            stage_name.as_ptr(),
            libc::AT_REMOVEDIR,
        )
    } != 0
    {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_argv_mode(
    executable: &Executable,
    cwd: &Directory,
    arguments: &[CString],
    stdout_limit: usize,
    output: Vec<u8>,
    process_arena: &mut PreparedProcessArena,
    close_pipe_after_leader: bool,
) -> Result<Vec<u8>, Error> {
    if arguments.len() > 32 || output.capacity() != stdout_limit || !output.is_empty() {
        return Err(Error::Invalid);
    }
    consume_process_arena(process_arena)?;
    recheck_executable(executable)?;
    recheck_directory(cwd)?;
    let mut pipe = [0; 2];
    if unsafe { libc::pipe(pipe.as_mut_ptr()) } != 0 {
        return Err(Error::Spawn);
    }
    let read_pipe = CheckedFd::new(pipe[0]);
    let write_pipe = CheckedFd::new(pipe[1]);
    if injected_settlement_failure!(UnixPipeReadFcntl) {
        return Err(Error::Spawn);
    }
    if unsafe { libc::fcntl(read_pipe.raw(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
        return Err(Error::Spawn);
    }
    if injected_settlement_failure!(UnixPipeWriteFcntl) {
        return Err(Error::Spawn);
    }
    if unsafe { libc::fcntl(write_pipe.raw(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
        return Err(Error::Spawn);
    }
    let dev_null = c"/dev/null";
    let null_fd = unsafe { libc::open(dev_null.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
    if null_fd < 0 {
        return Err(Error::Spawn);
    }
    let null_fd = CheckedFd::new(null_fd);
    let mut argv = [std::ptr::null::<libc::c_char>(); 34];
    for (index, argument) in arguments.iter().enumerate() {
        argv[index + 1] = argument.as_ptr();
    }
    let env = [std::ptr::null::<libc::c_char>()];
    let mut argv0 = [0_u8; 32_770];
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(Error::Spawn);
    }
    if pid == 0 {
        unsafe {
            if libc::close(read_pipe.raw()) != 0 {
                libc::_exit(126);
            }
            if libc::setpgid(0, 0) != 0 {
                libc::_exit(126);
            }
            let executable_fd =
                libc::fcntl(executable.file.file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3);
            if executable_fd < 0 {
                libc::_exit(126);
            }
            if libc::fchdir(cwd.file.as_raw_fd()) != 0
                || libc::dup2(null_fd.raw(), libc::STDIN_FILENO) < 0
                || libc::dup2(write_pipe.raw(), libc::STDOUT_FILENO) < 0
                || libc::dup2(null_fd.raw(), libc::STDERR_FILENO) < 0
            {
                libc::_exit(126);
            }
            if libc::fcntl(libc::STDIN_FILENO, libc::F_SETFD, 0) != 0
                || libc::fcntl(libc::STDOUT_FILENO, libc::F_SETFD, 0) != 0
                || libc::fcntl(libc::STDERR_FILENO, libc::F_SETFD, 0) != 0
            {
                libc::_exit(126);
            }
            if (write_pipe.raw() > libc::STDERR_FILENO && libc::close(write_pipe.raw()) != 0)
                || (null_fd.raw() > libc::STDERR_FILENO && libc::close(null_fd.raw()) != 0)
            {
                libc::_exit(126);
            }
            if executable_fd > 2 {
                if executable_fd > 3
                    && libc::syscall(
                        libc::SYS_close_range,
                        3_u32,
                        executable_fd.saturating_sub(1) as u32,
                        0_u32,
                    ) != 0
                {
                    libc::_exit(126);
                }
                if executable_fd < i32::MAX as libc::c_int
                    && libc::syscall(
                        libc::SYS_close_range,
                        (executable_fd + 1) as u32,
                        u32::MAX,
                        0_u32,
                    ) != 0
                {
                    libc::_exit(126);
                }
            }
            let mut executable_fd_path = [0_u8; 64];
            let executable_fd_format = b"/proc/self/fd/%d\0";
            let formatted = libc::snprintf(
                executable_fd_path.as_mut_ptr().cast(),
                executable_fd_path.len(),
                executable_fd_format.as_ptr().cast(),
                executable_fd,
            );
            if formatted <= 0 || formatted as usize >= executable_fd_path.len() {
                libc::_exit(126);
            }
            let argv0_length = libc::readlink(
                executable_fd_path.as_ptr().cast(),
                argv0.as_mut_ptr().cast(),
                argv0.len() - 1,
            );
            if argv0_length <= 0 {
                libc::_exit(126);
            }
            let argv0_length = argv0_length as usize;
            if argv0_length >= argv0.len() - 1 {
                libc::_exit(126);
            }
            argv0[argv0_length] = 0;
            argv[0] = argv0.as_ptr().cast();
            unsafe extern "C" {
                fn fexecve(
                    fd: libc::c_int,
                    argv: *const *const libc::c_char,
                    envp: *const *const libc::c_char,
                ) -> libc::c_int;
            }
            fexecve(executable_fd, argv.as_ptr(), env.as_ptr());
            libc::_exit(127);
        }
    }
    let _ = unsafe { libc::setpgid(pid, pid) };
    let write_close = write_pipe.close_injected(TestClosePoint::ParentWrite);
    let null_close = null_fd.close_injected(TestClosePoint::ParentNull);
    if write_close.is_err() || null_close.is_err() {
        must_settle_failed_group(pid, read_pipe, false);
        std::process::abort();
    }
    let (output, status) = drain_and_wait(
        pid,
        read_pipe,
        stdout_limit,
        output,
        close_pipe_after_leader,
    )?;
    if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
        #[cfg(test)]
        eprintln!("linux platform child status={status} args={arguments:?}");
        return Err(Error::Exit);
    }
    recheck_executable(executable)?;
    recheck_directory(cwd)?;
    Ok(output)
}

#[cfg(target_os = "linux")]
fn run_argv(
    executable: &Executable,
    cwd: &Directory,
    arguments: &[CString],
    stdout_limit: usize,
    output: Vec<u8>,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    run_argv_mode(
        executable,
        cwd,
        arguments,
        stdout_limit,
        output,
        process_arena,
        false,
    )
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum DarwinSpawnProfile {
    SuspendedHeld,
    InstalledArchive,
}

#[cfg(target_os = "macos")]
fn run_argv(
    executable: &Executable,
    cwd: &Directory,
    arguments: &[CString],
    stdout_limit: usize,
    output: Vec<u8>,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    run_argv_mode(
        executable,
        cwd,
        arguments,
        stdout_limit,
        output,
        process_arena,
        DarwinSpawnProfile::SuspendedHeld,
    )
}

#[cfg(target_os = "macos")]
fn run_archive_argv(
    executable: &Executable,
    cwd: &Directory,
    arguments: &[CString],
    stdout_limit: usize,
    output: Vec<u8>,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    run_argv_mode(
        executable,
        cwd,
        arguments,
        stdout_limit,
        output,
        process_arena,
        DarwinSpawnProfile::InstalledArchive,
    )
}

#[cfg(target_os = "linux")]
fn run_archive_argv(
    executable: &Executable,
    cwd: &Directory,
    arguments: &[CString],
    stdout_limit: usize,
    output: Vec<u8>,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    if stdout_limit != 0 {
        return Err(Error::Invalid);
    }
    run_argv_mode(
        executable,
        cwd,
        arguments,
        stdout_limit,
        output,
        process_arena,
        true,
    )
}

#[cfg(target_os = "macos")]
fn run_argv_mode(
    executable: &Executable,
    cwd: &Directory,
    arguments: &[CString],
    stdout_limit: usize,
    output: Vec<u8>,
    process_arena: &mut PreparedProcessArena,
    profile: DarwinSpawnProfile,
) -> Result<Vec<u8>, Error> {
    if arguments.len() > 32 || output.capacity() != stdout_limit || !output.is_empty() {
        return Err(Error::Invalid);
    }
    consume_process_arena(process_arena)?;
    #[repr(C)]
    struct RegionInfo {
        protection: u32,
        max_protection: u32,
        inheritance: u32,
        flags: u32,
        offset: u64,
        behavior: u32,
        user_wired: u32,
        tag: u32,
        resident: u32,
        shared_private: u32,
        swapped: u32,
        dirtied: u32,
        refs: u32,
        shadow: u32,
        share_mode: u32,
        private_resident: u32,
        shared_resident: u32,
        object: u32,
        depth: u32,
        address: u64,
        size: u64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct VnodeStat {
        dev: u32,
        mode: u16,
        nlink: u16,
        ino: u64,
        uid: u32,
        gid: u32,
        atime: i64,
        atime_ns: i64,
        mtime: i64,
        mtime_ns: i64,
        ctime: i64,
        ctime_ns: i64,
        birth: i64,
        birth_ns: i64,
        size: i64,
        blocks: i64,
        block_size: i32,
        flags: u32,
        generation: u32,
        rdev: u32,
        spare: [i64; 2],
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct VnodeInfo {
        stat: VnodeStat,
        kind: i32,
        pad: i32,
        fsid: [i32; 2],
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct VnodePath {
        info: VnodeInfo,
        path: [libc::c_char; 1024],
    }
    #[repr(C)]
    struct RegionPath {
        region: RegionInfo,
        vnode: VnodePath,
    }
    #[repr(C)]
    struct VnodePaths {
        cwd: VnodePath,
        root: VnodePath,
    }
    unsafe extern "C" {
        fn posix_spawn_file_actions_addfchdir_np(
            actions: *mut libc::posix_spawn_file_actions_t,
            fd: libc::c_int,
        ) -> libc::c_int;
    }
    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_pidinfo(
            pid: libc::c_int,
            flavor: libc::c_int,
            arg: u64,
            buffer: *mut libc::c_void,
            size: libc::c_int,
        ) -> libc::c_int;
    }
    recheck_executable(executable)?;
    recheck_executable_launch_path(executable)?;
    recheck_directory(cwd)?;
    let mut path = [0_u8; 1024];
    let executable_path = if let Some(launch_path) = executable.launch_path.as_deref() {
        launch_path
    } else {
        if unsafe { libc::fcntl(executable.file.file.as_raw_fd(), 50, path.as_mut_ptr()) } != 0 {
            return Err(Error::Changed);
        }
        unsafe { std::ffi::CStr::from_ptr(path.as_ptr().cast::<libc::c_char>()) }
    };
    let mut pipe = [0; 2];
    if unsafe { libc::pipe(pipe.as_mut_ptr()) } != 0 {
        return Err(Error::Spawn);
    }
    let read_pipe = CheckedFd::new(pipe[0]);
    let write_pipe = CheckedFd::new(pipe[1]);
    if injected_settlement_failure!(UnixPipeReadFcntl) {
        return Err(Error::Spawn);
    }
    if unsafe { libc::fcntl(read_pipe.raw(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
        return Err(Error::Spawn);
    }
    if injected_settlement_failure!(UnixPipeWriteFcntl) {
        return Err(Error::Spawn);
    }
    if unsafe { libc::fcntl(write_pipe.raw(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
        return Err(Error::Spawn);
    }
    let null_path = c"/dev/null";
    let null_fd = unsafe { libc::open(null_path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
    if null_fd < 0 {
        return Err(Error::Spawn);
    }
    let null_fd = CheckedFd::new(null_fd);
    let mut actions = std::ptr::null_mut();
    let mut attributes = std::ptr::null_mut();
    let mut pid = 0;
    let mut argv = [std::ptr::null_mut::<libc::c_char>(); 34];
    // Some admitted installed tools (notably Apple libtool) dispatch or
    // locate companion behavior from argv[0]. Bind it to the exact held
    // image path already vnode-attested for this suspended child.
    argv[0] = executable_path.as_ptr().cast_mut();
    for (index, argument) in arguments.iter().enumerate() {
        argv[index + 1] = argument.as_ptr().cast_mut();
    }
    let env = [
        match profile {
            DarwinSpawnProfile::SuspendedHeld => std::ptr::null(),
            DarwinSpawnProfile::InstalledArchive => c"TMPDIR=archive-tmp".as_ptr(),
        }
        .cast_mut(),
        std::ptr::null_mut::<libc::c_char>(),
    ];
    let flags = libc::POSIX_SPAWN_CLOEXEC_DEFAULT
        | libc::POSIX_SPAWN_SETPGROUP
        | libc::POSIX_SPAWN_START_SUSPENDED;
    let flags = libc::c_short::try_from(flags).map_err(|_| Error::Unsupported)?;
    recheck_executable_launch_path(executable)?;
    let spawn = unsafe {
        let init = libc::posix_spawn_file_actions_init(&mut actions);
        let attr_init = libc::posix_spawnattr_init(&mut attributes);
        let actions_initialized = init == 0;
        let attributes_initialized = attr_init == 0;
        let configured = actions_initialized
            && attributes_initialized
            && libc::posix_spawnattr_setflags(&mut attributes, flags) == 0
            && libc::posix_spawnattr_setpgroup(&mut attributes, 0) == 0
            && posix_spawn_file_actions_addfchdir_np(&mut actions, cwd.file.as_raw_fd()) == 0
            && libc::posix_spawn_file_actions_adddup2(&mut actions, null_fd.raw(), 0) == 0
            && libc::posix_spawn_file_actions_adddup2(&mut actions, write_pipe.raw(), 1) == 0
            && libc::posix_spawn_file_actions_adddup2(&mut actions, null_fd.raw(), 2) == 0;
        let result = if configured {
            libc::posix_spawn(
                &mut pid,
                executable_path.as_ptr(),
                &actions,
                &attributes,
                argv.as_ptr(),
                env.as_ptr(),
            )
        } else {
            libc::EINVAL
        };
        let actions_destroyed = !actions_initialized
            || (libc::posix_spawn_file_actions_destroy(&mut actions) == 0
                && !injected_settlement_failure!(DarwinActionsDestroy));
        let attributes_destroyed = !attributes_initialized
            || (libc::posix_spawnattr_destroy(&mut attributes) == 0
                && !injected_settlement_failure!(DarwinAttributesDestroy));
        (result, actions_destroyed && attributes_destroyed)
    };
    let write_close = write_pipe.close_injected(TestClosePoint::ParentWrite);
    let null_close = null_fd.close_injected(TestClosePoint::ParentNull);
    if !spawn.1 || write_close.is_err() || null_close.is_err() {
        if spawn.0 == 0 {
            must_settle_failed_group(pid, read_pipe, false);
        } else if read_pipe.close().is_err() {
            std::process::abort();
        }
        std::process::abort();
    }
    if spawn.0 != 0 {
        return Err(Error::Spawn);
    }
    let attest = (|| {
        if injected_settlement_failure!(DarwinAttest) {
            return Err(Error::Changed);
        }
        let mut cwd_info = std::mem::MaybeUninit::<VnodePaths>::zeroed();
        let cwd_size =
            libc::c_int::try_from(std::mem::size_of::<VnodePaths>()).map_err(|_| Error::Changed)?;
        let cwd_returned =
            unsafe { proc_pidinfo(pid, 9, 0, cwd_info.as_mut_ptr().cast(), cwd_size) };
        if cwd_returned != cwd_size {
            return Err(Error::Changed);
        }
        let cwd_info = unsafe { cwd_info.assume_init() };
        if u64::from(cwd_info.cwd.info.stat.dev) != cwd.dev
            || cwd_info.cwd.info.stat.ino != cwd.ino
            || u32::from(cwd_info.cwd.info.stat.mode) != cwd.mode
            || cwd_info.cwd.info.stat.generation != cwd.generation
            || cwd_info.cwd.info.kind != 2
        {
            return Err(Error::Changed);
        }
        let mut address = 0_u64;
        let mut matching = 0_u32;
        let mut enumerated = 0_u32;
        let mut terminal = false;
        let mut previous_end = None;
        for _ in 0..4096 {
            if previous_end.is_some_and(|end| end != address) {
                return Err(Error::Changed);
            }
            let mut info = std::mem::MaybeUninit::<RegionPath>::zeroed();
            let size = libc::c_int::try_from(std::mem::size_of::<RegionPath>())
                .map_err(|_| Error::Changed)?;
            unsafe {
                *libc::__error() = 0;
            }
            let returned = unsafe { proc_pidinfo(pid, 8, address, info.as_mut_ptr().cast(), size) };
            let query_errno = unsafe { *libc::__error() };
            if returned == 0 && query_errno == 0 {
                terminal = enumerated != 0;
                break;
            }
            if returned == 0 && query_errno == libc::EINVAL {
                terminal = enumerated != 0 && matching == 1;
                break;
            }
            if returned != size || query_errno != 0 {
                return Err(Error::Changed);
            }
            let info = unsafe { info.assume_init() };
            if info.region.size == 0 || info.region.address < address {
                return Err(Error::Changed);
            }
            enumerated = enumerated.checked_add(1).ok_or(Error::Changed)?;
            if u64::from(info.vnode.info.stat.dev) == executable.file.dev
                && info.vnode.info.stat.ino == executable.file.ino
                && info.vnode.info.stat.generation == executable.file.generation
                && info.vnode.info.stat.size >= 0
                && u64::try_from(info.vnode.info.stat.size).map_err(|_| Error::Changed)?
                    == executable.file.len
                && u32::from(info.vnode.info.stat.mode) == executable.file.mode
                && info.vnode.info.kind == 1
                && info.region.protection & libc::VM_PROT_EXECUTE as u32 != 0
                && info.region.offset == executable.slice_offset
            {
                matching = matching.checked_add(1).ok_or(Error::Changed)?;
            }
            address = info
                .region
                .address
                .checked_add(info.region.size)
                .ok_or(Error::Changed)?;
            previous_end = Some(address);
            if address == 0 {
                return Err(Error::Changed);
            }
        }
        if !terminal || matching != 1 {
            return Err(Error::Changed);
        }
        recheck_executable(executable)?;
        recheck_executable_launch_path(executable)?;
        recheck_directory(cwd)?;
        Ok(())
    })();
    let resumed = attest.is_ok()
        && !injected_settlement_failure!(DarwinSigcont)
        && unsafe { libc::kill(pid, libc::SIGCONT) } == 0;
    if attest.is_err() || !resumed {
        let selected = match attest {
            Ok(()) => Error::Spawn,
            Err(error) => error,
        };
        must_settle_failed_group(pid, read_pipe, false);
        return Err(selected);
    }
    let (output, status) = drain_and_wait(pid, read_pipe, stdout_limit, output, false)?;
    if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
        return Err(Error::Exit);
    }
    recheck_executable(executable)?;
    recheck_executable_launch_path(executable)?;
    recheck_directory(cwd)?;
    Ok(output)
}

fn argument(value: &str) -> Result<CString, Error> {
    CString::new(value).map_err(|_| Error::Invalid)
}

pub fn rustc_version(
    executable: &Executable,
    cwd: &Directory,
    maximum: usize,
) -> Result<Vec<u8>, Error> {
    let prepared = prepare_version_invocation("-vV", maximum.min(65_536))?;
    let mut process_arena = prepare_process_arena(1)?;
    version_prepared(executable, cwd, prepared, &mut process_arena)
}

pub fn clang_version(
    executable: &Executable,
    cwd: &Directory,
    maximum: usize,
) -> Result<Vec<u8>, Error> {
    let prepared = prepare_version_invocation("--version", maximum.min(65_536))?;
    let mut process_arena = prepare_process_arena(1)?;
    version_prepared(executable, cwd, prepared, &mut process_arena)
}

pub fn version_prepared(
    executable: &Executable,
    cwd: &Directory,
    prepared: PreparedVersionInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    let maximum = prepared.output.capacity();
    run_argv(
        executable,
        cwd,
        &[prepared.argument],
        maximum,
        prepared.output,
        process_arena,
    )
}

pub fn prepare_c_compile_invocation(
    target: &str,
    input: &OsStr,
    optimization: u8,
    sanitizers: bool,
    maximum: usize,
) -> Result<PreparedCCompileInvocation, Error> {
    let _ = c_name(input)?;
    if target.is_empty()
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || !matches!(optimization, 0 | 2)
        || (sanitizers && !cfg!(target_os = "linux"))
    {
        return Err(Error::Invalid);
    }
    let input = input.to_str().ok_or(Error::Invalid)?;
    let mut values = [""; 16];
    let mut count = 0usize;
    for value in ["-std=c11", "-target", target, "-Wall", "-Wextra", "-Werror"] {
        values[count] = value;
        count += 1;
    }
    if sanitizers {
        for value in ["-fsanitize=address,undefined", "-fno-sanitize-recover=all"] {
            values[count] = value;
            count += 1;
        }
    }
    for value in [
        if optimization == 0 { "-O0" } else { "-O2" },
        "-c",
        input,
        "-o",
        "-",
    ] {
        values[count] = value;
        count += 1;
    }
    #[cfg(target_os = "macos")]
    for value in [
        "-isysroot",
        "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk",
    ] {
        values[count] = value;
        count += 1;
    }
    Ok(PreparedCCompileInvocation(prepare_command(
        &values[..count],
        maximum.min(33_554_432),
    )?))
}

pub fn prepared_c_compile_owned_capacity(prepared: &PreparedCCompileInvocation) -> usize {
    prepared_command_owned_capacity(&prepared.0)
}

pub fn compile_c_prepared(
    executable: &Executable,
    cwd: &Directory,
    prepared: PreparedCCompileInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    let maximum = prepared.0.output.capacity();
    run_argv(
        executable,
        cwd,
        &prepared.0.arguments,
        maximum,
        prepared.0.output,
        process_arena,
    )
}

pub fn prepare_rust_compile_invocation(
    target: &str,
    source: &OsStr,
    output: &OsStr,
) -> Result<PreparedRustCompileInvocation, Error> {
    let _ = c_name(source)?;
    let output_name = prepare_relative_name(output)?;
    if target.is_empty()
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(Error::Invalid);
    }
    let command = prepare_command(
        &[
            "--edition=2021",
            "-Dwarnings",
            "--crate-type",
            "staticlib",
            "-C",
            "panic=unwind",
            "--target",
            target,
            source.to_str().ok_or(Error::Invalid)?,
            "-o",
            output.to_str().ok_or(Error::Invalid)?,
        ],
        0,
    )?;
    Ok(PreparedRustCompileInvocation {
        command,
        output_name,
    })
}

pub fn prepared_rust_compile_owned_capacity(prepared: &PreparedRustCompileInvocation) -> usize {
    prepared_command_owned_capacity(&prepared.command)
        .saturating_add(prepared.output_name.0.as_bytes_with_nul().len())
}

fn compile_rust_prepared_inner(
    rustc: &Executable,
    cwd: &Directory,
    prepared: PreparedRustCompileInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<RegularFile, Error> {
    if hold_regular_file_name_prepared(cwd, &prepared.output_name).is_ok() {
        return Err(Error::Exists);
    }
    if !run_argv(
        rustc,
        cwd,
        &prepared.command.arguments,
        0,
        prepared.command.output,
        process_arena,
    )
    .map_err(|error| trace_error("rustc", error))?
    .is_empty()
    {
        return Err(Error::OutputLimit);
    }
    hold_regular_file_name_prepared(cwd, &prepared.output_name)
}

pub fn compile_direct_rustc_prepared(
    rustc: &DirectRustc,
    cwd: &Directory,
    prepared: PreparedRustCompileInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<RegularFile, Error> {
    recheck_directory(&rustc.sysroot)?;
    compile_rust_prepared_inner(&rustc.executable, cwd, prepared, process_arena)
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
    for name in [harness, c_object, rust_archive, output] {
        let _ = c_name(name)?;
    }
    if linker.is_some()
        || vctools.is_some()
        || target.is_empty()
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || (sanitizers && !cfg!(target_os = "linux"))
    {
        return Err(Error::Invalid);
    }
    let mut values = [""; 20];
    let mut count = 0usize;
    for value in ["-target", target] {
        values[count] = value;
        count += 1;
    }
    if sanitizers {
        for value in ["-fsanitize=address,undefined", "-fno-sanitize-recover=all"] {
            values[count] = value;
            count += 1;
        }
    }
    #[cfg(target_os = "linux")]
    {
        values[count] = LINUX_LINKER_ARGUMENT;
        count += 1;
    }
    #[cfg(target_os = "macos")]
    {
        values[count] = "-Wl,-no_warn_duplicate_libraries";
        count += 1;
    }
    for value in [
        harness.to_str().ok_or(Error::Invalid)?,
        c_object.to_str().ok_or(Error::Invalid)?,
        rust_archive.to_str().ok_or(Error::Invalid)?,
        "-o",
        output.to_str().ok_or(Error::Invalid)?,
    ] {
        values[count] = value;
        count += 1;
    }
    #[cfg(target_os = "linux")]
    for value in LINUX_RUST_STATICLIB_NATIVE_LIBS {
        values[count] = value;
        count += 1;
    }
    #[cfg(target_os = "macos")]
    for value in [
        "-isysroot",
        "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk",
    ] {
        values[count] = value;
        count += 1;
    }
    Ok(PreparedLinkInvocation {
        command: prepare_command(&values[..count], 0)?,
        output_name: prepare_relative_name(output)?,
    })
}

pub fn prepared_link_owned_capacity(prepared: &PreparedLinkInvocation) -> usize {
    prepared_command_owned_capacity(&prepared.command)
        .saturating_add(prepared.output_name.0.as_bytes_with_nul().len())
}

pub fn link_prepared(
    clang: &Executable,
    linker: Option<(&Executable, &str)>,
    cwd: &Directory,
    prepared: PreparedLinkInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Executable, Error> {
    if linker.is_some() {
        return Err(Error::Invalid);
    }
    if hold_regular_file_name_prepared(cwd, &prepared.output_name).is_ok() {
        return Err(Error::Exists);
    }
    if !run_argv(
        clang,
        cwd,
        &prepared.command.arguments,
        0,
        prepared.command.output,
        process_arena,
    )
    .map_err(|error| trace_error("clang-link", error))?
    .is_empty()
    {
        return Err(Error::OutputLimit);
    }
    let file = hold_regular_file_name_prepared(cwd, &prepared.output_name)?;
    if file.mode & 0o111 == 0 {
        return Err(Error::Invalid);
    }
    let (slice_offset, slice_size) = executable_slice(&file)?;
    Ok(Executable {
        file,
        slice_offset,
        slice_size,
        #[cfg(target_os = "macos")]
        launch_path: None,
    })
}

pub fn prepare_archive_invocation(
    input: &OsStr,
    output: &OsStr,
) -> Result<PreparedArchiveInvocation, Error> {
    let _ = c_name(input)?;
    let input_name = prepare_relative_name(input)?;
    let output_name = prepare_relative_name(output)?;
    if input != OsStr::new("module.o") || output != OsStr::new("libsemaprax_native_rust_sdk.a") {
        return Err(Error::Invalid);
    }
    let input = input.to_str().ok_or(Error::Invalid)?;
    let output = output.to_str().ok_or(Error::Invalid)?;
    #[cfg(target_os = "linux")]
    let values = ["rcsD", output, input];
    #[cfg(target_os = "macos")]
    let values = ["-static", "-D", "-o", output, input];
    let output_capacity = 0;
    #[cfg(target_os = "macos")]
    let mut scratch_name = prepare_relative_name_arena("archive-tmp".len())?;
    #[cfg(target_os = "macos")]
    set_relative_name_arena(&mut scratch_name, OsStr::new("archive-tmp"))?;
    Ok(PreparedArchiveInvocation {
        command: prepare_command(&values, output_capacity)?,
        input_name,
        output_name,
        #[cfg(target_os = "macos")]
        scratch_name,
        #[cfg(target_os = "macos")]
        scratch_file: prepare_relative_name(OsStr::new("xcrun_db"))?,
        #[cfg(target_os = "macos")]
        scratch_inventory: prepare_discard_names([OsStr::new("xcrun_db")])?,
        #[cfg(target_os = "macos")]
        empty_scratch_inventory: prepare_discard_names([])?,
    })
}

pub fn prepared_archive_owned_capacity(prepared: &PreparedArchiveInvocation) -> usize {
    let capacity = prepared_command_owned_capacity(&prepared.command)
        .saturating_add(prepared.input_name.0.as_bytes_with_nul().len())
        .saturating_add(prepared.output_name.0.as_bytes_with_nul().len());
    #[cfg(target_os = "macos")]
    let capacity = capacity
        .saturating_add(relative_name_arena_capacity(&prepared.scratch_name))
        .saturating_add(prepared.scratch_file.0.as_bytes_with_nul().len())
        .saturating_add(prepared_discard_names_owned_capacity(
            &prepared.scratch_inventory,
        ));
    capacity
}

#[cfg(test)]
pub(super) fn test_prepared_archive_arguments(prepared: &PreparedArchiveInvocation) -> Vec<&[u8]> {
    prepared
        .command
        .arguments
        .iter()
        .map(|argument| argument.as_bytes())
        .collect()
}

fn recheck_named_regular(
    cwd: &Directory,
    name: &PreparedRelativeName,
    input: &RegularFile,
) -> Result<(), Error> {
    recheck_regular(input)?;
    let rebound = hold_regular_file_name_prepared(cwd, name)?;
    if rebound.dev != input.dev
        || rebound.ino != input.ino
        || rebound.mode != input.mode
        || rebound.len != input.len
        || rebound.digest != input.digest
        || cfg!(target_os = "macos") && {
            #[cfg(target_os = "macos")]
            {
                rebound.generation != input.generation
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        }
    {
        return Err(Error::Changed);
    }
    Ok(())
}

fn child_absent_impl(directory: &Directory, name: &PreparedRelativeName) -> Result<bool, Error> {
    let mut information = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            directory.file.as_raw_fd(),
            name.0.as_ptr(),
            information.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
    {
        return Ok(false);
    }
    if std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
        Ok(true)
    } else {
        Err(Error::Changed)
    }
}

#[cfg(target_os = "linux")]
fn create_owned_archive_seed(
    directory: &Directory,
    name: &PreparedRelativeName,
) -> Result<RegularFile, Error> {
    recheck_directory(directory)?;
    let fd = unsafe {
        libc::openat(
            directory.file.as_raw_fd(),
            name.0.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(Error::Exists);
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        // O_EXCL already changed the namespace. Without the held fd's
        // identity no later pathname can be proven safe to unlink.
        Err(_) => std::process::abort(),
    };
    let created_identity = identity(&metadata);
    let initialized = file
        .write_all(b"!<arch>\n")
        .and_then(|()| file.sync_data())
        .map_err(|_| Error::Changed);
    if let Err(error) = initialized {
        if discard_created_archive_identity(directory, name, created_identity).is_err() {
            std::process::abort();
        }
        return Err(error);
    }
    match authenticate_regular_file(file) {
        Ok(file) => Ok(file),
        Err(error) => {
            if discard_created_archive_identity(directory, name, created_identity).is_err() {
                std::process::abort();
            }
            Err(error)
        }
    }
}

#[cfg(target_os = "linux")]
fn discard_created_archive_identity(
    directory: &Directory,
    name: &PreparedRelativeName,
    created_identity: (u64, u64),
) -> Result<(), Error> {
    recheck_directory(directory)?;
    let fd = unsafe {
        libc::openat(
            directory.file.as_raw_fd(),
            name.0.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::Changed);
    }
    let rebound = unsafe { File::from_raw_fd(fd) };
    let metadata = rebound.metadata().map_err(|_| Error::Changed)?;
    if !metadata.is_file() || identity(&metadata) != created_identity {
        return Err(Error::Changed);
    }
    if unsafe { libc::unlinkat(directory.file.as_raw_fd(), name.0.as_ptr(), 0) } != 0 {
        return Err(Error::Changed);
    }
    recheck_directory(directory)
}

#[cfg(all(test, target_os = "macos"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestDarwinArchiveFailurePoint {
    ProcessOutput,
    ScratchCleanup,
    ArchiverRecheckBeforeHold,
    WorkingDirectoryRecheckBeforeHold,
    InputRecheckBeforeHold,
    OutputHold,
    ExactArchive,
    ArchiverRecheckAfterAuthentication,
    LaunchPathRecheck,
    WorkingDirectoryRecheckAfterAuthentication,
    InputRecheckAfterAuthentication,
    OutputRecheck,
}

#[cfg(all(test, target_os = "macos"))]
thread_local! {
    static TEST_ARCHIVE_POST_AUTHENTICATION_FAILURE: std::cell::Cell<bool> = const {
        std::cell::Cell::new(false)
    };
    static TEST_ARCHIVE_LATER_ACTIONS: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
    static TEST_ARCHIVE_SCRATCH_OPEN_FAILURE: std::cell::Cell<bool> = const {
        std::cell::Cell::new(false)
    };
    static TEST_ARCHIVE_FAILURE_POINT: std::cell::Cell<Option<TestDarwinArchiveFailurePoint>> = const {
        std::cell::Cell::new(None)
    };
}

#[cfg(all(test, target_os = "macos"))]
pub(super) fn test_inject_archive_post_authentication_failure(enabled: bool) {
    TEST_ARCHIVE_POST_AUTHENTICATION_FAILURE.with(|slot| slot.set(enabled));
}

#[cfg(all(test, target_os = "macos"))]
pub(super) fn test_reset_archive_later_actions() {
    TEST_ARCHIVE_LATER_ACTIONS.with(|slot| slot.set(0));
}

#[cfg(all(test, target_os = "macos"))]
pub(super) fn test_archive_later_actions() -> usize {
    TEST_ARCHIVE_LATER_ACTIONS.with(std::cell::Cell::get)
}

#[cfg(all(test, target_os = "macos"))]
pub(super) fn test_inject_archive_scratch_open_failure(enabled: bool) {
    TEST_ARCHIVE_SCRATCH_OPEN_FAILURE.with(|slot| slot.set(enabled));
}

#[cfg(all(test, target_os = "macos"))]
pub(super) fn test_inject_darwin_archive_failure(point: Option<TestDarwinArchiveFailurePoint>) {
    TEST_ARCHIVE_FAILURE_POINT.with(|slot| slot.set(point));
}

#[cfg(all(test, target_os = "macos"))]
fn record_archive_later_action() {
    TEST_ARCHIVE_LATER_ACTIONS.with(|slot| slot.set(slot.get() + 1));
}

#[cfg(all(not(test), target_os = "macos"))]
fn record_archive_later_action() {}

#[cfg(all(test, target_os = "macos"))]
fn archive_post_authentication_failure_injected() -> bool {
    TEST_ARCHIVE_POST_AUTHENTICATION_FAILURE.with(std::cell::Cell::get)
}

#[cfg(all(not(test), target_os = "macos"))]
fn archive_post_authentication_failure_injected() -> bool {
    false
}

#[cfg(all(test, target_os = "macos"))]
fn archive_scratch_open_failure_injected() -> bool {
    TEST_ARCHIVE_SCRATCH_OPEN_FAILURE.with(std::cell::Cell::get)
}

#[cfg(all(not(test), target_os = "macos"))]
fn archive_scratch_open_failure_injected() -> bool {
    false
}

#[cfg(all(test, target_os = "macos"))]
fn darwin_archive_failure_injected(point: TestDarwinArchiveFailurePoint) -> bool {
    TEST_ARCHIVE_FAILURE_POINT.with(|slot| slot.get() == Some(point))
}

#[cfg(all(test, target_os = "linux"))]
pub(super) fn test_archive_seed_round_trip(
    directory: &Directory,
    name: &OsStr,
) -> Result<(), Error> {
    let name = prepare_relative_name(name)?;
    let seed = create_owned_archive_seed(directory, &name)?;
    if read_exact(&seed, 8)? != b"!<arch>\n" {
        return Err(Error::Invalid);
    }
    discard_created_archive_identity(directory, &name, (seed.dev, seed.ino))?;
    if child_absent_impl(directory, &name)? {
        Ok(())
    } else {
        Err(Error::Changed)
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(super) fn test_regular_file_facts(file: &RegularFile) -> (u32, u64, u64) {
    (file.mode, file.dev, file.ino)
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(super) fn test_exact_archive_member(
    archive: &RegularFile,
    input: &RegularFile,
) -> Result<(), Error> {
    exact_archive_member(archive, input)
}

pub fn child_absent_prepared(
    directory: &Directory,
    name: &PreparedRelativeName,
) -> Result<bool, Error> {
    child_absent_impl(directory, name)
}

pub fn same_child_directory_prepared(
    parent: &Directory,
    name: &PreparedRelativeName,
    child: &Directory,
) -> Result<bool, Error> {
    recheck_directory(parent)?;
    recheck_directory(child)?;
    let rebound = open_directory_at(parent.file.as_raw_fd(), &name.0)?;
    Ok(prepared_directory_identity(&rebound) == prepared_directory_identity(child))
}

fn exact_archive_member(archive: &RegularFile, input: &RegularFile) -> Result<(), Error> {
    if archive.len < 68 || input.len == 0 {
        return Err(Error::Invalid);
    }
    let mut magic = [0_u8; 8];
    archive
        .file
        .read_exact_at(&mut magic, 0)
        .map_err(|_| Error::Changed)?;
    if magic != *b"!<arch>\n" {
        return Err(Error::Invalid);
    }
    let mut offset = 8_u64;
    let mut input_members = 0_u8;
    let mut members = 0_u8;
    while offset < archive.len {
        let mut header = [0_u8; 60];
        archive
            .file
            .read_exact_at(&mut header, offset)
            .map_err(|_| Error::Invalid)?;
        if header[58..] != *b"`\n" {
            return Err(Error::Invalid);
        }
        let size = archive_member_size(&header[48..58])?;
        let data = offset.checked_add(60).ok_or(Error::OutputLimit)?;
        let end = data.checked_add(size).ok_or(Error::OutputLimit)?;
        if end > archive.len {
            return Err(Error::Invalid);
        }
        let header_kind = archive_member_kind(&header[..16], b"module.o")?;
        exact_archive_member_metadata(&header, header_kind, input.mode)?;
        let (kind, member_data, member_size) = match header_kind {
            ArchiveMemberKind::Extended(length) => {
                let length = u64::try_from(length).map_err(|_| Error::OutputLimit)?;
                if length > size {
                    return Err(Error::Invalid);
                }
                let mut name = [0_u8; 255];
                let name_length = usize::try_from(length).map_err(|_| Error::OutputLimit)?;
                archive
                    .file
                    .read_exact_at(&mut name[..name_length], data)
                    .map_err(|_| Error::Changed)?;
                let name = archive_extended_name(&name[..name_length])?;
                let kind = archive_member_kind(name, b"module.o")?;
                if matches!(kind, ArchiveMemberKind::Extended(_)) {
                    return Err(Error::Invalid);
                }
                (
                    kind,
                    data.checked_add(length).ok_or(Error::OutputLimit)?,
                    size - length,
                )
            }
            kind => (kind, data, size),
        };
        #[cfg(target_os = "linux")]
        let admitted = matches!(
            (members, header_kind, kind),
            (
                0,
                ArchiveMemberKind::GnuLinkerIndex,
                ArchiveMemberKind::GnuLinkerIndex
            ) | (1, ArchiveMemberKind::Input, ArchiveMemberKind::Input)
        );
        #[cfg(target_os = "macos")]
        let admitted = matches!(
            (members, header_kind, kind),
            (
                0,
                ArchiveMemberKind::Extended(20),
                ArchiveMemberKind::BsdSortedLinkerIndex,
            ) | (1, ArchiveMemberKind::Extended(12), ArchiveMemberKind::Input,)
        );
        if !admitted {
            return Err(Error::Invalid);
        }
        match kind {
            ArchiveMemberKind::GnuLinkerIndex
            | ArchiveMemberKind::BsdSortedLinkerIndex
            | ArchiveMemberKind::LongNames => {}
            ArchiveMemberKind::Input => {
                input_members = input_members.checked_add(1).ok_or(Error::Invalid)?;
                if member_size != input.len {
                    return Err(Error::Invalid);
                }
                let mut compared = 0_u64;
                let mut archive_bytes = [0_u8; 8192];
                let mut input_bytes = [0_u8; 8192];
                while compared < member_size {
                    let count = usize::try_from((member_size - compared).min(8192))
                        .map_err(|_| Error::OutputLimit)?;
                    archive
                        .file
                        .read_exact_at(&mut archive_bytes[..count], member_data + compared)
                        .map_err(|_| Error::Changed)?;
                    input
                        .file
                        .read_exact_at(&mut input_bytes[..count], compared)
                        .map_err(|_| Error::Changed)?;
                    if archive_bytes[..count] != input_bytes[..count] {
                        return Err(Error::Invalid);
                    }
                    compared = compared
                        .checked_add(u64::try_from(count).map_err(|_| Error::OutputLimit)?)
                        .ok_or(Error::OutputLimit)?;
                }
            }
            ArchiveMemberKind::Extended(_) => return Err(Error::Invalid),
        }
        if size & 1 != 0 {
            let mut padding = [0_u8; 1];
            archive
                .file
                .read_exact_at(&mut padding, end)
                .map_err(|_| Error::Invalid)?;
            if padding != *b"\n" {
                return Err(Error::Invalid);
            }
        }
        offset = end.checked_add(size & 1).ok_or(Error::OutputLimit)?;
        members = members.checked_add(1).ok_or(Error::Invalid)?;
    }
    if offset != archive.len || input_members != 1 || members != 2 {
        return Err(Error::Invalid);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn discard_archive_scratch(
    cwd: &Directory,
    scratch: &Directory,
    prepared: &PreparedArchiveInvocation,
) -> Result<(), Error> {
    if child_absent_impl(scratch, &prepared.scratch_file)? {
        return discard_owned_stage_prepared(
            cwd,
            scratch,
            &prepared.scratch_name,
            &prepared.empty_scratch_inventory,
            &[],
            &[],
            #[cfg(debug_assertions)]
            None,
        );
    }
    let file = hold_regular_file_name_bounded_prepared(
        scratch,
        &prepared.scratch_file,
        SDK_ARCHIVE_MAX_BYTES,
    )?;
    discard_owned_stage_prepared(
        cwd,
        scratch,
        &prepared.scratch_name,
        &prepared.scratch_inventory,
        &[Some(&file)],
        &[None],
        #[cfg(debug_assertions)]
        None,
    )
}

#[cfg(target_os = "linux")]
pub fn archive_prepared(
    archiver: &Executable,
    cwd: &Directory,
    input: &RegularFile,
    mut prepared: PreparedArchiveInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<RegularFile, Error> {
    if !child_absent_impl(cwd, &prepared.output_name)? {
        return Err(Error::Exists);
    }
    recheck_named_regular(cwd, &prepared.input_name, input)?;
    recheck_executable(archiver)?;
    #[cfg(target_os = "macos")]
    recheck_executable_launch_path(archiver)?;
    recheck_directory(cwd)?;
    #[cfg(target_os = "linux")]
    let owned_output = create_owned_archive_seed(cwd, &prepared.output_name)?;
    let output_limit = prepared.command.output.capacity();
    let process_output = std::mem::take(&mut prepared.command.output);
    let process = run_archive_argv(
        archiver,
        cwd,
        &prepared.command.arguments,
        output_limit,
        process_output,
        process_arena,
    );
    let archiver_recheck = recheck_executable(archiver);
    let cwd_recheck = recheck_directory(cwd);
    let input_recheck = recheck_named_regular(cwd, &prepared.input_name, input);
    let authenticated = (|| {
        let output = process?;
        archiver_recheck?;
        cwd_recheck?;
        input_recheck?;
        if !output.is_empty() {
            return Err(Error::OutputLimit);
        }
        let archive = hold_regular_file_name_bounded_prepared(
            cwd,
            &prepared.output_name,
            SDK_ARCHIVE_MAX_BYTES,
        )?;
        if (archive.dev, archive.ino) != (owned_output.dev, owned_output.ino) {
            return Err(Error::Changed);
        }
        exact_archive_member(&archive, input)?;
        recheck_executable(archiver)?;
        recheck_directory(cwd)?;
        recheck_named_regular(cwd, &prepared.input_name, input)?;
        recheck_named_regular(cwd, &prepared.output_name, &archive)?;
        Ok(archive)
    })();
    match authenticated {
        Ok(archive) => Ok(archive),
        Err(error) => {
            discard_created_archive_identity(
                cwd,
                &prepared.output_name,
                (owned_output.dev, owned_output.ino),
            )?;
            Err(error)
        }
    }
}

#[cfg(target_os = "macos")]
fn darwin_archive_failure(
    error: Error,
    phase: crate::DarwinArchiveFailurePhase,
    settlement: crate::DarwinArchiveSettlement,
) -> crate::DarwinArchiveFailure {
    crate::DarwinArchiveFailure {
        error,
        phase,
        settlement,
    }
}

/// Runs Apple's archive tool with an explicit effect-settlement result. Once
/// the child may have created the output, every unauthenticated failure is
/// absorbing. Once exact archive authentication succeeds, later rejection
/// remains inert on every later rejection. No post-effect pathname cleanup is
/// permitted because compare-then-unlink cannot close namespace substitution.
#[cfg(target_os = "macos")]
pub fn archive_prepared_settled(
    archiver: &Executable,
    cwd: &Directory,
    input: &RegularFile,
    mut prepared: PreparedArchiveInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<RegularFile, crate::DarwinArchiveFailure> {
    use crate::DarwinArchiveFailurePhase as Phase;
    use crate::DarwinArchiveSettlement as Settlement;

    let preflight = |error| darwin_archive_failure(error, Phase::Preflight, Settlement::Settled);
    if !child_absent_impl(cwd, &prepared.output_name).map_err(preflight)? {
        return Err(preflight(Error::Exists));
    }
    recheck_named_regular(cwd, &prepared.input_name, input).map_err(preflight)?;
    recheck_executable(archiver).map_err(preflight)?;
    recheck_executable_launch_path(archiver).map_err(preflight)?;
    recheck_directory(cwd).map_err(preflight)?;
    let scratch = create_directory_new_prepared_settled(cwd, &prepared.scratch_name, 0o700)
        .map_err(|failure| {
            darwin_archive_failure(
                failure.error,
                Phase::ScratchCreation,
                if failure.namespace_created {
                    Settlement::Uncertain
                } else {
                    Settlement::Settled
                },
            )
        })?;
    let output_limit = prepared.command.output.capacity();
    let process_output = std::mem::take(&mut prepared.command.output);
    let uncertain = |error, phase| darwin_archive_failure(error, phase, Settlement::Uncertain);
    let output = match run_archive_argv(
        archiver,
        cwd,
        &prepared.command.arguments,
        output_limit,
        process_output,
        process_arena,
    ) {
        Ok(output) => output,
        Err(error) => return Err(uncertain(error, Phase::Process)),
    };
    #[cfg(test)]
    if darwin_archive_failure_injected(TestDarwinArchiveFailurePoint::ProcessOutput) {
        return Err(uncertain(Error::OutputLimit, Phase::ProcessOutput));
    }
    if !output.is_empty() {
        return Err(uncertain(Error::OutputLimit, Phase::ProcessOutput));
    }
    #[cfg(test)]
    if darwin_archive_failure_injected(TestDarwinArchiveFailurePoint::ScratchCleanup) {
        return Err(uncertain(Error::Changed, Phase::ScratchCleanup));
    }
    record_archive_later_action();
    discard_archive_scratch(cwd, &scratch, &prepared)
        .map_err(|error| uncertain(error, Phase::ScratchCleanup))?;
    #[cfg(test)]
    if darwin_archive_failure_injected(TestDarwinArchiveFailurePoint::ArchiverRecheckBeforeHold) {
        return Err(uncertain(Error::Changed, Phase::ArchiverRecheck));
    }
    record_archive_later_action();
    recheck_executable(archiver).map_err(|error| uncertain(error, Phase::ArchiverRecheck))?;
    #[cfg(test)]
    if darwin_archive_failure_injected(
        TestDarwinArchiveFailurePoint::WorkingDirectoryRecheckBeforeHold,
    ) {
        return Err(uncertain(Error::Changed, Phase::WorkingDirectoryRecheck));
    }
    record_archive_later_action();
    recheck_directory(cwd).map_err(|error| uncertain(error, Phase::WorkingDirectoryRecheck))?;
    #[cfg(test)]
    if darwin_archive_failure_injected(TestDarwinArchiveFailurePoint::InputRecheckBeforeHold) {
        return Err(uncertain(Error::Changed, Phase::InputRecheck));
    }
    record_archive_later_action();
    recheck_named_regular(cwd, &prepared.input_name, input)
        .map_err(|error| uncertain(error, Phase::InputRecheck))?;
    #[cfg(test)]
    if darwin_archive_failure_injected(TestDarwinArchiveFailurePoint::OutputHold) {
        return Err(uncertain(Error::Changed, Phase::OutputHold));
    }
    record_archive_later_action();
    let archive =
        hold_regular_file_name_bounded_prepared(cwd, &prepared.output_name, SDK_ARCHIVE_MAX_BYTES)
            .map_err(|error| uncertain(error, Phase::OutputHold))?;
    #[cfg(test)]
    if darwin_archive_failure_injected(TestDarwinArchiveFailurePoint::ExactArchive) {
        return Err(uncertain(Error::Changed, Phase::ExactArchive));
    }
    record_archive_later_action();
    exact_archive_member(&archive, input).map_err(|error| uncertain(error, Phase::ExactArchive))?;

    let post_authentication = (|| {
        if archive_post_authentication_failure_injected() {
            return Err((Error::Changed, Phase::ArchiverRecheck));
        }
        #[cfg(test)]
        if darwin_archive_failure_injected(
            TestDarwinArchiveFailurePoint::ArchiverRecheckAfterAuthentication,
        ) {
            return Err((Error::Changed, Phase::ArchiverRecheck));
        }
        record_archive_later_action();
        recheck_executable(archiver).map_err(|error| (error, Phase::ArchiverRecheck))?;
        #[cfg(test)]
        if darwin_archive_failure_injected(TestDarwinArchiveFailurePoint::LaunchPathRecheck) {
            return Err((Error::Changed, Phase::LaunchPathRecheck));
        }
        record_archive_later_action();
        recheck_executable_launch_path(archiver)
            .map_err(|error| (error, Phase::LaunchPathRecheck))?;
        #[cfg(test)]
        if darwin_archive_failure_injected(
            TestDarwinArchiveFailurePoint::WorkingDirectoryRecheckAfterAuthentication,
        ) {
            return Err((Error::Changed, Phase::WorkingDirectoryRecheck));
        }
        record_archive_later_action();
        recheck_directory(cwd).map_err(|error| (error, Phase::WorkingDirectoryRecheck))?;
        #[cfg(test)]
        if darwin_archive_failure_injected(
            TestDarwinArchiveFailurePoint::InputRecheckAfterAuthentication,
        ) {
            return Err((Error::Changed, Phase::InputRecheck));
        }
        record_archive_later_action();
        recheck_named_regular(cwd, &prepared.input_name, input)
            .map_err(|error| (error, Phase::InputRecheck))?;
        #[cfg(test)]
        if darwin_archive_failure_injected(TestDarwinArchiveFailurePoint::OutputRecheck) {
            return Err((Error::Changed, Phase::OutputRecheck));
        }
        record_archive_later_action();
        recheck_named_regular(cwd, &prepared.output_name, &archive)
            .map_err(|error| (error, Phase::OutputRecheck))?;
        Ok::<(), (Error, Phase)>(())
    })();
    post_authentication.map_err(|(error, phase)| uncertain(error, phase))?;
    Ok(archive)
}

pub fn prepare_run_invocation() -> Result<PreparedRunInvocation, Error> {
    Ok(PreparedRunInvocation(prepare_command(&[], 0)?))
}

pub fn prepared_run_owned_capacity(prepared: &PreparedRunInvocation) -> usize {
    prepared_command_owned_capacity(&prepared.0)
}

pub fn run_prepared(
    executable: &Executable,
    cwd: &Directory,
    prepared: PreparedRunInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<(), Error> {
    if run_argv(
        executable,
        cwd,
        &prepared.0.arguments,
        0,
        prepared.0.output,
        process_arena,
    )?
    .is_empty()
    {
        Ok(())
    } else {
        Err(Error::OutputLimit)
    }
}

pub fn compile_c_to_stdout(
    executable: &Executable,
    cwd: &Directory,
    target: &str,
    input: &OsStr,
    optimization: u8,
    sanitizers: bool,
    maximum: usize,
) -> Result<Vec<u8>, Error> {
    let _ = c_name(input)?;
    if target.is_empty()
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || !matches!(optimization, 0 | 2)
    {
        return Err(Error::Invalid);
    }
    let input = input.to_str().ok_or(Error::Invalid)?;
    let mut arguments = vec![
        argument("-std=c11")?,
        argument("-target")?,
        argument(target)?,
        argument("-Wall")?,
        argument("-Wextra")?,
        argument("-Werror")?,
        argument(if optimization == 0 { "-O0" } else { "-O2" })?,
        argument("-c")?,
        argument(input)?,
        argument("-o")?,
        argument("-")?,
    ];
    if sanitizers {
        if !cfg!(target_os = "linux") {
            return Err(Error::Unsupported);
        }
        arguments.insert(6, argument("-fsanitize=address,undefined")?);
        arguments.insert(7, argument("-fno-sanitize-recover=all")?);
    }
    #[cfg(target_os = "macos")]
    arguments.extend([
        argument("-isysroot")?,
        argument("/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk")?,
    ]);
    let mut process_arena = prepare_process_arena(1)?;
    run_argv(
        executable,
        cwd,
        &arguments,
        maximum.min(33_554_432),
        Vec::new(),
        &mut process_arena,
    )
}

pub fn execute_harness(executable: &Executable, cwd: &Directory) -> Result<(), Error> {
    execute_harness_with_output_limit(executable, cwd, 0)
}

pub(crate) fn execute_harness_with_output_limit(
    executable: &Executable,
    cwd: &Directory,
    stdout_limit: usize,
) -> Result<(), Error> {
    let mut process_arena = prepare_process_arena(1)?;
    let output = Vec::with_capacity(stdout_limit);
    if run_argv(
        executable,
        cwd,
        &[],
        stdout_limit,
        output,
        &mut process_arena,
    )?
    .is_empty()
    {
        Ok(())
    } else {
        Err(Error::OutputLimit)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn link_harness(
    clang: &Executable,
    linker: Option<(&Executable, &str)>,
    vctools: Option<&OsStr>,
    cwd: &Directory,
    target: &str,
    harness: &OsStr,
    c_object: &OsStr,
    rust_archive: &OsStr,
    output: &OsStr,
    sanitizers: bool,
) -> Result<Executable, Error> {
    for name in [harness, c_object, rust_archive, output] {
        let _ = c_name(name)?;
    }
    if linker.is_some()
        || vctools.is_some()
        || target.is_empty()
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || hold_regular_file(cwd, output).is_ok()
    {
        return Err(Error::Invalid);
    }
    if sanitizers && !cfg!(target_os = "linux") {
        return Err(Error::Unsupported);
    }
    let mut arguments = vec![
        argument("-target")?,
        argument(target)?,
        argument(harness.to_str().ok_or(Error::Invalid)?)?,
        argument(c_object.to_str().ok_or(Error::Invalid)?)?,
        argument(rust_archive.to_str().ok_or(Error::Invalid)?)?,
        argument("-o")?,
        argument(output.to_str().ok_or(Error::Invalid)?)?,
    ];
    #[cfg(target_os = "linux")]
    arguments.insert(2, argument(LINUX_LINKER_ARGUMENT)?);
    #[cfg(target_os = "linux")]
    arguments.extend(
        LINUX_RUST_STATICLIB_NATIVE_LIBS
            .into_iter()
            .map(argument)
            .collect::<Result<Vec<_>, _>>()?,
    );
    #[cfg(target_os = "macos")]
    arguments.insert(2, argument("-Wl,-no_warn_duplicate_libraries")?);
    if sanitizers {
        arguments.insert(2, argument("-fsanitize=address,undefined")?);
        arguments.insert(3, argument("-fno-sanitize-recover=all")?);
    }
    #[cfg(target_os = "macos")]
    arguments.extend([
        argument("-isysroot")?,
        argument("/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk")?,
    ]);
    let mut process_arena = prepare_process_arena(1)?;
    if !run_argv(clang, cwd, &arguments, 0, Vec::new(), &mut process_arena)
        .map_err(|error| trace_error("clang-link", error))?
        .is_empty()
    {
        return Err(Error::OutputLimit);
    }
    hold_executable(cwd, output)
}
