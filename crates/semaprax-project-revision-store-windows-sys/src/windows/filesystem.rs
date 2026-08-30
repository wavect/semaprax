fn validate_name(name: &str) -> Result<(), Error> {
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.contains(['/', '\\', ':', '\0', '<', '>', '"', '|', '?', '*'])
        || name.chars().any(char::is_control)
        || name.ends_with([' ', '.'])
    {
        return Err(Error::Invalid);
    }
    let stem = name
        .split('.')
        .next()
        .ok_or(Error::Invalid)?
        .to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "CLOCK$"
            | "COM¹"
            | "COM²"
            | "COM³"
            | "LPT¹"
            | "LPT²"
            | "LPT³"
    ) || stem
        .strip_prefix("COM")
        .is_some_and(|s| matches!(s, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        || stem
            .strip_prefix("LPT")
            .is_some_and(|s| matches!(s, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
    {
        return Err(Error::Invalid);
    }
    Ok(())
}

fn open_absolute_components(path: &Path) -> Result<StdFile, Error> {
    let spelling = path.to_str().ok_or(Error::Invalid)?;
    if spelling.contains('/')
        || (spelling.len() > 3 && spelling.ends_with('\\'))
        || spelling.split('\\').any(|part| matches!(part, "." | ".."))
        || (spelling.len() > 3 && spelling.split('\\').any(str::is_empty))
        || spelling.encode_utf16().count() > 32760
    {
        return Err(Error::Invalid);
    }
    let mut components = path.components();
    let drive = match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            std::path::Prefix::Disk(letter) => letter,
            _ => return Err(Error::Invalid),
        },
        _ => return Err(Error::Invalid),
    };
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(Error::Invalid);
    }
    let mut names = Vec::new();
    for component in components {
        let Component::Normal(component) = component else {
            return Err(Error::Invalid);
        };
        let name = component.to_str().ok_or(Error::Invalid)?;
        validate_name(name)?;
        names.push(name.to_owned());
        if names.len() > 128 {
            return Err(Error::Invalid);
        }
    }
    if names.is_empty() {
        return Err(Error::Invalid);
    }
    let anchor = [u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0];
    if unsafe { GetDriveTypeW(anchor.as_ptr()) } != DRIVE_FIXED {
        return Err(Error::Invalid);
    }
    let handle = unsafe {
        CreateFileW(
            anchor.as_ptr(),
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
    let mut current = unsafe { StdFile::from_raw_handle(handle.cast()) };
    authenticate_shape_only(&current, Kind::Directory)?;
    let count = names.len();
    for (index, name) in names.into_iter().enumerate() {
        require_directory_name_without_short_alias(&current, &name)?;
        let next = relative_file(
            &current,
            &name,
            if index + 1 == count {
                DIRECTORY_ACCESS
            } else {
                DIRECTORY_READ_ACCESS
            },
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
            None,
        )?;
        authenticate_shape_only(&next, Kind::Directory)?;
        close_file(current)?;
        current = next;
    }
    Ok(current)
}

fn relative_file(
    parent: &StdFile,
    name: &str,
    access: u32,
    disposition: u32,
    options: u32,
    security: Option<&SecurityDescriptor>,
) -> Result<StdFile, Error> {
    validate_name(name)?;
    let units: Vec<u16> = OsStr::new(name).encode_wide().collect();
    let bytes = units.len().checked_mul(2).ok_or(Error::Invalid)?;
    let unicode = UNICODE_STRING {
        Length: u16::try_from(bytes).map_err(|_| Error::Invalid)?,
        MaximumLength: u16::try_from(bytes).map_err(|_| Error::Invalid)?,
        Buffer: units.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(std::mem::size_of::<OBJECT_ATTRIBUTES>())
            .map_err(|_| Error::Invalid)?,
        RootDirectory: parent.as_raw_handle().cast(),
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: security.map_or(std::ptr::null(), |descriptor| {
            descriptor
                .as_ptr()
                .cast::<windows_sys::Win32::Security::SECURITY_DESCRIPTOR>()
                .cast_const()
        }),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut io = IO_STATUS_BLOCK::default();
    let mut handle: HANDLE = std::ptr::null_mut();
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
            options | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
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
    Ok(unsafe { StdFile::from_raw_handle(handle.cast()) })
}

fn open_child_directory(
    parent: &StdFile,
    name: &str,
    sid: &[u8],
    authority: Fact,
) -> Result<Directory, Error> {
    require_directory_name_without_short_alias(parent, name)?;
    let file = relative_file(
        parent,
        name,
        DIRECTORY_ACCESS,
        FILE_OPEN,
        FILE_DIRECTORY_FILE,
        None,
    )?;
    let fact = authenticate_handle(&file, Kind::Directory, sid)?;
    Ok(Directory {
        held: Held {
            file: CheckedFile::new(file),
            fact,
        },
        authority,
    })
}

fn create_child_directory(
    parent: &StdFile,
    name: &str,
    sd: &SecurityDescriptor,
    sid: &[u8],
    authority: Fact,
) -> Result<Directory, Error> {
    let file = relative_file(
        parent,
        name,
        DIRECTORY_ACCESS,
        FILE_CREATE,
        FILE_DIRECTORY_FILE,
        Some(sd),
    )?;
    let fact = authenticate_handle(&file, Kind::Directory, sid)?;
    require_directory_name_without_short_alias(parent, name)?;
    Ok(Directory {
        held: Held {
            file: CheckedFile::new(file),
            fact,
        },
        authority,
    })
}

fn fact(file: &StdFile) -> Result<Fact, Error> {
    let mut basic = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut basic) } == 0 {
        return Err(Error::Changed);
    }
    let mut id = FILE_ID_INFO::default();
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle().cast(),
            FileIdInfo,
            (&mut id as *mut FILE_ID_INFO).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(Error::Changed);
    }
    Ok(Fact {
        volume: id.VolumeSerialNumber,
        file_id: id.FileId.Identifier,
        attributes: basic.dwFileAttributes,
        links: basic.nNumberOfLinks,
        length: (u64::from(basic.nFileSizeHigh) << 32) | u64::from(basic.nFileSizeLow),
    })
}

