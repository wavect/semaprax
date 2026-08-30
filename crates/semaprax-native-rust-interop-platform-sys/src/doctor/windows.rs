//! Doctor-only Windows process lifetime. Never used by the build runner.
//! A suspended leader enters a non-breakaway job before any user code runs.

mod launch;

use super::{Fault, Prepared, ProbeError};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED, HANDLE, WAIT_OBJECT_0,
    WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::ReadFile;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, JobObjectBasicAccountingInformation, QueryInformationJobObject,
    TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
};
use windows_sys::Win32::System::Pipes::PeekNamedPipe;
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, ResumeThread, TerminateProcess, WaitForSingleObject,
};

struct Handle(Option<HANDLE>);

impl Handle {
    fn new(raw: HANDLE) -> Self {
        Self(Some(raw))
    }

    fn raw(&self) -> HANDLE {
        self.0.expect("doctor handle is owned")
    }

    fn try_close(mut self) -> bool {
        let raw = self.0.take().expect("doctor handle closes once");
        // SAFETY: exclusively owned live Win32 handle; removed before close.
        unsafe { CloseHandle(raw) != 0 }
    }

    fn close(self, probe: &Prepared) {
        if !self.try_close() || probe.injected(Fault::Close) {
            std::process::abort();
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(raw) = self.0.take() {
            // SAFETY: this is the only remaining owner and close attempt.
            if unsafe { CloseHandle(raw) } == 0 {
                std::process::abort();
            }
        }
    }
}

struct Child<'a> {
    process: Handle,
    thread: Handle,
    job: Handle,
    probe: &'a Prepared,
    assigned: bool,
    settled: bool,
}

