//! Prepared version, compile, link, and archive invocations, exact archive
//! admission, and the legacy harness entry points.

use super::*;

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
        || !matches!(optimization, 0..=2)
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
            "-mno-incremental-linker-compatible",
            "-Wall",
            "-Wextra",
            "-Werror",
            match optimization {
                0 => "-O0",
                1 => "-O1",
                _ => "-O2",
            },
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

#[cfg(test)]
pub(crate) fn test_prepared_c_compile_arguments(
    prepared: &PreparedCCompileInvocation,
) -> (&[String], usize) {
    (&prepared.0.arguments, prepared.0.arguments.capacity())
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
pub(crate) fn test_prepared_link_arguments(
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
    let file = hold_regular_file_name_external_read_prepared(cwd, &prepared.output_name)?;
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

pub fn prepare_archive_invocation(
    input: &OsStr,
    output: &OsStr,
) -> Result<PreparedArchiveInvocation, Error> {
    normal_name(input)?;
    normal_name(output)?;
    if input != OsStr::new("module.obj") || output != OsStr::new("semaprax_native_rust_sdk.lib") {
        return Err(Error::Invalid);
    }
    let input_text = input.to_str().ok_or(Error::Invalid)?;
    let output_text = output.to_str().ok_or(Error::Invalid)?;
    let mut output_argument = String::with_capacity(5 + output_text.len());
    output_argument.push_str("/OUT:");
    output_argument.push_str(output_text);
    if output_argument.capacity() != 5 + output_text.len() {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedArchiveInvocation {
        command: prepare_command(
            &["/NOLOGO", "/BREPRO", output_argument.as_str(), input_text],
            0,
        )?,
        input_name: prepare_relative_name(input)?,
        output_name: prepare_relative_name(output)?,
    })
}

pub fn prepared_archive_owned_capacity(prepared: &PreparedArchiveInvocation) -> usize {
    prepared_command_owned_capacity(&prepared.command)
        .saturating_add(
            prepared
                .input_name
                .0
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
        .saturating_add(
            prepared
                .output_name
                .0
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
}

#[cfg(test)]
pub(crate) fn test_prepared_archive_arguments(prepared: &PreparedArchiveInvocation) -> &[String] {
    &prepared.command.arguments
}

pub(super) fn recheck_named_regular(
    cwd: &Directory,
    name: &PreparedRelativeName,
    input: &RegularFile,
) -> Result<(), Error> {
    recheck_held_regular(input)?;
    let rebound = hold_regular_file_name_external_read_prepared(cwd, name)?;
    if rebound.identity != input.identity || rebound.digest != input.digest {
        return Err(Error::Changed);
    }
    Ok(())
}

fn child_absent_impl(directory: &Directory, name: &PreparedRelativeName) -> Result<bool, Error> {
    let byte_length = name.0.len().checked_mul(2).ok_or(Error::Invalid)?;
    let length = u16::try_from(byte_length).map_err(|_| Error::Invalid)?;
    let unicode = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: name.0.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(std::mem::size_of::<OBJECT_ATTRIBUTES>())
            .map_err(|_| Error::Changed)?,
        RootDirectory: directory.file.as_raw_handle().cast(),
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
            FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            &attributes,
            &mut io,
            std::ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            HELD_SHARE,
            FILE_OPEN,
            FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if status == STATUS_OBJECT_NAME_NOT_FOUND {
        return Ok(true);
    }
    if status < 0 || handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(Error::Changed);
    }
    if unsafe { CloseHandle(handle) } == 0 {
        return Err(Error::Changed);
    }
    Ok(false)
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
    let rebound = relative_file_prepared(
        &parent.file,
        name,
        DIRECTORY_READ_ACCESS,
        FILE_OPEN,
        FILE_DIRECTORY_FILE,
    )?;
    Ok(directory_information(&rebound)? == child.identity)
}

fn read_exact_offset(file: &File, mut bytes: &mut [u8], mut offset: u64) -> Result<(), Error> {
    while !bytes.is_empty() {
        let read = file.seek_read(bytes, offset).map_err(|_| Error::Changed)?;
        if read == 0 {
            return Err(Error::Invalid);
        }
        let (_, remaining) = bytes.split_at_mut(read);
        bytes = remaining;
        offset = offset
            .checked_add(u64::try_from(read).map_err(|_| Error::OutputLimit)?)
            .ok_or(Error::OutputLimit)?;
    }
    Ok(())
}

fn exact_archive_member(archive: &RegularFile, input: &RegularFile) -> Result<(), Error> {
    let archive_len = archive.identity.length;
    let input_len = input.identity.length;
    if archive_len < 68 || input_len == 0 {
        return Err(Error::Invalid);
    }
    let mut magic = [0_u8; 8];
    read_exact_offset(&archive.file, &mut magic, 0)?;
    if magic != *b"!<arch>\n" {
        return Err(Error::Invalid);
    }
    let mut offset = 8_u64;
    let mut input_members = 0_u8;
    let mut members = 0_u8;
    let mut empty_longnames = false;
    while offset < archive_len {
        let mut header = [0_u8; 60];
        read_exact_offset(&archive.file, &mut header, offset)?;
        if header[58..] != *b"`\n" {
            return Err(Error::Invalid);
        }
        let size = archive_member_size(&header[48..58])?;
        let data = offset.checked_add(60).ok_or(Error::OutputLimit)?;
        let end = data.checked_add(size).ok_or(Error::OutputLimit)?;
        if end > archive_len {
            return Err(Error::Invalid);
        }
        let header_kind = archive_member_kind(&header[..16], b"module.obj")?;
        exact_archive_member_metadata(&header, header_kind, 0)?;
        let (kind, member_data, member_size) = match header_kind {
            ArchiveMemberKind::Extended(length) => {
                let length = u64::try_from(length).map_err(|_| Error::OutputLimit)?;
                if length > size {
                    return Err(Error::Invalid);
                }
                let mut name = [0_u8; 255];
                let name_length = usize::try_from(length).map_err(|_| Error::OutputLimit)?;
                read_exact_offset(&archive.file, &mut name[..name_length], data)?;
                let name = archive_extended_name(&name[..name_length])?;
                let kind = archive_member_kind(name, b"module.obj")?;
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
        let ordered = matches!(
            (members, header_kind, kind),
            (
                0 | 1,
                ArchiveMemberKind::GnuLinkerIndex,
                ArchiveMemberKind::GnuLinkerIndex
            ) | (
                2,
                ArchiveMemberKind::LongNames,
                ArchiveMemberKind::LongNames
            ) | (2, ArchiveMemberKind::Input, ArchiveMemberKind::Input)
                | (3, ArchiveMemberKind::Input, ArchiveMemberKind::Input)
        );
        if !ordered
            || matches!(kind, ArchiveMemberKind::LongNames) && (size != 0 || members != 2)
            || members == 3 && !empty_longnames
        {
            return Err(Error::Invalid);
        }
        if matches!(kind, ArchiveMemberKind::LongNames) {
            empty_longnames = true;
        }
        match kind {
            ArchiveMemberKind::GnuLinkerIndex
            | ArchiveMemberKind::BsdSortedLinkerIndex
            | ArchiveMemberKind::LongNames => {}
            ArchiveMemberKind::Input => {
                input_members = input_members.checked_add(1).ok_or(Error::Invalid)?;
                if member_size != input_len {
                    return Err(Error::Invalid);
                }
                let mut compared = 0_u64;
                let mut archive_bytes = [0_u8; 8192];
                let mut input_bytes = [0_u8; 8192];
                while compared < member_size {
                    let count = usize::try_from((member_size - compared).min(8192))
                        .map_err(|_| Error::OutputLimit)?;
                    read_exact_offset(
                        &archive.file,
                        &mut archive_bytes[..count],
                        member_data + compared,
                    )?;
                    read_exact_offset(&input.file, &mut input_bytes[..count], compared)?;
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
            read_exact_offset(&archive.file, &mut padding, end)?;
            if padding != *b"\n" {
                return Err(Error::Invalid);
            }
        }
        offset = end.checked_add(size & 1).ok_or(Error::OutputLimit)?;
        members = members.checked_add(1).ok_or(Error::Invalid)?;
    }
    let expected_members = if empty_longnames { 4 } else { 3 };
    if offset != archive_len || input_members != 1 || members != expected_members {
        return Err(Error::Invalid);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn test_exact_archive_member(
    archive: &RegularFile,
    input: &RegularFile,
) -> Result<(), Error> {
    exact_archive_member(archive, input)
}

pub fn archive_prepared(
    archiver: &Executable,
    cwd: &Directory,
    input: &RegularFile,
    prepared: PreparedArchiveInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<RegularFile, Error> {
    if !child_absent_impl(cwd, &prepared.output_name)? {
        return Err(Error::Exists);
    }
    recheck_named_regular(cwd, &prepared.input_name, input)?;
    recheck_held_regular(&archiver.file)?;
    recheck_directory(cwd)?;
    let process = run_argv(
        archiver,
        cwd,
        &prepared.command.arguments,
        0,
        Some(prepared.command.command_line),
        Some(prepared.command.output),
        process_arena,
    );
    let archiver_recheck = recheck_held_regular(&archiver.file);
    let cwd_recheck = recheck_directory(cwd);
    let input_recheck = recheck_named_regular(cwd, &prepared.input_name, input);
    let output = process?;
    archiver_recheck?;
    cwd_recheck?;
    input_recheck?;
    if !output.is_empty() {
        return Err(Error::OutputLimit);
    }
    let archive = hold_regular_file_name_external_read_bounded_prepared(
        cwd,
        &prepared.output_name,
        SDK_ARCHIVE_MAX_BYTES,
    )?;
    exact_archive_member(&archive, input)?;
    recheck_held_regular(&archiver.file)?;
    recheck_directory(cwd)?;
    recheck_named_regular(cwd, &prepared.input_name, input)?;
    recheck_named_regular(cwd, &prepared.output_name, &archive)?;
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
    if sanitizers || !matches!(optimization, 0..=2) {
        return Err(Error::Invalid);
    }
    let arguments = vec![
        "-std=c11".to_owned(),
        "-target".to_owned(),
        target.to_owned(),
        "-Wall".to_owned(),
        "-Wextra".to_owned(),
        "-Werror".to_owned(),
        match optimization {
            0 => "-O0",
            1 => "-O1",
            _ => "-O2",
        }
        .to_owned(),
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
pub(crate) fn execute_harness_with_arguments(
    executable: &Executable,
    cwd: &Directory,
    arguments: &[String; 3],
) -> Result<(), Error> {
    let command_line = windows_command_line(arguments)?;
    let mut process_arena = prepare_process_arena(1)?;
    if run_argv(
        executable,
        cwd,
        arguments,
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
