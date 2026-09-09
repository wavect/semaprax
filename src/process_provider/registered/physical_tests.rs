//! Local physical provider evidence; these tests grant only their held fixture tool.
use super::*;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn fixture() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = std::env::temp_dir().join(format!("spx-held-process-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("fixture.c");
        std::fs::write(&source, r#"
#include <unistd.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <fcntl.h>
#include <stdio.h>
extern char **environ;
int main(int argc, char **argv) {
    if (argc < 2 || strcmp(argv[0], "registered-fixture")) return 90;
    if (!strcmp(argv[1], "inspect")) {
        if (!environ[0] || strcmp(environ[0], "ONLY=explicit") || environ[1]) return 91;
        for (int fd = 3; fd < 256; ++fd) if (fcntl(fd, F_GETFD) >= 0) return 92;
        char cwd[4096]; if (!getcwd(cwd, sizeof cwd)) return 93;
        write(1, "explicit", 8); write(2, "error", 5); return 23;
    }
    if (!strcmp(argv[1], "echo")) {
        unsigned char b[1024]; ssize_t n;
        while ((n = read(0, b, sizeof b)) > 0) {
            if (write(1, b, n) != n || write(2, b, n) != n) return 94;
        }
        return 0;
    }
    if (!strcmp(argv[1], "raw")) { write(1, argv[2], strlen(argv[2])); return 0; }
    if (!strcmp(argv[1], "signal")) { raise(SIGTERM); return 95; }
    if (!strcmp(argv[1], "timeout")) { for (;;) pause(); }
    if (!strcmp(argv[1], "overflow")) { char b[1024]; memset(b, 'x', sizeof b); for (;;) write(1, b, sizeof b); }
    if (!strcmp(argv[1], "child")) { if (fork() == 0) { for (;;) pause(); } return 0; }
    return 96;
}
"#).unwrap();
        let status = std::process::Command::new("/usr/bin/clang").args(["-std=c11", "-O2"])
            .arg(&source).arg("-o").arg(root.join("tool")).status().unwrap();
        assert!(status.success());
        root
    }).as_path()
}
fn provider(policy: fn(&[Vec<u8>]) -> bool) -> RegisteredProcessProvider {
    let root = fixture();
    let tool = HeldProcessTool::new(
        File::open(root.join("tool")).unwrap(),
        File::open(root).unwrap(),
        b"registered-fixture".to_vec(),
        vec![(b"ONLY".to_vec(), b"explicit".to_vec())],
        policy,
    )
    .unwrap();
    RegisteredProcessProvider::new([(7, tool)]).unwrap()
}
fn request(args: &[&[u8]], input: &[u8], timeout: u64, out: usize, err: usize) -> ProcessRequest {
    let mut wire = (args.len() as u32).to_le_bytes().to_vec();
    for arg in args {
        wire.extend_from_slice(&(arg.len() as u32).to_le_bytes());
        wire.extend_from_slice(arg);
    }
    ProcessRequest::from_wire(7, &wire, wire.len(), input, input.len(), timeout, out, err).unwrap()
}
#[test]
fn physical_registered_process_explicit_authority_and_nonzero_exit() {
    let mut host = provider(|args| args == [b"inspect".to_vec()]);
    let output = host.run(&request(&[b"inspect"], b"", 3000, 8, 5)).unwrap();
    assert_eq!(
        output.termination,
        super::super::ProcessTermination::Exited(23)
    );
    assert_eq!(output.stdout, b"explicit");
    assert_eq!(output.stderr, b"error");
    assert_eq!(
        host.run(&request(&[b"echo"], b"", 3000, 0, 0)),
        Err(ProcessFailure::AuthorityDenied)
    );
    host.settle().unwrap();
}
#[test]
fn physical_registered_process_simultaneous_pipes_raw_arguments_and_signal() {
    let mut host = provider(|_| true);
    let input = vec![0xab; 24_000];
    let output = host
        .run(&request(&[b"echo"], &input, 3000, input.len(), input.len()))
        .unwrap();
    assert_eq!(output.stdout, input);
    assert_eq!(output.stderr, input);
    let output = host
        .run(&request(&[b"raw", &[0xff, 0xfe]], b"", 3000, 2, 0))
        .unwrap();
    assert_eq!(output.stdout, [0xff, 0xfe]);
    let output = host.run(&request(&[b"signal"], b"", 3000, 0, 0)).unwrap();
    assert_eq!(
        output.termination,
        super::super::ProcessTermination::Signalled(libc::SIGTERM as u8)
    );
    host.settle().unwrap();
}
#[test]
fn physical_registered_process_timeout_overflow_and_recovery() {
    let mut host = provider(|_| true);
    assert_eq!(
        host.run(&request(&[b"timeout"], b"", 50, 0, 0)),
        Err(ProcessFailure::TimedOut)
    );
    host.settle().unwrap();
    assert_eq!(
        host.run(&request(&[b"overflow"], b"", 3000, 1024, 0)),
        Err(ProcessFailure::CapacityExceeded)
    );
    host.settle().unwrap();
    assert_eq!(
        host.run(&request(&[b"echo"], b"ok", 3000, 2, 2))
            .unwrap()
            .stdout,
        b"ok"
    );
}
#[test]
fn physical_registered_process_invalid_executable_is_launch_failure() {
    let root = fixture();
    let tool = HeldProcessTool::new(
        File::open(root.join("fixture.c")).unwrap(),
        File::open(root).unwrap(),
        b"registered-fixture".to_vec(),
        Vec::new(),
        |_| true,
    )
    .unwrap();
    let mut host = RegisteredProcessProvider::new([(7, tool)]).unwrap();
    assert_eq!(
        host.run(&request(&[b"echo"], b"", 3000, 0, 0)),
        Err(ProcessFailure::LaunchFailed)
    );
    host.settle().unwrap();
}

#[test]
fn physical_registered_process_uses_held_executable_after_path_replacement() {
    let root = fixture();
    let executable_path = root.join("replaceable-tool");
    let renamed_path = root.join("held-tool");
    std::fs::copy(root.join("tool"), &executable_path).unwrap();
    let executable = File::open(&executable_path).unwrap();
    let tool = HeldProcessTool::new(
        executable,
        File::open(root).unwrap(),
        b"registered-fixture".to_vec(),
        Vec::new(),
        |_| true,
    )
    .unwrap();
    std::fs::rename(&executable_path, &renamed_path).unwrap();
    std::fs::write(&executable_path, b"not the registered executable").unwrap();
    let mut host = RegisteredProcessProvider::new([(7, tool)]).unwrap();
    let output = host.run(&request(&[b"echo"], b"held", 3000, 4, 4)).unwrap();
    assert_eq!(output.stdout, b"held");
    host.settle().unwrap();
    std::fs::remove_file(executable_path).unwrap();
    std::fs::remove_file(renamed_path).unwrap();
}