impl Child<'_> {
    fn settle(&mut self) -> Instant {
        let deadline = Instant::now()
            .checked_add(self.probe.limits.settle)
            .unwrap_or_else(|| std::process::abort());
        // Even an injected failure performs the exact owned termination first.
        // No numeric PID, foreign process, or process outside this job is used.
        let killed = unsafe {
            if self.assigned {
                TerminateJobObject(self.job.raw(), 126)
            } else {
                TerminateProcess(self.process.raw(), 126)
            }
        } != 0;
        loop {
            // SAFETY: held process and job handles outlive all observations.
            let wait = unsafe { WaitForSingleObject(self.process.raw(), 0) };
            let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            let queried = !self.assigned
                || unsafe {
                    QueryInformationJobObject(
                        self.job.raw(),
                        JobObjectBasicAccountingInformation,
                        (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                        std::mem::size_of_val(&accounting) as u32,
                        std::ptr::null_mut(),
                    )
                } != 0;
            if !queried || !matches!(wait, WAIT_OBJECT_0 | WAIT_TIMEOUT) {
                std::process::abort();
            }
            if wait == WAIT_OBJECT_0 && (!self.assigned || accounting.ActiveProcesses == 0) {
                if !killed || self.probe.injected(Fault::Kill) || self.probe.injected(Fault::Settle)
                {
                    std::process::abort();
                }
                self.settled = true;
                return deadline;
            }
            if Instant::now() >= deadline {
                std::process::abort();
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

impl Drop for Child<'_> {
    fn drop(&mut self) {
        if !self.settled {
            self.settle();
        }
    }
}

pub(super) fn run(probe: &Prepared) -> Result<Vec<u8>, ProbeError> {
    let deadline = Instant::now()
        .checked_add(probe.limits.run)
        .unwrap_or_else(|| std::process::abort());
    // Allocate before any child can exist; later appends cannot grow this buffer.
    let mut output = Vec::with_capacity(probe.limits.output);
    let (mut child, stdout, stderr, null_input, stdout_writer, stderr_writer) =
        launch::spawn(probe)?;
    let mut primary = None;
    if probe.injected(Fault::Assign)
        // SAFETY: the leader remains suspended, with both handles exclusively held.
        || unsafe { AssignProcessToJobObject(child.job.raw(), child.process.raw()) } == 0
    {
        primary = Some(ProbeError::Spawn);
    } else {
        child.assigned = true;
        if probe.injected(Fault::Resume)
            // SAFETY: this is the primary thread returned suspended by CreateProcessW.
            || unsafe { ResumeThread(child.thread.raw()) } != 1
        {
            primary = Some(ProbeError::Spawn);
        }
    }
    // The suspended failure route is settled before closing any child resources.
    if primary.is_some() {
        child.settle();
    }
    for handle in [stdout_writer, stderr_writer, null_input] {
        if !handle.try_close() {
            // These handles are independent of the held job/process handles.
            // Quiesce the exact child before fail-stop on uncertain pipe close.
            if !child.settled {
                child.settle();
            }
            std::process::abort();
        }
    }

    let mut charged = 0usize;
    let mut ended = [false; 2];
    if primary.is_none() {
        loop {
            if probe.injected(Fault::Deadline) || Instant::now() >= deadline {
                primary = Some(ProbeError::Timeout);
                break;
            }
            // One bounded chunk from each pipe per turn; a flood cannot starve
            // process/deadline observation or the other stream.
            for (index, pipe) in [&stdout, &stderr].into_iter().enumerate() {
                if !ended[index] {
                    match drain(pipe, index == 0, &mut output, &mut charged, probe) {
                        Ok(eof) => ended[index] = eof,
                        Err(error) => {
                            primary = Some(error);
                            break;
                        }
                    }
                }
            }
            if primary.is_some() {
                break;
            }
            if probe.injected(Fault::Wait) {
                primary = Some(ProbeError::Io);
                break;
            }
            // EOF is not process exit, and process exit is not descendant exit.
            match unsafe { WaitForSingleObject(child.process.raw(), 0) } {
                WAIT_OBJECT_0 => {
                    let mut code = u32::MAX;
                    if unsafe { GetExitCodeProcess(child.process.raw(), &mut code) } == 0 {
                        primary = Some(ProbeError::Io);
                    } else if code != 0 {
                        primary = Some(ProbeError::Exit);
                    }
                    break;
                }
                WAIT_TIMEOUT => {}
                _ => {
                    primary = Some(ProbeError::Io);
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let settle_deadline = if child.settled {
        Instant::now()
    } else {
        child.settle()
    };
    // Drain only after the full owned job is quiescent. An escaped external pipe
    // holder cannot extend this finite capture phase or become kill authority.
    while primary.is_none() && !ended.iter().all(|value| *value) {
        if Instant::now() >= settle_deadline {
            primary = Some(ProbeError::Io);
            break;
        }
        for (index, pipe) in [&stdout, &stderr].into_iter().enumerate() {
            if !ended[index] {
                match drain(pipe, index == 0, &mut output, &mut charged, probe) {
                    Ok(eof) => ended[index] = eof,
                    Err(error) => {
                        primary = Some(error);
                        break;
                    }
                }
            }
        }
    }
    stdout.close(probe);
    stderr.close(probe);
    // Child::Drop has no effects after proven settlement; its fields check close.
    drop(child);
    primary.map_or(Ok(output), Err)
}

fn drain(
    pipe: &Handle,
    retain: bool,
    output: &mut Vec<u8>,
    charged: &mut usize,
    probe: &Prepared,
) -> Result<bool, ProbeError> {
    if probe.injected(Fault::Read) {
        return Err(ProbeError::Io);
    }
    let mut available = 0u32;
    // SAFETY: parent-only read handle, live output pointer, no borrowed data buffer.
    if unsafe {
        PeekNamedPipe(
            pipe.raw(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return match unsafe { GetLastError() } {
            ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED => Ok(true),
            _ => Err(ProbeError::Io),
        };
    }
    let available = available as usize;
    if available > probe.limits.output.saturating_sub(*charged) {
        return Err(ProbeError::OutputLimit);
    }
    if available == 0 {
        return Ok(false);
    }
    let mut buffer = [0u8; 8192];
    let count = available.min(buffer.len());
    let mut read = 0u32;
    // SAFETY: sole reader; Peek reported at least count bytes; bounded live buffer.
    if unsafe {
        ReadFile(
            pipe.raw(),
            buffer.as_mut_ptr().cast(),
            count as u32,
            &mut read,
            std::ptr::null_mut(),
        )
    } == 0
        || read == 0
        || read as usize > count
    {
        return Err(ProbeError::Io);
    }
    *charged += read as usize;
    if retain {
        output.extend_from_slice(&buffer[..read as usize]);
    }
    Ok(false)
}