fn authenticate_shape_only(file: &StdFile, kind: Kind) -> Result<Fact, Error> {
    let fact = fact(file)?;
    let mut standard = FILE_STANDARD_INFO::default();
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle().cast(),
            FileStandardInfo,
            (&mut standard as *mut FILE_STANDARD_INFO).cast(),
            std::mem::size_of::<FILE_STANDARD_INFO>() as u32,
        )
    } == 0
        || standard.DeletePending
    {
        return Err(Error::Changed);
    }
    let directory = fact.attributes & FILE_ATTRIBUTE_DIRECTORY != 0;
    if fact.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || directory != (kind == Kind::Directory)
        || (kind == Kind::File && fact.links != 1)
    {
        return Err(Error::Changed);
    }
    require_single_data_stream(file, kind)?;
    Ok(fact)
}

fn authenticate_handle(file: &StdFile, kind: Kind, sid: &[u8]) -> Result<Fact, Error> {
    let fact = authenticate_shape_only(file, kind)?;
    let _ = security_descriptor(file, sid)?;
    Ok(fact)
}

fn require_fact(file: &StdFile, expected: Fact, kind: Kind, sid: &[u8]) -> Result<(), Error> {
    if authenticate_handle(file, kind, sid)? != expected {
        return Err(Error::Changed);
    }
    Ok(())
}

fn rename_no_replace(stage: &StdFile, root: &StdFile, destination: &str) -> Result<(), Error> {
    let units: Vec<u16> = destination.encode_utf16().collect();
    let header = std::mem::offset_of!(RenameInformation, file_name);
    let total = header
        .checked_add(units.len().checked_mul(2).ok_or(Error::Invalid)?)
        .ok_or(Error::Invalid)?;
    let mut storage = vec![0u64; total.div_ceil(8)];
    let information = storage.as_mut_ptr().cast::<RenameInformation>();
    unsafe {
        (*information).flags = 0;
        (*information).root_directory = root.as_raw_handle().cast();
        (*information).file_name_length = (units.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(
            units.as_ptr(),
            (*information).file_name.as_mut_ptr(),
            units.len(),
        );
    }
    let mut io = IO_STATUS_BLOCK::default();
    let status = unsafe {
        NtSetInformationFile(
            stage.as_raw_handle().cast(),
            &mut io,
            information.cast(),
            total as u32,
            FileRenameInformationEx,
        )
    };
    if status == STATUS_OBJECT_NAME_COLLISION {
        return Err(Error::Exists);
    }
    if status < 0 {
        return Err(Error::Uncertain);
    }
    Ok(())
}

#[repr(C)]
struct RenameInformation {
    flags: u32,
    root_directory: HANDLE,
    file_name_length: u32,
    file_name: [u16; 1],
}

fn flush(file: &StdFile) -> Result<(), Error> {
    if unsafe { FlushFileBuffers(file.as_raw_handle().cast()) } == 0 {
        return Err(Error::Io);
    }
    Ok(())
}

fn close_file(file: StdFile) -> Result<(), Error> {
    let handle = file.into_raw_handle();
    if unsafe { CloseHandle(handle.cast()) } == 0 {
        Err(Error::Io)
    } else {
        Ok(())
    }
}
