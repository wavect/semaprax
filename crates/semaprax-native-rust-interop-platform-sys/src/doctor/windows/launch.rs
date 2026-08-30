//! Fixed command/environment and suspended Windows job launch for doctor.
use super::{Child, Fault, Handle, Prepared, ProbeError};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::{
    SetHandleInformation, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};
use windows_sys::Win32::System::JobObjects::{
    CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
    UpdateProcThreadAttribute, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

const MAX_WIDE: usize = 32767;

fn wide(value: &OsStr) -> Result<Vec<u16>, ProbeError> {
    let length = value.encode_wide().count();
    if length == 0 || length >= MAX_WIDE || value.encode_wide().any(|unit| unit == 0) {
        return Err(ProbeError::Invalid);
    }
    Ok(value.encode_wide().chain(Some(0)).collect())
}

fn environment(probe: &Prepared) -> Result<Vec<u16>, ProbeError> {
    // The common layer admits only fixed ASCII variable names. Windows requires
    // case-insensitive sorted names, with an extra NUL after the final row.
    let mut rows = probe.environment.iter().collect::<Vec<_>>();
    rows.sort_by_cached_key(|(key, _)| key.to_string_lossy().to_ascii_uppercase());
    let mut output = Vec::new();
    for (key, value) in rows {
        let key = wide(key)?;
        let value_len = value.encode_wide().count();
        if key.iter().any(|unit| *unit == u16::from(b'='))
            || value.encode_wide().any(|unit| unit == 0)
            || output
                .len()
                .checked_add(key.len())
                .and_then(|size| size.checked_add(value_len))
                .and_then(|size| size.checked_add(2))
                .is_none_or(|size| size > MAX_WIDE)
        {
            return Err(ProbeError::Invalid);
        }
        output.extend_from_slice(&key[..key.len() - 1]);
        output.push(u16::from(b'='));
        output.extend(value.encode_wide());
        output.push(0);
    }
    if output.is_empty() {
        output.push(0);
    }
    output.push(0);
    Ok(output)
}

fn pipe(security: &SECURITY_ATTRIBUTES) -> Result<(Handle, Handle), ProbeError> {
    let mut read = std::ptr::null_mut();
    let mut write = std::ptr::null_mut();
    // SAFETY: both writable outputs and the initialized security descriptor live.
    if unsafe { CreatePipe(&mut read, &mut write, security, 0) } == 0 {
        return Err(ProbeError::Spawn);
    }
    let read = Handle::new(read);
    let write = Handle::new(write);
    if unsafe { SetHandleInformation(read.raw(), HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(ProbeError::Spawn);
    }
    Ok((read, write))
}

struct Attributes(LPPROC_THREAD_ATTRIBUTE_LIST);
impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: initialized once; backing storage outlives this guard.
        unsafe { DeleteProcThreadAttributeList(self.0) };
    }
}

pub(super) fn spawn(
    probe: &Prepared,
) -> Result<(Child<'_>, Handle, Handle, Handle, Handle, Handle), ProbeError> {
    let application = wide(probe.path.as_os_str())?;
    let cwd = wide(probe.cwd.as_os_str())?;
    if application.contains(&u16::from(b'"')) || application.len() + 12 > MAX_WIDE {
        return Err(ProbeError::Invalid);
    }
    let mut command = Vec::with_capacity(application.len() + 12);
    command.push(u16::from(b'"'));
    command.extend_from_slice(&application[..application.len() - 1]);
    command.extend("\" --version\0".encode_utf16());
    let environment = environment(probe)?;
    let mut attribute_bytes = 0usize;
    // The sizing call is expected to fail, returning its required allocation.
    unsafe {
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes);
    }
    if attribute_bytes == 0 || attribute_bytes > 65536 {
        return Err(ProbeError::Spawn);
    }
    let words = attribute_bytes.div_ceil(std::mem::size_of::<usize>());
    let mut backing = vec![0usize; words];
    let pointer = backing.as_mut_ptr().cast();
    if unsafe { InitializeProcThreadAttributeList(pointer, 1, 0, &mut attribute_bytes) } == 0 {
        return Err(ProbeError::Spawn);
    }
    let attributes = Attributes(pointer);
    let security = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    let (stdout, stdout_writer) = pipe(&security)?;
    let (stderr, stderr_writer) = pipe(&security)?;
    let null = [u16::from(b'N'), u16::from(b'U'), u16::from(b'L'), 0];
    let raw_null = unsafe {
        CreateFileW(
            null.as_ptr(),
            FILE_GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &security,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if raw_null == INVALID_HANDLE_VALUE {
        return Err(ProbeError::Spawn);
    }
    let null_input = Handle::new(raw_null);
    let inherited = [null_input.raw(), stdout_writer.raw(), stderr_writer.raw()];
    if unsafe {
        UpdateProcThreadAttribute(
            attributes.0,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            inherited.as_ptr().cast(),
            std::mem::size_of_val(&inherited),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    } == 0
    {
        return Err(ProbeError::Spawn);
    }
    let raw_job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw_job.is_null() {
        return Err(ProbeError::Spawn);
    }
    let job = Handle::new(raw_job);
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if unsafe {
        SetInformationJobObject(
            job.raw(),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
    } == 0
    {
        return Err(ProbeError::Spawn);
    }
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = null_input.raw();
    startup.StartupInfo.hStdOutput = stdout_writer.raw();
    startup.StartupInfo.hStdError = stderr_writer.raw();
    startup.lpAttributeList = attributes.0;
    if probe.injected(Fault::Spawn) {
        return Err(ProbeError::Spawn);
    }
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: fixed argv, bounded NUL-terminated strings and environment, live
    // attribute list and handle inventory, initialized output; no user code runs.
    if unsafe {
        CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
            environment.as_ptr().cast(),
            cwd.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        )
    } == 0
    {
        return Err(ProbeError::Spawn);
    }
    let child = Child {
        process: Handle::new(process.hProcess),
        thread: Handle::new(process.hThread),
        job,
        probe,
        assigned: false,
        settled: false,
    };
    drop(attributes);
    Ok((
        child,
        stdout,
        stderr,
        null_input,
        stdout_writer,
        stderr_writer,
    ))
}
