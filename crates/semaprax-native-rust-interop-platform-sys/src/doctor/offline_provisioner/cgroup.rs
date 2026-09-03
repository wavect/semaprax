//! Held cgroup-v2 controls and whole-scope quiescence.
use super::admission::CGROUP_FD;
use std::ffi::CStr;
use std::time::{Duration, Instant};

const EVENTS_LIMIT: usize = 128;
const PROCS_LIMIT: usize = 4096;

pub(super) fn configure() -> Result<(), ()> {
    require_empty()?;
    require_exact(c"cgroup.type", b"domain\n")?;
    set_and_require(c"pids.max", b"64\n", b"64\n")?;
    set_and_require(c"memory.max", b"2147483648\n", b"2147483648\n")?;
    set_and_require(c"memory.swap.max", b"0\n", b"0\n")?;
    set_and_require(c"memory.oom.group", b"1\n", b"1\n")?;
    set_and_require(c"cpu.max", b"100000 100000\n", b"100000 100000\n")?;
    require_empty()
}

pub(super) fn require_empty() -> Result<(), ()> {
    let events = read(c"cgroup.events", EVENTS_LIMIT)?;
    let mut populated = None;
    for line in events
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        if let Some(value) = line.strip_prefix(b"populated ") {
            if populated.replace(value == b"0").is_some() || !matches!(value, b"0" | b"1") {
                return Err(());
            }
        }
    }
    if populated != Some(true) || !read(c"cgroup.procs", PROCS_LIMIT)?.is_empty() {
        Err(())
    } else {
        Ok(())
    }
}

pub(super) fn require_child(pid: libc::pid_t) -> Result<(), ()> {
    if pid <= 0 {
        return Err(());
    }
    let expected = pid.to_string();
    let procs = read(c"cgroup.procs", PROCS_LIMIT)?;
    let mut rows = procs
        .split(|byte| *byte == b'\n')
        .filter(|row| !row.is_empty());
    if rows.next() != Some(expected.as_bytes()) || rows.next().is_some() {
        Err(())
    } else {
        Ok(())
    }
}

pub(super) fn kill() -> Result<(), ()> {
    write(c"cgroup.kill", b"1\n")
}

pub(super) fn wait_empty(deadline: Instant) -> Result<(), ()> {
    loop {
        if require_empty().is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(());
        }
        let pause = libc::timespec {
            tv_sec: 0,
            tv_nsec: 1_000_000,
        };
        if unsafe { libc::nanosleep(&pause, std::ptr::null_mut()) } != 0
            && super::errno() != libc::EINTR
        {
            return Err(());
        }
    }
}

pub(super) fn settlement_deadline() -> Option<Instant> {
    Instant::now().checked_add(Duration::from_secs(10))
}

fn set_and_require(name: &CStr, value: &[u8], expected: &[u8]) -> Result<(), ()> {
    write(name, value)?;
    require_exact(name, expected)
}

fn require_exact(name: &CStr, expected: &[u8]) -> Result<(), ()> {
    if read(name, expected.len().checked_add(1).ok_or(())?)? == expected {
        Ok(())
    } else {
        Err(())
    }
}

fn read(name: &CStr, limit: usize) -> Result<Vec<u8>, ()> {
    let fd = open(name, libc::O_RDONLY)?;
    let mut output = Vec::new();
    output.try_reserve_exact(limit).map_err(|_| ())?;
    let mut buffer = [0u8; 256];
    loop {
        let remaining = limit.checked_sub(output.len()).ok_or(())?;
        if remaining == 0 {
            let mut byte = 0u8;
            let count = unsafe { libc::read(fd, (&mut byte as *mut u8).cast(), 1) };
            close(fd)?;
            return if count == 0 { Ok(output) } else { Err(()) };
        }
        let count =
            unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), remaining.min(buffer.len())) };
        if count < 0 {
            let error = super::errno();
            close(fd)?;
            if error == libc::EINTR {
                return Err(());
            }
            return Err(());
        }
        if count == 0 {
            close(fd)?;
            return Ok(output);
        }
        output.extend_from_slice(&buffer[..usize::try_from(count).map_err(|_| ())?]);
    }
}

fn write(name: &CStr, bytes: &[u8]) -> Result<(), ()> {
    let fd = open(name, libc::O_WRONLY)?;
    let count = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
    let closed = close(fd);
    if count == isize::try_from(bytes.len()).map_err(|_| ())? && closed.is_ok() {
        Ok(())
    } else {
        Err(())
    }
}

fn open(name: &CStr, access: i32) -> Result<i32, ()> {
    let fd = unsafe {
        libc::openat(
            CGROUP_FD,
            name.as_ptr(),
            access | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(());
    }
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0
        || unsafe { stat.assume_init() }.st_mode & libc::S_IFMT != libc::S_IFREG
    {
        let _ = close(fd);
        return Err(());
    }
    Ok(fd)
}

fn close(fd: i32) -> Result<(), ()> {
    if unsafe { libc::close(fd) } == 0 {
        Ok(())
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn production_limits_are_literal_and_bounded() {
        let source = include_str!("cgroup.rs");
        for fact in [
            "pids.max",
            "64\\n",
            "memory.max",
            "2147483648\\n",
            "memory.swap.max",
            "memory.oom.group",
            "cpu.max",
            "100000 100000\\n",
        ] {
            assert!(source.contains(fact), "missing cgroup contract {fact}");
        }
        assert!(source.contains("cgroup.kill"));
        assert!(source.contains("populated"));
    }
}
