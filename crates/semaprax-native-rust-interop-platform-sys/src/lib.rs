//! Audited operating-system quarantine for Native Rust Interop bundle builds.
//!
//! This crate is unpublished. Its public surface exists only so the sibling
//! safe facade can own opaque held objects without exposing handles upstream.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(unix)]
use std::ffi::CString;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write as _;
use std::path::Path;

#[cfg(test)]
static TEST_PREPARED_FILE_SYSCALL_ENTRIES: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn enter_prepared_file_syscalls<T>(resolved: Result<&T, Error>) -> Result<&T, Error> {
    let resolved = resolved?;
    #[cfg(test)]
    TEST_PREPARED_FILE_SYSCALL_ENTRIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(resolved)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Invalid,
    Exists,
    Changed,
    Unsupported,
    Spawn,
    Exit,
    OutputLimit,
}

#[cfg(test)]
#[allow(clippy::enum_variant_names)]
#[repr(u8)]
#[derive(Clone, Copy)]
enum TestSettlementFailure {
    #[cfg(unix)]
    UnixWait,
    #[cfg(unix)]
    UnixGroup,
    #[cfg(unix)]
    UnixSettleClose,
    #[cfg(unix)]
    UnixSuccessReadClose,
    #[cfg(unix)]
    UnixParentWriteClose,
    #[cfg(unix)]
    UnixParentNullClose,
    #[cfg(unix)]
    UnixPipeReadFcntl,
    #[cfg(unix)]
    UnixPipeWriteFcntl,
    #[cfg(unix)]
    UnixDrainFcntl,
    #[cfg(unix)]
    UnixPoll,
    #[cfg(unix)]
    UnixRead,
    #[cfg(unix)]
    UnixReadConversion,
    #[cfg(unix)]
    UnixWaitpid,
    #[cfg(unix)]
    UnixDeadline,
    #[cfg(target_os = "macos")]
    DarwinActionsDestroy,
    #[cfg(target_os = "macos")]
    DarwinAttributesDestroy,
    #[cfg(target_os = "macos")]
    DarwinAttest,
    #[cfg(target_os = "macos")]
    DarwinSigcont,
    #[cfg(target_os = "windows")]
    WindowsImage,
    #[cfg(target_os = "windows")]
    WindowsAssign,
    #[cfg(target_os = "windows")]
    WindowsResume,
    #[cfg(target_os = "windows")]
    WindowsPeek,
    #[cfg(target_os = "windows")]
    WindowsRead,
    #[cfg(target_os = "windows")]
    WindowsUnassigned,
    #[cfg(target_os = "windows")]
    WindowsTerminateProcess,
    #[cfg(target_os = "windows")]
    WindowsWaitUnassigned,
    #[cfg(target_os = "windows")]
    WindowsJob,
    #[cfg(target_os = "windows")]
    WindowsTerminateJob,
    #[cfg(target_os = "windows")]
    WindowsQueryJob,
}

