//! Complete immutable admission before namespace, cgroup or process mutation.
use super::capsule::{self, Artifact};
use crate::doctor::{
    offline_bundle::elf, offline_worker::wire, DoctorOfflineArchitecture, DoctorOfflineBundle,
    DoctorOfflineInput, DOCTOR_OFFLINE_INPUT_MAX_BYTES,
};
use sha2::{Digest as _, Sha256};
use std::fs::File;
use std::mem::ManuallyDrop;
use std::os::fd::FromRawFd;

pub(super) const CAPSULE_FD: i32 = 3;
pub(super) const REQUEST_FD: i32 = 4;
pub(super) const BUNDLE_FD: i32 = 5;
pub(super) const LAUNCHER_FD: i32 = 6;
pub(super) const WORKER_FD: i32 = 7;
pub(super) const COLLECTOR_FD: i32 = 8;
pub(super) const CGROUP_FD: i32 = 9;
pub(super) const PROC_FD: i32 = 10;

pub(super) fn validate() -> Result<(), ()> {
    for fd in 0..=PROC_FD {
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
            return Err(());
        }
    }
    require_wait_policy()?;
    require_directory_fs(CGROUP_FD, libc::CGROUP2_SUPER_MAGIC as libc::c_long)?;
    require_directory_fs(PROC_FD, libc::PROC_SUPER_MAGIC as libc::c_long)?;
    require_anonymous_pipe(0, libc::O_RDONLY)?;
    require_anonymous_pipe(1, libc::O_WRONLY)?;
    require_anonymous_pipe(2, libc::O_WRONLY)?;
    require_single_thread()?;
    require_exact_descriptor_inventory()?;

    // Trust is established before parsing authority-bearing request fields or
    // touching namespace/cgroup state. Acquisition only snapshots sealed bytes.
    let capsule_input = acquire(CAPSULE_FD, capsule::MAX_CAPSULE_BYTES)?;
    let capsule = capsule::parse_with_release_anchor(capsule_input.bytes()).map_err(|_| ())?;
    if capsule.architecture != native_architecture_byte() {
        return Err(());
    }

    if capsule.request().length > wire::MAX_REQUEST_BYTES as u64 {
        return Err(());
    }
    let request_input = acquire_binding(REQUEST_FD, capsule.request())?;
    let request = wire::Request::parse(request_input.bytes()).map_err(|_| ())?;
    if request.architecture != native_architecture()
        || request.target != capsule.target
        || capsule::roles_for_target(request.target) != Some(capsule.roles)
        || request.selector != capsule.selector
    {
        return Err(());
    }
    let bundle_input = acquire_binding(BUNDLE_FD, capsule.bundle())?;
    if bundle_input.bytes().len() != request.bundle_len
        || <[u8; 32]>::from(Sha256::digest(bundle_input.bytes())) != request.bundle_digest
    {
        return Err(());
    }
    let bundle = DoctorOfflineBundle::parse(bundle_input, &capsule.selector).map_err(|_| ())?;
    if bundle.architecture() != native_architecture()
        || request.roles().any(|(_, tool)| bundle.tool(tool).is_none())
    {
        return Err(());
    }
    drop(bundle);

    validate_image(LAUNCHER_FD, capsule.launcher())?;
    validate_image(WORKER_FD, capsule.worker())?;
    validate_image(COLLECTOR_FD, capsule.collector())?;
    validate_cgroup_files()?;
    Ok(())
}

fn acquire(fd: i32, limit: usize) -> Result<DoctorOfflineInput, ()> {
    let file = ManuallyDrop::new(unsafe { File::from_raw_fd(fd) });
    DoctorOfflineInput::acquire(&file, limit).map_err(|_| ())
}

fn acquire_binding(fd: i32, binding: Artifact) -> Result<DoctorOfflineInput, ()> {
    let limit = usize::try_from(binding.length).map_err(|_| ())?;
    let input = acquire(fd, limit)?;
    if input.bytes().len() != limit
        || <[u8; 32]>::from(Sha256::digest(input.bytes())) != binding.digest
    {
        return Err(());
    }
    Ok(input)
}

fn validate_image(fd: i32, binding: Artifact) -> Result<(), ()> {
    if binding.length > DOCTOR_OFFLINE_INPUT_MAX_BYTES as u64 {
        return Err(());
    }
    let input = acquire_binding(fd, binding)?;
    let seals = unsafe { libc::fcntl(fd, libc::F_GET_SEALS) };
    if seals < 0 || seals & libc::F_SEAL_EXEC == 0 {
        return Err(());
    }
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe { libc::fstat(fd, metadata.as_mut_ptr()) } != 0 {
        return Err(());
    }
    let mode = unsafe { metadata.assume_init() }.st_mode;
    if mode & 0o111 == 0 || mode & (libc::S_ISUID | libc::S_ISGID) != 0 {
        return Err(());
    }
    validate_static_elf(input.bytes())
}

