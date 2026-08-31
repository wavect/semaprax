//! Deliberately invalid handoff. The sentinel is an owned sibling, never an
//! arbitrary host process; survival detects lethal/stopping effects, not every
//! possible signaling syscall (for example signal zero).
use super::{fixture, launch, observe, sentinel_elf};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Sentinel {
    child: Child,
    pidfd: Option<File>,
}

impl Drop for Sentinel {
    fn drop(&mut self) {
        let result = self.settle();
        if let Err(error) = result {
            if std::thread::panicking() {
                eprintln!("sentinel settlement uncertain: {error}; reconcile cgroup");
            } else {
                panic!("sentinel settlement uncertain: {error}; reconcile cgroup");
            }
        }
    }
}

impl Sentinel {
    fn settle(&mut self) -> io::Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        // Until pidfd acquisition the exact unreaped Child remains owned. Its
        // ordinary kill/reap is the bounded fallback; no foreign reaper exists.
        if let Some(pidfd) = &self.pidfd {
            let signal = unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    pidfd.as_raw_fd(),
                    libc::SIGKILL,
                    std::ptr::null::<libc::siginfo_t>(),
                    0u32,
                )
            };
            if signal != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if self.child.try_wait()?.is_some() {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "sentinel exact reap",
                    ));
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        observe::stop(&mut self.child)
    }
}

fn sentinel() -> Sentinel {
    let path = launch::provisioned_path("SEMAPRAX_DOCTOR_WORKER");
    let image = launch::sealed(&sentinel_elf::executable(), true);
    let image_fd = image.as_raw_fd();
    let parent = std::process::id() as libc::pid_t;
    let mut command = Command::new(path);
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: all child work is allocation-free raw setup/exec; the literal
    // image cannot fork. Standard pipes are the sentinel's only interface.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                || libc::getppid() != parent
                || libc::syscall(
                    libc::SYS_close_range,
                    3u32,
                    u32::MAX,
                    libc::CLOSE_RANGE_CLOEXEC,
                ) != 0
            {
                return Err(io::Error::last_os_error());
            }
            let argv = [c"sentinel".as_ptr(), std::ptr::null()];
            let environment: [*const libc::c_char; 1] = [std::ptr::null()];
            libc::syscall(
                libc::SYS_execveat,
                image_fd,
                c"".as_ptr(),
                argv.as_ptr(),
                environment.as_ptr(),
                libc::AT_EMPTY_PATH,
            );
            Err(io::Error::last_os_error())
        });
    }
    // Arm cleanup immediately, before any fallible acquisition or assertion.
    let mut owned = Sentinel {
        child: command.spawn().expect("start owned sentinel"),
        pidfd: None,
    };
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, owned.child.id() as libc::pid_t, 0u32) };
    assert!(fd >= 0, "{}", io::Error::last_os_error());
    owned.pidfd = Some(unsafe { File::from_raw_fd(fd as i32) });
    let stdout = owned.child.stdout.as_mut().unwrap();
    let flags = unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
    require_bytes(stdout, b"ready\n");
    assert!(owned.child.try_wait().unwrap().is_none());
    owned
}

fn require_bytes(reader: &mut impl Read, expected: &[u8]) {
    let mut bytes = vec![0; expected.len()];
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut offset = 0;
    while offset != bytes.len() {
        match reader.read(&mut bytes[offset..]) {
            Ok(0) => panic!("sentinel EOF before required response"),
            Ok(count) => offset += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("sentinel response: {error}"),
        }
        assert!(Instant::now() < deadline, "sentinel response deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(bytes, expected);
}

fn finish_sentinel(owned: &mut Sentinel) {
    owned
        .child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&[42])
        .unwrap();
    let observation =
        observe::collect(&mut owned.child).expect("bounded sentinel response and exit");
    assert_eq!(observation.status.code(), Some(0));
    assert_eq!(observation.stdout, b"done\n");
    assert!(observation.stderr.is_empty());
}

fn nonchild_collector(request: &[u8], bundle: &[u8], sentinel: &File) -> Child {
    let path = launch::provisioned_path("SEMAPRAX_DOCTOR_COLLECTOR");
    let request = launch::sealed(request, false);
    let bundle = launch::sealed(bundle, false);
    let sentinel = launch::high(sentinel.as_raw_fd());
    let sources = [
        request.as_raw_fd(),
        bundle.as_raw_fd(),
        sentinel.as_raw_fd(),
    ];
    let _reservations = launch::reserve_destinations(request.as_raw_fd());
    let mut command = Command::new(path);
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: intentionally invalid pidfd handoff, with no actual worker to
    // orphan. Capture endpoints exist only in this provisioner child. Sources
    // are >=64; reservations keep std's exec-error pipe outside destinations.
    unsafe {
        command.pre_exec(move || {
            let reply = launch::high_pipe()?;
            let error = launch::high_pipe()?;
            if libc::close(reply[1]) != 0 || libc::close(error[1]) != 0 {
                return Err(io::Error::last_os_error());
            }
            for (destination, source) in [
                (3, sources[0]),
                (4, sources[1]),
                (5, sources[2]),
                (6, reply[0]),
                (7, error[0]),
            ] {
                if libc::dup2(source, destination) != destination {
                    return Err(io::Error::last_os_error());
                }
            }
            if libc::syscall(
                libc::SYS_close_range,
                8u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command
        .spawn()
        .expect("start deliberately nonchild collector handoff")
}

#[test]
#[ignore = "requires provisioned context, owned sibling sentinel and executable memfd support"]
fn nonchild_pidfd_rejects_without_killing_or_stopping_the_owned_sentinel() {
    assert_eq!(
        std::env::var("SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT").as_deref(),
        Ok("private-mapped-user-mount-clean-worker-cgroup-v1")
    );
    // Calibrate the exact ready/challenge/done protocol independently first.
    finish_sentinel(&mut sentinel());
    let bundle = fixture::bundle();
    super::healthy(observe::run(&fixture::request(&bundle), &bundle, None));
    let mut owned = sentinel();
    let mut collector = observe::OwnedCollector(nonchild_collector(
        &fixture::request(&bundle),
        &bundle,
        owned.pidfd.as_ref().unwrap(),
    ));
    super::rejected(observe::collect(&mut collector.0).expect("bounded nonchild rejection"));
    // No challenge was released before collector termination. A pending fatal
    // signal or stopped sentinel cannot now perform this successful response.
    finish_sentinel(&mut owned);
}
