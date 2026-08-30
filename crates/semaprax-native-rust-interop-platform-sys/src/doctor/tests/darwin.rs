//! Resource-limit mutation is confined to a fresh helper test process.
use super::*;

#[test]
fn descriptors_above_a_lowered_soft_limit_are_not_inherited() {
    let path = fixture("closed-high-fd");
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "doctor::tests::darwin::lowered_limit_subprocess",
            "--nocapture",
        ])
        .env("SEMAPRAX_DOCTOR_HIGH_FD_FIXTURE", path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("high-fd-rejected"));
}

#[test]
fn lowered_limit_subprocess() {
    let Some(path) = std::env::var_os("SEMAPRAX_DOCTOR_HIGH_FD_FIXTURE") else {
        return;
    };
    let prepared = prepare(Path::new(&path)).unwrap();
    let mut limit = std::mem::MaybeUninit::<libc::rlimit>::zeroed();
    assert_eq!(
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()) },
        0
    );
    let mut limit = unsafe { limit.assume_init() };
    // Provision within the subprocess's existing hard limit; do not increase it.
    assert!(limit.rlim_max > 512);
    if limit.rlim_cur <= 512 {
        limit.rlim_cur = 513;
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);
    }
    assert_eq!(unsafe { libc::fcntl(512, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
    let source = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY) };
    assert!(source >= 0 && source != 512);
    assert_eq!(unsafe { libc::dup2(source, 512) }, 512);
    assert_eq!(unsafe { libc::close(source) }, 0);
    assert_eq!(unsafe { libc::fcntl(512, libc::F_GETFD) }, 0);
    limit.rlim_cur = 64;
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);
    let result = run(&prepared);
    assert_eq!(unsafe { libc::close(512) }, 0);
    assert_eq!(result, Ok(b"rustc 1.88.0 (fixture)\n".to_vec()));
    println!("high-fd-rejected");
}
