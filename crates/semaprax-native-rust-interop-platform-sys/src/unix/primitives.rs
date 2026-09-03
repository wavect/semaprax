//! Digest, executable-slice, C-name, and child-settlement primitives shared by
//! the Unix held-handle phases.

use super::*;

pub(super) fn digest_file(file: &File, length: u64) -> Result<[u8; 32], Error> {
    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 8192];
    while offset < length {
        let remaining = usize::try_from((length - offset).min(buffer.len() as u64))
            .map_err(|_| Error::OutputLimit)?;
        let count = file
            .read_at(&mut buffer[..remaining], offset)
            .map_err(|_| Error::Changed)?;
        if count == 0 {
            return Err(Error::Changed);
        }
        hasher.update(&buffer[..count]);
        offset = offset
            .checked_add(u64::try_from(count).map_err(|_| Error::OutputLimit)?)
            .ok_or(Error::OutputLimit)?;
    }
    Ok(hasher.finalize().into())
}

pub(super) fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(target_os = "macos")]
pub(super) fn executable_slice(file: &RegularFile) -> Result<(u64, u64), Error> {
    let mut prefix = [0_u8; 32];
    if file
        .file
        .read_at(&mut prefix, 0)
        .map_err(|_| Error::Changed)?
        != prefix.len()
    {
        return Err(Error::Invalid);
    }
    let current_cpu = if cfg!(target_arch = "aarch64") {
        0x0100_000c_u32
    } else if cfg!(target_arch = "x86_64") {
        0x0100_0007_u32
    } else {
        return Err(Error::Unsupported);
    };
    let little = u32::from_le_bytes(prefix[0..4].try_into().map_err(|_| Error::Invalid)?);
    if little == 0xfeed_facf {
        let cpu = u32::from_le_bytes(prefix[4..8].try_into().map_err(|_| Error::Invalid)?);
        let subtype =
            u32::from_le_bytes(prefix[8..12].try_into().map_err(|_| Error::Invalid)?) & 0x00ff_ffff;
        let compatible_subtype = if cfg!(target_arch = "aarch64") {
            matches!(subtype, 0 | 2)
        } else {
            subtype == 3
        };
        let filetype = u32::from_le_bytes(prefix[12..16].try_into().map_err(|_| Error::Invalid)?);
        if cpu != current_cpu || !compatible_subtype || filetype != 2 {
            return Err(Error::Invalid);
        }
        return Ok((0, file.len));
    }
    let magic = u32::from_be_bytes(prefix[0..4].try_into().map_err(|_| Error::Invalid)?);
    let entry_size = match magic {
        0xcafe_babe => 20_usize,
        0xcafe_babf => 32_usize,
        _ => return Err(Error::Invalid),
    };
    let count = usize::try_from(u32::from_be_bytes(
        prefix[4..8].try_into().map_err(|_| Error::Invalid)?,
    ))
    .map_err(|_| Error::Invalid)?;
    if count == 0 || count > 64 {
        return Err(Error::Invalid);
    }
    let table_size = count.checked_mul(entry_size).ok_or(Error::Invalid)?;
    let table_end = 8_usize.checked_add(table_size).ok_or(Error::Invalid)?;
    if u64::try_from(table_end).map_err(|_| Error::Invalid)? > file.len {
        return Err(Error::Invalid);
    }
    let mut table = [0_u8; 64 * 32];
    if file
        .file
        .read_at(&mut table[..table_size], 8)
        .map_err(|_| Error::Changed)?
        != table_size
    {
        return Err(Error::Changed);
    }
    let mut rows = [(0_u32, 0_u32, 0_u64, 0_u64); 64];
    let mut row_count = 0usize;
    for index in 0..count {
        let start = index.checked_mul(entry_size).ok_or(Error::Invalid)?;
        let row = table.get(start..start + entry_size).ok_or(Error::Invalid)?;
        let cpu = u32::from_be_bytes(row[0..4].try_into().map_err(|_| Error::Invalid)?);
        let subtype = u32::from_be_bytes(row[4..8].try_into().map_err(|_| Error::Invalid)?);
        let (offset, size, alignment, reserved) = if entry_size == 20 {
            (
                u64::from(u32::from_be_bytes(
                    row[8..12].try_into().map_err(|_| Error::Invalid)?,
                )),
                u64::from(u32::from_be_bytes(
                    row[12..16].try_into().map_err(|_| Error::Invalid)?,
                )),
                u32::from_be_bytes(row[16..20].try_into().map_err(|_| Error::Invalid)?),
                0,
            )
        } else {
            (
                u64::from_be_bytes(row[8..16].try_into().map_err(|_| Error::Invalid)?),
                u64::from_be_bytes(row[16..24].try_into().map_err(|_| Error::Invalid)?),
                u32::from_be_bytes(row[24..28].try_into().map_err(|_| Error::Invalid)?),
                u32::from_be_bytes(row[28..32].try_into().map_err(|_| Error::Invalid)?),
            )
        };
        let end = offset.checked_add(size).ok_or(Error::Invalid)?;
        if size < 32
            || end > file.len
            || offset < u64::try_from(table_end).map_err(|_| Error::Invalid)?
            || alignment > 63
            || offset % (1_u64 << alignment) != 0
            || reserved != 0
            || rows[..row_count]
                .iter()
                .any(|(_, _, prior_offset, prior_end)| offset < *prior_end && *prior_offset < end)
            || rows[..row_count]
                .iter()
                .any(|(prior_cpu, prior_subtype, _, _)| {
                    *prior_cpu == cpu && *prior_subtype == subtype
                })
        {
            return Err(Error::Invalid);
        }
        rows[row_count] = (cpu, subtype, offset, end);
        row_count += 1;
    }
    let mut selected = None;
    for (cpu, subtype, offset, end) in &rows[..row_count] {
        let masked_subtype = *subtype & 0x00ff_ffff;
        let matches_current = *cpu == current_cpu
            && if cfg!(target_arch = "aarch64") {
                matches!(masked_subtype, 0 | 2)
            } else {
                masked_subtype == 3
            };
        if matches_current
            && selected
                .replace((masked_subtype, *offset, *end - *offset))
                .is_some()
        {
            return Err(Error::Invalid);
        }
    }
    let Some((selected_subtype, offset, size)) = selected else {
        return Err(Error::Invalid);
    };
    let mut header = [0_u8; 16];
    if file
        .file
        .read_at(&mut header, offset)
        .map_err(|_| Error::Changed)?
        != header.len()
    {
        return Err(Error::Changed);
    }
    if u32::from_le_bytes(header[0..4].try_into().map_err(|_| Error::Invalid)?) != 0xfeed_facf
        || u32::from_le_bytes(header[4..8].try_into().map_err(|_| Error::Invalid)?) != current_cpu
        || u32::from_le_bytes(header[12..16].try_into().map_err(|_| Error::Invalid)?) != 2
        || (u32::from_le_bytes(header[8..12].try_into().map_err(|_| Error::Invalid)?) & 0x00ff_ffff)
            != selected_subtype
    {
        return Err(Error::Invalid);
    }
    Ok((offset, size))
}

