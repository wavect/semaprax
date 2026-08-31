//! External provisioner observations: procfs is never exposed to the tool.
use super::{bundle, collect, executable, request, spawn, stop, SELECTOR};
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::process::Child;
use std::time::{Duration, Instant};

struct OwnedWorker(Child);
impl Drop for OwnedWorker {
    fn drop(&mut self) {
        // Panic/error paths still settle the directly owned supervisor. The
        // provisioner's cgroup remains responsible for uncertain descendants.
        stop(&mut self.0);
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::other(message)
}

fn bounded(path: &str, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(invalid("proc observation exceeds bound"));
    }
    Ok(bytes)
}

fn status(path: &str) -> io::Result<String> {
    String::from_utf8(bounded(path, 65_536)?).map_err(|_| invalid("non-UTF8 proc status"))
}

fn field<'a>(status: &'a str, name: &str) -> io::Result<&'a str> {
    let mut values = status.lines().filter_map(|line| line.strip_prefix(name));
    let value = values
        .next()
        .ok_or_else(|| invalid("missing proc status field"))?;
    if values.next().is_some() {
        return Err(invalid("duplicate proc status field"));
    }
    Ok(value.trim())
}

fn decimal(status: &str, name: &str) -> io::Result<u32> {
    field(status, name)?
        .parse()
        .map_err(|_| invalid("nondecimal proc field"))
}

fn pidfd(pid: u32) -> io::Result<OwnedFd> {
    let pid = i32::try_from(pid).map_err(|_| invalid("PID outside native range"))?;
    if pid <= 0 {
        return Err(invalid("nonpositive PID"));
    }
    let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0_u32) };
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(descriptor as i32) })
}

