//! Unix child launch modes and the prepared compile, link, and archive
//! invocation builders.

use super::*;

#[path = "archive.rs"]
mod archive;
pub use archive::*;

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
pub(super) fn run_archive_argv(
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
pub(super) fn run_archive_argv(
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

pub(super) fn argument(value: &str) -> Result<CString, Error> {
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
pub(crate) fn test_prepared_archive_arguments(prepared: &PreparedArchiveInvocation) -> Vec<&[u8]> {
    prepared
        .command
        .arguments
        .iter()
        .map(|argument| argument.as_bytes())
        .collect()
}
