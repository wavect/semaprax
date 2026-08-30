//! Physical socket-syscall denial only, not whole-program no-network evidence.
use super::*;

#[cfg(all(
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn control() {
    let output = Command::new(fixture("socket-control"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"socket-control-ok\n");
    assert!(output.stderr.is_empty());
}

#[test]
#[cfg(all(
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn socket_guard_denies_direct_and_exec_descendant_calls_without_restricting_host() {
    control();
    for (name, expected) in [
        ("socket-guard", &b"socket-guard-ok\n"[..]),
        (
            "socket-descendant",
            &b"socket-child-ok\nsocket-parent-ok\n"[..],
        ),
    ] {
        let prepared = prepare(&fixture(name)).unwrap();
        assert_eq!(run(&prepared).unwrap(), expected);
        // A successful socket control after each guarded invocation proves the
        // test host did not accidentally acquire the child's irreversible filter.
        control();
    }
}

#[test]
#[cfg(all(
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn socket_guard_kernel_setup_failure_never_enters_executable() {
    assert_eq!(
        run(&prepare(&fixture("socket-guard")).unwrap()).unwrap(),
        b"socket-guard-ok\n"
    );
    let path = fixture("socket-setup-sentinel");
    let marker = path.with_extension("entered");
    let direct = || {
        let output = Command::new(&path).arg("--version").output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"socket-setup-executed\n");
        assert!(output.stderr.is_empty());
        let metadata = std::fs::symlink_metadata(&marker).unwrap();
        assert!(metadata.file_type().is_file());
        assert_eq!(std::fs::read(&marker).unwrap(), b"executed\n");
    };
    assert_eq!(
        std::fs::symlink_metadata(&marker).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    direct();
    // Only the exact observed plain marker is removed, never the fixture tree.
    std::fs::remove_file(&marker).unwrap();
    let mut prepared = prepare(&path).unwrap();
    prepared.fault = Some(Fault::SocketGuard);
    // Production installs a real zero-length sock_fprog for this test fault:
    // kernel rejection selects child exit126 before exec, not a mocked return.
    assert_eq!(run(&prepared), Err(ProbeError::Exit));
    assert_eq!(
        std::fs::symlink_metadata(&marker).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    direct();
    control();
}

#[test]
#[cfg(not(all(
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
fn unsupported_linux_abi_rejects_without_fixture_compilation_or_fork() {
    let mut prepared = prepare(Path::new("/semaprax-doctor-no-such-executable")).unwrap();
    // Spawn would be selected after launch preparation if admission regressed.
    prepared.fault = Some(Fault::Spawn);
    assert_eq!(run(&prepared), Err(ProbeError::Unsupported));
}