fn signal(descriptor: &OwnedFd, signal: i32) -> io::Result<()> {
    if unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            descriptor.as_raw_fd(),
            signal,
            std::ptr::null::<libc::siginfo_t>(),
            0_u32,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn exited(descriptor: &OwnedFd) -> io::Result<bool> {
    let mut event = libc::pollfd {
        fd: descriptor.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    match unsafe { libc::poll(&mut event, 1, 0) } {
        0 => Ok(false),
        1 if event.revents & libc::POLLIN != 0
            && event.revents & (libc::POLLERR | libc::POLLNVAL) == 0 =>
        {
            Ok(true)
        }
        _ => Err(invalid("uncertain pidfd observation")),
    }
}

fn pause_worker(descriptor: &OwnedFd, pid: u32, deadline: Instant) -> io::Result<()> {
    signal(descriptor, libc::SIGSTOP)?;
    loop {
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        // Consume only stop events. No WEXITED means this cannot reap the
        // supervisor; omitting WNOWAIT prevents stale events on later retries.
        if unsafe {
            libc::waitid(
                libc::P_PIDFD,
                descriptor.as_raw_fd() as libc::id_t,
                &mut info,
                libc::WSTOPPED | libc::WNOHANG,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        if unsafe { info.si_pid() } != 0 {
            return if unsafe { info.si_pid() } == pid as i32
                && info.si_code == libc::CLD_STOPPED
                && unsafe { info.si_status() } == libc::SIGSTOP
            {
                Ok(())
            } else {
                Err(invalid("unexpected supervisor stop event"))
            };
        }
        if exited(descriptor)? || Instant::now() >= deadline {
            return Err(invalid("supervisor did not stop within observation budget"));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn child_pid(worker: u32) -> io::Result<Option<u32>> {
    let bytes = bounded(&format!("/proc/{worker}/task/{worker}/children"), 4096)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid("non-UTF8 child list"))?;
    let mut children = text.split_ascii_whitespace();
    let Some(child) = children.next() else {
        return Ok(None);
    };
    if children.next().is_some() {
        return Err(invalid("unexpected extra worker child"));
    }
    let child = child
        .parse::<u32>()
        .map_err(|_| invalid("invalid worker child PID"))?;
    if child == 0 || child == worker {
        return Err(invalid("invalid worker child identity"));
    }
    Ok(Some(child))
}

fn inspect_child(worker: u32, child: u32, image: &[u8]) -> io::Result<Option<OwnedFd>> {
    // The stopped single-threaded parent and no-other-reaper contract pin this
    // child PID, including if it becomes a zombie while being inspected.
    let descriptor = pidfd(child)?;
    if exited(&descriptor)? {
        return Err(invalid("tool exited before live observation"));
    }
    // The setup child still has its worker image until exec. An exact byte
    // comparison, not /proc's display pathname, distinguishes executable entry.
    let mut bytes = Vec::new();
    File::open(format!("/proc/{child}/exe"))?
        .take(image.len() as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes != image {
        return Ok(None);
    }
    let snapshot = status(&format!("/proc/{child}/status"))?;
    if decimal(&snapshot, "Pid:")? != child || decimal(&snapshot, "PPid:")? != worker {
        return Err(invalid("proc observation disagrees with pinned child"));
    }
    let nested = field(&snapshot, "NSpid:")?
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    if nested.len() < 2 || nested.last().copied() != Some("1") {
        return Err(invalid("tool is not init in a nested PID namespace"));
    }
    for name in ["CapInh:", "CapPrm:", "CapEff:", "CapBnd:", "CapAmb:"] {
        let value = field(&snapshot, name)?;
        if value.len() != 16 || value.bytes().any(|byte| byte != b'0') {
            return Err(invalid("post-exec capability set is not empty"));
        }
    }
    if decimal(&snapshot, "NoNewPrivs:")? != 1 || decimal(&snapshot, "Seccomp:")? != 2 {
        return Err(invalid("post-exec no-new-privileges/filter state missing"));
    }
    if exited(&descriptor)? {
        return Err(invalid("tool was not live after observation"));
    }
    Ok(Some(descriptor))
}

fn driver_context() -> io::Result<()> {
    // Procfs must number processes in the driver's PID namespace. This is a
    // provisioned-host requirement, not a fallback to alternate proc mounts.
    let snapshot = status("/proc/self/status")?;
    let own_pid = std::process::id();
    let nested = field(&snapshot, "NSpid:")?
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    if decimal(&snapshot, "Pid:")? != own_pid
        || nested.len() != 1
        || nested[0].parse::<u32>().ok() != Some(own_pid)
    {
        return Err(invalid("procfs PID namespace does not match the driver"));
    }
    let mut subreaper = -1_i32;
    if unsafe {
        libc::prctl(
            libc::PR_GET_CHILD_SUBREAPER,
            &mut subreaper as *mut i32,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if subreaper != 0 {
        return Err(invalid("driver must not adopt orphaned worker children"));
    }
    Ok(())
}

fn observe_and_kill(worker: &mut Child, image: &[u8]) -> io::Result<()> {
    let supervisor = pidfd(worker.id())?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let tool = loop {
        if Instant::now() >= deadline {
            return Err(invalid("tool never reached executable entry"));
        }
        pause_worker(&supervisor, worker.id(), deadline)?;
        if let Some(pid) = child_pid(worker.id())? {
            if let Some(tool) = inspect_child(worker.id(), pid, image)? {
                break tool;
            }
        }
        signal(&supervisor, libc::SIGCONT)?;
        std::thread::sleep(Duration::from_millis(1));
    };
    // The tool is spinning after exec while its supervisor is stopped. No
    // numeric child PID is used again, and the driver never signals the tool.
    signal(&supervisor, libc::SIGKILL)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut reaped = false;
    loop {
        if !reaped {
            reaped = worker.try_wait()?.is_some();
        }
        if reaped && exited(&tool)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(invalid("parent-death settlement not observed"));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
#[ignore = "requires provisioned worker, same-namespace procfs, inspection rights and external cgroup"]
fn post_exec_capabilities_and_supervisor_death_are_observed_externally() {
    driver_context().expect("provision procfs namespace and non-subreaper driver context");
    let image = executable(&[], false, true);
    let bundle = bundle(&image);
    let request = request(&bundle, 1, SELECTOR);
    let mut worker = OwnedWorker(spawn(&request, &bundle));
    observe_and_kill(&mut worker.0, &image)
        .expect("observe actual post-exec state and parent death");
    let (status, output, errors) = collect(&mut worker.0).unwrap();
    assert!(!status.success());
    assert!(output.is_empty(), "worker emitted a premature reply");
    assert!(errors.is_empty());
}

#[test]
fn proc_status_fields_reject_missing_duplicate_and_malformed_values() {
    assert_eq!(decimal("Pid:\t17\nPPid:\t9\n", "Pid:").unwrap(), 17);
    assert!(field("PPid:\t9\n", "Pid:").is_err());
    assert!(field("Pid:\t17\nPid:\t17\n", "Pid:").is_err());
    assert!(decimal("Pid:\tnot-a-pid\n", "Pid:").is_err());
}
