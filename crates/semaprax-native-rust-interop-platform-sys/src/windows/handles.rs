//! Held Windows directories, regular files, and executables, including tool
//! resolution, rustc discovery, and the prepared link-or-copy transfer.

use super::*;

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

pub fn hold_child_directory(parent: &Directory, name: &OsStr) -> Result<Directory, Error> {
    recheck_directory(parent)?;
    normal_name(name)?;
    let file = relative_file(
        &parent.file,
        name,
        DIRECTORY_READ_ACCESS,
        FILE_OPEN,
        FILE_DIRECTORY_FILE,
    )?;
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
    mode: u32,
) -> Result<Directory, Error> {
    create_directory_new_prepared_settled(parent, name, mode).map_err(|failure| failure.error)
}

pub fn create_directory_new_prepared_settled(
    parent: &Directory,
    name: &PreparedRelativeNameArena,
    _mode: u32,
) -> Result<Directory, CreateDirectoryNewFailure> {
    let settled = |error| CreateDirectoryNewFailure {
        error,
        namespace_created: false,
    };
    recheck_directory(parent).map_err(settled)?;
    let file = relative_file_arena(
        &parent.file,
        name,
        DIRECTORY_OWNED_ACCESS,
        FILE_CREATE,
        FILE_DIRECTORY_FILE,
    )
    .map_err(|error| CreateDirectoryNewFailure {
        error,
        // A non-collision NT create failure is conservatively uncertain: the
        // native call may have committed the namespace before failing to
        // return usable authority.
        namespace_created: error != Error::Exists,
    })?;
    let identity = directory_information(&file).map_err(|error| CreateDirectoryNewFailure {
        error,
        namespace_created: true,
    })?;
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

pub(super) fn hold_regular_file_name_external_read_prepared(
    directory: &Directory,
    name: &PreparedRelativeName,
) -> Result<RegularFile, Error> {
    let file = relative_file_prepared(
        &directory.file,
        name,
        REGULAR_READ_ACCESS,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )?;
    authenticate_regular_file(file)
}

pub(super) fn hold_regular_file_name_external_read_bounded_prepared(
    directory: &Directory,
    name: &PreparedRelativeName,
    maximum: u64,
) -> Result<RegularFile, Error> {
    let file = relative_file_prepared(
        &directory.file,
        name,
        REGULAR_READ_ACCESS,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )?;
    authenticate_regular_file_bounded(file, maximum)
}

#[cfg(test)]
pub(crate) fn test_hold_regular_file_name_bounded(
    directory: &Directory,
    name: &OsStr,
    maximum: u64,
) -> Result<RegularFile, Error> {
    let name = prepare_relative_name(name)?;
    hold_regular_file_name_external_read_bounded_prepared(directory, &name, maximum)
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
    let identity = information(&file)?;
    if identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !file.metadata().map_err(|_| Error::Changed)?.is_file()
    {
        return Err(Error::Changed);
    }
    if identity.length > maximum {
        return Err(Error::OutputLimit);
    }
    let digest = digest(&file, identity.length)?;
    Ok(RegularFile {
        file,
        identity,
        digest,
    })
}

pub(super) fn hold_regular_file_name_prepared(
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

pub fn hold_settled_regular_file_prepared<const N: usize>(
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    index: usize,
    tracked: &SettledRegularFile,
) -> Result<RegularFile, Error> {
    let name = enter_prepared_file_syscalls(prepared_discard_name(names, index))?;
    recheck_directory(directory)?;
    let rebound = hold_regular_file_name_prepared(directory, name)?;
    if rebound.identity != tracked.identity || rebound.digest != tracked.digest {
        return Err(Error::Changed);
    }
    Ok(rebound)
}

pub fn transition_regular_file_to_external_read_prepared<const N: usize>(
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    index: usize,
    tracked: &RegularFile,
) -> Result<RegularFile, Error> {
    let name = enter_prepared_file_syscalls(prepared_discard_name(names, index))?;
    recheck_directory(directory)?;
    recheck_held_regular(tracked)?;
    let rebound = hold_regular_file_name_external_read_prepared(directory, name)?;
    if rebound.identity != tracked.identity || rebound.digest != tracked.digest {
        return Err(Error::Changed);
    }
    Ok(rebound)
}

pub fn recheck_regular(file: &RegularFile) -> Result<(), Error> {
    recheck_held_regular(file)
}

pub(super) fn recheck_held_regular(file: &RegularFile) -> Result<(), Error> {
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
    recheck_directory(directory)?;
    let name = prepare_relative_name(name)?;
    let file = hold_regular_file_name_external_read_prepared(directory, &name)?;
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
    if usize::try_from(file.identity.length).map_err(|_| Error::OutputLimit)? != expected.len() {
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
    files
        .iter()
        .try_for_each(|file| recheck_held_regular(file))?;
    directories
        .iter()
        .try_for_each(|child| recheck_directory(child))?;
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut records = 0usize;
    let mut queries = 0usize;
    let mut restart = true;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > N.checked_add(3).ok_or(Error::OutputLimit)? {
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
        let mut offset = 0usize;
        loop {
            let header_end = offset
                .checked_add(std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, FileName))
                .ok_or(Error::Changed)?;
            if offset
                .checked_add(std::mem::size_of::<FILE_ID_EXTD_DIR_INFO>())
                .ok_or(Error::Changed)?
                > byte_length
                || header_end > byte_length
            {
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
            let name_bytes = usize::try_from(entry.FileNameLength).map_err(|_| Error::Changed)?;
            if !name_bytes.is_multiple_of(2) {
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
            records = records.checked_add(1).ok_or(Error::OutputLimit)?;
            if records > N.checked_add(2).ok_or(Error::OutputLimit)? {
                return Err(Error::Changed);
            }
            if !dot && !dot_dot {
                let index = prepared
                    .names
                    .names
                    .iter()
                    .position(|expected| {
                        prepared_matches_slice(expected.as_ref().expect("prepared name"), name)
                    })
                    .ok_or(Error::Changed)?;
                let expected = if index < F {
                    files[index].identity.file_id
                } else {
                    directories[index.checked_sub(F).ok_or(Error::Changed)?]
                        .identity
                        .file_id
                };
                if seen[index] || entry.FileId.Identifier != expected {
                    return Err(Error::Changed);
                }
                seen[index] = true;
                count = count.checked_add(1).ok_or(Error::OutputLimit)?;
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
        let rebound = relative_file_prepared(
            &directory.file,
            name,
            DIRECTORY_READ_ACCESS,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        )?;
        if directory_information(&rebound)? != child.identity {
            return Err(Error::Changed);
        }
    }
    recheck_directory(directory)?;
    files
        .iter()
        .try_for_each(|file| recheck_held_regular(file))?;
    directories
        .iter()
        .try_for_each(|child| recheck_directory(child))?;
    Ok(())
}
