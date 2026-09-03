//! Windows child launch, job settlement, and command-line preflight.

use super::*;

pub(super) fn run_argv(
    executable: &Executable,
    cwd: &Directory,
    arguments: &[String],
    stdout_limit: usize,
    prepared_command_line: Option<Vec<u16>>,
    prepared_output: Option<Vec<u8>>,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    if arguments.len() > 32
        || prepared_command_line.as_ref().is_none_or(Vec::is_empty)
        || prepared_output
            .as_ref()
            .is_none_or(|output| output.capacity() != stdout_limit || !output.is_empty())
    {
        return Err(Error::Invalid);
    }
    consume_process_arena(process_arena)?;
    struct CheckedHandle(Option<HANDLE>);
    impl CheckedHandle {
        fn new(handle: HANDLE) -> Self {
            Self(Some(handle))
        }

        fn raw(&self) -> HANDLE {
            self.0.expect("checked handle remains owned")
        }

        fn close(mut self) -> Result<(), Error> {
            let handle = self.0.take().expect("checked handle remains owned");
            if unsafe { CloseHandle(handle) } == 0 {
                Err(Error::Spawn)
            } else {
                Ok(())
            }
        }
    }
    impl Drop for CheckedHandle {
        fn drop(&mut self) {
            if let Some(handle) = self.0.take() {
                if unsafe { CloseHandle(handle) } == 0 {
                    std::process::abort();
                }
            }
        }
    }
    fn must_close(handles: [CheckedHandle; 4]) {
        let mut failed = false;
        for handle in handles {
            failed |= handle.close().is_err();
        }
        if failed {
            std::process::abort();
        }
    }
    recheck_held_regular(&executable.file)?;
    recheck_directory(cwd)?;
    final_path_prepared(&executable.file.file, &mut process_arena.application)?;
    final_path_prepared(&cwd.file, &mut process_arena.cwd)?;
    let mut command_line = prepared_command_line.ok_or(Error::Invalid)?;

    let security = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(std::mem::size_of::<SECURITY_ATTRIBUTES>())
            .map_err(|_| Error::Spawn)?,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    let mut read_pipe = std::ptr::null_mut();
    let mut write_pipe = std::ptr::null_mut();
    if unsafe { CreatePipe(&mut read_pipe, &mut write_pipe, &security, 0) } == 0 {
        return Err(Error::Spawn);
    }
    let read_pipe = CheckedHandle::new(read_pipe);
    let write_pipe = CheckedHandle::new(write_pipe);
    if unsafe { SetHandleInformation(read_pipe.raw(), HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(Error::Spawn);
    }
    let null_name = [u16::from(b'N'), u16::from(b'U'), u16::from(b'L'), 0];
    let null_handle = unsafe {
        CreateFileW(
            null_name.as_ptr(),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if null_handle == INVALID_HANDLE_VALUE {
        return Err(Error::Spawn);
    }
    let null_handle = CheckedHandle::new(null_handle);
    if unsafe { SetHandleInformation(null_handle.raw(), HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) }
        == 0
    {
        return Err(Error::Spawn);
    }

    let inherited = [null_handle.raw(), write_pipe.raw()];
    let mut attribute_bytes = process_arena.attribute_bytes;
    let attribute_words = attribute_bytes
        .checked_add(std::mem::size_of::<u64>() - 1)
        .and_then(|bytes| bytes.checked_div(std::mem::size_of::<u64>()))
        .ok_or(Error::OutputLimit)?;
    process_arena.attributes.resize(attribute_words, 0);
    if attribute_bytes
        > process_arena
            .attributes
            .len()
            .saturating_mul(std::mem::size_of::<u64>())
        || process_arena.attributes.capacity() != attribute_words
    {
        return Err(Error::OutputLimit);
    }
    let attribute_list = process_arena.attributes.as_mut_ptr().cast();
    if unsafe { InitializeProcThreadAttributeList(attribute_list, 1, 0, &mut attribute_bytes) } == 0
        || attribute_bytes != process_arena.attribute_bytes
    {
        return Err(Error::Spawn);
    }
    struct AttributeList(windows_sys::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST);
    impl Drop for AttributeList {
        fn drop(&mut self) {
            unsafe { DeleteProcThreadAttributeList(self.0) };
        }
    }
    let attribute_list = AttributeList(attribute_list);
    if unsafe {
        UpdateProcThreadAttribute(
            attribute_list.0,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            inherited.as_ptr().cast(),
            std::mem::size_of_val(&inherited),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    } == 0
    {
        return Err(Error::Spawn);
    }

    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        return Err(Error::Spawn);
    }
    let job = CheckedHandle::new(job);
    let mut job_limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    job_limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if unsafe {
        SetInformationJobObject(
            job.raw(),
            JobObjectExtendedLimitInformation,
            (&job_limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            u32::try_from(std::mem::size_of_val(&job_limits)).map_err(|_| Error::Spawn)?,
        )
    } == 0
    {
        return Err(Error::Spawn);
    }

    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb =
        u32::try_from(std::mem::size_of::<STARTUPINFOEXW>()).map_err(|_| Error::Spawn)?;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = null_handle.raw();
    startup.StartupInfo.hStdOutput = write_pipe.raw();
    startup.StartupInfo.hStdError = null_handle.raw();
    startup.lpAttributeList = attribute_list.0;
    let mut process = PROCESS_INFORMATION::default();
    let created = unsafe {
        CreateProcessW(
            process_arena.application.as_ptr(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            process_arena.environment.as_ptr().cast(),
            process_arena.cwd.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        )
    };
    drop(attribute_list);
    if created == 0 {
        if write_pipe.close().is_err() {
            std::process::abort();
        }
        return Err(Error::Spawn);
    }
    let process_handle = CheckedHandle::new(process.hProcess);
    let thread_handle = CheckedHandle::new(process.hThread);
    if write_pipe.close().is_err() {
        if terminate_unassigned(process_handle.raw()).is_err() {
            std::process::abort();
        }
        let mut failed = false;
        failed |= thread_handle.close().is_err();
        failed |= read_pipe.close().is_err();
        failed |= null_handle.close().is_err();
        failed |= process_handle.close().is_err();
        failed |= job.close().is_err();
        let _ = failed;
        std::process::abort();
    }

    fn must_terminate_unassigned(process: HANDLE) {
        if terminate_unassigned(process).is_err() {
            std::process::abort();
        }
    }

    fn must_settle_job(job: HANDLE, process: HANDLE, terminate: bool) {
        if settle_job(job, process, terminate).is_err() {
            std::process::abort();
        }
    }

    let image_matches = (|| {
        process_arena.application.clear();
        process_arena.application.resize(PROCESS_PATH_UNITS, 0);
        let mut image_len =
            u32::try_from(process_arena.application.len()).map_err(|_| Error::Spawn)?;
        if unsafe {
            QueryFullProcessImageNameW(
                process_handle.raw(),
                0,
                process_arena.application.as_mut_ptr(),
                &mut image_len,
            )
        } == 0
        {
            return Err(Error::Changed);
        }
        let image_len = usize::try_from(image_len).map_err(|_| Error::Spawn)?;
        if image_len == 0 || image_len.saturating_add(1) > PROCESS_PATH_UNITS {
            return Err(Error::OutputLimit);
        }
        process_arena.application.truncate(image_len);
        process_arena.application.push(0);
        let file_handle = unsafe {
            CreateFileW(
                process_arena.application.as_ptr(),
                REGULAR_READ_ACCESS,
                HELD_SHARE,
                std::ptr::null(),
                OPEN_EXISTING,
                NORMAL_FILE_FLAGS,
                std::ptr::null_mut(),
            )
        };
        if file_handle == INVALID_HANDLE_VALUE {
            return Err(Error::Changed);
        }
        let file = unsafe { File::from_raw_handle(file_handle.cast()) };
        let identity = information(&file)?;
        let bytes = digest(&file, identity.length)?;
        recheck_held_regular(&executable.file)?;
        recheck_directory(cwd)?;
        Ok(!injected_settlement_failure!(WindowsImage)
            && identity == executable.file.identity
            && bytes == executable.file.digest)
    })();
    if image_matches != Ok(true) {
        must_terminate_unassigned(process_handle.raw());
        return Err(Error::Changed);
    }
    if injected_settlement_failure!(WindowsAssign)
        || unsafe { AssignProcessToJobObject(job.raw(), process_handle.raw()) } == 0
    {
        must_terminate_unassigned(process_handle.raw());
        return Err(Error::Changed);
    }
    if injected_settlement_failure!(WindowsResume)
        || unsafe { ResumeThread(thread_handle.raw()) } == u32::MAX
    {
        must_settle_job(job.raw(), process_handle.raw(), true);
        return Err(Error::Spawn);
    }
    if thread_handle.close().is_err() {
        must_settle_job(job.raw(), process_handle.raw(), true);
        std::process::abort();
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let output_is_prepared = prepared_output.is_some();
    let mut output = prepared_output.unwrap_or_default();
    if output_is_prepared && (output.capacity() != stdout_limit || !output.is_empty()) {
        must_settle_job(job.raw(), process_handle.raw(), true);
        return Err(Error::OutputLimit);
    }
    let mut selected_error = None;
    loop {
        let mut available = 0_u32;
        if injected_settlement_failure!(WindowsPeek) {
            selected_error = Some(Error::Spawn);
            break;
        }
        if unsafe {
            PeekNamedPipe(
                read_pipe.raw(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        } == 0
        {
            let error = unsafe { GetLastError() };
            if !matches!(error, ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) {
                selected_error = Some(Error::Spawn);
                break;
            }
            available = 0;
        }
        while available != 0 {
            let count = usize::try_from(available).unwrap_or(usize::MAX).min(8192);
            if count > stdout_limit.saturating_sub(output.len()) {
                #[cfg(test)]
                test_remember_overflowing_stdout(read_pipe.raw(), count);
                selected_error = Some(Error::OutputLimit);
                break;
            }
            let mut buffer = [0_u8; 8192];
            let mut read = 0_u32;
            if injected_settlement_failure!(WindowsRead)
                || unsafe {
                    ReadFile(
                        read_pipe.raw(),
                        buffer.as_mut_ptr().cast(),
                        u32::try_from(count).map_err(|_| Error::Spawn)?,
                        &mut read,
                        std::ptr::null_mut(),
                    )
                } == 0
            {
                selected_error = Some(Error::Spawn);
                break;
            }
            let read = usize::try_from(read).map_err(|_| Error::Spawn)?;
            if read == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..read]);
            if output_is_prepared && output.capacity() != stdout_limit {
                selected_error = Some(Error::OutputLimit);
                break;
            }
            available = available.saturating_sub(u32::try_from(read).unwrap_or(u32::MAX));
        }
        if selected_error.is_some() {
            break;
        }
        match unsafe { WaitForSingleObject(process_handle.raw(), 0) } {
            WAIT_OBJECT_0 => break,
            WAIT_TIMEOUT => {}
            WAIT_FAILED => {
                selected_error = Some(Error::Spawn);
                break;
            }
            _ => {
                selected_error = Some(Error::Spawn);
                break;
            }
        }
        if std::time::Instant::now() >= deadline {
            selected_error = Some(Error::Spawn);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    if selected_error.is_none() {
        let mut exit_code = u32::MAX;
        if unsafe {
            windows_sys::Win32::System::Threading::GetExitCodeProcess(
                process_handle.raw(),
                &mut exit_code,
            )
        } == 0
        {
            selected_error = Some(Error::Spawn);
        } else if exit_code != 0 {
            selected_error = Some(Error::Exit);
        }
    }
    must_settle_job(job.raw(), process_handle.raw(), true);
    if let Some(error) = selected_error {
        must_close([read_pipe, null_handle, process_handle, job]);
        return Err(error);
    }
    let result = (|| {
        loop {
            let mut available = 0_u32;
            if unsafe {
                PeekNamedPipe(
                    read_pipe.raw(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                let error = unsafe { GetLastError() };
                if matches!(error, ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) {
                    break;
                }
                return Err(Error::Spawn);
            }
            if available == 0 {
                break;
            }
            let count = usize::try_from(available).unwrap_or(usize::MAX).min(8192);
            if count > stdout_limit.saturating_sub(output.len()) {
                #[cfg(test)]
                test_remember_overflowing_stdout(read_pipe.raw(), count);
                return Err(Error::OutputLimit);
            }
            let mut buffer = [0_u8; 8192];
            let mut read = 0_u32;
            if unsafe {
                ReadFile(
                    read_pipe.raw(),
                    buffer.as_mut_ptr().cast(),
                    u32::try_from(count).map_err(|_| Error::Spawn)?,
                    &mut read,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(Error::Spawn);
            }
            let read = usize::try_from(read).map_err(|_| Error::Spawn)?;
            if read == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..read]);
        }
        recheck_regular(&executable.file)?;
        recheck_directory(cwd)?;
        Ok(output)
    })();
    must_close([read_pipe, null_handle, process_handle, job]);
    result
}

fn terminate_unassigned(process: HANDLE) -> Result<(), Error> {
    let terminate_failed = unsafe { TerminateProcess(process, 126) } == 0;
    let wait = unsafe { WaitForSingleObject(process, 30_000) };
    if terminate_failed
        || wait != WAIT_OBJECT_0
        || injected_settlement_failure!(WindowsUnassigned)
        || injected_settlement_failure!(WindowsTerminateProcess)
        || injected_settlement_failure!(WindowsWaitUnassigned)
    {
        return Err(Error::Spawn);
    }
    Ok(())
}

fn settle_job(job: HANDLE, process: HANDLE, terminate: bool) -> Result<(), Error> {
    let terminate_failed = terminate
        && (unsafe { TerminateJobObject(job, 126) } == 0
            || injected_settlement_failure!(WindowsJob)
            || injected_settlement_failure!(WindowsTerminateJob));
    let leader_wait = unsafe { WaitForSingleObject(process, 30_000) };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        if injected_settlement_failure!(WindowsQueryJob)
            || unsafe {
                QueryInformationJobObject(
                    job,
                    JobObjectBasicAccountingInformation,
                    (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    u32::try_from(std::mem::size_of_val(&accounting)).map_err(|_| Error::Spawn)?,
                    std::ptr::null_mut(),
                )
            } == 0
        {
            return Err(Error::Spawn);
        }
        if accounting.ActiveProcesses == 0 {
            return if terminate_failed || leader_wait != WAIT_OBJECT_0 {
                Err(Error::Spawn)
            } else {
                Ok(())
            };
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Spawn);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

pub(super) fn wide_null(value: &OsStr) -> Result<Vec<u16>, Error> {
    let mut wide = value.encode_wide().collect::<Vec<_>>();
    if wide.is_empty() || wide.contains(&0) {
        return Err(Error::Invalid);
    }
    wide.push(0);
    Ok(wide)
}

pub(super) fn windows_command_line(arguments: &[String]) -> Result<Vec<u16>, Error> {
    let mut line = String::from("semaprax-native-rust-interop-tool");
    for argument in arguments {
        if argument.contains(['\0', '\r', '\n']) {
            return Err(Error::Invalid);
        }
        line.push(' ');
        let needs_quotes = argument.is_empty()
            || argument
                .chars()
                .any(|character| matches!(character, ' ' | '\t' | '"'));
        if !needs_quotes {
            line.push_str(argument);
            continue;
        }
        line.push('"');
        let mut slashes = 0_usize;
        for character in argument.chars() {
            if character == '\\' {
                slashes += 1;
            } else {
                if character == '"' {
                    line.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
                } else {
                    line.extend(std::iter::repeat_n('\\', slashes));
                }
                slashes = 0;
                line.push(character);
            }
        }
        line.extend(std::iter::repeat_n('\\', slashes * 2));
        line.push('"');
    }
    wide_null(OsStr::new(&line))
}

pub(super) fn preflight_windows_command_line(arguments: &[&[&str]]) -> Result<(), Error> {
    let mut units = "semaprax-native-rust-interop-tool"
        .encode_utf16()
        .count()
        .checked_add(1)
        .ok_or(Error::OutputLimit)?;
    for parts in arguments {
        units = units.checked_add(3).ok_or(Error::OutputLimit)?;
        let mut slashes = 0_usize;
        for character in parts.iter().flat_map(|part| part.chars()) {
            if matches!(character, '\0' | '\r' | '\n') {
                return Err(Error::Invalid);
            }
            if character == '\\' {
                slashes = slashes.checked_add(1).ok_or(Error::OutputLimit)?;
                continue;
            }
            let escaped_slashes = if character == '"' {
                slashes
                    .checked_mul(2)
                    .and_then(|count| count.checked_add(1))
                    .ok_or(Error::OutputLimit)?
            } else {
                slashes
            };
            units = units
                .checked_add(escaped_slashes)
                .and_then(|count| count.checked_add(character.len_utf16()))
                .ok_or(Error::OutputLimit)?;
            slashes = 0;
        }
        units = units
            .checked_add(slashes.checked_mul(2).ok_or(Error::OutputLimit)?)
            .ok_or(Error::OutputLimit)?;
    }
    if units > MAX_COMMAND_LINE_UNITS {
        return Err(Error::OutputLimit);
    }
    Ok(())
}