fn validate_static_elf(bytes: &[u8]) -> Result<(), ()> {
    match elf::validate(bytes, native_architecture()) {
        Ok(None) => Ok(()),
        // A PT_INTERP path would let execveat reopen an ambient filesystem
        // loader despite the held launcher descriptor. This check does not
        // authenticate kernel binfmt policy; an approved native-ELF-only
        // binfmt configuration remains a trusted provisioner precondition.
        Ok(Some(_)) | Err(_) => Err(()),
    }
}

fn native_architecture() -> DoctorOfflineArchitecture {
    if cfg!(target_arch = "x86_64") {
        DoctorOfflineArchitecture::LinuxX86_64
    } else {
        DoctorOfflineArchitecture::LinuxAarch64
    }
}

fn native_architecture_byte() -> u8 {
    if cfg!(target_arch = "x86_64") {
        1
    } else {
        2
    }
}

fn require_wait_policy() -> Result<(), ()> {
    let mut action = std::mem::MaybeUninit::<libc::sigaction>::zeroed();
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), action.as_mut_ptr()) } != 0 {
        return Err(());
    }
    let action = unsafe { action.assume_init() };
    if action.sa_sigaction != libc::SIG_DFL || action.sa_flags & libc::SA_NOCLDWAIT != 0 {
        Err(())
    } else {
        Ok(())
    }
}

fn require_directory_fs(fd: i32, magic: libc::c_long) -> Result<(), ()> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::zeroed();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0
        || unsafe { libc::fstatfs(fd, filesystem.as_mut_ptr()) } != 0
    {
        return Err(());
    }
    let stat = unsafe { stat.assume_init() };
    let filesystem = unsafe { filesystem.assume_init() };
    if stat.st_mode & libc::S_IFMT != libc::S_IFDIR || filesystem.f_type != magic {
        Err(())
    } else {
        Ok(())
    }
}

fn require_anonymous_pipe(fd: i32, access: i32) -> Result<(), ()> {
    if unsafe { libc::fcntl(fd, libc::F_GETPIPE_SZ) } < 0 {
        return Err(());
    }
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if flags < 0
        || flags & libc::O_ACCMODE != access
        || unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0
        || unsafe { stat.assume_init() }.st_mode & libc::S_IFMT != libc::S_IFIFO
    {
        return Err(());
    }
    let path = match fd {
        0 => c"self/fd/0",
        1 => c"self/fd/1",
        2 => c"self/fd/2",
        _ => return Err(()),
    };
    let mut target = [0u8; 64];
    let length = unsafe {
        libc::readlinkat(
            PROC_FD,
            path.as_ptr(),
            target.as_mut_ptr().cast(),
            target.len(),
        )
    };
    let length = usize::try_from(length).map_err(|_| ())?;
    if length == target.len() || !anonymous_pipe_link(&target[..length]) {
        Err(())
    } else {
        Ok(())
    }
}

fn anonymous_pipe_link(bytes: &[u8]) -> bool {
    let Some(inode) = bytes
        .strip_prefix(b"pipe:[")
        .and_then(|bytes| bytes.strip_suffix(b"]"))
    else {
        return false;
    };
    !inode.is_empty() && inode.iter().all(u8::is_ascii_digit) && inode[0] != b'0'
}

fn require_exact_descriptor_inventory() -> Result<(), ()> {
    let directory = unsafe {
        libc::openat(
            PROC_FD,
            c"self/fd".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if directory < 0 {
        return Err(());
    }
    let result = scan_descriptors(directory);
    if unsafe { libc::close(directory) } != 0 {
        return Err(());
    }
    result
}

fn require_single_thread() -> Result<(), ()> {
    let directory = unsafe {
        libc::openat(
            PROC_FD,
            c"self/task".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if directory < 0 {
        return Err(());
    }
    let result = scan_threads(directory);
    if unsafe { libc::close(directory) } != 0 {
        return Err(());
    }
    result
}

fn scan_threads(directory: i32) -> Result<(), ()> {
    let expected = unsafe { libc::getpid() };
    let mut seen = false;
    scan_directory(directory, |name| {
        let tid = parse_decimal(name)?;
        if seen || tid != expected {
            return Err(());
        }
        seen = true;
        Ok(())
    })?;
    if seen {
        Ok(())
    } else {
        Err(())
    }
}

fn scan_descriptors(directory: i32) -> Result<(), ()> {
    scan_directory(directory, |name| {
        let fd = parse_decimal(name)?;
        if !(0..=PROC_FD).contains(&fd) && fd != directory {
            return Err(());
        }
        Ok(())
    })
}

fn scan_directory(
    directory: i32,
    mut accept: impl FnMut(&[u8]) -> Result<(), ()>,
) -> Result<(), ()> {
    let mut buffer = [0u8; 4096];
    loop {
        let count = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                directory,
                buffer.as_mut_ptr(),
                buffer.len(),
            )
        };
        if count < 0 {
            return Err(());
        }
        if count == 0 {
            return Ok(());
        }
        let mut cursor = 0usize;
        let count = usize::try_from(count).map_err(|_| ())?;
        while cursor < count {
            if count - cursor < 19 {
                return Err(());
            }
            let record = &buffer[cursor..count];
            let length = usize::from(u16::from_ne_bytes([record[16], record[17]]));
            if length < 20
                || cursor
                    .checked_add(length)
                    .filter(|end| *end <= count)
                    .is_none()
            {
                return Err(());
            }
            let name = &record[19..length];
            let end = name.iter().position(|byte| *byte == 0).ok_or(())?;
            let name = &name[..end];
            if name != b"." && name != b".." {
                accept(name)?;
            }
            cursor += length;
        }
    }
}

fn parse_decimal(bytes: &[u8]) -> Result<i32, ()> {
    if bytes.is_empty() {
        return Err(());
    }
    let mut value = 0i32;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return Err(());
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(i32::from(*byte - b'0')))
            .ok_or(())?;
    }
    Ok(value)
}

