//! Exact archive admission, archiver settlement, and the legacy harness entry
//! points that run held tools.

use super::*;

pub(crate) fn recheck_named_regular(
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
pub(crate) enum TestDarwinArchiveFailurePoint {
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
pub(crate) fn test_inject_archive_post_authentication_failure(enabled: bool) {
    TEST_ARCHIVE_POST_AUTHENTICATION_FAILURE.with(|slot| slot.set(enabled));
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn test_reset_archive_later_actions() {
    TEST_ARCHIVE_LATER_ACTIONS.with(|slot| slot.set(0));
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn test_archive_later_actions() -> usize {
    TEST_ARCHIVE_LATER_ACTIONS.with(std::cell::Cell::get)
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn test_inject_archive_scratch_open_failure(enabled: bool) {
    TEST_ARCHIVE_SCRATCH_OPEN_FAILURE.with(|slot| slot.set(enabled));
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn test_inject_darwin_archive_failure(point: Option<TestDarwinArchiveFailurePoint>) {
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
pub(crate) fn archive_scratch_open_failure_injected() -> bool {
    TEST_ARCHIVE_SCRATCH_OPEN_FAILURE.with(std::cell::Cell::get)
}

#[cfg(all(not(test), target_os = "macos"))]
pub(crate) fn archive_scratch_open_failure_injected() -> bool {
    false
}

#[cfg(all(test, target_os = "macos"))]
fn darwin_archive_failure_injected(point: TestDarwinArchiveFailurePoint) -> bool {
    TEST_ARCHIVE_FAILURE_POINT.with(|slot| slot.get() == Some(point))
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn test_archive_seed_round_trip(
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
pub(crate) fn test_regular_file_facts(file: &RegularFile) -> (u32, u64, u64) {
    (file.mode, file.dev, file.ino)
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) fn test_exact_archive_member(
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
