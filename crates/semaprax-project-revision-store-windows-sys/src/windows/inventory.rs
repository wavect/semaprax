fn require_ntfs_local(file: &StdFile) -> Result<(), Error> {
    let mut fs_name = [0u16; 16];
    let mut flags = 0u32;
    if unsafe {
        GetVolumeInformationByHandleW(
            file.as_raw_handle().cast(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut flags,
            fs_name.as_mut_ptr(),
            fs_name.len() as u32,
        )
    } == 0
    {
        return Err(Error::Changed);
    }
    let end = fs_name
        .iter()
        .position(|u| *u == 0)
        .unwrap_or(fs_name.len());
    if OsString::from_wide(&fs_name[..end]).to_string_lossy() != "NTFS" {
        return Err(Error::Invalid);
    }
    let required = FILE_PERSISTENT_ACLS
        | FILE_UNICODE_ON_DISK
        | FILE_CASE_PRESERVED_NAMES
        | FILE_CASE_SENSITIVE_SEARCH
        | FILE_NAMED_STREAMS;
    if flags & required != required {
        return Err(Error::Invalid);
    }
    Ok(())
}

fn require_single_data_stream(file: &StdFile, kind: Kind) -> Result<(), Error> {
    let mut storage = vec![0u64; MAX_STREAM_QUERY_BYTES / std::mem::size_of::<u64>()];
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(storage.as_mut_ptr().cast::<u8>(), MAX_STREAM_QUERY_BYTES)
    };
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle().cast(),
            FileStreamInfo,
            bytes.as_mut_ptr().cast(),
            bytes.len() as u32,
        )
    } == 0
    {
        if kind == Kind::Directory && unsafe { GetLastError() } == ERROR_HANDLE_EOF {
            return Ok(());
        }
        if unsafe { GetLastError() } == ERROR_MORE_DATA {
            return Err(Error::Changed);
        }
        return Err(Error::Changed);
    }
    if kind == Kind::Directory {
        return Err(Error::Changed);
    }
    let mut offset = 0usize;
    let mut count = 0usize;
    loop {
        let header = std::mem::offset_of!(FILE_STREAM_INFO, StreamName);
        if offset
            .checked_add(std::mem::size_of::<FILE_STREAM_INFO>())
            .is_none_or(|end| end > bytes.len())
        {
            return Err(Error::Changed);
        }
        let info = unsafe { &*(bytes.as_ptr().add(offset).cast::<FILE_STREAM_INFO>()) };
        if info.StreamNameLength % 2 != 0 {
            return Err(Error::Changed);
        }
        let units = info.StreamNameLength as usize / 2;
        let record_end = offset
            .checked_add(header)
            .and_then(|end| end.checked_add(info.StreamNameLength as usize))
            .ok_or(Error::Changed)?;
        if record_end > bytes.len() {
            return Err(Error::Changed);
        }
        let name = unsafe { std::slice::from_raw_parts(info.StreamName.as_ptr(), units) };
        if OsString::from_wide(name) != OsStr::new("::$DATA") {
            return Err(Error::Changed);
        }
        count += 1;
        if info.NextEntryOffset == 0 {
            break;
        }
        let next = info.NextEntryOffset as usize;
        if next < header + info.StreamNameLength as usize || !next.is_multiple_of(8) {
            return Err(Error::Changed);
        }
        offset = offset.checked_add(next).ok_or(Error::Changed)?;
    }
    if count != 1 {
        return Err(Error::Changed);
    }
    Ok(())
}