#[cfg(test)]
static TEST_SETTLEMENT_FAILURES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
#[allow(dead_code)]
fn set_test_settlement_failures(points: &[TestSettlementFailure]) {
    let mut mask = 0_u64;
    for point in points {
        mask |= 1_u64 << (*point as u8);
    }
    TEST_SETTLEMENT_FAILURES.store(mask, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
fn test_settlement_failure(point: TestSettlementFailure) -> bool {
    TEST_SETTLEMENT_FAILURES.load(std::sync::atomic::Ordering::SeqCst) & (1_u64 << (point as u8))
        != 0
}

macro_rules! injected_settlement_failure {
    ($point:ident) => {{
        #[cfg(test)]
        {
            test_settlement_failure(TestSettlementFailure::$point)
        }
        #[cfg(not(test))]
        {
            false
        }
    }};
}

#[cfg(all(test, unix))]
fn trace_error(context: &str, error: Error) -> Error {
    eprintln!("platform {context}: {error:?}");
    error
}

#[cfg(all(not(test), unix))]
fn trace_error(_: &str, error: Error) -> Error {
    error
}

#[cfg(unix)]
mod platform {
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

    pub struct Executable {
        file: RegularFile,
        slice_offset: u64,
        slice_size: u64,
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

    pub fn prepared_rustc_version_owned_capacity(
        prepared: &PreparedRustcVersionInvocation,
    ) -> usize {
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

    fn relative_name_arena_cstr(
        arena: &PreparedRelativeNameArena,
    ) -> Result<&std::ffi::CStr, Error> {
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
            let subtype = u32::from_le_bytes(prefix[8..12].try_into().map_err(|_| Error::Invalid)?)
                & 0x00ff_ffff;
            let compatible_subtype = if cfg!(target_arch = "aarch64") {
                matches!(subtype, 0 | 2)
            } else {
                subtype == 3
            };
            let filetype =
                u32::from_le_bytes(prefix[12..16].try_into().map_err(|_| Error::Invalid)?);
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
                    .any(|(_, _, prior_offset, prior_end)| {
                        offset < *prior_end && *prior_offset < end
                    })
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
            || u32::from_le_bytes(header[4..8].try_into().map_err(|_| Error::Invalid)?)
                != current_cpu
            || u32::from_le_bytes(header[12..16].try_into().map_err(|_| Error::Invalid)?) != 2
            || (u32::from_le_bytes(header[8..12].try_into().map_err(|_| Error::Invalid)?)
                & 0x00ff_ffff)
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
        let program_offset =
            u64::from_le_bytes(header[32..40].try_into().map_err(|_| Error::Invalid)?);
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
            let memory_size =
                u64::from_le_bytes(row[40..48].try_into().map_err(|_| Error::Invalid)?);
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
            let returned =
                unsafe { proc_listpgrppids(pid, members.as_mut_ptr().cast(), member_bytes) };
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
                || (polled < 0
                    && std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR))
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

    pub fn same_directory_path(directory: &Directory, path: &Path) -> Result<bool, Error> {
        recheck_directory(directory)?;
        let rebound = hold_directory(path)?;
        Ok((rebound.dev, rebound.ino, rebound.mode)
            == (directory.dev, directory.ino, directory.mode))
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
        recheck_directory(parent)?;
        let name = relative_name_arena_cstr(name)?;
        let mode = libc::mode_t::try_from(mode).map_err(|_| Error::Invalid)?;
        let result = unsafe { libc::mkdirat(parent.file.as_raw_fd(), name.as_ptr(), mode) };
        if result != 0 {
            return Err(match std::io::Error::last_os_error().raw_os_error() {
                Some(libc::EEXIST) => Error::Exists,
                _ => Error::Changed,
            });
        }
        open_directory_at(parent.file.as_raw_fd(), name)
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
        let metadata = file.metadata().map_err(|_| Error::Changed)?;
        if !metadata.is_file() {
            return Err(Error::Changed);
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

    fn hold_executable_cstr(
        directory: &Directory,
        name: &std::ffi::CStr,
    ) -> Result<Executable, Error> {
        let file = hold_regular_file_cstr(directory, name)?;
        if file.mode & 0o111 == 0 {
            return Err(Error::Invalid);
        }
        let (slice_offset, slice_size) = executable_slice(&file)?;
        Ok(Executable {
            file,
            slice_offset,
            slice_size,
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

    pub fn hold_external_executable(path: &Path) -> Result<Executable, Error> {
        let parent = path.parent().ok_or(Error::Invalid)?;
        let name = path.file_name().ok_or(Error::Invalid)?;
        let directory = hold_directory(parent)?;
        hold_executable(&directory, name)
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
            let inode =
                unsafe { std::ptr::read_unaligned(bytes.as_ptr().add(offset).cast::<u64>()) };
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
            let record =
                usize::from(unsafe { std::ptr::addr_of!((*entry).d_reclen).read_unaligned() });
            let name_length =
                usize::from(unsafe { std::ptr::addr_of!((*entry).d_namlen).read_unaligned() });
            let name_end = header_end.checked_add(name_length).ok_or(Error::Changed)?;
            let next = offset.checked_add(record).ok_or(Error::Changed)?;
            if record < header + 1
                || record % 4 != 0
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
            return Err(Error::Exists);
        }
        Ok(())
    }

    pub fn discard_owned_stage_prepared<const N: usize>(
        parent: &Directory,
        stage: &Directory,
        stage_name: &PreparedRelativeNameArena,
        names: &PreparedDiscardNames<N>,
        files: &[Option<&RegularFile>; N],
        #[cfg(debug_assertions)] failure_after_delete: Option<usize>,
    ) -> Result<(), Error> {
        recheck_directory(parent)?;
        recheck_directory(stage)?;
        let stage_name = relative_name_arena_cstr(stage_name)?;
        let rebound = open_directory_at(parent.file.as_raw_fd(), stage_name)?;
        if (rebound.dev, rebound.ino, rebound.mode) != (stage.dev, stage.ino, stage.mode) {
            return Err(Error::Changed);
        }
        let attached = files.iter().take_while(|file| file.is_some()).count();
        if files[attached..].iter().any(Option::is_some) {
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
                let actual =
                    unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
                if actual == b"." || actual == b".." {
                    continue;
                }
                let Some(index) = names.names[..attached].iter().position(|expected| {
                    expected.as_ref().expect("validated").0.as_bytes() == actual
                }) else {
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

        for (file, name) in files[..attached].iter().zip(&names.names[..attached]) {
            let file = file.expect("attached prefix");
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
    fn run_argv(
        executable: &Executable,
        cwd: &Directory,
        arguments: &[CString],
        stdout_limit: usize,
        output: Vec<u8>,
        process_arena: &mut PreparedProcessArena,
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
        let (output, status) = drain_and_wait(pid, read_pipe, stdout_limit, output)?;
        if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
            #[cfg(test)]
            eprintln!("linux platform child status={status} args={arguments:?}");
            return Err(Error::Exit);
        }
        recheck_executable(executable)?;
        recheck_directory(cwd)?;
        Ok(output)
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
        recheck_directory(cwd)?;
        let mut path = [0_u8; 1024];
        if unsafe { libc::fcntl(executable.file.file.as_raw_fd(), 50, path.as_mut_ptr()) } != 0 {
            return Err(Error::Changed);
        }
        let executable_path =
            unsafe { std::ffi::CStr::from_ptr(path.as_ptr().cast::<libc::c_char>()) };
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
        argv[0] = c"semaprax-native-rust-interop-tool".as_ptr().cast_mut();
        for (index, argument) in arguments.iter().enumerate() {
            argv[index + 1] = argument.as_ptr().cast_mut();
        }
        let env = [std::ptr::null_mut::<libc::c_char>()];
        let flags = libc::c_short::try_from(
            libc::POSIX_SPAWN_START_SUSPENDED
                | libc::POSIX_SPAWN_CLOEXEC_DEFAULT
                | libc::POSIX_SPAWN_SETPGROUP,
        )
        .map_err(|_| Error::Unsupported)?;
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
            let cwd_size = libc::c_int::try_from(std::mem::size_of::<VnodePaths>())
                .map_err(|_| Error::Changed)?;
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
                let returned =
                    unsafe { proc_pidinfo(pid, 8, address, info.as_mut_ptr().cast(), size) };
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
        let (output, status) = drain_and_wait(pid, read_pipe, stdout_limit, output)?;
        #[cfg(test)]
        if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
            eprintln!("platform child status={status} args={arguments:?}");
        }
        if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
            #[cfg(test)]
            eprintln!("darwin platform child status={status} args={arguments:?}");
            return Err(Error::Exit);
        }
        recheck_executable(executable)?;
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
        })
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
}

#[cfg(windows)]
mod platform {
    use super::*;
    use sha2::{Digest as _, Sha256};
    use std::io::{Read as _, Seek as _, SeekFrom};
    use std::os::windows::ffi::OsStrExt as _;
    use std::os::windows::fs::FileExt as _;
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, IntoRawHandle as _};
    use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
    use windows_sys::Wdk::Storage::FileSystem::{
        FileLinkInformationEx, NtCreateFile, NtSetInformationFile, FILE_CREATE,
        FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
        FILE_SYNCHRONOUS_IO_NONALERT,
    };
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, SetHandleInformation, ERROR_BROKEN_PIPE,
        ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_FILES, ERROR_PIPE_NOT_CONNECTED, HANDLE,
        HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, STATUS_OBJECT_NAME_COLLISION, UNICODE_STRING,
        WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FileDispositionInfoEx, FileIdBothDirectoryInfo,
        FileIdBothDirectoryRestartInfo, FileIdExtdDirectoryInfo, FileIdExtdDirectoryRestartInfo,
        FileIdInfo, FileRenameInfoEx, GetFileInformationByHandle, GetFileInformationByHandleEx,
        GetFinalPathNameByHandleW, ReadFile, SetFileInformationByHandle,
        BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY,
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_DELETE_CHILD, FILE_DISPOSITION_FLAG_DELETE, FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
        FILE_DISPOSITION_INFO_EX, FILE_EXECUTE, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_ID_BOTH_DIR_INFO,
        FILE_ID_EXTD_DIR_INFO, FILE_ID_INFO, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, FILE_WRITE_ATTRIBUTES,
        OPEN_EXISTING, SYNCHRONIZE,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
        JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
        TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        QueryFullProcessImageNameW, ResumeThread, TerminateProcess, UpdateProcThreadAttribute,
        WaitForSingleObject, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
        EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
        STARTF_USESTDHANDLES, STARTUPINFOEXW,
    };
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

    const NORMAL_FILE_FLAGS: u32 = FILE_FLAG_OPEN_REPARSE_POINT;
    const DIRECTORY_FLAGS: u32 = FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT;
    const DIRECTORY_READ_ACCESS: u32 =
        FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE;
    const DIRECTORY_OWNED_ACCESS: u32 = DIRECTORY_READ_ACCESS
        | FILE_ADD_FILE
        | FILE_ADD_SUBDIRECTORY
        | FILE_DELETE_CHILD
        | FILE_WRITE_ATTRIBUTES
        | DELETE;
    const REGULAR_READ_ACCESS: u32 = FILE_GENERIC_READ | FILE_EXECUTE | SYNCHRONIZE;
    const REGULAR_OWNED_ACCESS: u32 = FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE | SYNCHRONIZE;
    const HELD_SHARE: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
    const OBJ_CASE_INSENSITIVE: u32 = 0x40;

    #[derive(Clone, Copy, Eq, PartialEq)]
    struct Identity {
        volume: u64,
        file_id: [u8; 16],
        attributes: u32,
        length: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct DirectoryIdentity {
        volume: u64,
        file_id: [u8; 16],
        stable_attributes: u32,
    }

    pub struct Directory {
        file: File,
        identity: DirectoryIdentity,
    }

    pub struct RegularFile {
        file: File,
        identity: Identity,
        digest: [u8; 32],
    }

    pub struct Executable {
        file: RegularFile,
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

    pub struct PreparedRelativeName(Vec<u16>);

    pub struct PreparedRelativeNameArena {
        units: Vec<u16>,
        maximum: usize,
    }

    pub struct PreparedVersionInvocation {
        command_line: Vec<u16>,
        output: Vec<u8>,
    }

    pub struct PreparedSysrootInvocation(PreparedVersionInvocation);
    pub struct PreparedRustcVersionInvocation(PreparedVersionInvocation);

    const PROCESS_PATH_UNITS: usize = 32_769;
    const MAX_TOOL_PATH_UNITS: usize = 32_768;
    const MAX_COMMAND_LINE_UNITS: usize = 32_767;
    const MAX_PROCESS_ENVIRONMENT_UNITS: usize = 32_768;
    const MAX_PROCESS_ATTRIBUTE_BYTES: usize = 1_048_576;

    pub struct PreparedProcessArena {
        application: Vec<u16>,
        cwd: Vec<u16>,
        environment: Vec<u16>,
        attributes: Vec<u64>,
        attribute_bytes: usize,
        remaining: usize,
    }

    pub struct PreparedProcessArenaPlan {
        uses: usize,
        attribute_bytes: usize,
        attribute_words: usize,
        environment_units: usize,
        owned_capacity: usize,
    }

    pub struct PreparedToolResolver {
        candidate: Vec<u16>,
        canonical: Vec<u16>,
        display: String,
        fallback: Vec<u16>,
        maximum: usize,
    }

    struct PreparedCommand {
        arguments: Vec<String>,
        command_line: Vec<u16>,
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
    pub struct PreparedRunInvocation(PreparedCommand);

    const WINDOWS_DYNAMIC_CRT_LINK_ARGS: [&str; 2] = ["-Xlinker", "/NODEFAULTLIB:libcmt"];

    const WINDOWS_RUST_STATICLIB_NATIVE_LIBS: [&str; 7] = [
        "-lkernel32",
        "-ladvapi32",
        "-ldbghelp",
        "-lntdll",
        "-luserenv",
        "-lws2_32",
        "-lmsvcrt",
    ];

    fn prepare_command(values: &[&str], output_capacity: usize) -> Result<PreparedCommand, Error> {
        let mut arguments = Vec::with_capacity(values.len());
        if arguments.capacity() != values.len() {
            return Err(Error::OutputLimit);
        }
        for value in values {
            arguments.push((*value).to_owned());
        }
        let command_line = windows_command_line(&arguments)?;
        let output = Vec::with_capacity(output_capacity);
        if output.capacity() != output_capacity {
            return Err(Error::OutputLimit);
        }
        Ok(PreparedCommand {
            arguments,
            command_line,
            output,
        })
    }

    fn prepared_command_owned_capacity(command: &PreparedCommand) -> usize {
        command
            .arguments
            .capacity()
            .saturating_mul(std::mem::size_of::<String>())
            .saturating_add(
                command
                    .arguments
                    .iter()
                    .map(String::capacity)
                    .sum::<usize>(),
            )
            .saturating_add(
                command
                    .command_line
                    .capacity()
                    .saturating_mul(std::mem::size_of::<u16>()),
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
            || fallback.contains(['/', '\\', '\0'])
        {
            return Err(Error::Invalid);
        }
        let candidate = Vec::with_capacity(maximum);
        let canonical = Vec::with_capacity(maximum);
        let display_capacity = maximum.checked_mul(3).ok_or(Error::OutputLimit)?;
        let display = String::with_capacity(display_capacity);
        let fallback = fallback.encode_utf16().collect::<Vec<_>>();
        if candidate.capacity() != maximum
            || canonical.capacity() != maximum
            || display.capacity() != display_capacity
        {
            return Err(Error::OutputLimit);
        }
        Ok(PreparedToolResolver {
            candidate,
            canonical,
            display,
            fallback,
            maximum,
        })
    }

    pub fn prepared_tool_resolver_owned_capacity(prepared: &PreparedToolResolver) -> usize {
        prepared
            .candidate
            .capacity()
            .saturating_add(prepared.canonical.capacity())
            .saturating_add(prepared.fallback.capacity())
            .saturating_mul(std::mem::size_of::<u16>())
            .saturating_add(prepared.display.capacity())
    }

    pub fn prepare_version_invocation(
        argument: &str,
        maximum: usize,
    ) -> Result<PreparedVersionInvocation, Error> {
        if maximum > 65_536 {
            return Err(Error::OutputLimit);
        }
        let command_line = windows_command_line(&[argument.to_owned()])?;
        let output = Vec::with_capacity(maximum);
        if output.capacity() != maximum {
            return Err(Error::OutputLimit);
        }
        Ok(PreparedVersionInvocation {
            command_line,
            output,
        })
    }

    pub fn prepared_version_owned_capacity(prepared: &PreparedVersionInvocation) -> usize {
        prepared
            .command_line
            .capacity()
            .saturating_mul(std::mem::size_of::<u16>())
            .saturating_add(prepared.output.capacity())
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

    pub fn prepared_rustc_version_owned_capacity(
        prepared: &PreparedRustcVersionInvocation,
    ) -> usize {
        prepared_version_owned_capacity(&prepared.0)
    }

    pub(super) fn process_arena_plan(
        uses: usize,
        attribute_bytes: usize,
        environment_units: usize,
    ) -> Result<PreparedProcessArenaPlan, Error> {
        if uses == 0 || uses > 32 {
            return Err(Error::Invalid);
        }
        if attribute_bytes == 0 {
            return Err(Error::Unsupported);
        }
        if attribute_bytes > MAX_PROCESS_ATTRIBUTE_BYTES {
            return Err(Error::OutputLimit);
        }
        if !(2..=MAX_PROCESS_ENVIRONMENT_UNITS).contains(&environment_units) {
            return Err(Error::OutputLimit);
        }
        let attribute_words = attribute_bytes
            .checked_add(std::mem::size_of::<u64>() - 1)
            .and_then(|bytes| bytes.checked_div(std::mem::size_of::<u64>()))
            .ok_or(Error::OutputLimit)?;
        let path_capacity = PROCESS_PATH_UNITS
            .checked_mul(std::mem::size_of::<u16>())
            .and_then(|bytes| bytes.checked_mul(2))
            .ok_or(Error::OutputLimit)?;
        let owned_capacity = attribute_words
            .checked_mul(std::mem::size_of::<u64>())
            .and_then(|bytes| path_capacity.checked_add(bytes))
            .and_then(|bytes| {
                environment_units
                    .checked_mul(std::mem::size_of::<u16>())
                    .and_then(|environment| bytes.checked_add(environment))
            })
            .ok_or(Error::OutputLimit)?;
        Ok(PreparedProcessArenaPlan {
            uses,
            attribute_bytes,
            attribute_words,
            environment_units,
            owned_capacity,
        })
    }

    fn process_environment_units(
        include: Option<&OsStr>,
        libraries: Option<&OsStr>,
    ) -> Result<usize, Error> {
        match (include, libraries) {
            (None, None) => Ok(2),
            (Some(include), Some(libraries)) => {
                let include_units = include.encode_wide().try_fold(0usize, |count, unit| {
                    if unit == 0 {
                        Err(Error::Invalid)
                    } else {
                        count.checked_add(1).ok_or(Error::OutputLimit)
                    }
                })?;
                let library_units = libraries.encode_wide().try_fold(0usize, |count, unit| {
                    if unit == 0 {
                        Err(Error::Invalid)
                    } else {
                        count.checked_add(1).ok_or(Error::OutputLimit)
                    }
                })?;
                8usize
                    .checked_add(include_units)
                    .and_then(|units| units.checked_add(1))
                    .and_then(|units| units.checked_add(4))
                    .and_then(|units| units.checked_add(library_units))
                    .and_then(|units| units.checked_add(2))
                    .filter(|units| *units <= MAX_PROCESS_ENVIRONMENT_UNITS)
                    .ok_or(Error::OutputLimit)
            }
            _ => Err(Error::Invalid),
        }
    }

    pub fn prepare_process_arena_plan(uses: usize) -> Result<PreparedProcessArenaPlan, Error> {
        if uses == 0 || uses > 32 {
            return Err(Error::Invalid);
        }
        let mut attribute_bytes = 0_usize;
        let initialized = unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes)
        };
        if initialized != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
            return Err(Error::Unsupported);
        }
        process_arena_plan(uses, attribute_bytes, 2)
    }

    pub fn prepare_process_arena_plan_with_environment(
        uses: usize,
        include: Option<&OsStr>,
        libraries: Option<&OsStr>,
    ) -> Result<PreparedProcessArenaPlan, Error> {
        let environment_units = process_environment_units(include, libraries)?;
        let mut attribute_bytes = 0_usize;
        let initialized = unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes)
        };
        if initialized != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
            return Err(Error::Unsupported);
        }
        process_arena_plan(uses, attribute_bytes, environment_units)
    }

    pub fn prepared_process_arena_plan_capacity(plan: &PreparedProcessArenaPlan) -> usize {
        plan.owned_capacity
    }

    pub fn materialize_process_arena(
        plan: PreparedProcessArenaPlan,
    ) -> Result<PreparedProcessArena, Error> {
        materialize_process_arena_with_environment(plan, None, None)
    }

    pub fn materialize_process_arena_with_environment(
        plan: PreparedProcessArenaPlan,
        include: Option<&OsStr>,
        libraries: Option<&OsStr>,
    ) -> Result<PreparedProcessArena, Error> {
        if process_environment_units(include, libraries)? != plan.environment_units {
            return Err(Error::Invalid);
        }
        let application = Vec::with_capacity(PROCESS_PATH_UNITS);
        let cwd = Vec::with_capacity(PROCESS_PATH_UNITS);
        let mut environment = Vec::with_capacity(plan.environment_units);
        match (include, libraries) {
            (None, None) => environment.extend([0, 0]),
            (Some(include), Some(libraries)) => {
                environment.extend("INCLUDE=".encode_utf16());
                environment.extend(include.encode_wide());
                environment.push(0);
                environment.extend("LIB=".encode_utf16());
                environment.extend(libraries.encode_wide());
                environment.extend([0, 0]);
            }
            _ => return Err(Error::Invalid),
        }
        let attributes = Vec::with_capacity(plan.attribute_words);
        if application.capacity() != PROCESS_PATH_UNITS
            || cwd.capacity() != PROCESS_PATH_UNITS
            || environment.capacity() != plan.environment_units
            || environment.len() != plan.environment_units
            || attributes.capacity() != plan.attribute_words
        {
            return Err(Error::OutputLimit);
        }
        Ok(PreparedProcessArena {
            application,
            cwd,
            environment,
            attributes,
            attribute_bytes: plan.attribute_bytes,
            remaining: plan.uses,
        })
    }

    pub fn prepare_process_arena(uses: usize) -> Result<PreparedProcessArena, Error> {
        materialize_process_arena(prepare_process_arena_plan(uses)?)
    }

    pub fn prepared_process_arena_owned_capacity(prepared: &PreparedProcessArena) -> usize {
        prepared
            .application
            .capacity()
            .saturating_mul(std::mem::size_of::<u16>())
            .saturating_add(
                prepared
                    .environment
                    .capacity()
                    .saturating_mul(std::mem::size_of::<u16>()),
            )
            .saturating_add(
                prepared
                    .cwd
                    .capacity()
                    .saturating_mul(std::mem::size_of::<u16>()),
            )
            .saturating_add(
                prepared
                    .attributes
                    .capacity()
                    .saturating_mul(std::mem::size_of::<u64>()),
            )
    }

    pub fn prepared_process_arena_remaining(prepared: &PreparedProcessArena) -> usize {
        prepared.remaining
    }

    pub(super) fn consume_process_arena(prepared: &mut PreparedProcessArena) -> Result<(), Error> {
        let attribute_words = prepared
            .attribute_bytes
            .checked_add(std::mem::size_of::<u64>() - 1)
            .and_then(|bytes| bytes.checked_div(std::mem::size_of::<u64>()))
            .ok_or(Error::OutputLimit)?;
        if prepared.application.capacity() != PROCESS_PATH_UNITS
            || prepared.cwd.capacity() != PROCESS_PATH_UNITS
            || !(2..=MAX_PROCESS_ENVIRONMENT_UNITS).contains(&prepared.environment.capacity())
            || prepared.environment.len() != prepared.environment.capacity()
            || !prepared.environment.ends_with(&[0, 0])
            || prepared.attributes.capacity() != attribute_words
        {
            return Err(Error::OutputLimit);
        }
        prepared.remaining = prepared
            .remaining
            .checked_sub(1)
            .ok_or(Error::OutputLimit)?;
        prepared.application.clear();
        prepared.cwd.clear();
        prepared.attributes.clear();
        Ok(())
    }

    pub struct PreparedDiscardNames<const N: usize> {
        names: [Option<PreparedRelativeName>; N],
    }

    pub struct PreparedLinkOrCopy {
        destination_index: usize,
        storage: Box<[usize]>,
        total: usize,
        #[cfg(debug_assertions)]
        fail_before_authentication: bool,
    }

    const INVENTORY_EXACT_ARENA_WORDS: usize = 8192;

    pub struct PreparedInventoryExact<const N: usize> {
        names: [Option<PreparedRelativeName>; N],
        bindings: [(usize, usize); N],
        storage: Box<[u64]>,
        directory_identity: Option<DirectoryIdentity>,
        remaining: u8,
    }

    pub struct PreparedPublishDirectory {
        storage: Box<[usize]>,
        total: usize,
        name_units: usize,
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

    fn prepared_name_bindings<const N: usize>(
        names: &PreparedDiscardNames<N>,
    ) -> Result<[(usize, usize); N], Error> {
        let mut bindings = [(0, 0); N];
        for (index, binding) in bindings.iter_mut().enumerate() {
            let name = prepared_discard_name(names, index)?;
            *binding = (name.0.as_ptr() as usize, name.0.len());
        }
        Ok(bindings)
    }

    pub fn inventory_exact_required_capacity<const N: usize>(
        names: &PreparedDiscardNames<N>,
    ) -> Result<usize, Error> {
        let names = (0..N).try_fold(0usize, |total, index| {
            total
                .checked_add(
                    prepared_discard_name(names, index)?
                        .0
                        .len()
                        .checked_mul(std::mem::size_of::<u16>())
                        .ok_or(Error::OutputLimit)?,
                )
                .ok_or(Error::OutputLimit)
        })?;
        names
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
            let mut units = Vec::with_capacity(source.0.len());
            units.extend_from_slice(&source.0);
            if units.capacity() != source.0.len() {
                return Err(Error::OutputLimit);
            }
            *slot = Some(PreparedRelativeName(units));
        }
        let storage = vec![0_u64; INVENTORY_EXACT_ARENA_WORDS].into_boxed_slice();
        Ok(PreparedInventoryExact {
            names: copied,
            bindings,
            storage,
            directory_identity: None,
            remaining: 2,
        })
    }

    pub fn prepared_inventory_exact_owned_capacity<const N: usize>(
        prepared: &PreparedInventoryExact<N>,
    ) -> usize {
        prepared
            .names
            .iter()
            .filter_map(Option::as_ref)
            .map(|name| name.0.capacity().saturating_mul(std::mem::size_of::<u16>()))
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

    fn publish_information_layout(name_units: usize) -> Result<(usize, usize), Error> {
        let name_bytes = name_units.checked_mul(2).ok_or(Error::OutputLimit)?;
        let fixed = std::mem::offset_of!(NamedInformation, file_name);
        let total = fixed
            .checked_add(name_bytes)
            .ok_or(Error::OutputLimit)?
            .max(std::mem::size_of::<NamedInformation>());
        let words = total
            .checked_add(std::mem::size_of::<usize>() - 1)
            .ok_or(Error::OutputLimit)?
            / std::mem::size_of::<usize>();
        Ok((total, words))
    }

    pub fn publish_directory_required_capacity(name: &OsStr) -> Result<usize, Error> {
        let text = prepared_normal_name(name)?;
        let (_, words) = publish_information_layout(text.encode_utf16().count())?;
        words
            .checked_mul(std::mem::size_of::<usize>())
            .ok_or(Error::OutputLimit)
    }

    pub fn prepare_publish_directory(name: &OsStr) -> Result<PreparedPublishDirectory, Error> {
        let text = prepared_normal_name(name)?;
        let name_units = text.encode_utf16().count();
        let (total, words) = publish_information_layout(name_units)?;
        let exact_capacity = words
            .checked_mul(std::mem::size_of::<usize>())
            .ok_or(Error::OutputLimit)?;
        let mut storage = vec![0_usize; words].into_boxed_slice();
        let information = storage.as_mut_ptr().cast::<NamedInformation>();
        unsafe {
            (*information).flags = 0;
            (*information).root_directory = std::ptr::null_mut();
            (*information).file_name_length =
                u32::try_from(name_units.checked_mul(2).ok_or(Error::OutputLimit)?)
                    .map_err(|_| Error::OutputLimit)?;
            let destination = std::ptr::addr_of_mut!((*information).file_name).cast::<u16>();
            for (index, unit) in text.encode_utf16().enumerate() {
                destination.add(index).write(unit);
            }
        }
        Ok(PreparedPublishDirectory {
            storage,
            total,
            name_units,
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
        prepared
            .storage
            .len()
            .saturating_mul(std::mem::size_of::<usize>())
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

    pub fn prepare_link_or_copy<const N: usize>(
        names: &PreparedDiscardNames<N>,
        destination_index: usize,
    ) -> Result<PreparedLinkOrCopy, Error> {
        let name = prepared_discard_name(names, destination_index)?;
        let (total, words) = link_information_layout(name)?;
        let name_bytes = name.0.len().checked_mul(2).ok_or(Error::OutputLimit)?;
        let mut storage = vec![0_usize; words].into_boxed_slice();
        let information = storage.as_mut_ptr().cast::<NamedInformation>();
        unsafe {
            (*information).flags = 0;
            (*information).root_directory = std::ptr::null_mut();
            (*information).file_name_length =
                u32::try_from(name_bytes).map_err(|_| Error::OutputLimit)?;
            std::ptr::copy_nonoverlapping(
                name.0.as_ptr(),
                std::ptr::addr_of_mut!((*information).file_name).cast::<u16>(),
                name.0.len(),
            );
        }
        Ok(PreparedLinkOrCopy {
            destination_index,
            storage,
            total,
            #[cfg(debug_assertions)]
            fail_before_authentication: false,
        })
    }

    fn link_information_layout(name: &PreparedRelativeName) -> Result<(usize, usize), Error> {
        let name_bytes = name.0.len().checked_mul(2).ok_or(Error::OutputLimit)?;
        let fixed = std::mem::offset_of!(NamedInformation, file_name);
        let total = fixed
            .checked_add(name_bytes)
            .ok_or(Error::OutputLimit)?
            .max(std::mem::size_of::<NamedInformation>());
        let words = total
            .checked_add(std::mem::size_of::<usize>() - 1)
            .ok_or(Error::OutputLimit)?
            / std::mem::size_of::<usize>();
        Ok((total, words))
    }

    pub fn link_or_copy_required_capacity<const N: usize>(
        names: &PreparedDiscardNames<N>,
        destination_index: usize,
    ) -> Result<usize, Error> {
        let name = prepared_discard_name(names, destination_index)?;
        let (_, words) = link_information_layout(name)?;
        words
            .checked_mul(std::mem::size_of::<usize>())
            .ok_or(Error::OutputLimit)
    }

    pub fn prepared_link_or_copy_owned_capacity(prepared: &PreparedLinkOrCopy) -> usize {
        prepared
            .storage
            .len()
            .saturating_mul(std::mem::size_of::<usize>())
    }

    #[cfg(debug_assertions)]
    pub fn inject_link_or_copy_failure_before_authentication(prepared: &mut PreparedLinkOrCopy) {
        prepared.fail_before_authentication = true;
    }

    fn ascii_fold(value: u16) -> u16 {
        if value >= u16::from(b'a') && value <= u16::from(b'z') {
            value - u16::from(b'a' - b'A')
        } else {
            value
        }
    }

    fn prepared_names_equal(left: &PreparedRelativeName, right: &PreparedRelativeName) -> bool {
        left.0.len() == right.0.len()
            && left
                .0
                .iter()
                .zip(&right.0)
                .all(|(left, right)| ascii_fold(*left) == ascii_fold(*right))
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

    fn prepared_matches_slice(expected: &PreparedRelativeName, actual: &[u16]) -> bool {
        expected.0.len() == actual.len()
            && expected
                .0
                .iter()
                .zip(actual)
                .all(|(expected, actual)| ascii_fold(*expected) == ascii_fold(*actual))
    }

    fn prepared_normal_name(name: &OsStr) -> Result<&str, Error> {
        let text = name.to_str().ok_or(Error::Invalid)?;
        if text.is_empty()
            || !text.is_ascii()
            || matches!(text, "." | "..")
            || text.contains(['/', '\\', '\0'])
            || text.ends_with([' ', '.'])
            || text.contains(':')
        {
            return Err(Error::Invalid);
        }
        let stem = text.split('.').next().ok_or(Error::Invalid)?;
        if ["CON", "PRN", "AUX", "NUL", "CLOCK$"]
            .iter()
            .any(|reserved| stem.eq_ignore_ascii_case(reserved))
            || (stem.len() == 4
                && (stem[..3].eq_ignore_ascii_case("COM") || stem[..3].eq_ignore_ascii_case("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(Error::Invalid);
        }
        Ok(text)
    }

    pub fn prepare_relative_name(name: &OsStr) -> Result<PreparedRelativeName, Error> {
        let text = prepared_normal_name(name)?;
        let exact = text.encode_utf16().count();
        let mut encoded = Vec::with_capacity(exact);
        encoded.extend(text.encode_utf16());
        if encoded.len() != exact || encoded.capacity() != exact {
            return Err(Error::OutputLimit);
        }
        Ok(PreparedRelativeName(encoded))
    }

    pub fn prepare_relative_name_arena(maximum: usize) -> Result<PreparedRelativeNameArena, Error> {
        let units = Vec::with_capacity(maximum);
        if units.capacity() != maximum {
            return Err(Error::OutputLimit);
        }
        Ok(PreparedRelativeNameArena { units, maximum })
    }

    pub fn set_relative_name_arena(
        arena: &mut PreparedRelativeNameArena,
        name: &OsStr,
    ) -> Result<(), Error> {
        let text = prepared_normal_name(name)?;
        if text.encode_utf16().count() > arena.maximum {
            return Err(Error::OutputLimit);
        }
        arena.units.clear();
        arena.units.extend(text.encode_utf16());
        if arena.units.capacity() != arena.maximum {
            return Err(Error::OutputLimit);
        }
        Ok(())
    }

    pub fn relative_name_arena_capacity(arena: &PreparedRelativeNameArena) -> usize {
        arena.units.capacity()
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
                if prepared_names_equal(
                    names[left].as_ref().expect("validated"),
                    names[right].as_ref().expect("validated"),
                ) {
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
            .map(|name| name.0.capacity().saturating_mul(std::mem::size_of::<u16>()))
            .sum()
    }

    fn normal_name(name: &OsStr) -> Result<(), Error> {
        let text = name.to_str().ok_or(Error::Invalid)?;
        if text.is_empty()
            || !text.is_ascii()
            || matches!(text, "." | "..")
            || text.contains(['/', '\\', '\0'])
            || text.ends_with([' ', '.'])
            || text.contains(':')
        {
            return Err(Error::Invalid);
        }
        let stem = text
            .split('.')
            .next()
            .ok_or(Error::Invalid)?
            .to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
            || stem.strip_prefix("COM").is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
            || stem.strip_prefix("LPT").is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    fn open_directory(path: &Path) -> Result<File, Error> {
        open_absolute(path, DIRECTORY_READ_ACCESS, DIRECTORY_FLAGS)
    }

    fn open_absolute(path: &Path, access: u32, flags: u32) -> Result<File, Error> {
        let path = wide_null(path.as_os_str())?;
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                access,
                HELD_SHARE,
                std::ptr::null(),
                OPEN_EXISTING,
                flags,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(Error::Changed);
        }
        Ok(unsafe { File::from_raw_handle(handle.cast()) })
    }

    fn relative_file(
        parent: &File,
        name: &OsStr,
        access: u32,
        disposition: u32,
        create_options: u32,
    ) -> Result<File, Error> {
        let name = prepare_relative_name(name)?;
        relative_file_prepared(parent, &name, access, disposition, create_options)
    }

    fn relative_file_prepared(
        parent: &File,
        name: &PreparedRelativeName,
        access: u32,
        disposition: u32,
        create_options: u32,
    ) -> Result<File, Error> {
        relative_file_units(parent, &name.0, access, disposition, create_options)
    }

    fn relative_file_arena(
        parent: &File,
        name: &PreparedRelativeNameArena,
        access: u32,
        disposition: u32,
        create_options: u32,
    ) -> Result<File, Error> {
        relative_file_units(parent, &name.units, access, disposition, create_options)
    }

    fn relative_file_units(
        parent: &File,
        name: &[u16],
        access: u32,
        disposition: u32,
        create_options: u32,
    ) -> Result<File, Error> {
        let byte_length = name.len().checked_mul(2).ok_or(Error::Invalid)?;
        let length = u16::try_from(byte_length).map_err(|_| Error::Invalid)?;
        let unicode = UNICODE_STRING {
            Length: length,
            MaximumLength: length,
            Buffer: name.as_ptr().cast_mut(),
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: u32::try_from(std::mem::size_of::<OBJECT_ATTRIBUTES>())
                .map_err(|_| Error::Changed)?,
            RootDirectory: parent.as_raw_handle().cast(),
            ObjectName: &unicode,
            Attributes: OBJ_CASE_INSENSITIVE,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        let mut io = IO_STATUS_BLOCK::default();
        let mut handle = std::ptr::null_mut();
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                access,
                &attributes,
                &mut io,
                std::ptr::null(),
                FILE_ATTRIBUTE_NORMAL,
                HELD_SHARE,
                disposition,
                create_options | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null(),
                0,
            )
        };
        if status < 0 {
            return Err(if status == STATUS_OBJECT_NAME_COLLISION {
                Error::Exists
            } else {
                Error::Changed
            });
        }
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(Error::Changed);
        }
        Ok(unsafe { File::from_raw_handle(handle.cast()) })
    }

    fn open_relative_regular_read(parent: &Directory, name: &OsStr) -> Result<File, Error> {
        relative_file(
            &parent.file,
            name,
            REGULAR_READ_ACCESS,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        )
    }

    fn information(file: &File) -> Result<Identity, Error> {
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        if unsafe {
            GetFileInformationByHandle(
                file.as_raw_handle().cast::<core::ffi::c_void>(),
                &mut information,
            )
        } == 0
        {
            return Err(Error::Changed);
        }
        let mut file_id = FILE_ID_INFO::default();
        if unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle().cast::<core::ffi::c_void>(),
                FileIdInfo,
                (&mut file_id as *mut FILE_ID_INFO).cast(),
                u32::try_from(std::mem::size_of::<FILE_ID_INFO>()).map_err(|_| Error::Changed)?,
            )
        } == 0
        {
            return Err(Error::Changed);
        }
        Ok(Identity {
            volume: file_id.VolumeSerialNumber,
            file_id: file_id.FileId.Identifier,
            attributes: information.dwFileAttributes,
            length: (u64::from(information.nFileSizeHigh) << 32)
                | u64::from(information.nFileSizeLow),
        })
    }

    fn stable_directory_identity(identity: Identity) -> Result<DirectoryIdentity, Error> {
        let stable_attributes =
            identity.attributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT);
        if stable_attributes != FILE_ATTRIBUTE_DIRECTORY {
            return Err(Error::Changed);
        }
        Ok(DirectoryIdentity {
            volume: identity.volume,
            file_id: identity.file_id,
            stable_attributes,
        })
    }

    fn directory_information(file: &File) -> Result<DirectoryIdentity, Error> {
        let identity = stable_directory_identity(information(file)?)?;
        if !file.metadata().map_err(|_| Error::Changed)?.is_dir() {
            return Err(Error::Changed);
        }
        Ok(identity)
    }

    fn digest(file: &File, length: u64) -> Result<[u8; 32], Error> {
        let mut file = file.try_clone().map_err(|_| Error::Changed)?;
        file.seek(SeekFrom::Start(0)).map_err(|_| Error::Changed)?;
        let mut hasher = Sha256::new();
        let mut remaining = length;
        let mut buffer = [0_u8; 8192];
        while remaining != 0 {
            let maximum = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|_| Error::OutputLimit)?;
            let count = file
                .read(&mut buffer[..maximum])
                .map_err(|_| Error::Changed)?;
            if count == 0 {
                return Err(Error::Changed);
            }
            hasher.update(&buffer[..count]);
            remaining -= u64::try_from(count).map_err(|_| Error::OutputLimit)?;
        }
        Ok(hasher.finalize().into())
    }

    fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }

    fn final_path_prepared(file: &File, output: &mut Vec<u16>) -> Result<(), Error> {
        let maximum = output.capacity();
        if maximum != PROCESS_PATH_UNITS {
            return Err(Error::OutputLimit);
        }
        output.clear();
        output.resize(maximum, 0);
        let written = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle().cast(),
                output.as_mut_ptr(),
                u32::try_from(maximum).map_err(|_| Error::OutputLimit)?,
                0,
            )
        };
        let written = usize::try_from(written).map_err(|_| Error::Changed)?;
        if written == 0 || written >= maximum {
            return Err(Error::OutputLimit);
        }
        output.truncate(written);
        let prefix = [
            u16::from(b'\\'),
            u16::from(b'\\'),
            u16::from(b'?'),
            u16::from(b'\\'),
        ];
        if output.starts_with(&prefix) {
            output.copy_within(prefix.len().., 0);
            output.truncate(written - prefix.len());
        }
        output.push(0);
        if output.capacity() != maximum {
            return Err(Error::OutputLimit);
        }
        Ok(())
    }

    pub fn hold_directory(path: &Path) -> Result<Directory, Error> {
        if !path.is_absolute() {
            return Err(Error::Invalid);
        }
        let canonical = path.canonicalize().map_err(|_| Error::Changed)?;
        if canonical != path {
            return Err(Error::Changed);
        }
        let file = open_directory(path)?;
        let identity = directory_information(&file)?;
        Ok(Directory { file, identity })
    }

    pub fn recheck_directory(directory: &Directory) -> Result<(), Error> {
        if directory_information(&directory.file)? != directory.identity {
            return Err(Error::Changed);
        }
        Ok(())
    }

    pub fn same_directory_path(directory: &Directory, path: &Path) -> Result<bool, Error> {
        if !path.is_absolute() {
            return Err(Error::Invalid);
        }
        let rebound = open_directory(path)?;
        Ok(directory_information(&rebound)? == directory.identity)
    }

    pub fn create_directory_new(
        parent: &Directory,
        name: &OsStr,
        _mode: u32,
    ) -> Result<Directory, Error> {
        recheck_directory(parent)?;
        normal_name(name)?;
        let file = relative_file(
            &parent.file,
            name,
            DIRECTORY_OWNED_ACCESS,
            FILE_CREATE,
            FILE_DIRECTORY_FILE,
        )?;
        let identity = directory_information(&file)?;
        Ok(Directory { file, identity })
    }

    pub fn create_directory_new_prepared(
        parent: &Directory,
        name: &PreparedRelativeNameArena,
        _mode: u32,
    ) -> Result<Directory, Error> {
        recheck_directory(parent)?;
        let file = relative_file_arena(
            &parent.file,
            name,
            DIRECTORY_OWNED_ACCESS,
            FILE_CREATE,
            FILE_DIRECTORY_FILE,
        )?;
        let identity = directory_information(&file)?;
        Ok(Directory { file, identity })
    }

    pub fn write_file_new(
        directory: &Directory,
        name: &OsStr,
        bytes: &[u8],
        _mode: u32,
    ) -> Result<RegularFile, Error> {
        recheck_directory(directory)?;
        normal_name(name)?;
        let mut file = relative_file(
            &directory.file,
            name,
            REGULAR_OWNED_ACCESS,
            FILE_CREATE,
            FILE_NON_DIRECTORY_FILE,
        )?;
        file.write_all(bytes).map_err(|_| Error::Changed)?;
        file.sync_all().map_err(|_| Error::Changed)?;
        let identity = information(&file)?;
        if identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || !file.metadata().map_err(|_| Error::Changed)?.is_file()
        {
            return Err(Error::Changed);
        }
        let digest = digest(&file, identity.length)?;
        Ok(RegularFile {
            file,
            identity,
            digest,
        })
    }

    pub fn write_file_new_prepared<const N: usize>(
        directory: &Directory,
        names: &PreparedDiscardNames<N>,
        index: usize,
        bytes: &[u8],
        _mode: u32,
    ) -> Result<RegularFile, Error> {
        let name = enter_prepared_file_syscalls(prepared_discard_name(names, index))?;
        recheck_directory(directory)?;
        let mut file = relative_file_prepared(
            &directory.file,
            name,
            REGULAR_OWNED_ACCESS,
            FILE_CREATE,
            FILE_NON_DIRECTORY_FILE,
        )?;
        file.write_all(bytes).map_err(|_| Error::Changed)?;
        file.sync_all().map_err(|_| Error::Changed)?;
        authenticate_regular_file(file)
    }

    pub fn hold_regular_file(directory: &Directory, name: &OsStr) -> Result<RegularFile, Error> {
        recheck_directory(directory)?;
        let name = prepare_relative_name(name)?;
        hold_regular_file_name_prepared(directory, &name)
    }

    fn authenticate_regular_file(file: File) -> Result<RegularFile, Error> {
        let identity = information(&file)?;
        if identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || !file.metadata().map_err(|_| Error::Changed)?.is_file()
        {
            return Err(Error::Changed);
        }
        let digest = digest(&file, identity.length)?;
        Ok(RegularFile {
            file,
            identity,
            digest,
        })
    }

    fn hold_regular_file_name_prepared(
        directory: &Directory,
        name: &PreparedRelativeName,
    ) -> Result<RegularFile, Error> {
        let file = relative_file_prepared(
            &directory.file,
            name,
            REGULAR_OWNED_ACCESS,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        )?;
        authenticate_regular_file(file)
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
        if rebound.identity != tracked.identity || rebound.digest != tracked.digest {
            return Err(Error::Changed);
        }
        Ok(rebound)
    }

    pub fn recheck_regular(file: &RegularFile) -> Result<(), Error> {
        recheck_held_regular(file)
    }

    fn recheck_held_regular(file: &RegularFile) -> Result<(), Error> {
        let held = information(&file.file)?;
        if held != file.identity || digest(&file.file, file.identity.length)? != file.digest {
            return Err(Error::Changed);
        }
        Ok(())
    }

    pub fn hold_external_executable(path: &Path) -> Result<Executable, Error> {
        let parent_path = path
            .parent()
            .ok_or(Error::Invalid)?
            .canonicalize()
            .map_err(|_| Error::Changed)?;
        let directory = hold_directory(&parent_path)?;
        let name = path.file_name().ok_or(Error::Invalid)?;
        normal_name(name)?;
        let file = open_relative_regular_read(&directory, name)?;
        let identity = information(&file)?;
        if identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || !file.metadata().map_err(|_| Error::Changed)?.is_file()
        {
            return Err(Error::Changed);
        }
        let digest = digest(&file, identity.length)?;
        let regular = RegularFile {
            file,
            identity,
            digest,
        };
        let mut prefix = [0_u8; 2];
        let mut duplicate = regular.file.try_clone().map_err(|_| Error::Changed)?;
        duplicate
            .seek(SeekFrom::Start(0))
            .map_err(|_| Error::Changed)?;
        duplicate
            .read_exact(&mut prefix)
            .map_err(|_| Error::Invalid)?;
        if prefix != *b"MZ" {
            return Err(Error::Invalid);
        }
        Ok(Executable { file: regular })
    }

    fn append_tool_fallback(prepared: &mut PreparedToolResolver) -> Result<(), Error> {
        if prepared.candidate.first() == Some(&u16::from(b'"'))
            && prepared.candidate.last() == Some(&u16::from(b'"'))
            && prepared.candidate.len() >= 2
        {
            prepared.candidate.remove(0);
            prepared.candidate.pop();
        }
        if prepared.candidate.is_empty() {
            prepared.candidate.push(u16::from(b'.'));
        }
        if !prepared.candidate.ends_with(&[u16::from(b'/')])
            && !prepared.candidate.ends_with(&[u16::from(b'\\')])
        {
            prepared.candidate.push(u16::from(b'\\'));
        }
        if prepared
            .candidate
            .len()
            .checked_add(prepared.fallback.len())
            .and_then(|length| length.checked_add(1))
            .is_none_or(|length| length > prepared.maximum)
        {
            return Err(Error::OutputLimit);
        }
        prepared.candidate.extend_from_slice(&prepared.fallback);
        Ok(())
    }

    fn hold_tool_candidate(
        prepared: &mut PreparedToolResolver,
        record_display: bool,
    ) -> Result<Option<Executable>, Error> {
        if prepared.candidate.len().saturating_add(1) > prepared.maximum {
            return Err(Error::OutputLimit);
        }
        prepared.candidate.push(0);
        let handle = unsafe {
            CreateFileW(
                prepared.candidate.as_ptr(),
                REGULAR_READ_ACCESS,
                HELD_SHARE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            )
        };
        prepared.candidate.pop();
        if handle == INVALID_HANDLE_VALUE {
            return Ok(None);
        }
        let file = unsafe { File::from_raw_handle(handle.cast()) };
        let identity = information(&file)?;
        if !file.metadata().map_err(|_| Error::Changed)?.is_file() {
            return Ok(None);
        }
        if record_display {
            prepared.canonical.clear();
            prepared.canonical.resize(prepared.maximum, 0);
            let written = unsafe {
                GetFinalPathNameByHandleW(
                    file.as_raw_handle().cast(),
                    prepared.canonical.as_mut_ptr(),
                    u32::try_from(prepared.canonical.len()).map_err(|_| Error::OutputLimit)?,
                    0,
                )
            };
            let written = usize::try_from(written).map_err(|_| Error::Changed)?;
            if written == 0 || written >= prepared.maximum {
                return Err(Error::OutputLimit);
            }
            prepared.canonical.truncate(written);
            let prefix = [
                u16::from(b'\\'),
                u16::from(b'\\'),
                u16::from(b'?'),
                u16::from(b'\\'),
            ];
            if prepared.canonical.starts_with(&prefix) {
                prepared.canonical.copy_within(prefix.len().., 0);
                prepared.canonical.truncate(written - prefix.len());
            }
            prepared.display.clear();
            for character in char::decode_utf16(prepared.canonical.iter().copied()) {
                prepared
                    .display
                    .push(character.map_err(|_| Error::Invalid)?);
            }
            if prepared.display.capacity() != prepared.maximum.saturating_mul(3) {
                return Err(Error::OutputLimit);
            }
        }
        let digest = digest(&file, identity.length)?;
        let regular = RegularFile {
            file,
            identity,
            digest,
        };
        let mut prefix = [0_u8; 2];
        let mut duplicate = regular.file.try_clone().map_err(|_| Error::Changed)?;
        duplicate
            .seek(SeekFrom::Start(0))
            .map_err(|_| Error::Changed)?;
        duplicate
            .read_exact(&mut prefix)
            .map_err(|_| Error::Invalid)?;
        if prefix != *b"MZ" {
            return Err(Error::Invalid);
        }
        Ok(Some(Executable { file: regular }))
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
            prepared.candidate.clear();
            for unit in configured.encode_wide() {
                if prepared.candidate.len().saturating_add(1) > prepared.maximum {
                    return Err(Error::OutputLimit);
                }
                prepared.candidate.push(unit);
            }
            if prepared.candidate.is_empty() {
                return Err(Error::Invalid);
            }
            let executable = hold_tool_candidate(&mut prepared, true)?.ok_or(Error::Changed)?;
            let path = std::mem::take(&mut prepared.display);
            return Ok((executable, path, prepared));
        }
        let paths = paths.ok_or(Error::Invalid)?;
        prepared.candidate.clear();
        for unit in paths.encode_wide().chain(std::iter::once(u16::from(b';'))) {
            if unit == u16::from(b';') {
                append_tool_fallback(&mut prepared)?;
                if let Some(executable) = hold_tool_candidate(&mut prepared, true)? {
                    let path = std::mem::take(&mut prepared.display);
                    return Ok((executable, path, prepared));
                }
                prepared.candidate.clear();
            } else {
                if prepared.candidate.len().saturating_add(2) > prepared.maximum {
                    return Err(Error::OutputLimit);
                }
                prepared.candidate.push(unit);
            }
        }
        Err(Error::Changed)
    }

    fn windows_sysroot_line_actual(output: &[u8]) -> Result<&str, Error> {
        let line = output.strip_suffix(b"\n").ok_or(Error::Invalid)?;
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() || line.contains(&0) || line.contains(&b'\n') || line.contains(&b'\r') {
            return Err(Error::Invalid);
        }
        std::str::from_utf8(line).map_err(|_| Error::Invalid)
    }

    fn windows_sysroot_directory_actual(
        prepared: &mut PreparedToolResolver,
        output: &[u8],
    ) -> Result<Directory, Error> {
        let line = windows_sysroot_line_actual(output)?;
        prepared.candidate.clear();
        for unit in line.encode_utf16() {
            if prepared.candidate.len().saturating_add(1) > prepared.maximum {
                return Err(Error::OutputLimit);
            }
            prepared.candidate.push(unit);
        }
        let drive = prepared.candidate.first().copied().ok_or(Error::Invalid)?;
        let drive_is_ascii_alphabetic =
            u8::try_from(drive).is_ok_and(|unit| unit.is_ascii_alphabetic());
        if prepared.candidate.capacity() != prepared.maximum
            || prepared.candidate.len() < 4
            || !drive_is_ascii_alphabetic
            || prepared.candidate[1] != u16::from(b':')
            || !matches!(prepared.candidate[2], 47 | 92)
            || matches!(prepared.candidate.last(), Some(47 | 92))
        {
            return Err(Error::Invalid);
        }
        let root = [prepared.candidate[0], u16::from(b':'), u16::from(b'\\'), 0];
        let handle = unsafe {
            CreateFileW(
                root.as_ptr(),
                DIRECTORY_READ_ACCESS,
                HELD_SHARE,
                std::ptr::null(),
                OPEN_EXISTING,
                DIRECTORY_FLAGS,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(Error::Changed);
        }
        let file = unsafe { File::from_raw_handle(handle.cast()) };
        let identity = directory_information(&file)?;
        let mut current = Directory { file, identity };
        let mut start = 3usize;
        while start < prepared.candidate.len() {
            let end = prepared.candidate[start..]
                .iter()
                .position(|unit| matches!(*unit, 47 | 92))
                .map_or(prepared.candidate.len(), |offset| start + offset);
            let component = &prepared.candidate[start..end];
            if component.is_empty()
                || component == [u16::from(b'.')]
                || component == [u16::from(b'.'), u16::from(b'.')]
            {
                return Err(Error::Invalid);
            }
            let file = relative_file_units(
                &current.file,
                component,
                DIRECTORY_READ_ACCESS,
                FILE_OPEN,
                FILE_DIRECTORY_FILE,
            )?;
            let identity = directory_information(&file)?;
            current = Directory { file, identity };
            start = end.saturating_add(1);
        }
        Ok(current)
    }

    pub fn hold_rustc_discovery_prepared(
        mut prepared: PreparedToolResolver,
        configured: &OsStr,
    ) -> Result<RustcDiscovery, Error> {
        if !Path::new(configured).is_absolute() {
            return Err(Error::Invalid);
        }
        prepared.candidate.clear();
        for unit in configured.encode_wide() {
            if prepared.candidate.len().saturating_add(1) >= prepared.maximum {
                return Err(Error::OutputLimit);
            }
            prepared.candidate.push(unit);
        }
        if prepared.candidate.is_empty() {
            return Err(Error::Invalid);
        }
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

    pub fn hold_direct_rustc_prepared(
        discovery: RustcDiscovery,
        output: &[u8],
    ) -> Result<DirectRustc, Error> {
        let RustcDiscovery {
            executable,
            mut resolver,
        } = discovery;
        drop(executable);
        let sysroot = windows_sysroot_directory_actual(&mut resolver, output)?;
        let bin = relative_file_units(
            &sysroot.file,
            &[98, 105, 110],
            DIRECTORY_READ_ACCESS,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        )?;
        let bin_identity = information(&bin)?;
        if bin_identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || !bin.metadata().map_err(|_| Error::Changed)?.is_dir()
        {
            return Err(Error::Changed);
        }
        let file = relative_file_units(
            &bin,
            &[114, 117, 115, 116, 99, 46, 101, 120, 101],
            REGULAR_READ_ACCESS,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        )?;
        let identity = information(&file)?;
        if identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || !file.metadata().map_err(|_| Error::Changed)?.is_file()
        {
            return Err(Error::Changed);
        }
        let digest = digest(&file, identity.length)?;
        let regular = RegularFile {
            file,
            identity,
            digest,
        };
        let mut prefix = [0_u8; 2];
        let mut duplicate = regular.file.try_clone().map_err(|_| Error::Changed)?;
        duplicate
            .seek(SeekFrom::Start(0))
            .map_err(|_| Error::Changed)?;
        duplicate
            .read_exact(&mut prefix)
            .map_err(|_| Error::Invalid)?;
        if prefix != *b"MZ" {
            return Err(Error::Invalid);
        }
        Ok(DirectRustc {
            executable: Executable { file: regular },
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
        let rebound = windows_sysroot_directory_actual(&mut prepared, output)?;
        if rebound.identity != direct.sysroot.identity {
            return Err(Error::Changed);
        }
        recheck_held_regular(&direct.executable.file)?;
        recheck_directory(&direct.sysroot)
    }

    pub fn hold_executable(directory: &Directory, name: &OsStr) -> Result<Executable, Error> {
        let file = hold_regular_file(directory, name)?;
        let mut prefix = [0_u8; 2];
        let mut duplicate = file.file.try_clone().map_err(|_| Error::Changed)?;
        duplicate
            .seek(SeekFrom::Start(0))
            .map_err(|_| Error::Changed)?;
        duplicate
            .read_exact(&mut prefix)
            .map_err(|_| Error::Invalid)?;
        if prefix != *b"MZ" {
            return Err(Error::Invalid);
        }
        Ok(Executable { file })
    }

    pub fn executable_regular_file(executable: &Executable) -> Result<RegularFile, Error> {
        recheck_held_regular(&executable.file)?;
        Ok(RegularFile {
            file: executable
                .file
                .file
                .try_clone()
                .map_err(|_| Error::Changed)?,
            identity: executable.file.identity,
            digest: executable.file.digest,
        })
    }

    pub fn read_exact(file: &RegularFile, maximum: usize) -> Result<Vec<u8>, Error> {
        recheck_regular(file)?;
        let length = usize::try_from(file.identity.length).map_err(|_| Error::OutputLimit)?;
        if length > maximum {
            return Err(Error::OutputLimit);
        }
        let mut bytes = vec![0_u8; length];
        let mut duplicate = file.file.try_clone().map_err(|_| Error::Changed)?;
        duplicate
            .seek(SeekFrom::Start(0))
            .map_err(|_| Error::Changed)?;
        duplicate
            .read_exact(&mut bytes)
            .map_err(|_| Error::Changed)?;
        recheck_regular(file)?;
        Ok(bytes)
    }

    pub fn compare_exact(
        file: &RegularFile,
        expected: &[u8],
        scratch: &mut [u8; 8192],
    ) -> Result<bool, Error> {
        recheck_regular(file)?;
        if usize::try_from(file.identity.length).map_err(|_| Error::OutputLimit)? != expected.len()
        {
            return Ok(false);
        }
        let mut offset = 0usize;
        while offset < expected.len() {
            let chunk = (expected.len() - offset).min(scratch.len());
            let count = file
                .file
                .seek_read(
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
        mut prepared: PreparedLinkOrCopy,
        source: &RegularFile,
        directory: &Directory,
        names: &PreparedDiscardNames<N>,
        destination_index: usize,
        source_bytes: &[u8],
    ) -> Result<RegularFile, Error> {
        if prepared.destination_index != destination_index {
            return Err(Error::Invalid);
        }
        let name = prepared_discard_name(names, destination_index)?;
        if usize::try_from(source.identity.length).map_err(|_| Error::OutputLimit)?
            != source_bytes.len()
            || digest_bytes(source_bytes) != source.digest
        {
            return Err(Error::Changed);
        }
        recheck_held_regular(source)?;
        recheck_directory(directory)?;
        let information = prepared.storage.as_mut_ptr().cast::<NamedInformation>();
        unsafe {
            (*information).root_directory = directory.file.as_raw_handle().cast();
        }
        let mut io = IO_STATUS_BLOCK::default();
        let status = unsafe {
            NtSetInformationFile(
                source.file.as_raw_handle().cast(),
                &mut io,
                prepared.storage.as_mut_ptr().cast(),
                u32::try_from(prepared.total).map_err(|_| Error::Invalid)?,
                FileLinkInformationEx,
            )
        };
        if status < 0 {
            return Err(if status == STATUS_OBJECT_NAME_COLLISION {
                Error::Exists
            } else {
                Error::Changed
            });
        }
        #[cfg(debug_assertions)]
        if prepared.fail_before_authentication {
            return Err(Error::Changed);
        }
        let destination = hold_regular_file_name_prepared(directory, name)?;
        if destination.identity != source.identity || destination.digest != source.digest {
            return Err(Error::Changed);
        }
        Ok(destination)
    }

    pub fn inventory_exact_prepared<const N: usize>(
        prepared: &mut PreparedInventoryExact<N>,
        directory: &Directory,
        names: &PreparedDiscardNames<N>,
        files: [Option<&RegularFile>; N],
    ) -> Result<(), Error> {
        if prepared.remaining == 0
            || prepared.bindings != prepared_name_bindings(names)?
            || files.iter().any(Option::is_none)
        {
            return Err(Error::Invalid);
        }
        match prepared.directory_identity {
            Some(first) if first != directory.identity => return Err(Error::Changed),
            None => prepared.directory_identity = Some(directory.identity),
            Some(_) => {}
        }
        prepared.remaining -= 1;
        recheck_directory(directory)?;
        for file in files.iter().flatten() {
            recheck_held_regular(file)?;
        }
        let mut seen = [false; N];
        let mut count = 0usize;
        let mut raw_records = 0usize;
        let mut queries = 0usize;
        let mut saw_dot = false;
        let mut saw_dot_dot = false;
        let maximum_records = N.checked_add(2).ok_or(Error::OutputLimit)?;
        let maximum_queries = N.checked_add(3).ok_or(Error::OutputLimit)?;
        let mut restart = true;
        loop {
            queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
            if queries > maximum_queries {
                return Err(Error::Changed);
            }
            prepared.storage.fill(u64::MAX);
            let class = if restart {
                FileIdExtdDirectoryRestartInfo
            } else {
                FileIdExtdDirectoryInfo
            };
            restart = false;
            let ok = unsafe {
                GetFileInformationByHandleEx(
                    directory.file.as_raw_handle().cast(),
                    class,
                    prepared.storage.as_mut_ptr().cast(),
                    u32::try_from(prepared.storage.len() * std::mem::size_of::<u64>())
                        .map_err(|_| Error::Changed)?,
                )
            };
            if ok == 0 {
                if unsafe { GetLastError() } == ERROR_NO_MORE_FILES {
                    break;
                }
                return Err(Error::Changed);
            }
            let byte_length = prepared.storage.len() * std::mem::size_of::<u64>();
            let mut offset = 0_usize;
            loop {
                let record_header_end = offset
                    .checked_add(std::mem::size_of::<FILE_ID_EXTD_DIR_INFO>())
                    .ok_or(Error::Changed)?;
                let header_end = offset
                    .checked_add(std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, FileName))
                    .ok_or(Error::Changed)?;
                if record_header_end > byte_length || header_end > byte_length {
                    return Err(Error::Changed);
                }
                let entry = unsafe {
                    &*prepared
                        .storage
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset)
                        .cast::<FILE_ID_EXTD_DIR_INFO>()
                };
                let name_bytes =
                    usize::try_from(entry.FileNameLength).map_err(|_| Error::Changed)?;
                if name_bytes % 2 != 0 {
                    return Err(Error::Changed);
                }
                let name_end = header_end.checked_add(name_bytes).ok_or(Error::Changed)?;
                if name_end > byte_length {
                    return Err(Error::Changed);
                }
                let name = unsafe {
                    std::slice::from_raw_parts(
                        prepared
                            .storage
                            .as_ptr()
                            .cast::<u8>()
                            .add(header_end)
                            .cast::<u16>(),
                        name_bytes / 2,
                    )
                };
                let dot = name == [u16::from(b'.')];
                let dot_dot = name == [u16::from(b'.'), u16::from(b'.')];
                raw_records = raw_records.checked_add(1).ok_or(Error::OutputLimit)?;
                if raw_records > maximum_records {
                    return Err(Error::Changed);
                }
                if dot {
                    if saw_dot {
                        return Err(Error::Changed);
                    }
                    saw_dot = true;
                } else if dot_dot {
                    if saw_dot_dot {
                        return Err(Error::Changed);
                    }
                    saw_dot_dot = true;
                }
                if !dot && !dot_dot {
                    let Some(index) = prepared.names.iter().position(|expected| {
                        prepared_matches_slice(expected.as_ref().expect("prepared name"), name)
                    }) else {
                        return Err(Error::Changed);
                    };
                    let tracked = files[index].expect("attached");
                    if seen[index] || entry.FileId.Identifier != tracked.identity.file_id {
                        return Err(Error::Changed);
                    }
                    seen[index] = true;
                    count = count.checked_add(1).ok_or(Error::OutputLimit)?;
                    if count > N {
                        return Err(Error::Changed);
                    }
                }
                if entry.NextEntryOffset == 0 {
                    break;
                }
                let next = usize::try_from(entry.NextEntryOffset).map_err(|_| Error::Changed)?;
                let minimum = name_end.checked_sub(offset).ok_or(Error::Changed)?;
                let next_end = offset.checked_add(next).ok_or(Error::Changed)?;
                if next < minimum
                    || next % std::mem::align_of::<FILE_ID_EXTD_DIR_INFO>() != 0
                    || next_end > byte_length
                {
                    return Err(Error::Changed);
                }
                offset = next_end;
            }
        }
        if count != N || seen.iter().any(|seen| !seen) {
            return Err(Error::Changed);
        }
        recheck_directory(directory)?;
        for file in files.iter().flatten() {
            recheck_held_regular(file)?;
        }
        Ok(())
    }

    fn observe_publish_rebound(
        parent: &Directory,
        stage_name: &PreparedRelativeNameArena,
        fail_information: bool,
        fail_close: bool,
    ) -> Result<DirectoryIdentity, Error> {
        let file = relative_file_arena(
            &parent.file,
            stage_name,
            DIRECTORY_READ_ACCESS,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        )?;
        let handle = file.into_raw_handle();
        let file = std::mem::ManuallyDrop::new(unsafe { File::from_raw_handle(handle) });
        let observed = if fail_information {
            Err(Error::Changed)
        } else {
            directory_information(&file)
        };
        let close_failed = unsafe { CloseHandle(handle.cast()) } == 0;
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
        let output = prepared_normal_name(output_name)?;
        let information = prepared.storage.as_mut_ptr().cast::<NamedInformation>();
        let stored_name = unsafe {
            std::slice::from_raw_parts(
                std::ptr::addr_of!((*information).file_name).cast::<u16>(),
                prepared.name_units,
            )
        };
        let total = u32::try_from(prepared.total).map_err(|_| Error::Invalid)?;
        if prepared.remaining != 1
            || prepared.exact_capacity
                != prepared
                    .storage
                    .len()
                    .saturating_mul(std::mem::size_of::<usize>())
            || !output.encode_utf16().eq(stored_name.iter().copied())
            || stage_name.units.is_empty()
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
            != stage.identity
        {
            return Err(Error::Changed);
        }
        #[cfg(debug_assertions)]
        if prepared.fail_rename {
            return Err(Error::Changed);
        }
        unsafe {
            (*information).root_directory = parent.file.as_raw_handle().cast();
        }
        if unsafe {
            SetFileInformationByHandle(
                stage.file.as_raw_handle().cast(),
                FileRenameInfoEx,
                information.cast(),
                total,
            )
        } == 0
        {
            return Err(Error::Exists);
        }
        Ok(())
    }

    pub fn discard_owned_stage_prepared<const N: usize>(
        parent: &Directory,
        stage: &Directory,
        stage_name: &PreparedRelativeNameArena,
        names: &PreparedDiscardNames<N>,
        files: &[Option<&RegularFile>; N],
        #[cfg(debug_assertions)] failure_after_delete: Option<usize>,
    ) -> Result<(), Error> {
        recheck_directory(parent)?;
        recheck_directory(stage)?;
        let rebound = relative_file_arena(
            &parent.file,
            stage_name,
            DIRECTORY_READ_ACCESS,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        )?;
        if directory_information(&rebound)? != stage.identity {
            return Err(Error::Changed);
        }
        let attached = files.iter().take_while(|file| file.is_some()).count();
        if files[attached..].iter().any(Option::is_some) {
            return Err(Error::Invalid);
        }

        let mut seen = [false; N];
        let mut storage = [0_u64; 8192];
        let mut restart = true;
        loop {
            let class = if restart {
                FileIdBothDirectoryRestartInfo
            } else {
                FileIdBothDirectoryInfo
            };
            restart = false;
            let ok = unsafe {
                GetFileInformationByHandleEx(
                    stage.file.as_raw_handle().cast(),
                    class,
                    storage.as_mut_ptr().cast(),
                    u32::try_from(storage.len() * std::mem::size_of::<u64>())
                        .map_err(|_| Error::Changed)?,
                )
            };
            if ok == 0 {
                if unsafe { GetLastError() } == ERROR_NO_MORE_FILES {
                    break;
                }
                return Err(Error::Changed);
            }
            let byte_length = storage.len() * std::mem::size_of::<u64>();
            let mut offset = 0_usize;
            loop {
                let header_end = offset
                    .checked_add(std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName))
                    .ok_or(Error::Changed)?;
                if header_end > byte_length {
                    return Err(Error::Changed);
                }
                let entry = unsafe {
                    &*storage
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset)
                        .cast::<FILE_ID_BOTH_DIR_INFO>()
                };
                let name_bytes =
                    usize::try_from(entry.FileNameLength).map_err(|_| Error::Changed)?;
                if name_bytes % 2 != 0 {
                    return Err(Error::Changed);
                }
                let name_end = header_end.checked_add(name_bytes).ok_or(Error::Changed)?;
                if name_end > byte_length {
                    return Err(Error::Changed);
                }
                let actual = unsafe {
                    std::slice::from_raw_parts(
                        storage.as_ptr().cast::<u8>().add(header_end).cast::<u16>(),
                        name_bytes / 2,
                    )
                };
                let dot = actual == [u16::from(b'.')];
                let dot_dot = actual == [u16::from(b'.'), u16::from(b'.')];
                if !dot && !dot_dot {
                    let Some(index) = names.names[..attached].iter().position(|expected| {
                        prepared_matches_slice(expected.as_ref().expect("validated"), actual)
                    }) else {
                        return Err(Error::Changed);
                    };
                    if seen[index] {
                        return Err(Error::Changed);
                    }
                    seen[index] = true;
                }
                if entry.NextEntryOffset == 0 {
                    break;
                }
                let next = usize::try_from(entry.NextEntryOffset).map_err(|_| Error::Changed)?;
                if next == 0 || next % std::mem::align_of::<FILE_ID_BOTH_DIR_INFO>() != 0 {
                    return Err(Error::Changed);
                }
                offset = offset.checked_add(next).ok_or(Error::Changed)?;
            }
        }
        if seen[..attached].iter().any(|seen| !seen) {
            return Err(Error::Changed);
        }
        for (index, file) in files[..attached].iter().enumerate() {
            let file = file.expect("attached prefix");
            recheck_held_regular(file)?;
            let name = names.names[index].as_ref().expect("validated");
            if hold_regular_file_name_prepared(stage, name)?.identity != file.identity {
                return Err(Error::Changed);
            }
        }
        for (deleted, file) in files[..attached].iter().flatten().enumerate() {
            #[cfg(not(debug_assertions))]
            let _ = deleted;
            #[cfg(debug_assertions)]
            if failure_after_delete == Some(deleted) {
                return Err(Error::Changed);
            }
            disposition_delete(&file.file)?;
        }
        #[cfg(debug_assertions)]
        if failure_after_delete == Some(attached) {
            return Err(Error::Changed);
        }
        disposition_delete(&stage.file)
    }

    #[repr(C)]
    struct NamedInformation {
        flags: u32,
        root_directory: HANDLE,
        file_name_length: u32,
        file_name: [u16; 1],
    }

    fn disposition_delete(file: &File) -> Result<(), Error> {
        let information = FILE_DISPOSITION_INFO_EX {
            Flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
        };
        if unsafe {
            SetFileInformationByHandle(
                file.as_raw_handle().cast(),
                FileDispositionInfoEx,
                (&information as *const FILE_DISPOSITION_INFO_EX).cast(),
                u32::try_from(std::mem::size_of_val(&information)).map_err(|_| Error::Changed)?,
            )
        } == 0
        {
            return Err(Error::Changed);
        }
        Ok(())
    }

    fn run_argv(
        executable: &Executable,
        cwd: &Directory,
        arguments: &[String],
        stdout_limit: usize,
        prepared_command_line: Option<Vec<u16>>,
        prepared_output: Option<Vec<u8>>,
        process_arena: &mut PreparedProcessArena,
    ) -> Result<Vec<u8>, Error> {
        if arguments.len() > 32
            || prepared_command_line.as_ref().is_none_or(Vec::is_empty)
            || prepared_output
                .as_ref()
                .is_none_or(|output| output.capacity() != stdout_limit || !output.is_empty())
        {
            return Err(Error::Invalid);
        }
        consume_process_arena(process_arena)?;
        struct CheckedHandle(Option<HANDLE>);
        impl CheckedHandle {
            fn new(handle: HANDLE) -> Self {
                Self(Some(handle))
            }

            fn raw(&self) -> HANDLE {
                self.0.expect("checked handle remains owned")
            }

            fn close(mut self) -> Result<(), Error> {
                let handle = self.0.take().expect("checked handle remains owned");
                if unsafe { CloseHandle(handle) } == 0 {
                    Err(Error::Spawn)
                } else {
                    Ok(())
                }
            }
        }
        impl Drop for CheckedHandle {
            fn drop(&mut self) {
                if let Some(handle) = self.0.take() {
                    if unsafe { CloseHandle(handle) } == 0 {
                        std::process::abort();
                    }
                }
            }
        }
        fn must_close(handles: [CheckedHandle; 4]) {
            let mut failed = false;
            for handle in handles {
                failed |= handle.close().is_err();
            }
            if failed {
                std::process::abort();
            }
        }
        recheck_held_regular(&executable.file)?;
        recheck_directory(cwd)?;
        final_path_prepared(&executable.file.file, &mut process_arena.application)?;
        final_path_prepared(&cwd.file, &mut process_arena.cwd)?;
        let mut command_line = prepared_command_line.ok_or(Error::Invalid)?;

        let security = SECURITY_ATTRIBUTES {
            nLength: u32::try_from(std::mem::size_of::<SECURITY_ATTRIBUTES>())
                .map_err(|_| Error::Spawn)?,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: 1,
        };
        let mut read_pipe = std::ptr::null_mut();
        let mut write_pipe = std::ptr::null_mut();
        if unsafe { CreatePipe(&mut read_pipe, &mut write_pipe, &security, 0) } == 0 {
            return Err(Error::Spawn);
        }
        let read_pipe = CheckedHandle::new(read_pipe);
        let write_pipe = CheckedHandle::new(write_pipe);
        if unsafe { SetHandleInformation(read_pipe.raw(), HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(Error::Spawn);
        }
        let null_name = [u16::from(b'N'), u16::from(b'U'), u16::from(b'L'), 0];
        let null_handle = unsafe {
            CreateFileW(
                null_name.as_ptr(),
                FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            )
        };
        if null_handle == INVALID_HANDLE_VALUE {
            return Err(Error::Spawn);
        }
        let null_handle = CheckedHandle::new(null_handle);
        if unsafe {
            SetHandleInformation(null_handle.raw(), HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)
        } == 0
        {
            return Err(Error::Spawn);
        }

        let inherited = [null_handle.raw(), write_pipe.raw()];
        let mut attribute_bytes = process_arena.attribute_bytes;
        let attribute_words = attribute_bytes
            .checked_add(std::mem::size_of::<u64>() - 1)
            .and_then(|bytes| bytes.checked_div(std::mem::size_of::<u64>()))
            .ok_or(Error::OutputLimit)?;
        process_arena.attributes.resize(attribute_words, 0);
        if attribute_bytes
            > process_arena
                .attributes
                .len()
                .saturating_mul(std::mem::size_of::<u64>())
            || process_arena.attributes.capacity() != attribute_words
        {
            return Err(Error::OutputLimit);
        }
        let attribute_list = process_arena.attributes.as_mut_ptr().cast();
        if unsafe { InitializeProcThreadAttributeList(attribute_list, 1, 0, &mut attribute_bytes) }
            == 0
            || attribute_bytes != process_arena.attribute_bytes
        {
            return Err(Error::Spawn);
        }
        struct AttributeList(windows_sys::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST);
        impl Drop for AttributeList {
            fn drop(&mut self) {
                unsafe { DeleteProcThreadAttributeList(self.0) };
            }
        }
        let attribute_list = AttributeList(attribute_list);
        if unsafe {
            UpdateProcThreadAttribute(
                attribute_list.0,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                inherited.as_ptr().cast(),
                std::mem::size_of_val(&inherited),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        } == 0
        {
            return Err(Error::Spawn);
        }

        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(Error::Spawn);
        }
        let job = CheckedHandle::new(job);
        let mut job_limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        job_limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&job_limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                u32::try_from(std::mem::size_of_val(&job_limits)).map_err(|_| Error::Spawn)?,
            )
        } == 0
        {
            return Err(Error::Spawn);
        }

        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb =
            u32::try_from(std::mem::size_of::<STARTUPINFOEXW>()).map_err(|_| Error::Spawn)?;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = null_handle.raw();
        startup.StartupInfo.hStdOutput = write_pipe.raw();
        startup.StartupInfo.hStdError = null_handle.raw();
        startup.lpAttributeList = attribute_list.0;
        let mut process = PROCESS_INFORMATION::default();
        let created = unsafe {
            CreateProcessW(
                process_arena.application.as_ptr(),
                command_line.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                process_arena.environment.as_ptr().cast(),
                process_arena.cwd.as_ptr(),
                &startup.StartupInfo,
                &mut process,
            )
        };
        drop(attribute_list);
        if created == 0 {
            if write_pipe.close().is_err() {
                std::process::abort();
            }
            return Err(Error::Spawn);
        }
        let process_handle = CheckedHandle::new(process.hProcess);
        let thread_handle = CheckedHandle::new(process.hThread);
        if write_pipe.close().is_err() {
            if terminate_unassigned(process_handle.raw()).is_err() {
                std::process::abort();
            }
            let mut failed = false;
            failed |= thread_handle.close().is_err();
            failed |= read_pipe.close().is_err();
            failed |= null_handle.close().is_err();
            failed |= process_handle.close().is_err();
            failed |= job.close().is_err();
            let _ = failed;
            std::process::abort();
        }

        fn must_terminate_unassigned(process: HANDLE) {
            if terminate_unassigned(process).is_err() {
                std::process::abort();
            }
        }

        fn must_settle_job(job: HANDLE, process: HANDLE, terminate: bool) {
            if settle_job(job, process, terminate).is_err() {
                std::process::abort();
            }
        }

        let image_matches = (|| {
            process_arena.application.clear();
            process_arena.application.resize(PROCESS_PATH_UNITS, 0);
            let mut image_len =
                u32::try_from(process_arena.application.len()).map_err(|_| Error::Spawn)?;
            if unsafe {
                QueryFullProcessImageNameW(
                    process_handle.raw(),
                    0,
                    process_arena.application.as_mut_ptr(),
                    &mut image_len,
                )
            } == 0
            {
                return Err(Error::Changed);
            }
            let image_len = usize::try_from(image_len).map_err(|_| Error::Spawn)?;
            if image_len == 0 || image_len.saturating_add(1) > PROCESS_PATH_UNITS {
                return Err(Error::OutputLimit);
            }
            process_arena.application.truncate(image_len);
            process_arena.application.push(0);
            let file_handle = unsafe {
                CreateFileW(
                    process_arena.application.as_ptr(),
                    REGULAR_READ_ACCESS,
                    HELD_SHARE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    NORMAL_FILE_FLAGS,
                    std::ptr::null_mut(),
                )
            };
            if file_handle == INVALID_HANDLE_VALUE {
                return Err(Error::Changed);
            }
            let file = unsafe { File::from_raw_handle(file_handle.cast()) };
            let identity = information(&file)?;
            let bytes = digest(&file, identity.length)?;
            recheck_held_regular(&executable.file)?;
            recheck_directory(cwd)?;
            Ok(!injected_settlement_failure!(WindowsImage)
                && identity == executable.file.identity
                && bytes == executable.file.digest)
        })();
        if image_matches != Ok(true) {
            must_terminate_unassigned(process_handle.raw());
            return Err(Error::Changed);
        }
        if injected_settlement_failure!(WindowsAssign)
            || unsafe { AssignProcessToJobObject(job.raw(), process_handle.raw()) } == 0
        {
            must_terminate_unassigned(process_handle.raw());
            return Err(Error::Changed);
        }
        if injected_settlement_failure!(WindowsResume)
            || unsafe { ResumeThread(thread_handle.raw()) } == u32::MAX
        {
            must_settle_job(job.raw(), process_handle.raw(), true);
            return Err(Error::Spawn);
        }
        if thread_handle.close().is_err() {
            must_settle_job(job.raw(), process_handle.raw(), true);
            std::process::abort();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let output_is_prepared = prepared_output.is_some();
        let mut output = prepared_output.unwrap_or_default();
        if output_is_prepared && (output.capacity() != stdout_limit || !output.is_empty()) {
            must_settle_job(job.raw(), process_handle.raw(), true);
            return Err(Error::OutputLimit);
        }
        let mut selected_error = None;
        loop {
            let mut available = 0_u32;
            if injected_settlement_failure!(WindowsPeek) {
                selected_error = Some(Error::Spawn);
                break;
            }
            if unsafe {
                PeekNamedPipe(
                    read_pipe.raw(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                let error = unsafe { GetLastError() };
                if !matches!(error, ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) {
                    selected_error = Some(Error::Spawn);
                    break;
                }
                available = 0;
            }
            while available != 0 {
                let count = usize::try_from(available).unwrap_or(usize::MAX).min(8192);
                if count > stdout_limit.saturating_sub(output.len()) {
                    selected_error = Some(Error::OutputLimit);
                    break;
                }
                let mut buffer = [0_u8; 8192];
                let mut read = 0_u32;
                if injected_settlement_failure!(WindowsRead)
                    || unsafe {
                        ReadFile(
                            read_pipe.raw(),
                            buffer.as_mut_ptr().cast(),
                            u32::try_from(count).map_err(|_| Error::Spawn)?,
                            &mut read,
                            std::ptr::null_mut(),
                        )
                    } == 0
                {
                    selected_error = Some(Error::Spawn);
                    break;
                }
                let read = usize::try_from(read).map_err(|_| Error::Spawn)?;
                if read == 0 {
                    break;
                }
                output.extend_from_slice(&buffer[..read]);
                if output_is_prepared && output.capacity() != stdout_limit {
                    selected_error = Some(Error::OutputLimit);
                    break;
                }
                available = available.saturating_sub(u32::try_from(read).unwrap_or(u32::MAX));
            }
            if selected_error.is_some() {
                break;
            }
            match unsafe { WaitForSingleObject(process_handle.raw(), 0) } {
                WAIT_OBJECT_0 => break,
                WAIT_TIMEOUT => {}
                WAIT_FAILED => {
                    selected_error = Some(Error::Spawn);
                    break;
                }
                _ => {
                    selected_error = Some(Error::Spawn);
                    break;
                }
            }
            if std::time::Instant::now() >= deadline {
                selected_error = Some(Error::Spawn);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        if selected_error.is_none() {
            let mut exit_code = u32::MAX;
            if unsafe {
                windows_sys::Win32::System::Threading::GetExitCodeProcess(
                    process_handle.raw(),
                    &mut exit_code,
                )
            } == 0
            {
                selected_error = Some(Error::Spawn);
            } else if exit_code != 0 {
                selected_error = Some(Error::Exit);
            }
        }
        must_settle_job(job.raw(), process_handle.raw(), true);
        if let Some(error) = selected_error {
            must_close([read_pipe, null_handle, process_handle, job]);
            return Err(error);
        }
        let result = (|| {
            loop {
                let mut available = 0_u32;
                if unsafe {
                    PeekNamedPipe(
                        read_pipe.raw(),
                        std::ptr::null_mut(),
                        0,
                        std::ptr::null_mut(),
                        &mut available,
                        std::ptr::null_mut(),
                    )
                } == 0
                {
                    let error = unsafe { GetLastError() };
                    if matches!(error, ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) {
                        break;
                    }
                    return Err(Error::Spawn);
                }
                if available == 0 {
                    break;
                }
                let count = usize::try_from(available).unwrap_or(usize::MAX).min(8192);
                if count > stdout_limit.saturating_sub(output.len()) {
                    return Err(Error::OutputLimit);
                }
                let mut buffer = [0_u8; 8192];
                let mut read = 0_u32;
                if unsafe {
                    ReadFile(
                        read_pipe.raw(),
                        buffer.as_mut_ptr().cast(),
                        u32::try_from(count).map_err(|_| Error::Spawn)?,
                        &mut read,
                        std::ptr::null_mut(),
                    )
                } == 0
                {
                    return Err(Error::Spawn);
                }
                let read = usize::try_from(read).map_err(|_| Error::Spawn)?;
                if read == 0 {
                    break;
                }
                output.extend_from_slice(&buffer[..read]);
            }
            recheck_regular(&executable.file)?;
            recheck_directory(cwd)?;
            Ok(output)
        })();
        must_close([read_pipe, null_handle, process_handle, job]);
        result
    }

    fn terminate_unassigned(process: HANDLE) -> Result<(), Error> {
        let terminate_failed = unsafe { TerminateProcess(process, 126) } == 0;
        let wait = unsafe { WaitForSingleObject(process, 30_000) };
        if terminate_failed
            || wait != WAIT_OBJECT_0
            || injected_settlement_failure!(WindowsUnassigned)
            || injected_settlement_failure!(WindowsTerminateProcess)
            || injected_settlement_failure!(WindowsWaitUnassigned)
        {
            return Err(Error::Spawn);
        }
        Ok(())
    }

    fn settle_job(job: HANDLE, process: HANDLE, terminate: bool) -> Result<(), Error> {
        let terminate_failed = terminate
            && (unsafe { TerminateJobObject(job, 126) } == 0
                || injected_settlement_failure!(WindowsJob)
                || injected_settlement_failure!(WindowsTerminateJob));
        let leader_wait = unsafe { WaitForSingleObject(process, 30_000) };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            if injected_settlement_failure!(WindowsQueryJob)
                || unsafe {
                    QueryInformationJobObject(
                        job,
                        JobObjectBasicAccountingInformation,
                        (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                        u32::try_from(std::mem::size_of_val(&accounting))
                            .map_err(|_| Error::Spawn)?,
                        std::ptr::null_mut(),
                    )
                } == 0
            {
                return Err(Error::Spawn);
            }
            if accounting.ActiveProcesses == 0 {
                return if terminate_failed || leader_wait != WAIT_OBJECT_0 {
                    Err(Error::Spawn)
                } else {
                    Ok(())
                };
            }
            if std::time::Instant::now() >= deadline {
                return Err(Error::Spawn);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    fn wide_null(value: &OsStr) -> Result<Vec<u16>, Error> {
        let mut wide = value.encode_wide().collect::<Vec<_>>();
        if wide.is_empty() || wide.contains(&0) {
            return Err(Error::Invalid);
        }
        wide.push(0);
        Ok(wide)
    }

    fn windows_command_line(arguments: &[String]) -> Result<Vec<u16>, Error> {
        let mut line = String::from("semaprax-native-rust-interop-tool");
        for argument in arguments {
            if argument.contains(['\0', '\r', '\n']) {
                return Err(Error::Invalid);
            }
            line.push(' ');
            line.push('"');
            let mut slashes = 0_usize;
            for character in argument.chars() {
                if character == '\\' {
                    slashes += 1;
                } else {
                    if character == '"' {
                        line.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
                    } else {
                        line.extend(std::iter::repeat_n('\\', slashes));
                    }
                    slashes = 0;
                    line.push(character);
                }
            }
            line.extend(std::iter::repeat_n('\\', slashes * 2));
            line.push('"');
        }
        wide_null(OsStr::new(&line))
    }

    fn preflight_windows_command_line(arguments: &[&[&str]]) -> Result<(), Error> {
        let mut units = "semaprax-native-rust-interop-tool"
            .encode_utf16()
            .count()
            .checked_add(1)
            .ok_or(Error::OutputLimit)?;
        for parts in arguments {
            units = units.checked_add(3).ok_or(Error::OutputLimit)?;
            let mut slashes = 0_usize;
            for character in parts.iter().flat_map(|part| part.chars()) {
                if matches!(character, '\0' | '\r' | '\n') {
                    return Err(Error::Invalid);
                }
                if character == '\\' {
                    slashes = slashes.checked_add(1).ok_or(Error::OutputLimit)?;
                    continue;
                }
                let escaped_slashes = if character == '"' {
                    slashes
                        .checked_mul(2)
                        .and_then(|count| count.checked_add(1))
                        .ok_or(Error::OutputLimit)?
                } else {
                    slashes
                };
                units = units
                    .checked_add(escaped_slashes)
                    .and_then(|count| count.checked_add(character.len_utf16()))
                    .ok_or(Error::OutputLimit)?;
                slashes = 0;
            }
            units = units
                .checked_add(slashes.checked_mul(2).ok_or(Error::OutputLimit)?)
                .ok_or(Error::OutputLimit)?;
        }
        if units > MAX_COMMAND_LINE_UNITS {
            return Err(Error::OutputLimit);
        }
        Ok(())
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
            &[],
            maximum,
            Some(prepared.command_line),
            Some(prepared.output),
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
        normal_name(input)?;
        if sanitizers
            || !matches!(optimization, 0 | 2)
            || target.is_empty()
            || !target
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(Error::Invalid);
        }
        let input = input.to_str().ok_or(Error::Invalid)?;
        Ok(PreparedCCompileInvocation(prepare_command(
            &[
                "-std=c11",
                "-target",
                target,
                "-Wall",
                "-Wextra",
                "-Werror",
                if optimization == 0 { "-O0" } else { "-O2" },
                "-c",
                input,
                "-o",
                "-",
            ],
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
            Some(prepared.0.command_line),
            Some(prepared.0.output),
            process_arena,
        )
    }

    pub fn prepare_rust_compile_invocation(
        target: &str,
        source: &OsStr,
        output: &OsStr,
    ) -> Result<PreparedRustCompileInvocation, Error> {
        normal_name(source)?;
        normal_name(output)?;
        if target.is_empty()
            || !target
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(Error::Invalid);
        }
        Ok(PreparedRustCompileInvocation {
            command: prepare_command(
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
            )?,
            output_name: prepare_relative_name(output)?,
        })
    }

    pub fn prepared_rust_compile_owned_capacity(prepared: &PreparedRustCompileInvocation) -> usize {
        prepared_command_owned_capacity(&prepared.command).saturating_add(
            prepared
                .output_name
                .0
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
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
            Some(prepared.command.command_line),
            Some(prepared.command.output),
            process_arena,
        )?
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
            normal_name(name)?;
        }
        let linker = linker
            .filter(|path| std::path::Path::new(path).is_absolute())
            .and_then(OsStr::to_str)
            .ok_or(Error::Invalid)?;
        let vctools = vctools
            .filter(|path| std::path::Path::new(path).is_absolute())
            .and_then(OsStr::to_str)
            .ok_or(Error::Invalid)?;
        let linker_units = linker.encode_utf16().count();
        let vctools_units = vctools.encode_utf16().count();
        if linker_units == 0
            || linker_units > MAX_TOOL_PATH_UNITS
            || vctools_units == 0
            || vctools_units > MAX_TOOL_PATH_UNITS
        {
            return Err(Error::OutputLimit);
        }
        if std::path::Path::new(linker).strip_prefix(vctools).ok()
            != Some(std::path::Path::new(r"bin\Hostx64\x64\link.exe"))
        {
            return Err(Error::Invalid);
        }
        if sanitizers
            || target.is_empty()
            || !target
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(Error::Invalid);
        }
        let harness = harness.to_str().ok_or(Error::Invalid)?;
        let c_object = c_object.to_str().ok_or(Error::Invalid)?;
        let rust_archive = rust_archive.to_str().ok_or(Error::Invalid)?;
        let output_text = output.to_str().ok_or(Error::Invalid)?;
        let argument_parts: [&[&str]; 19] = [
            &["-target"],
            &[target],
            &["-Xmicrosoft-visualc-tools-root"],
            &[vctools],
            &["-fuse-ld=link"],
            &[WINDOWS_DYNAMIC_CRT_LINK_ARGS[0]],
            &[WINDOWS_DYNAMIC_CRT_LINK_ARGS[1]],
            &[harness],
            &[c_object],
            &[rust_archive],
            &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[0]],
            &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[1]],
            &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[2]],
            &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[3]],
            &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[4]],
            &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[5]],
            &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[6]],
            &["-o"],
            &[output_text],
        ];
        preflight_windows_command_line(&argument_parts)?;
        let mut arguments = Vec::with_capacity(argument_parts.len());
        for parts in argument_parts {
            let capacity = parts.iter().try_fold(0_usize, |total, part| {
                total.checked_add(part.len()).ok_or(Error::OutputLimit)
            })?;
            let mut argument = String::with_capacity(capacity);
            for part in parts {
                argument.push_str(part);
            }
            if argument.capacity() != capacity {
                return Err(Error::OutputLimit);
            }
            arguments.push(argument);
        }
        if arguments.len() != 19 || arguments.capacity() != 19 {
            return Err(Error::OutputLimit);
        }
        let command_line = windows_command_line(&arguments)?;
        Ok(PreparedLinkInvocation {
            command: PreparedCommand {
                arguments,
                command_line,
                output: Vec::new(),
            },
            output_name: prepare_relative_name(output)?,
        })
    }

    pub fn prepared_link_owned_capacity(prepared: &PreparedLinkInvocation) -> usize {
        prepared_command_owned_capacity(&prepared.command).saturating_add(
            prepared
                .output_name
                .0
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
    }

    #[cfg(test)]
    pub(super) fn test_prepared_link_arguments(
        prepared: &PreparedLinkInvocation,
    ) -> (&[String], usize) {
        (
            &prepared.command.arguments,
            prepared.command.arguments.capacity(),
        )
    }

    pub fn link_prepared(
        clang: &Executable,
        linker: Option<(&Executable, &str)>,
        cwd: &Directory,
        prepared: PreparedLinkInvocation,
        process_arena: &mut PreparedProcessArena,
    ) -> Result<Executable, Error> {
        let (linker, linker_path) = linker.ok_or(Error::Invalid)?;
        let vctools = prepared.command.arguments.get(3).ok_or(Error::Invalid)?;
        if prepared.command.arguments.get(2).map(String::as_str)
            != Some("-Xmicrosoft-visualc-tools-root")
            || prepared.command.arguments.get(4).map(String::as_str) != Some("-fuse-ld=link")
            || std::path::Path::new(linker_path).strip_prefix(vctools).ok()
                != Some(std::path::Path::new(r"bin\Hostx64\x64\link.exe"))
        {
            return Err(Error::Invalid);
        }
        if hold_regular_file_name_prepared(cwd, &prepared.output_name).is_ok() {
            return Err(Error::Exists);
        }
        recheck_held_regular(&linker.file)?;
        let process_output = run_argv(
            clang,
            cwd,
            &prepared.command.arguments,
            0,
            Some(prepared.command.command_line),
            Some(prepared.command.output),
            process_arena,
        );
        let linker_recheck = recheck_held_regular(&linker.file);
        let process_output = process_output?;
        linker_recheck?;
        if !process_output.is_empty() {
            return Err(Error::OutputLimit);
        }
        let file = hold_regular_file_name_prepared(cwd, &prepared.output_name)?;
        let mut prefix = [0_u8; 2];
        let mut duplicate = file.file.try_clone().map_err(|_| Error::Changed)?;
        duplicate
            .seek(SeekFrom::Start(0))
            .map_err(|_| Error::Changed)?;
        duplicate
            .read_exact(&mut prefix)
            .map_err(|_| Error::Invalid)?;
        if prefix != *b"MZ" {
            return Err(Error::Invalid);
        }
        Ok(Executable { file })
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
            Some(prepared.0.command_line),
            Some(prepared.0.output),
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
        normal_name(input)?;
        if sanitizers || !matches!(optimization, 0 | 2) {
            return Err(Error::Invalid);
        }
        let arguments = vec![
            "-std=c11".to_owned(),
            "-target".to_owned(),
            target.to_owned(),
            "-Wall".to_owned(),
            "-Wextra".to_owned(),
            "-Werror".to_owned(),
            if optimization == 0 {
                "-O0".to_owned()
            } else {
                "-O2".to_owned()
            },
            "-c".to_owned(),
            input.to_string_lossy().into_owned(),
            "-o".to_owned(),
            "-".to_owned(),
        ];
        let command_line = windows_command_line(&arguments)?;
        let output = Vec::with_capacity(maximum.min(33_554_432));
        let mut process_arena = prepare_process_arena(1)?;
        run_argv(
            executable,
            cwd,
            &arguments,
            maximum.min(33_554_432),
            Some(command_line),
            Some(output),
            &mut process_arena,
        )
    }

    pub fn execute_harness(executable: &Executable, cwd: &Directory) -> Result<(), Error> {
        let command_line = windows_command_line(&[])?;
        let mut process_arena = prepare_process_arena(1)?;
        if run_argv(
            executable,
            cwd,
            &[],
            0,
            Some(command_line),
            Some(Vec::new()),
            &mut process_arena,
        )?
        .is_empty()
        {
            Ok(())
        } else {
            Err(Error::OutputLimit)
        }
    }

    #[cfg(test)]
    pub(super) fn execute_harness_with_argument(
        executable: &Executable,
        cwd: &Directory,
        argument: &str,
    ) -> Result<(), Error> {
        let arguments = [argument.to_owned()];
        let command_line = windows_command_line(&arguments)?;
        let mut process_arena = prepare_process_arena(1)?;
        if run_argv(
            executable,
            cwd,
            &arguments,
            0,
            Some(command_line),
            Some(Vec::new()),
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
            normal_name(name)?;
        }
        let (linker, linker_path) = linker.ok_or(Error::Invalid)?;
        let vctools = vctools
            .filter(|path| std::path::Path::new(path).is_absolute())
            .and_then(OsStr::to_str)
            .ok_or(Error::Invalid)?;
        if !std::path::Path::new(linker_path).is_absolute()
            || std::path::Path::new(linker_path).strip_prefix(vctools).ok()
                != Some(std::path::Path::new(r"bin\Hostx64\x64\link.exe"))
            || sanitizers
        {
            return Err(Error::Invalid);
        }
        let mut arguments = vec!["-target".to_owned(), target.to_owned()];
        arguments.extend([
            "-Xmicrosoft-visualc-tools-root".to_owned(),
            vctools.to_owned(),
            "-fuse-ld=link".to_owned(),
        ]);
        arguments.extend(WINDOWS_DYNAMIC_CRT_LINK_ARGS.into_iter().map(str::to_owned));
        arguments.extend([
            harness.to_string_lossy().into_owned(),
            c_object.to_string_lossy().into_owned(),
            rust_archive.to_string_lossy().into_owned(),
        ]);
        arguments.extend(
            WINDOWS_RUST_STATICLIB_NATIVE_LIBS
                .into_iter()
                .map(str::to_owned),
        );
        arguments.extend(["-o".to_owned(), output.to_string_lossy().into_owned()]);
        let command_line = windows_command_line(&arguments)?;
        let mut process_arena = prepare_process_arena(1)?;
        recheck_held_regular(&linker.file)?;
        let process_output = run_argv(
            clang,
            cwd,
            &arguments,
            0,
            Some(command_line),
            Some(Vec::new()),
            &mut process_arena,
        );
        let linker_recheck = recheck_held_regular(&linker.file);
        let process_output = process_output?;
        linker_recheck?;
        if !process_output.is_empty() {
            return Err(Error::OutputLimit);
        }
        hold_executable(cwd, output)
    }
}

pub use platform::*;

#[cfg(test)]
mod tests {
    use super::{enter_prepared_file_syscalls, Error, TEST_PREPARED_FILE_SYSCALL_ENTRIES};
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    use super::{set_test_settlement_failures, TestSettlementFailure};
    use std::sync::atomic::Ordering;

    #[cfg(target_os = "linux")]
    fn linux_runner_failure_helper(
        points: &[TestSettlementFailure],
        expected: Option<Error>,
        sentinel: &str,
    ) {
        let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
            return;
        };
        set_test_settlement_failures(points);
        let root = std::path::PathBuf::from(root);
        let directory = super::platform::hold_directory(&root).unwrap();
        let executable =
            super::platform::hold_executable(&directory, std::ffi::OsStr::new("noisy")).unwrap();
        let result =
            super::platform::execute_harness_with_output_limit(&executable, &directory, 65_536);
        if let Some(expected) = expected {
            assert_eq!(result, Err(expected));
        }
        std::fs::write(root.join(sentinel), b"returned").unwrap();
    }

    #[cfg(target_os = "linux")]
    macro_rules! linux_runner_helper {
        ($name:ident, [$($point:ident),+], $expected:expr, $sentinel:literal) => {
            #[test]
            fn $name() {
                linux_runner_failure_helper(
                    &[$(TestSettlementFailure::$point),+],
                    $expected,
                    $sentinel,
                );
            }
        };
    }

    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_pipe_read_fcntl,
        [UnixPipeReadFcntl],
        Some(Error::Spawn),
        "settled"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_pipe_write_fcntl,
        [UnixPipeWriteFcntl],
        Some(Error::Spawn),
        "settled"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_drain_fcntl,
        [UnixDrainFcntl],
        Some(Error::Spawn),
        "settled"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(helper_linux_poll, [UnixPoll], Some(Error::Spawn), "settled");
    #[cfg(target_os = "linux")]
    linux_runner_helper!(helper_linux_read, [UnixRead], Some(Error::Spawn), "settled");
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_read_conversion,
        [UnixReadConversion],
        Some(Error::OutputLimit),
        "settled"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_waitpid,
        [UnixWaitpid],
        Some(Error::Spawn),
        "settled"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_deadline,
        [UnixDeadline],
        Some(Error::Spawn),
        "settled"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_parent_write_close,
        [UnixParentWriteClose],
        None,
        "post-fail-stop"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_parent_null_close,
        [UnixParentNullClose],
        None,
        "post-fail-stop"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_settle_close,
        [UnixDrainFcntl, UnixSettleClose],
        None,
        "post-fail-stop"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_success_read_close,
        [UnixSuccessReadClose],
        None,
        "post-fail-stop"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_wait_settlement,
        [UnixDrainFcntl, UnixWait],
        None,
        "post-fail-stop"
    );
    #[cfg(target_os = "linux")]
    linux_runner_helper!(
        helper_linux_group_settlement,
        [UnixDrainFcntl, UnixGroup],
        None,
        "post-fail-stop"
    );

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_runner_boundaries_settle_or_fail_stop_without_later_action() {
        use std::os::unix::process::ExitStatusExt as _;
        use std::process::Command;

        let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let root = parent.join(format!(
            "semaprax-sys-runner-boundaries-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("noisy.c");
        std::fs::write(
            &source,
            "#include <stdio.h>\n#include <unistd.h>\nint main(void){FILE *f=fopen(\"leader.pid\",\"w\");if(!f)return 2;fprintf(f,\"%ld\",(long)getpid());fclose(f);if(write(1,\"x\",1)!=1)return 2;sleep(1);return 0;}\n",
        )
        .unwrap();
        let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
        let built = Command::new(compiler)
            .env("TMPDIR", &root)
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
            .arg(&source)
            .arg("-o")
            .arg(root.join("noisy"))
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let current = std::env::current_exe().unwrap();
        for helper in [
            "tests::helper_linux_pipe_read_fcntl",
            "tests::helper_linux_pipe_write_fcntl",
            "tests::helper_linux_drain_fcntl",
            "tests::helper_linux_poll",
            "tests::helper_linux_read",
            "tests::helper_linux_read_conversion",
            "tests::helper_linux_waitpid",
            "tests::helper_linux_deadline",
        ] {
            let sentinel = root.join("settled");
            let _ = std::fs::remove_file(&sentinel);
            let status = Command::new(&current)
                .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
                .args(["--exact", helper, "--nocapture"])
                .status()
                .unwrap();
            assert!(status.success(), "settled boundary failed: {helper}");
            assert!(
                sentinel.exists(),
                "settled boundary did not return: {helper}"
            );
        }
        for helper in [
            "tests::helper_linux_parent_write_close",
            "tests::helper_linux_parent_null_close",
            "tests::helper_linux_settle_close",
            "tests::helper_linux_success_read_close",
            "tests::helper_linux_wait_settlement",
            "tests::helper_linux_group_settlement",
        ] {
            let sentinel = root.join("post-fail-stop");
            let _ = std::fs::remove_file(&sentinel);
            let status = Command::new(&current)
                .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
                .args(["--exact", helper, "--nocapture"])
                .status()
                .unwrap();
            assert!(!status.success(), "fail-stop boundary returned: {helper}");
            assert!(
                status.signal().is_some(),
                "fail-stop did not abort: {helper}"
            );
            assert!(
                !sentinel.exists(),
                "later action ran after fail-stop: {helper}"
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(target_os = "macos")]
    fn darwin_failure_helper(points: &[TestSettlementFailure]) {
        let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
            return;
        };
        set_test_settlement_failures(points);
        let root = std::path::PathBuf::from(root);
        let directory = super::platform::hold_directory(&root).unwrap();
        let executable =
            super::platform::hold_executable(&directory, std::ffi::OsStr::new("quiet")).unwrap();
        let _ = super::platform::execute_harness(&executable, &directory);
        std::fs::write(root.join("post-fail-stop"), b"returned").unwrap();
    }

    #[cfg(target_os = "macos")]
    fn darwin_returning_failure_helper(point: TestSettlementFailure, expected: Error) {
        let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
            return;
        };
        set_test_settlement_failures(&[point]);
        let root = std::path::PathBuf::from(root);
        let directory = super::platform::hold_directory(&root).unwrap();
        let executable =
            super::platform::hold_executable(&directory, std::ffi::OsStr::new("quiet")).unwrap();
        assert_eq!(
            super::platform::execute_harness(&executable, &directory),
            Err(expected)
        );
        std::fs::write(root.join("post-return"), b"returned").unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_darwin_actions_destroy() {
        darwin_failure_helper(&[TestSettlementFailure::DarwinActionsDestroy]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_darwin_attributes_destroy() {
        darwin_failure_helper(&[TestSettlementFailure::DarwinAttributesDestroy]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_darwin_attest_settlement_fail_stop() {
        darwin_failure_helper(&[
            TestSettlementFailure::DarwinAttest,
            TestSettlementFailure::UnixWait,
        ]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_darwin_sigcont_settlement_fail_stop() {
        darwin_failure_helper(&[
            TestSettlementFailure::DarwinSigcont,
            TestSettlementFailure::UnixGroup,
        ]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_darwin_attest_returns_changed_after_settlement() {
        darwin_returning_failure_helper(TestSettlementFailure::DarwinAttest, Error::Changed);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_darwin_sigcont_returns_spawn_after_settlement() {
        darwin_returning_failure_helper(TestSettlementFailure::DarwinSigcont, Error::Spawn);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn darwin_spawn_resource_destroy_uncertainty_fail_stops_without_later_action() {
        use std::os::unix::process::ExitStatusExt as _;
        use std::process::Command;

        let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let root = parent.join(format!(
            "semaprax-sys-darwin-destroy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("quiet.c");
        std::fs::write(&source, "int main(void){return 0;}\n").unwrap();
        let compiler = std::env::var_os("CC").unwrap_or_else(|| "/usr/bin/cc".into());
        let built = Command::new(compiler)
            .env_clear()
            .env("TMPDIR", &root)
            .env("PATH", "/usr/bin:/bin")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
            .arg(&source)
            .arg("-o")
            .arg(root.join("quiet"))
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let current = std::env::current_exe().unwrap();
        for helper in [
            "tests::helper_darwin_attest_returns_changed_after_settlement",
            "tests::helper_darwin_sigcont_returns_spawn_after_settlement",
        ] {
            let sentinel = root.join("post-return");
            let _ = std::fs::remove_file(&sentinel);
            let status = Command::new(&current)
                .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
                .args(["--exact", helper, "--nocapture"])
                .status()
                .unwrap();
            assert!(
                status.success(),
                "settled operation did not return: {helper}"
            );
            assert!(
                sentinel.exists(),
                "post-return sentinel missing after settled operation: {helper}"
            );
        }
        for helper in [
            "tests::helper_darwin_actions_destroy",
            "tests::helper_darwin_attributes_destroy",
            "tests::helper_darwin_attest_settlement_fail_stop",
            "tests::helper_darwin_sigcont_settlement_fail_stop",
        ] {
            let sentinel = root.join("post-fail-stop");
            let _ = std::fs::remove_file(&sentinel);
            let status = Command::new(&current)
                .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
                .args(["--exact", helper, "--nocapture"])
                .status()
                .unwrap();
            assert!(!status.success(), "destroy uncertainty returned: {helper}");
            assert!(
                status.signal().is_some(),
                "destroy uncertainty did not abort: {helper}"
            );
            assert!(
                !sentinel.exists(),
                "later action ran after destroy uncertainty"
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(target_os = "windows")]
    fn windows_runner_failure_helper(
        points: &[TestSettlementFailure],
        executable_name: &str,
        expected: Option<Error>,
        bounded_output: bool,
        sentinel: &str,
    ) {
        let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
            return;
        };
        set_test_settlement_failures(points);
        let root = std::path::PathBuf::from(root);
        let directory = super::platform::hold_directory(&root).unwrap();
        let executable =
            super::platform::hold_executable(&directory, std::ffi::OsStr::new(executable_name))
                .unwrap();
        let result = if bounded_output {
            super::platform::clang_version(&executable, &directory, 64).map(|_| ())
        } else {
            super::platform::execute_harness(&executable, &directory)
        };
        if let Some(expected) = expected {
            assert_eq!(result, Err(expected));
        }
        std::fs::write(root.join(sentinel), b"returned").unwrap();
    }

    #[cfg(target_os = "windows")]
    macro_rules! windows_runner_helper {
        ($name:ident, [$($point:ident),+], $exe:literal, $expected:expr, $bounded:expr, $sentinel:literal) => {
            #[test]
            fn $name() {
                windows_runner_failure_helper(
                    &[$(TestSettlementFailure::$point),+],
                    $exe,
                    $expected,
                    $bounded,
                    $sentinel,
                );
            }
        };
    }

    #[cfg(target_os = "windows")]
    windows_runner_helper!(
        helper_windows_image,
        [WindowsImage],
        "quiet.exe",
        Some(Error::Changed),
        false,
        "settled"
    );
    #[cfg(target_os = "windows")]
    windows_runner_helper!(
        helper_windows_assign,
        [WindowsAssign],
        "quiet.exe",
        Some(Error::Changed),
        false,
        "settled"
    );
    #[cfg(target_os = "windows")]
    windows_runner_helper!(
        helper_windows_resume,
        [WindowsResume],
        "quiet.exe",
        Some(Error::Spawn),
        false,
        "settled"
    );
    #[cfg(target_os = "windows")]
    windows_runner_helper!(
        helper_windows_peek,
        [WindowsPeek],
        "quiet.exe",
        Some(Error::Spawn),
        false,
        "settled"
    );
    #[cfg(target_os = "windows")]
    windows_runner_helper!(
        helper_windows_read,
        [WindowsRead],
        "output.exe",
        Some(Error::Spawn),
        true,
        "settled"
    );
    #[cfg(target_os = "windows")]
    windows_runner_helper!(
        helper_windows_unassigned_fail_stop,
        [WindowsImage, WindowsTerminateProcess],
        "quiet.exe",
        None,
        false,
        "post-fail-stop"
    );
    #[cfg(target_os = "windows")]
    windows_runner_helper!(
        helper_windows_wait_unassigned_fail_stop,
        [WindowsImage, WindowsWaitUnassigned],
        "quiet.exe",
        None,
        false,
        "post-fail-stop"
    );
    #[cfg(target_os = "windows")]
    windows_runner_helper!(
        helper_windows_terminate_job_fail_stop,
        [WindowsPeek, WindowsTerminateJob],
        "quiet.exe",
        None,
        false,
        "post-fail-stop"
    );
    #[cfg(target_os = "windows")]
    windows_runner_helper!(
        helper_windows_query_job_fail_stop,
        [WindowsPeek, WindowsQueryJob],
        "quiet.exe",
        None,
        false,
        "post-fail-stop"
    );

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_runner_failures_use_only_explicit_test_state() {
        use std::process::Command;

        let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let root = parent.join(format!(
            "semaprax-sys-runner-boundaries-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        for (name, source) in [
            ("quiet", "int main(void){return 0;}\n"),
            (
                "output",
                "#include <windows.h>\n#include <stdio.h>\nint main(void){fputs(\"x\",stdout);fflush(stdout);Sleep(30000);return 0;}\n",
            ),
            (
                "handle_probe",
                "#include <windows.h>\n#include <stdint.h>\n#include <stdlib.h>\nint main(int argc,char **argv){if(argc!=2)return 7;char *end=0;uintptr_t handle=(uintptr_t)_strtoui64(argv[1],&end,10);if(!end||*end)return 6;DWORD flags=0;if(getenv(\"PATH\")!=0)return 8;if(GetHandleInformation((HANDLE)handle,&flags))return 9;return 0;}\n",
            ),
        ] {
            let source_path = root.join(format!("{name}.c"));
            std::fs::write(&source_path, source).unwrap();
            let compiler = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
            let built = Command::new(compiler)
                .env("TMP", &root)
                .env("TEMP", &root)
                .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
                .arg(&source_path)
                .arg("-o")
                .arg(root.join(format!("{name}.exe")))
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
        }
        use std::os::windows::io::AsRawHandle as _;
        let inherited = std::fs::File::open("NUL").unwrap();
        let raw = inherited.as_raw_handle();
        assert_ne!(
            unsafe {
                windows_sys::Win32::Foundation::SetHandleInformation(
                    raw.cast(),
                    windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
                    windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
                )
            },
            0
        );
        let directory = super::platform::hold_directory(&root).unwrap();
        let executable =
            super::platform::hold_executable(&directory, std::ffi::OsStr::new("handle_probe.exe"))
                .unwrap();
        super::platform::execute_harness_with_argument(
            &executable,
            &directory,
            &(raw as usize).to_string(),
        )
        .unwrap();
        drop(executable);
        drop(directory);
        drop(inherited);
        let current = std::env::current_exe().unwrap();
        for helper in [
            "tests::helper_windows_image",
            "tests::helper_windows_assign",
            "tests::helper_windows_resume",
            "tests::helper_windows_peek",
            "tests::helper_windows_read",
        ] {
            let sentinel = root.join("settled");
            let _ = std::fs::remove_file(&sentinel);
            let status = Command::new(&current)
                .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
                .args(["--exact", helper, "--nocapture"])
                .status()
                .unwrap();
            assert!(status.success(), "settled boundary failed: {helper}");
            assert!(
                sentinel.exists(),
                "settled boundary did not return: {helper}"
            );
        }
        for helper in [
            "tests::helper_windows_unassigned_fail_stop",
            "tests::helper_windows_wait_unassigned_fail_stop",
            "tests::helper_windows_terminate_job_fail_stop",
            "tests::helper_windows_query_job_fail_stop",
        ] {
            let sentinel = root.join("post-fail-stop");
            let _ = std::fs::remove_file(&sentinel);
            let status = Command::new(&current)
                .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
                .args(["--exact", helper, "--nocapture"])
                .status()
                .unwrap();
            assert!(!status.success(), "fail-stop boundary returned: {helper}");
            assert!(
                !sentinel.exists(),
                "later action ran after fail-stop: {helper}"
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(target_os = "linux")]
    fn inventory_record(name: &[u8], inode: u64) -> Vec<u8> {
        let length = (19 + name.len() + 1 + 7) & !7;
        let mut bytes = vec![0_u8; length];
        bytes[..8].copy_from_slice(&inode.to_ne_bytes());
        bytes[16..18].copy_from_slice(&u16::try_from(length).unwrap().to_ne_bytes());
        bytes[18] = 8;
        bytes[19..19 + name.len()].copy_from_slice(name);
        bytes
    }

    #[cfg(unix)]
    fn with_inventory_fixture(
        root: &std::path::Path,
        action: impl FnOnce(
            &super::platform::Directory,
            &super::platform::PreparedDiscardNames<1>,
            &super::platform::RegularFile,
            &mut super::platform::PreparedInventoryExact<1>,
        ),
    ) {
        use std::ffi::OsStr;

        let _ = std::fs::remove_dir_all(root);
        std::fs::create_dir_all(root).unwrap();
        let directory = super::platform::hold_directory(root).unwrap();
        let names = super::platform::prepare_discard_names([OsStr::new("a")]).unwrap();
        let file =
            super::platform::write_file_new_prepared(&directory, &names, 0, b"inventory", 0o600)
                .unwrap();
        let mut prepared = super::platform::prepare_inventory_exact(&names).unwrap();
        action(&directory, &names, &file, &mut prepared);
        drop((prepared, file, names, directory));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    fn inventory_record(name: &[u8], inode: u64) -> Vec<u8> {
        let length = (21 + name.len() + 1 + 3) & !3;
        let mut bytes = vec![0_u8; length];
        bytes[..8].copy_from_slice(&inode.to_ne_bytes());
        bytes[16..18].copy_from_slice(&u16::try_from(length).unwrap().to_ne_bytes());
        bytes[18..20].copy_from_slice(&u16::try_from(name.len()).unwrap().to_ne_bytes());
        bytes[20] = 8;
        bytes[21..21 + name.len()].copy_from_slice(name);
        bytes
    }

    #[test]
    fn prepared_file_syscall_gate_resolves_name_before_entry() {
        TEST_PREPARED_FILE_SYSCALL_ENTRIES.store(0, Ordering::Relaxed);
        assert_eq!(
            enter_prepared_file_syscalls::<()>(Err(Error::Invalid)),
            Err(Error::Invalid)
        );
        assert_eq!(
            TEST_PREPARED_FILE_SYSCALL_ENTRIES.load(Ordering::Relaxed),
            0
        );

        let resolved = ();
        assert!(enter_prepared_file_syscalls(Ok(&resolved)).is_ok());
        assert_eq!(
            TEST_PREPARED_FILE_SYSCALL_ENTRIES.load(Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn production_source_exposes_no_prepared_file_syscall_observer() {
        let source = include_str!("lib.rs");
        assert!(!source.contains(concat!("pub fn reset_prepared_file_", "syscall_entries")));
        assert!(!source.contains(concat!("pub fn prepared_file_", "syscall_entries")));
        assert!(!source.contains(concat!("static PREPARED_FILE_", "SYSCALL_ENTRIES")));
    }

    #[cfg(unix)]
    #[test]
    fn prepared_inventory_record_parser_rejects_malformed_and_stale_bytes() {
        use super::platform::test_parse_inventory_records;

        let valid = inventory_record(b"a", 7);
        assert_eq!(
            test_parse_inventory_records(&valid, &[(b"a".as_slice(), 7)]),
            Ok(())
        );

        let header = if cfg!(target_os = "macos") { 21 } else { 19 };
        assert!(test_parse_inventory_records(&vec![0_u8; header - 1], &[]).is_err());
        for record_length in [0_u16, 8, 21, u16::try_from(valid.len() + 8).unwrap()] {
            let mut malformed = valid.clone();
            malformed[16..18].copy_from_slice(&record_length.to_ne_bytes());
            assert!(test_parse_inventory_records(&malformed, &[(b"a".as_slice(), 7)]).is_err());
        }

        let mut missing_nul = valid.clone();
        let terminator = header + 1;
        missing_nul[terminator..].fill(0xff);
        assert!(test_parse_inventory_records(&missing_nul, &[(b"a".as_slice(), 7)]).is_err());

        let early_nul = inventory_record(b"a\0late", 7);
        #[cfg(target_os = "linux")]
        assert_eq!(
            test_parse_inventory_records(&early_nul, &[(b"a".as_slice(), 7)]),
            Ok(())
        );
        #[cfg(target_os = "macos")]
        assert!(test_parse_inventory_records(&early_nul, &[(b"a".as_slice(), 7)]).is_err());

        let mut nonzero_padding = valid.clone();
        nonzero_padding[terminator + 1..].fill(0xa5);
        assert_eq!(
            test_parse_inventory_records(&nonzero_padding, &[(b"a".as_slice(), 7)]),
            Ok(())
        );

        let mut poisoned_tail = valid.clone();
        poisoned_tail.extend_from_slice(&[0xff; 3]);
        assert!(test_parse_inventory_records(&poisoned_tail, &[(b"a".as_slice(), 7)]).is_err());

        let mut duplicate = valid.clone();
        duplicate.extend_from_slice(&valid);
        assert!(test_parse_inventory_records(&duplicate, &[(b"a".as_slice(), 7)]).is_err());
        assert!(test_parse_inventory_records(
            &inventory_record(b"unknown", 7),
            &[(b"a".as_slice(), 7)]
        )
        .is_err());
        #[cfg(target_os = "linux")]
        assert!(
            test_parse_inventory_records(&inventory_record(b"a", 0), &[(b"a".as_slice(), 0)])
                .is_err()
        );

        #[cfg(target_os = "macos")]
        {
            let mut with_tombstone = inventory_record(b"a", 7);
            with_tombstone.extend_from_slice(&inventory_record(b"", 0));
            with_tombstone.extend_from_slice(&inventory_record(b"b", 8));
            assert_eq!(
                test_parse_inventory_records(
                    &with_tombstone,
                    &[(b"a".as_slice(), 7), (b"b".as_slice(), 8)]
                ),
                Ok(())
            );
            let overlong = inventory_record(&vec![b'a'; 1024], 7);
            assert!(test_parse_inventory_records(&overlong, &[]).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn prepared_inventory_seek_reset_and_authentication_failures_are_bounded() {
        let base = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("semaprax-inventory-failure-{}", std::process::id()));
        for (suffix, failures, expected_scans) in [
            ("initial", (true, false, false, false), 0),
            ("reset", (false, true, false, false), 1),
            ("authentication", (false, false, true, false), 1),
        ] {
            let root = base.join(suffix);
            with_inventory_fixture(&root, |directory, names, file, prepared| {
                super::platform::test_inventory_exact_failures(
                    prepared, failures.0, failures.1, failures.2, failures.3,
                );
                assert!(super::platform::inventory_exact_prepared(
                    prepared,
                    directory,
                    names,
                    [Some(file)]
                )
                .is_err());
                assert_eq!(
                    super::platform::test_inventory_exact_scan_entries(prepared),
                    expected_scans
                );
                assert_eq!(
                    super::platform::prepared_inventory_exact_remaining(prepared),
                    1
                );
            });
        }
        let _ = std::fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn prepared_inventory_rebound_close_failure_child() {
        let Ok(root) = std::env::var("SEMAPRAX_INVENTORY_CLOSE_FAILURE_ROOT") else {
            return;
        };
        let root = std::path::Path::new(&root);
        with_inventory_fixture(root, |directory, names, file, prepared| {
            super::platform::test_inventory_exact_failures(prepared, false, false, true, true);
            let _ =
                super::platform::inventory_exact_prepared(prepared, directory, names, [Some(file)]);
            std::fs::write(root.join("later-action"), b"must not exist").unwrap();
        });
    }

    #[cfg(unix)]
    #[test]
    fn prepared_inventory_rebound_close_failure_is_fail_stop() {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-inventory-close-failure-{}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("tests::prepared_inventory_rebound_close_failure_child")
            .arg("--nocapture")
            .env("SEMAPRAX_INVENTORY_CLOSE_FAILURE_ROOT", &root)
            .status()
            .unwrap();
        assert!(!status.success());
        assert!(!root.join("later-action").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn linux_prepared_transfer_has_injection_and_allocation_free_copy_fallback_contract() {
        let source = include_str!("lib.rs");
        let link = source
            .find("libc::AT_EMPTY_PATH")
            .expect("Linux prepared transfer uses the held source descriptor");
        let fallback = source[link..]
            .find("fn copy_regular_file_new_prepared")
            .map(|offset| link + offset)
            .expect("Linux fallback is independently authored");
        let linux = &source[link..fallback];
        let linked = linux.find("if result == 0").expect("link success branch");
        let injected = linux
            .find("if fail_before_authentication")
            .expect("debug failure precedes reopen authentication");
        let reopened = linux
            .find("hold_regular_file_name_prepared")
            .expect("prepared destination reopen");
        assert!(linked < injected && injected < reopened);
        for errno in ["libc::EPERM", "libc::EACCES", "libc::EOPNOTSUPP"] {
            assert!(linux.contains(errno));
        }
        assert!(linux.contains("copy_regular_file_new_prepared("));

        let copy = &source[fallback..];
        for required in [
            "libc::O_EXCL",
            "file.write_all(source_bytes)",
            "file.sync_data()",
            "authenticate_regular_file(file)",
        ] {
            assert!(copy.contains(required));
        }
        assert!(!source.contains(concat!("CAP_DAC_", "READ_SEARCH")));
    }

    #[test]
    fn prepared_inventory_exact_source_contract_is_raw_bounded_and_allocation_free() {
        let source = include_str!("lib.rs");
        let linux_start = source
            .find("#[cfg(target_os = \"linux\")]\n    fn parse_linux_inventory_records")
            .expect("Linux raw inventory scanner");
        let darwin_start = source[linux_start..]
            .find("#[cfg(target_os = \"macos\")]\n    fn parse_darwin_inventory_records")
            .map(|offset| linux_start + offset)
            .expect("Darwin raw inventory scanner");
        let inventory_start = source[darwin_start..]
            .find("pub fn inventory_exact_prepared")
            .map(|offset| darwin_start + offset)
            .expect("Unix prepared inventory entry point");
        let linux = &source[linux_start..darwin_start];
        let darwin = &source[darwin_start..inventory_start];
        for required in [
            "libc::SYS_getdents64",
            "let bytes_limit = libc::c_uint::try_from(capacity)",
            "prepared.storage.fill(u64::MAX)",
            "record < 20",
            "record % std::mem::align_of::<u64>() != 0",
            "next > bytes.len()",
            "maximum_records",
            "maximum_queries",
        ] {
            assert!(
                linux.contains(required),
                "missing Linux contract: {required}"
            );
        }
        for required in [
            "SYS_GETDIRENTRIES64",
            "let bytes_limit: libc::size_t",
            "let mut base: libc::off_t",
            "prepared.storage.fill(u64::MAX)",
            "record % 4 != 0",
            "name_length > 1023",
            "name_end >= next",
            "next > bytes.len()",
            "maximum_records",
            "maximum_queries",
        ] {
            assert!(
                darwin.contains(required),
                "missing Darwin contract: {required}"
            );
        }
        for forbidden in ["fdopendir", "readdir", "BTreeSet", "to_vec("] {
            assert!(!linux.contains(forbidden));
            assert!(!darwin.contains(forbidden));
        }

        let windows_start = source
            .match_indices("pub fn inventory_exact_prepared")
            .nth(1)
            .map(|(offset, _)| offset)
            .expect("Windows prepared inventory entry point");
        let windows_end = source[windows_start..]
            .find("pub fn publish_directory_new")
            .map(|offset| windows_start + offset)
            .expect("end of Windows inventory scanner");
        let windows = &source[windows_start..windows_end];
        for required in [
            "FileIdExtdDirectoryRestartInfo",
            "FILE_ID_EXTD_DIR_INFO",
            "prepared.storage.fill(u64::MAX)",
            "entry.FileId.Identifier != tracked.identity.file_id",
            "std::mem::size_of::<FILE_ID_EXTD_DIR_INFO>()",
            "record_header_end > byte_length",
            "next < minimum",
            "next_end > byte_length",
            "maximum_records",
            "maximum_queries",
        ] {
            assert!(
                windows.contains(required),
                "missing Windows contract: {required}"
            );
        }
        let full_header_bound = windows
            .find("if record_header_end > byte_length")
            .expect("complete Windows record must fit");
        let entry_reference = windows
            .find("let entry = unsafe")
            .expect("Windows entry reference");
        assert!(full_header_bound < entry_reference);
        assert!(!windows.contains("FILE_ID_BOTH_DIR_INFO"));
        assert!(!windows.contains("String::from_utf16"));
    }

    #[test]
    fn prepared_publish_source_contract_has_no_late_name_or_handle_allocation() {
        let source = include_str!("lib.rs");
        let unix_start = source
            .find("fn observe_publish_rebound")
            .expect("Unix prepared publish");
        let unix_end = source[unix_start..]
            .find("pub fn discard_owned_stage_prepared")
            .map(|offset| unix_start + offset)
            .expect("end Unix prepared publish");
        let unix = &source[unix_start..unix_end];
        for required in [
            "prepared.remaining != 1",
            "prepared.exact_capacity",
            "relative_name_arena_cstr",
            "observe_publish_rebound",
            "prepared_directory_identity(stage)",
            "libc::SYS_renameat2",
            "renameatx_np",
        ] {
            assert!(
                unix.contains(required),
                "missing Unix publish contract: {required}"
            );
        }
        for forbidden in ["c_name(", "try_clone", "CString::new", "Vec::"] {
            assert!(
                !unix.contains(forbidden),
                "late Unix publish operation: {forbidden}"
            );
        }

        let windows_start = source
            .match_indices("fn observe_publish_rebound")
            .nth(1)
            .map(|(offset, _)| offset)
            .expect("Windows prepared publish");
        let windows_end = source[windows_start..]
            .find("pub fn discard_owned_stage_prepared")
            .map(|offset| windows_start + offset)
            .expect("end Windows prepared publish");
        let windows = &source[windows_start..windows_end];
        for required in [
            "prepared.remaining != 1",
            "prepared.exact_capacity",
            "relative_file_arena",
            "observe_publish_rebound",
            "SetFileInformationByHandle",
            "FileRenameInfoEx",
        ] {
            assert!(
                windows.contains(required),
                "missing Windows publish contract: {required}"
            );
        }
        for forbidden in [
            "prepare_relative_name(",
            "named_information(",
            "try_clone",
            "collect::<Vec",
        ] {
            assert!(
                !windows.contains(forbidden),
                "late Windows publish operation: {forbidden}"
            );
        }
    }

    #[test]
    fn settlement_failure_injection_is_test_local_and_has_no_ambient_control() {
        let source = include_str!("lib.rs");
        let ambient_name = ["SEMAPRAX_NATIVE_RUST", "_INTEROP_TEST_SETTLEMENT_FAILURE"].concat();
        assert!(!source.contains(&ambient_name));
        assert!(source.contains("#[cfg(test)]\nstatic TEST_SETTLEMENT_FAILURES"));
        assert!(source.contains("#[cfg(not(test))]\n        {\n            false\n        }"));
        let obsolete_function = ["fn injected_settlement_", "failure(point: &str)"].concat();
        assert!(!source.contains(&obsolete_function));
    }

    #[test]
    fn prepared_process_arena_is_exact_and_consumes_twelve_without_growth() {
        let plan = super::platform::prepare_process_arena_plan(12).unwrap();
        let required = super::platform::prepared_process_arena_plan_capacity(&plan);
        let mut arena = super::platform::materialize_process_arena(plan).unwrap();
        let capacity = super::platform::prepared_process_arena_owned_capacity(&arena);
        assert_eq!(capacity, required);
        #[cfg(windows)]
        assert!((131_080 + 8..=1_245_188).contains(&capacity));
        for remaining in (0..12).rev() {
            super::platform::consume_process_arena(&mut arena).unwrap();
            assert_eq!(
                super::platform::prepared_process_arena_remaining(&arena),
                remaining
            );
            assert_eq!(
                super::platform::prepared_process_arena_owned_capacity(&arena),
                capacity
            );
        }
        assert_eq!(
            super::platform::consume_process_arena(&mut arena),
            Err(Error::OutputLimit)
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_process_arena_attribute_plan_is_exact_aligned_and_bounded() {
        const MAX_ATTRIBUTE_BYTES: usize = 1_048_576;
        for attribute_bytes in [1, 8, 9, 65_537, MAX_ATTRIBUTE_BYTES] {
            let plan = super::platform::process_arena_plan(12, attribute_bytes, 2).unwrap();
            let aligned =
                attribute_bytes.div_ceil(std::mem::size_of::<u64>()) * std::mem::size_of::<u64>();
            assert_eq!(
                super::platform::prepared_process_arena_plan_capacity(&plan),
                131_080 + aligned
            );
            let arena = super::platform::materialize_process_arena(plan).unwrap();
            assert_eq!(
                super::platform::prepared_process_arena_owned_capacity(&arena),
                131_080 + aligned
            );
        }
        assert!(matches!(
            super::platform::process_arena_plan(12, 0, 2),
            Err(Error::Unsupported)
        ));
        assert!(matches!(
            super::platform::process_arena_plan(12, MAX_ATTRIBUTE_BYTES + 1, 2),
            Err(Error::OutputLimit)
        ));

        let include = std::ffi::OsStr::new(r"C:\sdk\include;C:\msvc\include");
        let libraries = std::ffi::OsStr::new(r"C:\sdk\lib;C:\msvc\lib");
        let plan = super::platform::prepare_process_arena_plan_with_environment(
            12,
            Some(include),
            Some(libraries),
        )
        .unwrap();
        let required = super::platform::prepared_process_arena_plan_capacity(&plan);
        let arena = super::platform::materialize_process_arena_with_environment(
            plan,
            Some(include),
            Some(libraries),
        )
        .unwrap();
        assert_eq!(
            super::platform::prepared_process_arena_owned_capacity(&arena),
            required
        );
    }

    #[cfg(unix)]
    #[test]
    fn sysroot_output_is_one_nonempty_absolute_utf8_line() {
        assert_eq!(
            super::platform::one_sysroot_line(b"/toolchain\n"),
            Ok(&b"/toolchain"[..])
        );
        assert_eq!(
            super::platform::one_sysroot_line(b"/toolchain\r\n"),
            Ok(&b"/toolchain"[..])
        );
        for invalid in [
            &b""[..],
            &b"/toolchain"[..],
            &b"\n"[..],
            &b"/one\n/two\n"[..],
            &b"/one\0two\n"[..],
            &[0xff, b'\n'],
        ] {
            assert_eq!(
                super::platform::one_sysroot_line(invalid),
                Err(Error::Invalid)
            );
        }
        let resolver = super::platform::prepare_tool_resolver("rustc", 32_768).unwrap();
        assert!(matches!(
            super::platform::hold_rustc_discovery_prepared(
                resolver,
                std::ffi::OsStr::new("relative-rustc")
            ),
            Err(Error::Invalid)
        ));
    }

    #[test]
    fn direct_rustc_and_windows_process_source_contract_is_closed() {
        let source = include_str!("lib.rs");
        let discovery_symbol = ["pub fn hold_rustc_", "discovery_prepared"].concat();
        let direct_compile_symbol = ["pub fn compile_direct_", "rustc_prepared"].concat();
        let generic_worker = ["fn compile_rust_", "prepared_inner"].concat();
        assert_eq!(source.matches(&discovery_symbol).count(), 2);
        assert_eq!(source.matches(&direct_compile_symbol).count(), 2);
        assert_eq!(source.matches(&generic_worker).count(), 2);
        let generic_public = ["pub fn compile_rust_", "prepared("].concat();
        let legacy_public = ["pub fn compile_rust_", "staticlib("].concat();
        let misplaced = ["misplaced_windows_", "direct_rustc"].concat();
        assert!(!source.contains(&generic_public));
        assert!(!source.contains(&legacy_public));
        assert!(!source.contains(&misplaced));

        let windows_start = source.find("fn run_argv(\n        executable: &Executable,\n        cwd: &Directory,\n        arguments: &[String]").unwrap();
        let windows_end = source[windows_start..]
            .find("fn terminate_unassigned")
            .map(|offset| windows_start + offset)
            .unwrap();
        let windows = &source[windows_start..windows_end];
        for required in [
            "final_path_prepared(&executable.file.file, &mut process_arena.application)",
            "final_path_prepared(&cwd.file, &mut process_arena.cwd)",
            "process_arena.application.resize(PROCESS_PATH_UNITS, 0)",
            "process_arena.environment.as_ptr().cast()",
            "let mut attribute_bytes = process_arena.attribute_bytes",
            "process_arena.attributes.resize(attribute_words, 0)",
            "let null_name = [u16::from(b'N'), u16::from(b'U'), u16::from(b'L'), 0]",
            "must_terminate_unassigned(process_handle.raw())",
            "failed |= thread_handle.close().is_err()",
            "failed |= job.close().is_err()",
        ] {
            assert!(
                windows.contains(required),
                "missing Windows process contract: {required}"
            );
        }
        for forbidden in [
            "final_path(&executable.file.file)",
            "OpenOptions",
            "vec![0_u8; attribute_bytes]",
            "String::from_utf16",
            "PathBuf::from",
            "InitializeProcThreadAttributeList(std::ptr::null_mut()",
            "let empty_environment",
        ] {
            assert!(
                !windows.contains(forbidden),
                "late Windows process allocation: {forbidden}"
            );
        }
        let obsolete_attribute_words = ["PROCESS_ATTRIBUTE_", "WORDS"].concat();
        assert!(!source.contains(&obsolete_attribute_words));
        assert!(source.contains("pub fn prepare_process_arena_plan(uses: usize)"));
        assert!(source.contains(
            "InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes)"
        ));
    }

    #[test]
    fn windows_directory_identity_source_excludes_mutable_length_and_binds_all_rechecks() {
        let source = include_str!("lib.rs");
        let start = source.find("struct DirectoryIdentity").unwrap();
        let end = source[start..]
            .find("pub struct Directory")
            .map(|offset| start + offset)
            .unwrap();
        let identity = &source[start..end];
        for required in ["volume: u64", "file_id: [u8; 16]", "stable_attributes: u32"] {
            assert!(identity.contains(required));
        }
        assert!(!identity.contains("length:"));

        let windows_start = source.find("#[cfg(windows)]\nmod platform").unwrap();
        let windows = &source[windows_start..];
        for required in [
            "identity: DirectoryIdentity",
            "directory_identity: Option<DirectoryIdentity>",
            "FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT",
            "stable_attributes != FILE_ATTRIBUTE_DIRECTORY",
            "directory_information(&directory.file)? != directory.identity",
            "directory_information(&rebound)? == directory.identity",
            "directory_information(&rebound)? != stage.identity",
            "Result<DirectoryIdentity, Error>",
        ] {
            assert!(
                windows.contains(required),
                "missing stable Windows directory identity contract: {required}",
            );
        }
    }

    #[test]
    fn linux_rust_staticlib_link_tail_is_frozen_for_prepared_and_legacy_paths() {
        let source = include_str!("lib.rs");
        let unix_start = source.find("#[cfg(unix)]\nmod platform").unwrap();
        let windows_start = source.find("#[cfg(windows)]\nmod platform").unwrap();
        let unix = &source[unix_start..windows_start];
        let native_start = unix
            .find("const LINUX_RUST_STATICLIB_NATIVE_LIBS: [&str; 7]")
            .unwrap();
        let native_end = unix[native_start..]
            .find("\n    ];")
            .map(|offset| native_start + offset + "\n    ];".len())
            .unwrap();
        let native = &unix[native_start..native_end];
        let mut previous = 0usize;
        for required in [
            "-lgcc_s",
            "-lutil",
            "-lrt",
            "-lpthread",
            "-lm",
            "-ldl",
            "-lc",
        ] {
            let offset = native.find(required).unwrap();
            assert!(
                offset >= previous,
                "Linux native-static library order changed"
            );
            previous = offset;
        }
        assert_eq!(
            unix.matches("LINUX_RUST_STATICLIB_NATIVE_LIBS")
                .count(),
            3,
            "the frozen Linux native-static library tail must have one definition and exactly two link consumers",
        );
        assert_eq!(
            unix.matches("LINUX_LINKER_ARGUMENT").count(),
            3,
            "the absolute Linux linker argument must have one definition and exactly two link consumers",
        );

        let prepared_start = unix.find("pub fn prepare_link_invocation(").unwrap();
        let prepared_end = unix[prepared_start..]
            .find("pub fn prepared_link_owned_capacity(")
            .map(|offset| prepared_start + offset)
            .unwrap();
        let prepared = &unix[prepared_start..prepared_end];
        let prepared_linker = prepared
            .find("values[count] = LINUX_LINKER_ARGUMENT")
            .unwrap();
        let prepared_archive = prepared.find("rust_archive.to_str()").unwrap();
        let prepared_output = prepared.find("output.to_str()").unwrap();
        let prepared_tail = prepared
            .find("for value in LINUX_RUST_STATICLIB_NATIVE_LIBS")
            .unwrap();
        assert!(
            prepared_linker < prepared_archive
                && prepared_archive < prepared_output
                && prepared_output < prepared_tail
        );

        let legacy_start = unix.find("pub fn link_harness(").unwrap();
        let legacy_end = unix[legacy_start..]
            .find("let mut process_arena = prepare_process_arena(1)?")
            .map(|offset| legacy_start + offset)
            .unwrap();
        let legacy = &unix[legacy_start..legacy_end];
        assert!(legacy.contains("arguments.insert(2, argument(LINUX_LINKER_ARGUMENT)?)"));
        let legacy_archive = legacy.find("rust_archive.to_str()").unwrap();
        let legacy_output = legacy.find("output.to_str()").unwrap();
        let legacy_tail = legacy.find("LINUX_RUST_STATICLIB_NATIVE_LIBS").unwrap();
        assert!(legacy_archive < legacy_output && legacy_output < legacy_tail);
    }

    #[test]
    fn windows_rust_staticlib_link_tail_is_frozen_after_the_archive() {
        let source = include_str!("lib.rs");
        let windows_start = source.find("#[cfg(windows)]\nmod platform").unwrap();
        let windows_end = source[windows_start..]
            .find("\npub use platform::*;")
            .map(|offset| windows_start + offset)
            .unwrap();
        let windows = &source[windows_start..windows_end];
        let crt_start = windows
            .find("const WINDOWS_DYNAMIC_CRT_LINK_ARGS: [&str; 2]")
            .unwrap();
        let crt_end = windows[crt_start..]
            .find("];")
            .map(|offset| crt_start + offset + 2)
            .unwrap();
        let crt = &windows[crt_start..crt_end];
        assert!(crt.contains("\"-Xlinker\", \"/NODEFAULTLIB:libcmt\""));
        assert_eq!(windows.matches("WINDOWS_DYNAMIC_CRT_LINK_ARGS").count(), 4);
        let native_start = windows
            .find("const WINDOWS_RUST_STATICLIB_NATIVE_LIBS: [&str; 7]")
            .unwrap();
        let native_end = windows[native_start..]
            .find("\n    ];")
            .map(|offset| native_start + offset + "\n    ];".len())
            .unwrap();
        let native = &windows[native_start..native_end];
        let mut previous = 0usize;
        for required in [
            "-lkernel32",
            "-ladvapi32",
            "-ldbghelp",
            "-lntdll",
            "-luserenv",
            "-lws2_32",
            "-lmsvcrt",
        ] {
            let offset = native.find(required).unwrap();
            assert!(
                offset >= previous,
                "Windows native-static library order changed"
            );
            previous = offset;
        }
        assert_eq!(
            windows
                .matches("WINDOWS_RUST_STATICLIB_NATIVE_LIBS")
                .count(),
            9,
            "the frozen Windows native-static library tail must have one definition, seven indexed prepared entries, and one legacy consumer",
        );

        let prepared_start = windows.find("pub fn prepare_link_invocation(").unwrap();
        let prepared_end = windows[prepared_start..]
            .find("pub fn prepared_link_owned_capacity(")
            .map(|offset| prepared_start + offset)
            .unwrap();
        let prepared = &windows[prepared_start..prepared_end];
        let arguments_start = prepared.find("let argument_parts:").unwrap();
        let arguments_end = prepared[arguments_start..]
            .find("preflight_windows_command_line(&argument_parts)?")
            .map(|offset| arguments_start + offset)
            .unwrap();
        let arguments = &prepared[arguments_start..arguments_end];
        let prepared_linker = arguments.find("&[\"-fuse-ld=link\"]").unwrap();
        let prepared_vctools = arguments
            .find("&[\"-Xmicrosoft-visualc-tools-root\"]")
            .unwrap();
        let prepared_crt = arguments.find("WINDOWS_DYNAMIC_CRT_LINK_ARGS").unwrap();
        let prepared_archive = arguments.find("&[rust_archive]").unwrap();
        let prepared_tail = arguments
            .find("WINDOWS_RUST_STATICLIB_NATIVE_LIBS")
            .unwrap();
        let prepared_output = arguments.find("&[\"-o\"]").unwrap();
        assert!(
            prepared_vctools < prepared_linker
                && prepared_linker < prepared_crt
                && prepared_crt < prepared_archive
                && prepared_archive < prepared_tail
                && prepared_tail < prepared_output
        );
        assert!(
            prepared.find("linker_units > MAX_TOOL_PATH_UNITS").unwrap()
                < prepared
                    .find("Vec::with_capacity(argument_parts.len())")
                    .unwrap()
        );

        let legacy_start = windows.find("pub fn link_harness(").unwrap();
        let legacy_end = windows[legacy_start..]
            .find("let command_line = windows_command_line(&arguments)?")
            .map(|offset| legacy_start + offset)
            .unwrap();
        let legacy = &windows[legacy_start..legacy_end];
        let legacy_linker = legacy.find("\"-fuse-ld=link\".to_owned()").unwrap();
        let legacy_vctools = legacy
            .find("\"-Xmicrosoft-visualc-tools-root\".to_owned()")
            .unwrap();
        let legacy_crt = legacy.find("WINDOWS_DYNAMIC_CRT_LINK_ARGS").unwrap();
        let legacy_archive = legacy.find("rust_archive.to_string_lossy()").unwrap();
        let legacy_tail = legacy.find("WINDOWS_RUST_STATICLIB_NATIVE_LIBS").unwrap();
        let legacy_output = legacy.find("arguments.extend([\"-o\"").unwrap();
        assert!(
            legacy_vctools < legacy_linker
                && legacy_linker < legacy_crt
                && legacy_crt < legacy_archive
                && legacy_archive < legacy_tail
                && legacy_tail < legacy_output
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_prepared_link_owns_the_exact_native_static_tail() {
        for linker in [None, Some(std::ffi::OsStr::new("relative\\link.exe"))] {
            assert!(matches!(
                super::platform::prepare_link_invocation(
                    "x86_64-pc-windows-msvc",
                    linker,
                    Some(std::ffi::OsStr::new(
                        r"C:\Program Files\Microsoft Visual Studio\Lïnk",
                    )),
                    std::ffi::OsStr::new("main.obj"),
                    std::ffi::OsStr::new("module.obj"),
                    std::ffi::OsStr::new("bridge.lib"),
                    std::ffi::OsStr::new("output.exe"),
                    false,
                ),
                Err(Error::Invalid)
            ));
        }
        let vctools = r"C:\Program Files\Microsoft Visual Studio\Lïnk";
        let linker = r"C:\Program Files\Microsoft Visual Studio\Lïnk\bin\Hostx64\x64\link.exe";
        let prepared = super::platform::prepare_link_invocation(
            "x86_64-pc-windows-msvc",
            Some(std::ffi::OsStr::new(linker)),
            Some(std::ffi::OsStr::new(vctools)),
            std::ffi::OsStr::new("main.obj"),
            std::ffi::OsStr::new("module.obj"),
            std::ffi::OsStr::new("bridge.lib"),
            std::ffi::OsStr::new("output.exe"),
            false,
        )
        .unwrap();
        let owned = super::platform::prepared_link_owned_capacity(&prepared);
        let expected = [
            "-target",
            "x86_64-pc-windows-msvc",
            "-Xmicrosoft-visualc-tools-root",
            r"C:\Program Files\Microsoft Visual Studio\Lïnk",
            "-fuse-ld=link",
            "-Xlinker",
            "/NODEFAULTLIB:libcmt",
            "main.obj",
            "module.obj",
            "bridge.lib",
            "-lkernel32",
            "-ladvapi32",
            "-ldbghelp",
            "-lntdll",
            "-luserenv",
            "-lws2_32",
            "-lmsvcrt",
            "-o",
            "output.exe",
        ];
        let (arguments, capacity) = super::platform::test_prepared_link_arguments(&prepared);
        assert!(arguments.iter().map(String::as_str).eq(expected));
        assert_eq!(capacity, expected.len());
        assert_eq!(
            super::platform::prepared_link_owned_capacity(&prepared),
            owned,
        );
    }

    #[test]
    fn linux_runner_uses_the_held_executable_path_as_argv0_before_fexecve() {
        let source = include_str!("lib.rs");
        let start = source
            .find("#[cfg(target_os = \"linux\")]\n    fn run_argv(")
            .unwrap();
        let end = source[start..]
            .find("#[cfg(target_os = \"macos\")]\n    fn run_argv(")
            .map(|offset| start + offset)
            .unwrap();
        let runner = &source[start..end];
        assert!(!runner.contains("semaprax-native-rust-interop-tool"));
        for required in [
            "let executable_fd_format = b\"/proc/self/fd/%d\\0\"",
            "libc::snprintf(",
            "libc::readlink(",
            "argv[0] = argv0.as_ptr().cast()",
            "fexecve(executable_fd, argv.as_ptr(), env.as_ptr())",
        ] {
            assert!(
                runner.contains(required),
                "missing Linux argv0 contract: {required}"
            );
        }
        let duplicated = runner.find("libc::F_DUPFD").unwrap();
        let readlink = runner.find("libc::readlink(").unwrap();
        let argv0 = runner.find("argv[0] = argv0.as_ptr().cast()").unwrap();
        let execute = runner
            .find("fexecve(executable_fd, argv.as_ptr(), env.as_ptr())")
            .unwrap();
        assert!(duplicated < readlink && readlink < argv0 && argv0 < execute);
    }

    #[cfg(windows)]
    #[test]
    fn windows_directory_identity_survives_full_inventory_and_rejects_foreign_or_substituted_path()
    {
        use std::ffi::OsStr;

        let root = std::env::temp_dir().join(format!(
            "semaprax-windows-directory-identity-{}",
            std::process::id(),
        ));
        let stage_path = root.join("stage");
        let displaced_path = root.join("displaced");
        let foreign_path = root.join("foreign");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&stage_path).unwrap();
        std::fs::create_dir(&foreign_path).unwrap();
        let stage = super::platform::hold_directory(&stage_path).unwrap();
        let names = super::platform::prepare_discard_names([
            OsStr::new("a"),
            OsStr::new("b"),
            OsStr::new("c"),
            OsStr::new("d"),
            OsStr::new("e"),
            OsStr::new("f"),
            OsStr::new("g"),
        ])
        .unwrap();
        let files = (0..7)
            .map(|index| {
                super::platform::write_file_new_prepared(
                    &stage,
                    &names,
                    index,
                    &[u8::try_from(index).unwrap()],
                    0,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        super::platform::recheck_directory(&stage).unwrap();
        let mut inventory = super::platform::prepare_inventory_exact(&names).unwrap();
        let attached = std::array::from_fn(|index| Some(&files[index]));
        super::platform::inventory_exact_prepared(&mut inventory, &stage, &names, attached)
            .unwrap();
        assert!(!super::platform::same_directory_path(&stage, &foreign_path).unwrap());

        drop((inventory, files, names));
        std::fs::rename(&stage_path, &displaced_path).unwrap();
        std::fs::create_dir(&stage_path).unwrap();
        super::platform::recheck_directory(&stage).unwrap();
        assert!(!super::platform::same_directory_path(&stage, &stage_path).unwrap());
        drop(stage);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