fn validate_cgroup_files() -> Result<(), ()> {
    for (name, write) in [
        (c"cgroup.procs", true),
        (c"cgroup.events", false),
        (c"cgroup.type", false),
        (c"cgroup.kill", true),
        (c"pids.max", true),
        (c"memory.max", true),
        (c"memory.swap.max", true),
        (c"memory.oom.group", true),
        (c"cpu.max", true),
    ] {
        let flags = if write {
            libc::O_WRONLY
        } else {
            libc::O_RDONLY
        };
        let fd = unsafe {
            libc::openat(
                CGROUP_FD,
                name.as_ptr(),
                flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(());
        }
        let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
        let valid = unsafe { libc::fstat(fd, stat.as_mut_ptr()) } == 0
            && unsafe { stat.assume_init() }.st_mode & libc::S_IFMT == libc::S_IFREG;
        if unsafe { libc::close(fd) } != 0 || !valid {
            return Err(());
        }
    }
    super::cgroup::require_empty()
}

#[cfg(test)]
mod tests {
    use super::{
        anonymous_pipe_link, native_architecture, parse_decimal, scan_descriptors,
        validate_static_elf,
    };

    fn elf(interpreter: bool) -> Vec<u8> {
        let mut bytes = vec![0u8; 128];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
        let machine = match native_architecture() {
            crate::doctor::DoctorOfflineArchitecture::LinuxX86_64 => 62u16,
            crate::doctor::DoctorOfflineArchitecture::LinuxAarch64 => 183u16,
        };
        bytes[18..20].copy_from_slice(&machine.to_le_bytes());
        bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
        bytes[32..40].copy_from_slice(&64u64.to_le_bytes());
        bytes[52..54].copy_from_slice(&64u16.to_le_bytes());
        bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
        bytes[56..58].copy_from_slice(&1u16.to_le_bytes());
        if interpreter {
            bytes[64..68].copy_from_slice(&3u32.to_le_bytes());
            bytes[72..80].copy_from_slice(&120u64.to_le_bytes());
            bytes[96..104].copy_from_slice(&8u64.to_le_bytes());
            bytes[120..128].copy_from_slice(b"/lib/ld\0");
        }
        bytes
    }

    #[test]
    fn decimal_descriptor_parser_is_closed() {
        assert_eq!(parse_decimal(b"0"), Ok(0));
        assert_eq!(parse_decimal(b"10"), Ok(10));
        assert_eq!(parse_decimal(b""), Err(()));
        assert_eq!(parse_decimal(b"+1"), Err(()));
        assert_eq!(parse_decimal(b"01x"), Err(()));
        assert_eq!(parse_decimal(b"999999999999999999999"), Err(()));
    }

    #[test]
    fn only_procfs_anonymous_pipe_links_are_admitted() {
        assert!(anonymous_pipe_link(b"pipe:[123]"));
        for hostile in [
            b"pipe:[]".as_slice(),
            b"pipe:[0]",
            b"pipe:[01x]",
            b"/tmp/fifo",
            b"socket:[123]",
        ] {
            assert!(!anonymous_pipe_link(hostile));
        }
    }

    #[test]
    fn descriptor_scan_rejects_a_foreign_live_descriptor() {
        // Take a descriptor above the fixed inventory rather than the lowest
        // free one: the harness's own descriptor count decides whether `dup`
        // lands inside the admitted range, which left this rejection resting on
        // the environment. `F_DUPFD_CLOEXEC` allocates at or above the floor
        // without displacing anything the harness already holds.
        let extra = unsafe { libc::fcntl(0, libc::F_DUPFD_CLOEXEC, super::PROC_FD + 1) };
        if extra < 0 {
            return;
        }
        // The current test process now intentionally violates the production
        // fixed inventory.
        let directory = unsafe {
            libc::open(
                c"/proc/self/fd".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        assert!(directory >= 0);
        assert_eq!(scan_descriptors(directory), Err(()));
        assert_eq!(unsafe { libc::close(directory) }, 0);
        assert_eq!(unsafe { libc::close(extra) }, 0);
    }

    #[test]
    fn held_launcher_must_not_reopen_an_ambient_elf_interpreter() {
        assert_eq!(validate_static_elf(&elf(false)), Ok(()));
        assert_eq!(validate_static_elf(&elf(true)), Err(()));
    }
}