fn enumerate(directory: &StdFile, sid: &[u8]) -> Result<Vec<InventoryEntry>, Error> {
    let mut storage = vec![0u64; MAX_DIRECTORY_QUERY_BYTES / std::mem::size_of::<u64>()];
    let buffer = unsafe {
        std::slice::from_raw_parts_mut(storage.as_mut_ptr().cast::<u8>(), MAX_DIRECTORY_QUERY_BYTES)
    };
    let mut entries = Vec::new();
    let mut restart = true;
    let mut pages = 0usize;
    let mut records = 0usize;
    loop {
        pages = pages.checked_add(1).ok_or(Error::Limit)?;
        if pages > 294 {
            return Err(Error::Limit);
        }
        let class = if restart {
            FileIdBothDirectoryRestartInfo
        } else {
            FileIdBothDirectoryInfo
        };
        restart = false;
        if unsafe {
            GetFileInformationByHandleEx(
                directory.as_raw_handle().cast(),
                class,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
            )
        } == 0
        {
            if unsafe { GetLastError() } == ERROR_NO_MORE_FILES {
                break;
            }
            return Err(Error::Changed);
        }
        let mut offset = 0usize;
        loop {
            records = records.checked_add(1).ok_or(Error::Limit)?;
            if records > 293 {
                return Err(Error::Limit);
            }
            let header = std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
            if offset
                .checked_add(std::mem::size_of::<FILE_ID_BOTH_DIR_INFO>())
                .is_none_or(|end| end > buffer.len())
            {
                return Err(Error::Changed);
            }
            let info = unsafe { &*(buffer.as_ptr().add(offset).cast::<FILE_ID_BOTH_DIR_INFO>()) };
            if info.ShortNameLength != 0 || info.FileNameLength % 2 != 0 {
                return Err(Error::Changed);
            }
            let record_end = offset
                .checked_add(header)
                .and_then(|end| end.checked_add(info.FileNameLength as usize))
                .ok_or(Error::Changed)?;
            if record_end > buffer.len() {
                return Err(Error::Changed);
            }
            let units = info.FileNameLength as usize / 2;
            let name = unsafe { std::slice::from_raw_parts(info.FileName.as_ptr(), units) };
            let name = OsString::from_wide(name)
                .into_string()
                .map_err(|_| Error::Invalid)?;
            if name != "." && name != ".." {
                if entries.len() >= 291 {
                    return Err(Error::Limit);
                }
                validate_name(&name)?;
                let kind = if info.FileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
                    Kind::Directory
                } else {
                    Kind::File
                };
                let file = relative_file(
                    directory,
                    &name,
                    if kind == Kind::Directory {
                        DIRECTORY_ACCESS
                    } else {
                        FILE_GENERIC_READ | SYNCHRONIZE
                    },
                    FILE_OPEN,
                    if kind == Kind::Directory {
                        FILE_DIRECTORY_FILE
                    } else {
                        FILE_NON_DIRECTORY_FILE
                    },
                    None,
                )?;
                let fact = authenticate_handle(&file, kind, sid)?;
                close_file(file)?;
                entries.push(InventoryEntry { name, kind, fact });
            }
            if info.NextEntryOffset == 0 {
                break;
            }
            let next = info.NextEntryOffset as usize;
            if next < header + info.FileNameLength as usize || !next.is_multiple_of(8) {
                return Err(Error::Changed);
            }
            offset = offset.checked_add(next).ok_or(Error::Changed)?;
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    if entries
        .windows(2)
        .any(|w| w[0].name.eq_ignore_ascii_case(&w[1].name))
    {
        return Err(Error::Changed);
    }
    Ok(entries)
}

fn require_directory_name_without_short_alias(parent: &StdFile, name: &str) -> Result<(), Error> {
    validate_name(name)?;
    find_directory_name(parent, name)
}

fn find_directory_name(directory: &StdFile, target: &str) -> Result<(), Error> {
    let mut storage = vec![0u64; MAX_DIRECTORY_QUERY_BYTES / std::mem::size_of::<u64>()];
    let buffer = unsafe {
        std::slice::from_raw_parts_mut(storage.as_mut_ptr().cast::<u8>(), MAX_DIRECTORY_QUERY_BYTES)
    };
    let mut restart = true;
    let mut pages = 0usize;
    let mut records = 0usize;
    loop {
        pages = pages.checked_add(1).ok_or(Error::Limit)?;
        if pages > 64 {
            return Err(Error::Limit);
        }
        let class = if restart {
            FileIdBothDirectoryRestartInfo
        } else {
            FileIdBothDirectoryInfo
        };
        restart = false;
        if unsafe {
            GetFileInformationByHandleEx(
                directory.as_raw_handle().cast(),
                class,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
            )
        } == 0
        {
            return Err(Error::Changed);
        }
        let mut offset = 0usize;
        loop {
            records = records.checked_add(1).ok_or(Error::Limit)?;
            if records > 4096 {
                return Err(Error::Limit);
            }
            let header = std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
            if offset
                .checked_add(std::mem::size_of::<FILE_ID_BOTH_DIR_INFO>())
                .is_none_or(|end| end > buffer.len())
            {
                return Err(Error::Changed);
            }
            let info = unsafe { &*(buffer.as_ptr().add(offset).cast::<FILE_ID_BOTH_DIR_INFO>()) };
            if info.FileNameLength % 2 != 0 {
                return Err(Error::Changed);
            }
            let record_end = offset
                .checked_add(header)
                .and_then(|end| end.checked_add(info.FileNameLength as usize))
                .ok_or(Error::Changed)?;
            if record_end > buffer.len() {
                return Err(Error::Changed);
            }
            let units = info.FileNameLength as usize / 2;
            let name = OsString::from_wide(unsafe {
                std::slice::from_raw_parts(info.FileName.as_ptr(), units)
            })
            .into_string()
            .map_err(|_| Error::Invalid)?;
            if name.eq_ignore_ascii_case(target) {
                return if name == target && info.ShortNameLength == 0 {
                    Ok(())
                } else {
                    Err(Error::Changed)
                };
            }
            if info.NextEntryOffset == 0 {
                break;
            }
            let next = info.NextEntryOffset as usize;
            if next < header + info.FileNameLength as usize || !next.is_multiple_of(8) {
                return Err(Error::Changed);
            }
            offset = offset.checked_add(next).ok_or(Error::Changed)?;
        }
    }
}
