//! Bounded outer observation. Reaping the collector does not settle escaped
//! descendants: the external provisioner must reconcile its entire cgroup.
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

pub struct Observation {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub(super) struct OwnedCollector(pub Child);

impl Drop for OwnedCollector {
    fn drop(&mut self) {
        if let Err(error) = stop(&mut self.0) {
            let message =
                format!("collector settlement uncertain: {error}; reconcile fixture cgroup");
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

pub fn run(request: &[u8], bundle: &[u8], surrogate: Option<&[u8]>) -> Observation {
    run_child(super::launch::spawn(request, bundle, surrogate))
}

pub(super) fn run_prepared(request: &std::fs::File, bundle: &std::fs::File) -> Observation {
    run_child(super::launch::spawn_prepared(request, bundle))
}

fn run_child(child: Child) -> Observation {
    let mut owned = OwnedCollector(child);
    // Guard stays armed across reads and assertion unwinding. try_wait caches
    // the exact status, so Drop never signals an already reaped numeric PID.
    collect(&mut owned.0).expect("bounded collector observation; reconcile cgroup on uncertainty")
}

pub(super) fn stop(child: &mut Child) -> io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    let _ = child.kill(); // Concurrent exit is resolved only by exact reap.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "collector reap deadline",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

// A missing stdout is permitted only when the sink fixture has deliberately
// closed its sole reader after observing an exact canonical report prefix.
pub(super) fn collect(child: &mut Child) -> io::Result<Observation> {
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take().unwrap();
    drop(child.stdin.take());
    for fd in stdout
        .iter()
        .map(AsRawFd::as_raw_fd)
        .chain([stderr.as_raw_fd()])
    {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let mut output = Vec::new();
    let mut errors = Vec::new();
    let mut eof = [stdout.is_none(), false];
    let mut status = None;
    // Preserve the real 60s collector budget, its 5s settlement budget and
    // report delivery. There is no production deadline override for tests.
    let deadline = Instant::now() + Duration::from_secs(75);
    loop {
        for (index, stream, bytes) in [
            (
                0,
                stdout.as_mut().map(|stream| stream as &mut dyn Read),
                &mut output,
            ),
            (1, Some(&mut stderr as &mut dyn Read), &mut errors),
        ] {
            if eof[index] {
                continue;
            }
            let stream = stream.expect("only deliberately closed stdout may be absent");
            let mut chunk = [0; 8192];
            match stream.read(&mut chunk) {
                Ok(0) => eof[index] = true,
                Ok(count) => {
                    if bytes.len() + count > 2 * 1024 * 1024 {
                        return Err(io::Error::other("collector fixture capture bound"));
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
        }
        if status.is_none() {
            status = child.try_wait()?;
        }
        if let (true, Some(status)) = (eof == [true; 2], status) {
            return Ok(Observation {
                status,
                stdout: output,
                stderr: errors,
            });
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "collector fixture deadline",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
