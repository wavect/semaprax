//! Raw bounded directory scanning plus the prepared publish and discard
//! settlements that depend on it.

use super::*;

#[cfg(target_os = "linux")]
fn parse_linux_inventory_records(
    bytes: &[u8],
    mut admit: impl FnMut(&[u8], u64) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut offset = 0usize;
    while offset < bytes.len() {
        let header_end = offset.checked_add(19).ok_or(Error::Changed)?;
        if header_end > bytes.len() {
            return Err(Error::Changed);
        }
        let inode = unsafe { std::ptr::read_unaligned(bytes.as_ptr().add(offset).cast::<u64>()) };
        let record = usize::from(unsafe {
            std::ptr::read_unaligned(bytes.as_ptr().add(offset + 16).cast::<u16>())
        });
        let next = offset.checked_add(record).ok_or(Error::Changed)?;
        if record < 20
            || record % std::mem::align_of::<u64>() != 0
            || next <= offset
            || next > bytes.len()
        {
            return Err(Error::Changed);
        }
        let name = &bytes[header_end..next];
        let nul = name
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(Error::Changed)?;
        if nul == 0 || inode == 0 {
            return Err(Error::Changed);
        }
        admit(&name[..nul], inode)?;
        offset = next;
    }
    if offset != bytes.len() {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn test_parse_inventory_records(
    bytes: &[u8],
    expected: &[(&[u8], u64)],
) -> Result<(), Error> {
    if expected.len() > 16 {
        return Err(Error::Invalid);
    }
    let mut seen = [false; 16];
    parse_linux_inventory_records(bytes, |name, inode| {
        let Some(index) = expected.iter().position(|(expected_name, expected_inode)| {
            *expected_name == name && *expected_inode == inode
        }) else {
            return Err(Error::Changed);
        };
        if seen[index] {
            return Err(Error::Changed);
        }
        seen[index] = true;
        Ok(())
    })?;
    if seen[..expected.len()].iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn scan_prepared_directory<const N: usize>(
    prepared: &mut PreparedInventoryExact<N>,
    directory: &Directory,
    files: &[Option<&RegularFile>; N],
) -> Result<(), Error> {
    #[cfg(test)]
    {
        prepared.scan_entries = prepared.scan_entries.saturating_add(1);
    }
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut raw_records = 0usize;
    let mut queries = 0usize;
    let mut saw_dot = false;
    let mut saw_dot_dot = false;
    let maximum_records = N.checked_add(2).ok_or(Error::OutputLimit)?;
    let maximum_queries = N.checked_add(3).ok_or(Error::OutputLimit)?;
    let capacity = prepared
        .storage
        .len()
        .checked_mul(std::mem::size_of::<u64>())
        .ok_or(Error::OutputLimit)?;
    let bytes_limit = libc::c_uint::try_from(capacity).map_err(|_| Error::OutputLimit)?;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > maximum_queries {
            return Err(Error::Changed);
        }
        prepared.storage.fill(u64::MAX);
        let read = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                directory.file.as_raw_fd(),
                prepared.storage.as_mut_ptr().cast::<u8>(),
                bytes_limit,
            )
        };
        if read < 0 {
            return Err(Error::Changed);
        }
        let used = usize::try_from(libc::c_uint::try_from(read).map_err(|_| Error::Changed)?)
            .map_err(|_| Error::Changed)?;
        if used == 0 {
            break;
        }
        if used > capacity {
            return Err(Error::Changed);
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(prepared.storage.as_ptr().cast::<u8>(), used) };
        parse_linux_inventory_records(bytes, |name, inode| {
            raw_records = raw_records.checked_add(1).ok_or(Error::OutputLimit)?;
            if raw_records > maximum_records {
                return Err(Error::Changed);
            }
            if name == b"." {
                if saw_dot {
                    return Err(Error::Changed);
                }
                saw_dot = true;
            } else if name == b".." {
                if saw_dot_dot {
                    return Err(Error::Changed);
                }
                saw_dot_dot = true;
            }
            admit_inventory_entry(prepared, files, &mut seen, &mut count, name, inode)
        })?;
    }
    if count != N || seen.iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn parse_darwin_inventory_records(
    bytes: &[u8],
    mut admit: impl FnMut(&[u8], u64) -> Result<(), Error>,
) -> Result<(), Error> {
    let header = std::mem::offset_of!(libc::dirent, d_name);
    let mut offset = 0usize;
    while offset < bytes.len() {
        let header_end = offset.checked_add(header).ok_or(Error::Changed)?;
        if header_end > bytes.len() {
            return Err(Error::Changed);
        }
        let entry = unsafe { bytes.as_ptr().add(offset).cast::<libc::dirent>() };
        let inode = unsafe { std::ptr::addr_of!((*entry).d_ino).read_unaligned() };
        let record = usize::from(unsafe { std::ptr::addr_of!((*entry).d_reclen).read_unaligned() });
        let name_length =
            usize::from(unsafe { std::ptr::addr_of!((*entry).d_namlen).read_unaligned() });
        let name_end = header_end.checked_add(name_length).ok_or(Error::Changed)?;
        let next = offset.checked_add(record).ok_or(Error::Changed)?;
        if record < header + 1
            || !record.is_multiple_of(4)
            || name_length > 1023
            || name_end >= next
            || next <= offset
            || next > bytes.len()
        {
            return Err(Error::Changed);
        }
        let name = &bytes[header_end..name_end];
        if bytes[name_end] != 0 || name.contains(&0) {
            return Err(Error::Changed);
        }
        if inode == 0 {
            offset = next;
            continue;
        }
        if name.is_empty() {
            return Err(Error::Changed);
        }
        admit(name, inode)?;
        offset = next;
    }
    if offset != bytes.len() {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn test_parse_inventory_records(
    bytes: &[u8],
    expected: &[(&[u8], u64)],
) -> Result<(), Error> {
    if expected.len() > 16 {
        return Err(Error::Invalid);
    }
    let mut seen = [false; 16];
    parse_darwin_inventory_records(bytes, |name, inode| {
        let Some(index) = expected.iter().position(|(expected_name, expected_inode)| {
            *expected_name == name && *expected_inode == inode
        }) else {
            return Err(Error::Changed);
        };
        if seen[index] {
            return Err(Error::Changed);
        }
        seen[index] = true;
        Ok(())
    })?;
    if seen[..expected.len()].iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn scan_prepared_directory<const N: usize>(
    prepared: &mut PreparedInventoryExact<N>,
    directory: &Directory,
    files: &[Option<&RegularFile>; N],
) -> Result<(), Error> {
    #[cfg(test)]
    {
        prepared.scan_entries = prepared.scan_entries.saturating_add(1);
    }
    const SYS_GETDIRENTRIES64: libc::c_int = 344;
    const _: () = assert!(std::mem::size_of::<libc::off_t>() == 8);
    const _: () = assert!(std::mem::offset_of!(libc::dirent, d_ino) == 0);
    const _: () = assert!(std::mem::offset_of!(libc::dirent, d_reclen) == 16);
    const _: () = assert!(std::mem::offset_of!(libc::dirent, d_namlen) == 18);
    const _: () = assert!(std::mem::offset_of!(libc::dirent, d_name) == 21);
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut raw_records = 0usize;
    let mut queries = 0usize;
    let mut saw_dot = false;
    let mut saw_dot_dot = false;
    let maximum_records = N.checked_add(2).ok_or(Error::OutputLimit)?;
    let maximum_queries = N.checked_add(3).ok_or(Error::OutputLimit)?;
    let capacity = prepared
        .storage
        .len()
        .checked_mul(std::mem::size_of::<u64>())
        .ok_or(Error::OutputLimit)?;
    let bytes_limit: libc::size_t = capacity;
    let mut base: libc::off_t = 0;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > maximum_queries {
            return Err(Error::Changed);
        }
        prepared.storage.fill(u64::MAX);
        let read = unsafe {
            libc::syscall(
                SYS_GETDIRENTRIES64,
                directory.file.as_raw_fd(),
                prepared.storage.as_mut_ptr().cast::<libc::c_char>(),
                bytes_limit,
                &mut base,
            )
        };
        if read < 0 {
            return Err(Error::Changed);
        }
        let used = usize::try_from(read).map_err(|_| Error::Changed)?;
        if used == 0 {
            break;
        }
        if used > capacity {
            return Err(Error::Changed);
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(prepared.storage.as_ptr().cast::<u8>(), used) };
        parse_darwin_inventory_records(bytes, |name, inode| {
            raw_records = raw_records.checked_add(1).ok_or(Error::OutputLimit)?;
            if raw_records > maximum_records {
                return Err(Error::Changed);
            }
            if name == b"." {
                if saw_dot {
                    return Err(Error::Changed);
                }
                saw_dot = true;
            } else if name == b".." {
                if saw_dot_dot {
                    return Err(Error::Changed);
                }
                saw_dot_dot = true;
            }
            admit_inventory_entry(prepared, files, &mut seen, &mut count, name, inode)
        })?;
    }
    if count != N || seen.iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

fn admit_inventory_typed_entry<const N: usize, const F: usize, const D: usize>(
    prepared: &PreparedInventoryEntriesExact<N>,
    files: &[&RegularFile; F],
    directories: &[&Directory; D],
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
        .names
        .iter()
        .position(|expected| expected.as_ref().expect("prepared name").0.as_bytes() == actual)
    else {
        return Err(Error::Changed);
    };
    let expected_inode = if index < F {
        files[index].ino
    } else {
        directories[index.checked_sub(F).ok_or(Error::Changed)?].ino
    };
    if seen[index] || inode != expected_inode {
        return Err(Error::Changed);
    }
    seen[index] = true;
    *count = count.checked_add(1).ok_or(Error::OutputLimit)?;
    if *count > N {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn scan_prepared_entries_directory<const N: usize, const F: usize, const D: usize>(
    prepared: &mut PreparedInventoryEntriesExact<N>,
    directory: &Directory,
    files: &[&RegularFile; F],
    directories: &[&Directory; D],
) -> Result<(), Error> {
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut raw_records = 0usize;
    let mut queries = 0usize;
    let maximum_records = N.checked_add(2).ok_or(Error::OutputLimit)?;
    let maximum_queries = N.checked_add(3).ok_or(Error::OutputLimit)?;
    let capacity = prepared
        .storage
        .len()
        .checked_mul(std::mem::size_of::<u64>())
        .ok_or(Error::OutputLimit)?;
    let bytes_limit = libc::c_uint::try_from(capacity).map_err(|_| Error::OutputLimit)?;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > maximum_queries {
            return Err(Error::Changed);
        }
        prepared.storage.fill(u64::MAX);
        let read = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                directory.file.as_raw_fd(),
                prepared.storage.as_mut_ptr().cast::<u8>(),
                bytes_limit,
            )
        };
        if read < 0 {
            return Err(Error::Changed);
        }
        let used = usize::try_from(libc::c_uint::try_from(read).map_err(|_| Error::Changed)?)
            .map_err(|_| Error::Changed)?;
        if used == 0 {
            break;
        }
        if used > capacity {
            return Err(Error::Changed);
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(prepared.storage.as_ptr().cast::<u8>(), used) };
        parse_linux_inventory_records(bytes, |name, inode| {
            raw_records = raw_records.checked_add(1).ok_or(Error::OutputLimit)?;
            if raw_records > maximum_records {
                return Err(Error::Changed);
            }
            admit_inventory_typed_entry(
                prepared,
                files,
                directories,
                &mut seen,
                &mut count,
                name,
                inode,
            )
        })?;
    }
    if count != N || seen.iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn scan_prepared_entries_directory<const N: usize, const F: usize, const D: usize>(
    prepared: &mut PreparedInventoryEntriesExact<N>,
    directory: &Directory,
    files: &[&RegularFile; F],
    directories: &[&Directory; D],
) -> Result<(), Error> {
    const SYS_GETDIRENTRIES64: libc::c_int = 344;
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut raw_records = 0usize;
    let mut queries = 0usize;
    let maximum_records = N.checked_add(2).ok_or(Error::OutputLimit)?;
    let maximum_queries = N.checked_add(3).ok_or(Error::OutputLimit)?;
    let capacity = prepared
        .storage
        .len()
        .checked_mul(std::mem::size_of::<u64>())
        .ok_or(Error::OutputLimit)?;
    let mut base: libc::off_t = 0;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > maximum_queries {
            return Err(Error::Changed);
        }
        prepared.storage.fill(u64::MAX);
        let read = unsafe {
            libc::syscall(
                SYS_GETDIRENTRIES64,
                directory.file.as_raw_fd(),
                prepared.storage.as_mut_ptr().cast::<libc::c_char>(),
                capacity,
                &mut base,
            )
        };
        if read < 0 {
            return Err(Error::Changed);
        }
        let used = usize::try_from(read).map_err(|_| Error::Changed)?;
        if used == 0 {
            break;
        }
        if used > capacity {
            return Err(Error::Changed);
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(prepared.storage.as_ptr().cast::<u8>(), used) };
        parse_darwin_inventory_records(bytes, |name, inode| {
            raw_records = raw_records.checked_add(1).ok_or(Error::OutputLimit)?;
            if raw_records > maximum_records {
                return Err(Error::Changed);
            }
            admit_inventory_typed_entry(
                prepared,
                files,
                directories,
                &mut seen,
                &mut count,
                name,
                inode,
            )
        })?;
    }
    if count != N || seen.iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    Ok(())
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
    for file in files {
        recheck_regular(file)?;
    }
    for child in directories {
        recheck_directory(child)?;
    }
    if unsafe { libc::lseek(directory.file.as_raw_fd(), 0, libc::SEEK_SET) } < 0 {
        return Err(Error::Changed);
    }
    let scan = scan_prepared_entries_directory(prepared, directory, &files, &directories);
    let reset = unsafe { libc::lseek(directory.file.as_raw_fd(), 0, libc::SEEK_SET) };
    if scan.is_err() || reset < 0 {
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
        let rebound = open_directory_at(directory.file.as_raw_fd(), &name.0)?;
        if prepared_directory_identity(&rebound) != prepared_directory_identity(child) {
            return Err(Error::Changed);
        }
    }
    recheck_directory(directory)?;
    for file in files {
        recheck_regular(file)?;
    }
    for child in directories {
        recheck_directory(child)?;
    }
    Ok(())
}

pub fn inventory_exact_prepared<const N: usize>(
    prepared: &mut PreparedInventoryExact<N>,
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    files: [Option<&RegularFile>; N],
) -> Result<(), Error> {
    let current_bindings = prepared_name_bindings(names)?;
    if prepared.remaining == 0
        || prepared.bindings != current_bindings
        || files.iter().any(Option::is_none)
    {
        return Err(Error::Invalid);
    }
    let directory_identity = prepared_directory_identity(directory);
    match prepared.directory_identity {
        Some(first) if first != directory_identity => return Err(Error::Changed),
        None => prepared.directory_identity = Some(directory_identity),
        Some(_) => {}
    }
    prepared.remaining -= 1;
    recheck_directory(directory)?;
    for file in files.iter().flatten() {
        recheck_regular(file)?;
    }
    #[cfg(test)]
    let fail_initial_seek = prepared.fail_initial_seek;
    #[cfg(not(test))]
    let fail_initial_seek = false;
    if fail_initial_seek
        || unsafe { libc::lseek(directory.file.as_raw_fd(), 0, libc::SEEK_SET) } < 0
    {
        return Err(Error::Changed);
    }
    let scan = scan_prepared_directory(prepared, directory, &files);
    #[cfg(test)]
    let fail_reset_seek = prepared.fail_reset_seek;
    #[cfg(not(test))]
    let fail_reset_seek = false;
    let reset = if fail_reset_seek {
        -1
    } else {
        unsafe { libc::lseek(directory.file.as_raw_fd(), 0, libc::SEEK_SET) }
    };
    if scan.is_err() || reset < 0 {
        return Err(Error::Changed);
    }
    for (index, tracked) in files.iter().enumerate() {
        #[cfg(test)]
        let (fail_authentication, fail_close) = (
            prepared.fail_rebound_authentication,
            prepared.fail_rebound_close,
        );
        #[cfg(not(test))]
        let (fail_authentication, fail_close) = (false, false);
        let rebound = observe_inventory_rebound(
            directory,
            prepared.names[index].as_ref().expect("prepared name"),
            fail_authentication,
            fail_close,
        )?;
        if !same_regular_identity(&rebound, tracked.expect("attached")) {
            return Err(Error::Changed);
        }
    }
    recheck_directory(directory)?;
    for file in files.iter().flatten() {
        recheck_regular(file)?;
    }
    Ok(())
}

fn observe_publish_rebound(
    parent: &Directory,
    name: &std::ffi::CStr,
    fail_information: bool,
    fail_close: bool,
) -> Result<PreparedDirectoryIdentity, Error> {
    let descriptor = unsafe {
        libc::openat(
            parent.file.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        return Err(Error::Changed);
    }
    let file = std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(descriptor) });
    let observed = (|| {
        if fail_information {
            return Err(Error::Changed);
        }
        let metadata = file.metadata().map_err(|_| Error::Changed)?;
        if !metadata.is_dir() {
            return Err(Error::Changed);
        }
        let (dev, ino) = identity(&metadata);
        Ok(PreparedDirectoryIdentity {
            dev,
            ino,
            mode: metadata.mode(),
            #[cfg(target_os = "macos")]
            generation: metadata_generation(&metadata),
        })
    })();
    let close_failed = unsafe { libc::close(descriptor) } != 0;
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
    let stage_name = relative_name_arena_cstr(stage_name)?;
    let output_bytes = validated_c_name_bytes(output_name)?;
    if prepared.remaining != 1
        || prepared.exact_capacity != prepared.destination.as_bytes_with_nul().len()
        || prepared.destination.as_bytes() != output_bytes
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
    if observe_publish_rebound(parent, stage_name, fail_information, fail_close)?
        != prepared_directory_identity(stage)
    {
        return Err(Error::Changed);
    }
    #[cfg(debug_assertions)]
    if prepared.fail_rename {
        return Err(Error::Changed);
    }
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            parent.file.as_raw_fd(),
            stage_name.as_ptr(),
            parent.file.as_raw_fd(),
            prepared.destination.as_ptr(),
            1_u32,
        )
    } as i32;
    #[cfg(target_os = "macos")]
    let result = unsafe {
        unsafe extern "C" {
            fn renameatx_np(
                fromfd: libc::c_int,
                from: *const libc::c_char,
                tofd: libc::c_int,
                to: *const libc::c_char,
                flags: libc::c_uint,
            ) -> libc::c_int;
        }
        renameatx_np(
            parent.file.as_raw_fd(),
            stage_name.as_ptr(),
            parent.file.as_raw_fd(),
            prepared.destination.as_ptr(),
            0x0000_0004,
        )
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let result = -1;
    if result != 0 {
        let errno = std::io::Error::last_os_error().raw_os_error();
        return Err(
            if errno == Some(libc::EEXIST) || errno == Some(libc::ENOTEMPTY) {
                Error::Exists
            } else {
                Error::Changed
            },
        );
    }
    Ok(())
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
    let stage_name = relative_name_arena_cstr(stage_name)?;
    let rebound = open_directory_at(parent.file.as_raw_fd(), stage_name)?;
    if (rebound.dev, rebound.ino, rebound.mode) != (stage.dev, stage.ino, stage.mode) {
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

    let duplicate = stage.file.try_clone().map_err(|_| Error::Changed)?;
    let fd = unsafe { libc::dup(duplicate.as_raw_fd()) };
    if fd < 0 {
        return Err(Error::Changed);
    }
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        unsafe { libc::close(fd) };
        return Err(Error::Changed);
    }
    let scan = (|| {
        unsafe { libc::rewinddir(stream) };
        let mut seen = [false; N];
        let mut count = 0usize;
        loop {
            #[cfg(target_os = "linux")]
            unsafe {
                *libc::__errno_location() = 0;
            }
            #[cfg(target_os = "macos")]
            unsafe {
                *libc::__error() = 0;
            }
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                #[cfg(target_os = "linux")]
                let errno = unsafe { *libc::__errno_location() };
                #[cfg(target_os = "macos")]
                let errno = unsafe { *libc::__error() };
                if errno != 0 {
                    return Err(Error::Changed);
                }
                break;
            }
            let actual = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if actual == b"." || actual == b".." {
                continue;
            }
            let Some(index) = names.names[..attached]
                .iter()
                .position(|expected| expected.as_ref().expect("validated").0.as_bytes() == actual)
            else {
                return Err(Error::Changed);
            };
            if seen[index] {
                return Err(Error::Changed);
            }
            seen[index] = true;
            count = count.checked_add(1).ok_or(Error::OutputLimit)?;
        }
        if count != attached || seen[..attached].iter().any(|seen| !seen) {
            return Err(Error::Changed);
        }
        Ok(())
    })();
    unsafe { libc::closedir(stream) };
    scan?;

    for (index, name) in names.names[..attached].iter().enumerate() {
        let file = files[index]
            .or_else(|| settled[index].map(|file| &file.0))
            .expect("attached prefix");
        recheck_regular(file)?;
        let name = name.as_ref().expect("validated");
        let rebound = hold_regular_file_name_prepared(stage, name)?;
        if (
            rebound.dev,
            rebound.ino,
            rebound.mode,
            rebound.len,
            rebound.digest,
        ) != (file.dev, file.ino, file.mode, file.len, file.digest)
        {
            return Err(Error::Changed);
        }
    }
    for (deleted, name) in names.names[..attached].iter().enumerate() {
        #[cfg(not(debug_assertions))]
        let _ = deleted;
        #[cfg(debug_assertions)]
        if failure_after_delete == Some(deleted) {
            return Err(Error::Changed);
        }
        let name = name.as_ref().expect("validated");
        if unsafe { libc::unlinkat(stage.file.as_raw_fd(), name.0.as_ptr(), 0) } != 0 {
            return Err(Error::Changed);
        }
    }
    #[cfg(debug_assertions)]
    if failure_after_delete == Some(attached) {
        return Err(Error::Changed);
    }
    if unsafe {
        libc::unlinkat(
            parent.file.as_raw_fd(),
            stage_name.as_ptr(),
            libc::AT_REMOVEDIR,
        )
    } != 0
    {
        return Err(Error::Changed);
    }
    Ok(())
}
