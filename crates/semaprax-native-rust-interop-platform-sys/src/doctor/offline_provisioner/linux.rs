//! Dedicated supervisor: trust/admission, isolated clone, capture and settlement.
use super::{admission, cgroup};
use std::time::{Duration, Instant};

mod capture;
mod child;
mod lifetime;
use lifetime::Lifetime;

const HIGH_FD: i32 = 64;
const CLONE_CLEAR_SIGHAND: u64 = 1 << 32;
const CLONE_INTO_CGROUP: u64 = 1 << 33;

#[derive(Clone, Copy)]
pub(super) struct Prepared {
    pub(super) request: i32,
    pub(super) bundle: i32,
    pub(super) launcher: i32,
    pub(super) worker: i32,
    pub(super) collector: i32,
    pub(super) stdin: [i32; 2],
    pub(super) report: [i32; 2],
    pub(super) error: [i32; 2],
    pub(super) ready: [i32; 2],
    pub(super) parent: libc::pid_t,
    pub(super) uid_map: MapRow,
    pub(super) gid_map: MapRow,
}

#[derive(Clone, Copy)]
pub(super) struct MapRow {
    bytes: [u8; 32],
    length: usize,
}

impl MapRow {
    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

impl Prepared {
    pub(super) fn inventory(self) -> [i32; 13] {
        [
            self.request,
            self.bundle,
            self.launcher,
            self.worker,
            self.collector,
            self.stdin[0],
            self.stdin[1],
            self.report[0],
            self.report[1],
            self.error[0],
            self.error[1],
            self.ready[0],
            self.ready[1],
        ]
    }
}

#[repr(C)]
#[derive(Default)]
struct CloneArgs {
    flags: u64,
    pidfd: u64,
    child_tid: u64,
    parent_tid: u64,
    exit_signal: u64,
    stack: u64,
    stack_size: u64,
    tls: u64,
    set_tid: u64,
    set_tid_size: u64,
    cgroup: u64,
}

pub(super) fn entry() -> ! {
    if std::env::args_os().count() != 1 || std::env::vars_os().next().is_some() {
        stop();
    }
    let started = Instant::now();
    admission::validate().unwrap_or_else(|()| stop());
    let prepared = prepare().unwrap_or_else(|()| stop());
    // Finish the launch-inventory descriptor/allocation preparation before the
    // first cgroup control mutation. A partial limit write can then never be
    // followed by an attempt to grow that inventory.
    cgroup::configure().unwrap_or_else(|()| stop());
    let mut pidfd = -1i32;
    let arguments = CloneArgs {
        flags: libc::CLONE_PIDFD as u64
            | CLONE_CLEAR_SIGHAND
            | CLONE_INTO_CGROUP
            | libc::CLONE_NEWUSER as u64
            | libc::CLONE_NEWNS as u64
            | libc::CLONE_NEWNET as u64
            | libc::CLONE_NEWIPC as u64
            | libc::CLONE_NEWUTS as u64,
        pidfd: (&mut pidfd as *mut i32) as u64,
        exit_signal: libc::SIGCHLD as u64,
        cgroup: admission::CGROUP_FD as u64,
        ..CloneArgs::default()
    };
    let pid = unsafe {
        libc::syscall(
            libc::SYS_clone3,
            &arguments as *const CloneArgs,
            std::mem::size_of::<CloneArgs>(),
        )
    };
    if pid < 0 {
        stop();
    }
    if pid == 0 {
        child::enter(&prepared);
    }
    let pid = pid as libc::pid_t;
    let mut lifetime = Lifetime::new(pidfd, pid);
    if parent_handoff(&prepared, pid).is_err() {
        lifetime.abort();
    }
    close_parent_sources(&prepared).unwrap_or_else(|()| lifetime.abort());
    let deadline = started
        .checked_add(Duration::from_secs(120))
        .unwrap_or_else(|| lifetime.abort());
    let (report, status) = capture::collect(
        &mut lifetime,
        prepared.report[0],
        prepared.error[0],
        deadline,
    )
    .unwrap_or_else(|()| lifetime.abort());
    lifetime.complete().unwrap_or_else(|()| lifetime.abort());
    for fd in [0, 2, admission::CGROUP_FD] {
        close(fd).unwrap_or_else(|()| stop());
    }
    publish(&report, status)
}

fn prepare() -> Result<Prepared, ()> {
    let request = high(admission::REQUEST_FD)?;
    let bundle = high(admission::BUNDLE_FD)?;
    let launcher = high(admission::LAUNCHER_FD)?;
    let worker = high(admission::WORKER_FD)?;
    let collector = high(admission::COLLECTOR_FD)?;
    Ok(Prepared {
        request,
        bundle,
        launcher,
        worker,
        collector,
        stdin: pipe()?,
        report: pipe()?,
        error: pipe()?,
        ready: pipe()?,
        parent: unsafe { libc::getpid() },
        uid_map: map_row(unsafe { libc::geteuid() })?,
        gid_map: map_row(unsafe { libc::getegid() })?,
    })
}

fn parent_handoff(prepared: &Prepared, pid: libc::pid_t) -> Result<(), ()> {
    write_map(pid, c"setgroups", b"deny\n")?;
    write_map(pid, c"uid_map", prepared.uid_map.bytes())?;
    write_map(pid, c"gid_map", prepared.gid_map.bytes())?;
    cgroup::require_child(pid)?;
    if unsafe { libc::write(prepared.ready[1], b"1".as_ptr().cast(), 1) } != 1 {
        return Err(());
    }
    close(prepared.ready[1])
}

fn map_row(identity: u32) -> Result<MapRow, ()> {
    let mut digits = [0u8; 10];
    let count = decimal(&mut digits, identity)?;
    let mut bytes = [0u8; 32];
    bytes[..2].copy_from_slice(b"0 ");
    bytes[2..2 + count].copy_from_slice(&digits[..count]);
    bytes[2 + count..5 + count].copy_from_slice(b" 1\n");
    Ok(MapRow {
        bytes,
        length: 5 + count,
    })
}

fn write_map(pid: libc::pid_t, name: &std::ffi::CStr, bytes: &[u8]) -> Result<(), ()> {
    let mut path = [0u8; 64];
    decimal_path(&mut path, pid, name.to_bytes())?;
    // The unreaped clone and retained pidfd pin this numeric proc entry. fd10
    // was independently authenticated as procfs before clone.
    let fd = unsafe {
        libc::openat(
            admission::PROC_FD,
            path.as_ptr().cast(),
            libc::O_WRONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(());
    }
    let count = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
    let closed = close(fd);
    if count == isize::try_from(bytes.len()).map_err(|_| ())? && closed.is_ok() {
        Ok(())
    } else {
        Err(())
    }
}

fn decimal_path(buffer: &mut [u8; 64], pid: libc::pid_t, name: &[u8]) -> Result<usize, ()> {
    let pid = u32::try_from(pid).map_err(|_| ())?;
    let mut digits = [0u8; 10];
    let count = decimal(&mut digits, pid)?;
    let length = count
        .checked_add(1)
        .and_then(|value| value.checked_add(name.len()))
        .and_then(|value| value.checked_add(1))
        .filter(|length| *length <= buffer.len())
        .ok_or(())?;
    buffer[..count].copy_from_slice(&digits[..count]);
    buffer[count] = b'/';
    buffer[count + 1..count + 1 + name.len()].copy_from_slice(name);
    buffer[length - 1] = 0;
    Ok(length)
}

fn decimal(buffer: &mut [u8; 10], mut value: u32) -> Result<usize, ()> {
    let mut reverse = [0u8; 10];
    let mut count = 0usize;
    loop {
        let slot = reverse.get_mut(count).ok_or(())?;
        *slot = b'0' + (value % 10) as u8;
        count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for index in 0..count {
        buffer[index] = reverse[count - index - 1];
    }
    Ok(count)
}

fn close_parent_sources(prepared: &Prepared) -> Result<(), ()> {
    close(prepared.stdin[0])?;
    close(prepared.stdin[1])?;
    close(prepared.report[1])?;
    close(prepared.error[1])?;
    close(prepared.ready[0])?;
    for fd in [
        prepared.request,
        prepared.bundle,
        prepared.launcher,
        prepared.worker,
        prepared.collector,
    ] {
        close(fd)?;
    }
    for fd in 3..=8 {
        close(fd)?;
    }
    close(admission::PROC_FD)?;
    Ok(())
}

fn publish(report: &[u8], status: u8) -> ! {
    if report.is_empty()
        || report.len() > 2 * 1024 * 1024
        || report.last() != Some(&b'\n')
        || !matches!(status, 0 | 1)
    {
        stop();
    }
    let mut signals = std::mem::MaybeUninit::<libc::sigset_t>::zeroed();
    let signals = unsafe {
        let pointer = signals.as_mut_ptr();
        if libc::sigemptyset(pointer) != 0
            || libc::sigaddset(pointer, libc::SIGPIPE) != 0
            || libc::sigprocmask(libc::SIG_BLOCK, pointer, std::ptr::null_mut()) != 0
        {
            stop();
        }
        signals.assume_init()
    };
    let _signals = signals;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(5))
        .unwrap_or_else(|| stop());
    let flags = unsafe { libc::fcntl(1, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(1, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0 {
        stop();
    }
    let mut offset = 0usize;
    while offset < report.len() {
        let count =
            unsafe { libc::write(1, report[offset..].as_ptr().cast(), report.len() - offset) };
        if count > 0 {
            offset += usize::try_from(count).unwrap_or_else(|_| stop());
            continue;
        }
        if count < 0 && super::errno() == libc::EINTR {
            continue;
        }
        if count < 0 && super::errno() == libc::EAGAIN && Instant::now() < deadline {
            let mut poll = libc::pollfd {
                fd: 1,
                events: libc::POLLOUT,
                revents: 0,
            };
            let _ = unsafe { libc::poll(&mut poll, 1, 10) };
            continue;
        }
        stop();
    }
    if close(1).is_err() {
        stop();
    }
    unsafe { libc::_exit(status as i32) }
}

fn pipe() -> Result<[i32; 2], ()> {
    let mut pair = [-1; 2];
    if unsafe { libc::pipe2(pair.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(());
    }
    let high_pair = [high(pair[0])?, high(pair[1])?];
    close(pair[0])?;
    close(pair[1])?;
    Ok(high_pair)
}

fn high(fd: i32) -> Result<i32, ()> {
    let result = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, HIGH_FD) };
    if result < HIGH_FD {
        Err(())
    } else {
        Ok(result)
    }
}

pub(super) fn close(fd: i32) -> Result<(), ()> {
    if unsafe { libc::close(fd) } == 0 {
        Ok(())
    } else {
        Err(())
    }
}

fn stop() -> ! {
    unsafe { libc::_exit(126) }
}

#[cfg(test)]
mod tests {
    use super::decimal_path;

    #[test]
    fn child_proc_paths_are_stack_bounded_and_canonical() {
        let mut bytes = [0u8; 64];
        let length = decimal_path(&mut bytes, 12345, b"uid_map").unwrap();
        assert_eq!(&bytes[..length], b"12345/uid_map\0");
        assert!(decimal_path(&mut bytes, -1, b"uid_map").is_err());
    }
}
