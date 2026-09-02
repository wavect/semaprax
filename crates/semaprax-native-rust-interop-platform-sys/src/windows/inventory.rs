//! Bounded directory inventory scanning plus the prepared publish and discard
//! settlements and their deletion dispositions.

use super::*;

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

pub(super) fn observe_publish_rebound(
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
    if observe_publish_rebound(parent, stage_name, fail_information, fail_close)? != stage.identity
    {
        return Err(Error::Changed);
    }
    #[cfg(debug_assertions)]
    if prepared.fail_rename {
        return Err(Error::Changed);
    }
    let byte_length = stage_name
        .units
        .len()
        .checked_mul(2)
        .ok_or(Error::Invalid)?;
    let length = u16::try_from(byte_length).map_err(|_| Error::Invalid)?;
    let unicode = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: stage_name.units.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(std::mem::size_of::<OBJECT_ATTRIBUTES>())
            .map_err(|_| Error::Changed)?,
        RootDirectory: parent.file.as_raw_handle().cast(),
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut rename_handle: HANDLE = std::ptr::null_mut();
    {
        let mut open_io = IO_STATUS_BLOCK::default();
        let status = unsafe {
            NtCreateFile(
                &mut rename_handle,
                DELETE,
                &attributes,
                &mut open_io,
                std::ptr::null(),
                FILE_ATTRIBUTE_NORMAL,
                HELD_SHARE,
                FILE_OPEN,
                FILE_DIRECTORY_FILE,
                std::ptr::null(),
                0,
            )
        };
        if status < 0 || rename_handle.is_null() || rename_handle == INVALID_HANDLE_VALUE {
            return Err(Error::Changed);
        }
    }
    unsafe {
        (*information).root_directory = parent.file.as_raw_handle().cast();
    }
    let mut io = IO_STATUS_BLOCK::default();
    #[cfg(test)]
    let mut attempted_statuses = [0_i32; 11];
    const RENAME_BACKOFF_MILLIS: [u64; 10] = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512];
    let outcome = (|| {
        #[allow(unused_variables)]
        for (attempt, backoff_millis) in RENAME_BACKOFF_MILLIS.into_iter().enumerate() {
            unsafe {
                (*information).flags =
                    windows_sys::Win32::System::WindowsProgramming::FILE_RENAME_FLAG_POSIX_SEMANTICS;
            }
            let mut extended = || unsafe {
                NtSetInformationFile(
                    rename_handle,
                    &mut io,
                    information.cast(),
                    total,
                    FileRenameInformationEx,
                )
            };
            #[cfg(test)]
            let status = if prepared.force_extended_rejection {
                prepared.observed_extended_flags = Some(unsafe { (*information).flags });
                // STATUS_INVALID_INFO_CLASS: deterministically select the
                // real native legacy call without changing process-global state.
                0xc000_0003_u32 as i32
            } else {
                extended()
            };
            #[cfg(not(test))]
            let status = extended();
            #[cfg(test)]
            {
                attempted_statuses[attempt] = status;
            }
            if status == STATUS_OBJECT_NAME_COLLISION {
                return Err(Error::Exists);
            }
            if status >= 0 {
                return Ok(());
            }
            if !matches!(
                status,
                STATUS_SHARING_VIOLATION | STATUS_ACCESS_DENIED | STATUS_DELETE_PENDING
            ) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(backoff_millis));
        }
        let status = unsafe {
            // The legacy class interprets this union as BOOLEAN
            // ReplaceIfExists, not ULONG Flags. POSIX_SEMANTICS is nonzero:
            // clear the entire field before reusing the buffer for legacy.
            (*information).flags = 0;
            #[cfg(test)]
            {
                prepared.observed_legacy_flags =
                    Some(((*information).flags, information.cast::<u8>().read()));
            }
            NtSetInformationFile(
                rename_handle.cast(),
                &mut io,
                information.cast(),
                total,
                FileRenameInformation,
            )
        };
        #[cfg(test)]
        {
            attempted_statuses[10] = status;
        }
        if status == STATUS_OBJECT_NAME_COLLISION {
            return Err(Error::Exists);
        }
        if status >= 0 {
            return Ok(());
        }
        Err(Error::Changed)
    })();
    if unsafe { CloseHandle(rename_handle) } == 0 {
        std::process::abort();
    }
    #[cfg(test)]
    test_remember_publish_statuses(&attempted_statuses);
    outcome
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
            let name_bytes = usize::try_from(entry.FileNameLength).map_err(|_| Error::Changed)?;
            if !name_bytes.is_multiple_of(2) {
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
    let mut deletion_handles: [Option<RegularFile>; N] = std::array::from_fn(|_| None);
    for index in 0..attached {
        let (identity, digest) = if let Some(file) = files[index] {
            recheck_held_regular(file)?;
            (file.identity, file.digest)
        } else {
            let file = settled[index].expect("attached settled prefix");
            (file.identity, file.digest)
        };
        let name = names.names[index].as_ref().expect("validated");
        let rebound = hold_regular_file_name_prepared(stage, name)?;
        if rebound.identity != identity || rebound.digest != digest {
            return Err(Error::Changed);
        }
        deletion_handles[index] = Some(rebound);
    }
    let mut disposition_error = None;
    for (deleted, file) in deletion_handles[..attached].iter().flatten().enumerate() {
        #[cfg(not(debug_assertions))]
        let _ = deleted;
        #[cfg(debug_assertions)]
        if failure_after_delete == Some(deleted) {
            disposition_error = Some(Error::Changed);
            break;
        }
        if let Err(error) = disposition_delete(&file.file) {
            disposition_error = Some(error);
            break;
        }
    }
    #[cfg(debug_assertions)]
    if disposition_error.is_none() && failure_after_delete == Some(attached) {
        disposition_error = Some(Error::Changed);
    }
    must_close_deletion_handles(&mut deletion_handles[..attached]);
    if let Some(error) = disposition_error {
        return Err(error);
    }
    let stage_deletion = relative_file_arena(
        &parent.file,
        stage_name,
        DIRECTORY_OWNED_ACCESS,
        FILE_OPEN,
        FILE_DIRECTORY_FILE,
    )?;
    let observed = directory_information(&stage_deletion);
    if observed != Ok(stage.identity) {
        must_close_file(stage_deletion);
        return Err(Error::Changed);
    }
    disposition_delete_and_close(stage_deletion)
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

fn disposition_delete_and_close(file: File) -> Result<(), Error> {
    let result = disposition_delete(&file);
    must_close_file(file);
    result
}

fn must_close_file(file: File) {
    let handle = file.into_raw_handle();
    if unsafe { CloseHandle(handle.cast()) } == 0 {
        std::process::abort();
    }
}

fn must_close_deletion_handles(files: &mut [Option<RegularFile>]) {
    let mut close_failed = false;
    for file in files {
        let RegularFile { file, .. } = file.take().expect("authenticated deletion handle");
        let handle = file.into_raw_handle();
        close_failed |= unsafe { CloseHandle(handle.cast()) } == 0;
    }
    if close_failed {
        std::process::abort();
    }
}