#[cfg(target_os = "linux")]
pub(super) fn executable_slice(file: &RegularFile) -> Result<(u64, u64), Error> {
    let mut header = [0_u8; 64];
    if file
        .file
        .read_at(&mut header, 0)
        .map_err(|_| Error::Changed)?
        != header.len()
        || &header[..4] != b"\x7fELF"
        || header[4] != 2
        || header[5] != 1
        || header[6] != 1
        || !matches!(u16::from_le_bytes([header[16], header[17]]), 2 | 3)
        || u32::from_le_bytes(header[20..24].try_into().map_err(|_| Error::Invalid)?) != 1
        || u16::from_le_bytes([header[52], header[53]]) != 64
    {
        return Err(Error::Invalid);
    }
    let machine = u16::from_le_bytes([header[18], header[19]]);
    if (cfg!(target_arch = "x86_64") && machine != 62)
        || (cfg!(target_arch = "aarch64") && machine != 183)
    {
        return Err(Error::Invalid);
    }
    let program_offset = u64::from_le_bytes(header[32..40].try_into().map_err(|_| Error::Invalid)?);
    let entry_size = usize::from(u16::from_le_bytes([header[54], header[55]]));
    let entry_count = usize::from(u16::from_le_bytes([header[56], header[57]]));
    if entry_size != 56 || entry_count == 0 || entry_count > 4096 {
        return Err(Error::Invalid);
    }
    let table_size = entry_size.checked_mul(entry_count).ok_or(Error::Invalid)?;
    let table_end = program_offset
        .checked_add(u64::try_from(table_size).map_err(|_| Error::Invalid)?)
        .ok_or(Error::Invalid)?;
    if table_end > file.len {
        return Err(Error::Invalid);
    }
    let mut table = [0_u8; 56 * 4096];
    if file
        .file
        .read_at(&mut table[..table_size], program_offset)
        .map_err(|_| Error::Changed)?
        != table_size
    {
        return Err(Error::Changed);
    }
    let entry = u64::from_le_bytes(header[24..32].try_into().map_err(|_| Error::Invalid)?);
    let mut executable_load = false;
    for row in table[..table_size].chunks_exact(entry_size) {
        let kind = u32::from_le_bytes(row[0..4].try_into().map_err(|_| Error::Invalid)?);
        let flags = u32::from_le_bytes(row[4..8].try_into().map_err(|_| Error::Invalid)?);
        let offset = u64::from_le_bytes(row[8..16].try_into().map_err(|_| Error::Invalid)?);
        let virtual_address =
            u64::from_le_bytes(row[16..24].try_into().map_err(|_| Error::Invalid)?);
        let file_size = u64::from_le_bytes(row[32..40].try_into().map_err(|_| Error::Invalid)?);
        let memory_size = u64::from_le_bytes(row[40..48].try_into().map_err(|_| Error::Invalid)?);
        let alignment = u64::from_le_bytes(row[48..56].try_into().map_err(|_| Error::Invalid)?);
        if offset
            .checked_add(file_size)
            .is_none_or(|end| end > file.len)
            || virtual_address.checked_add(memory_size).is_none()
            || file_size > memory_size
            || (alignment > 1
                && (!alignment.is_power_of_two()
                    || offset % alignment != virtual_address % alignment))
        {
            return Err(Error::Invalid);
        }
        if kind == 1
            && flags & 1 != 0
            && entry >= virtual_address
            && entry
                < virtual_address
                    .checked_add(memory_size)
                    .ok_or(Error::Invalid)?
        {
            executable_load = true;
        }
    }
    if !executable_load {
        return Err(Error::Invalid);
    }
    Ok((0, file.len))
}

