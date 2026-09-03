//! Unix held-handle filesystem, process, and archive authority.
//!
//! This root owns the held-handle types, the frozen Linux link tail, and the
//! shared prepared-plan storage. The phases that operate on them live in
//! submodules: [`plans`] builds exact preflight capacity, [`primitives`]
//! supplies digests and child settlement, [`handles`] holds directories,
//! files, and tools, [`inventory`] scans and publishes stage directories, and
//! [`process`] launches children and builds invocations. [`process::archive`]
//! owns exact archive admission and the legacy harness entry points; it nests
//! under [`process`] because it drives the private launch helpers directly.
use super::*;
use sha2::{Digest as _, Sha256};
use std::os::fd::{AsRawFd as _, FromRawFd as _, RawFd};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{FileExt as _, MetadataExt as _};

#[path = "unix/handles.rs"]
mod handles;
#[path = "unix/inventory.rs"]
mod inventory;
#[path = "unix/plans.rs"]
mod plans;
#[path = "unix/primitives.rs"]
mod primitives;
#[path = "unix/process.rs"]
mod process;

pub use handles::*;
pub use inventory::*;
pub use plans::*;
use primitives::*;
pub use process::*;

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
