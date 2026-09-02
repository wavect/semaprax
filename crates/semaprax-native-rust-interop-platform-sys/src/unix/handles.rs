//! Held Unix directories, regular files, and executables, including tool
//! resolution, rustc discovery, and the prepared link-or-copy transfer.

use super::*;

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

pub(super) fn hold_regular_file_name_prepared(
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

pub(super) fn hold_regular_file_name_bounded_prepared(
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

pub(crate) fn one_sysroot_line(output: &[u8]) -> Result<&[u8], Error> {
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

pub(super) fn recheck_executable(executable: &Executable) -> Result<(), Error> {
    recheck_regular(&executable.file)?;
    if executable_slice(&executable.file)? != (executable.slice_offset, executable.slice_size) {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn recheck_executable_launch_path(executable: &Executable) -> Result<(), Error> {
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

pub(super) fn admit_inventory_entry<const N: usize>(
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

pub(super) fn prepared_directory_identity(directory: &Directory) -> PreparedDirectoryIdentity {
    PreparedDirectoryIdentity {
        dev: directory.dev,
        ino: directory.ino,
        mode: directory.mode,
        #[cfg(target_os = "macos")]
        generation: directory.generation,
    }
}

pub(super) struct ObservedRegularIdentity {
    dev: u64,
    ino: u64,
    mode: u32,
    len: u64,
    digest: [u8; 32],
    #[cfg(target_os = "macos")]
    generation: u32,
}

pub(super) fn same_regular_identity(left: &ObservedRegularIdentity, right: &RegularFile) -> bool {
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

pub(super) fn observe_inventory_rebound(
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