pub(super) fn validated_c_name_bytes(name: &OsStr) -> Result<&[u8], Error> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes == b"."
        || bytes == b".."
        || bytes.contains(&b'/')
        || bytes.contains(&0)
    {
        return Err(Error::Invalid);
    }
    Ok(bytes)
}

pub(super) fn c_name(name: &OsStr) -> Result<CString, Error> {
    CString::new(validated_c_name_bytes(name)?).map_err(|_| Error::Invalid)
}

fn wait_child(pid: libc::pid_t, kill_first: bool) -> Result<libc::c_int, Error> {
    if kill_first {
        let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
        let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let mut status = 0;
        let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if waited == pid {
            return Ok(status);
        }
        if waited == 0 {
            if std::time::Instant::now() >= deadline {
                let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
                return Err(Error::Spawn);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::EINTR) => continue,
            // ECHILD is accepted only after the kernel proves that this exact
            // pid is no longer a waitable child. No retry or stronger signal
            // authority is used.
            Some(libc::ECHILD) => return Err(Error::Spawn),
            _ => return Err(Error::Spawn),
        }
    }
}

#[cfg(target_os = "macos")]
fn quiesce_group_before_reap(pid: libc::pid_t) -> Result<(), Error> {
    const MAX_GROUP_MEMBERS: usize = 4096;

    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_listpgrppids(
            pgrpid: libc::pid_t,
            buffer: *mut libc::c_void,
            buffersize: libc::c_int,
        ) -> libc::c_int;
    }

    if unsafe { libc::kill(-pid, libc::SIGKILL) } != 0
        && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    {
        return Err(Error::Spawn);
    }
    let mut members = [0 as libc::pid_t; MAX_GROUP_MEMBERS];
    let member_bytes =
        libc::c_int::try_from(std::mem::size_of_val(&members)).map_err(|_| Error::Spawn)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        unsafe {
            *libc::__error() = 0;
        }
        let required = unsafe { proc_listpgrppids(pid, std::ptr::null_mut(), 0) };
        let required_errno = unsafe { *libc::__error() };
        // proc_listpgrppids returns a PID count, unlike proc_listpids,
        // which returns a byte count. The wrapper reports kernel failure
        // as zero, so errno must be bound independently.
        if (required == 0 && required_errno != 0)
            || required < 0
            || usize::try_from(required).map_err(|_| Error::Spawn)? > MAX_GROUP_MEMBERS
        {
            return Err(Error::Spawn);
        }
        members.fill(0);
        unsafe {
            *libc::__error() = 0;
        }
        let returned = unsafe { proc_listpgrppids(pid, members.as_mut_ptr().cast(), member_bytes) };
        let returned_errno = unsafe { *libc::__error() };
        if (returned == 0 && returned_errno != 0)
            || returned < 0
            || usize::try_from(returned).map_err(|_| Error::Spawn)? > MAX_GROUP_MEMBERS
        {
            return Err(Error::Spawn);
        }
        let count = usize::try_from(returned).map_err(|_| Error::Spawn)?;
        if members[..count].iter().all(|member| *member == pid) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Spawn);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn quiesce_group(pid: libc::pid_t) -> Result<(), Error> {
    let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let result = unsafe { libc::kill(-pid, 0) };
        if result != 0 {
            match std::io::Error::last_os_error().raw_os_error() {
                Some(libc::ESRCH) => return Ok(()),
                Some(libc::EINTR) => continue,
                _ => return Err(Error::Spawn),
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Spawn);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn settle_failed_group(
    pid: libc::pid_t,
    pipe: CheckedFd,
    leader_reaped: bool,
) -> Result<(), Error> {
    let close_failed = pipe.close_injected(TestClosePoint::Settle).is_err();
    #[cfg(target_os = "linux")]
    let mut leader = if leader_reaped {
        let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
        Ok(())
    } else {
        wait_child(pid, true).map(|_| ())
    };
    #[cfg(target_os = "linux")]
    let mut group = quiesce_group(pid);
    #[cfg(target_os = "macos")]
    let mut group = if leader_reaped {
        Ok(())
    } else {
        quiesce_group_before_reap(pid)
    };
    #[cfg(target_os = "macos")]
    let mut leader = if leader_reaped || group.is_err() {
        if leader_reaped {
            Ok(())
        } else {
            Err(Error::Spawn)
        }
    } else {
        wait_child(pid, false).map(|_| ())
    };
    if injected_settlement_failure!(UnixWait) {
        leader = Err(Error::Spawn);
    }
    if injected_settlement_failure!(UnixGroup) {
        group = Err(Error::Spawn);
    }
    if close_failed || leader.is_err() || group.is_err() {
        Err(Error::Spawn)
    } else {
        Ok(())
    }
}

pub(super) fn must_settle_failed_group(pid: libc::pid_t, pipe: CheckedFd, leader_reaped: bool) {
    if settle_failed_group(pid, pipe, leader_reaped).is_err() {
        std::process::abort();
    }
}

pub(super) fn drain_and_wait(
    pid: libc::pid_t,
    pipe: CheckedFd,
    stdout_limit: usize,
    mut output: Vec<u8>,
    close_pipe_after_leader: bool,
) -> Result<(Vec<u8>, libc::c_int), Error> {
    if injected_settlement_failure!(UnixDrainFcntl)
        || unsafe { libc::fcntl(pipe.raw(), libc::F_SETFL, libc::O_NONBLOCK) } != 0
    {
        must_settle_failed_group(pid, pipe, false);
        return Err(Error::Spawn);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let fixed_output = output.capacity() != 0 || stdout_limit == 0;
    if (fixed_output && output.capacity() != stdout_limit) || !output.is_empty() {
        must_settle_failed_group(pid, pipe, false);
        return Err(Error::OutputLimit);
    }
    let mut status = None;
    let mut eof = false;
    loop {
        let mut poll_fd = libc::pollfd {
            fd: pipe.raw(),
            events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
            revents: 0,
        };
        let polled = unsafe { libc::poll(&mut poll_fd, 1, 25) };
        if injected_settlement_failure!(UnixPoll)
            || (polled < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR))
        {
            must_settle_failed_group(pid, pipe, status.is_some());
            return Err(Error::Spawn);
        }
        if polled > 0 {
            loop {
                let mut buffer = [0_u8; 8192];
                if injected_settlement_failure!(UnixRead) {
                    must_settle_failed_group(pid, pipe, status.is_some());
                    return Err(Error::Spawn);
                }
                let read =
                    unsafe { libc::read(pipe.raw(), buffer.as_mut_ptr().cast(), buffer.len()) };
                match read.cmp(&0) {
                    std::cmp::Ordering::Greater => {
                        if injected_settlement_failure!(UnixReadConversion) {
                            must_settle_failed_group(pid, pipe, status.is_some());
                            return Err(Error::OutputLimit);
                        }
                        let count = match usize::try_from(read) {
                            Ok(count) => count,
                            Err(_) => {
                                must_settle_failed_group(pid, pipe, status.is_some());
                                return Err(Error::OutputLimit);
                            }
                        };
                        if count > stdout_limit.saturating_sub(output.len()) {
                            must_settle_failed_group(pid, pipe, status.is_some());
                            return Err(Error::OutputLimit);
                        }
                        output.extend_from_slice(&buffer[..count]);
                        if fixed_output && output.capacity() != stdout_limit {
                            must_settle_failed_group(pid, pipe, status.is_some());
                            return Err(Error::OutputLimit);
                        }
                    }
                    std::cmp::Ordering::Equal => {
                        eof = true;
                        break;
                    }
                    std::cmp::Ordering::Less => {
                        match std::io::Error::last_os_error().raw_os_error() {
                            Some(libc::EAGAIN) => break,
                            Some(libc::EINTR) => continue,
                            _ => {
                                must_settle_failed_group(pid, pipe, status.is_some());
                                return Err(Error::Spawn);
                            }
                        }
                    }
                }
            }
        }
        if status.is_none() {
            let mut child_status = 0;
            if injected_settlement_failure!(UnixWaitpid) {
                must_settle_failed_group(pid, pipe, false);
                return Err(Error::Spawn);
            }
            let waited = unsafe { libc::waitpid(pid, &mut child_status, libc::WNOHANG) };
            match waited {
                waited if waited == pid => {
                    status = Some(child_status);
                    if !eof {
                        // A descendant retaining the private pipe is not part of
                        // the admitted tool result. Close the whole private group.
                        let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
                        if close_pipe_after_leader {
                            // Archive stdout is fixed at zero bytes. Once the
                            // leader is reaped and its entire owned group is
                            // quiescent, EOF carries no additional authority:
                            // an unrelated holder must not keep publication
                            // waiting. Drain bytes already committed to the pipe,
                            // then close the read end without waiting for HUP.
                            if quiesce_group(pid).is_err() {
                                std::process::abort();
                            }
                            let result = loop {
                                let mut byte = [0_u8; 1];
                                let read = unsafe {
                                    libc::read(pipe.raw(), byte.as_mut_ptr().cast(), byte.len())
                                };
                                if read > 0 {
                                    break Err(Error::OutputLimit);
                                }
                                if read == 0 {
                                    break Ok((output, child_status));
                                }
                                match std::io::Error::last_os_error().raw_os_error() {
                                    Some(libc::EAGAIN) => break Ok((output, child_status)),
                                    Some(libc::EINTR) => continue,
                                    _ => break Err(Error::Spawn),
                                }
                            };
                            if pipe.close_injected(TestClosePoint::SuccessRead).is_err() {
                                std::process::abort();
                            }
                            return result;
                        }
                    }
                }
                0 => {}
                -1 if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) => {}
                _ => {
                    must_settle_failed_group(pid, pipe, status.is_some());
                    return Err(Error::Spawn);
                }
            }
        }
        if eof {
            if let Some(status) = status {
                // Quiesce every descendant in the private group even if it
                // closed stdout before the leader exited.
                let close_failed = pipe.close_injected(TestClosePoint::SuccessRead).is_err();
                let group = quiesce_group(pid);
                if close_failed || group.is_err() {
                    std::process::abort();
                }
                return Ok((output, status));
            }
        }
        if injected_settlement_failure!(UnixDeadline) || std::time::Instant::now() >= deadline {
            must_settle_failed_group(pid, pipe, status.is_some());
            return Err(Error::Spawn);
        }
    }
}

pub(super) fn identity(metadata: &std::fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

pub(super) fn open_directory_at(parent: RawFd, name: &std::ffi::CStr) -> Result<Directory, Error> {
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::Changed);
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file.metadata().map_err(|_| Error::Changed)?;
    let (dev, ino) = identity(&metadata);
    Ok(Directory {
        file,
        dev,
        ino,
        mode: metadata.mode(),
        #[cfg(target_os = "macos")]
        generation: metadata_generation(&metadata),
    })
}
