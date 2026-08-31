//! Physical, opt-in worker fixtures. The driver is the trusted provisioner,
//! outside the worker's offline guarantee; it creates no namespace itself.
use super::{wire, ProbeError};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const SELECTOR: &str = "worker-fixture";
const VERSION: &[u8] = b"clang version 1.0.0\n";

fn worker() -> PathBuf {
    // An acknowledgement is NOT an attestation. The external provisioner also
    // owns a cgroup/resource boundary and cleanup on driver/worker uncertainty.
    assert_eq!(
        std::env::var("SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT").as_deref(),
        Ok("private-mapped-user-mount-clean-worker-cgroup-v1")
    );
    let path = PathBuf::from(std::env::var_os("SEMAPRAX_DOCTOR_WORKER").expect("provision worker"));
    assert!(path.is_absolute());
    assert!(std::fs::metadata(&path).unwrap().is_file());
    path
}

fn architecture() -> u8 {
    if cfg!(target_arch = "x86_64") {
        1
    } else {
        2
    }
}

fn request(bundle: &[u8], target: u8, selector: &str) -> Vec<u8> {
    let mut bytes = b"SPXDWK1\0".to_vec();
    bytes.extend_from_slice(&[1, architecture(), target, [4, 1, 2, 7][target as usize]]);
    bytes.extend_from_slice(&[0x37; 32]);
    bytes.extend_from_slice(&(bundle.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&Sha256::digest(bundle));
    bytes.push(u8::try_from(selector.len()).unwrap());
    bytes.extend_from_slice(selector.as_bytes());
    wire::Request::parse(&bytes).unwrap();
    bytes
}

fn bundle(elf: &[u8]) -> Vec<u8> {
    let mut bytes = b"SPXDOC1\0".to_vec();
    bytes.extend_from_slice(&[architecture(), 1]);
    bytes.extend_from_slice(&(SELECTOR.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(SELECTOR.as_bytes());
    bytes.extend_from_slice(&9u16.to_le_bytes());
    bytes.extend_from_slice(&[1, 0]);
    bytes.extend_from_slice(&(elf.len() as u64).to_le_bytes());
    bytes.extend_from_slice(b"bin/clang");
    bytes.extend_from_slice(elf);
    bytes
}

fn sealed(bytes: &[u8]) -> File {
    // SAFETY: fresh anonymous descriptor, owned solely by this provisioner.
    let fd = unsafe {
        libc::memfd_create(
            c"worker-test".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    assert!(fd >= 0, "{}", io::Error::last_os_error());
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes).unwrap();
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, 15) }, 0);
    file
}

fn high_duplicate(file: &File) -> File {
    let fd = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 64) };
    assert!(fd >= 64, "{}", io::Error::last_os_error());
    unsafe { File::from_raw_fd(fd) }
}

fn stop(child: &mut Child) {
    if child.try_wait().expect("observe owned worker").is_some() {
        return;
    }
    // A concurrent exit may make kill fail. Only exact reap resolves it.
    let kill = child.kill();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().expect("reap owned worker").is_some() {
            return;
        }
        assert!(Instant::now() < deadline,
            "worker settlement uncertain ({kill:?}); external cgroup provisioner must terminate the entire fixture");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn collect(child: &mut Child) -> io::Result<(ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    drop(child.stdin.take());
    let mut output = Vec::new();
    let mut errors = Vec::new();
    let mut eof = [false; 2];
    let mut status = None;
    // Three sequential 10s calls, settlement and bounded reply fit this outer
    // observation budget. No read/wait operation below blocks on child output.
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        for (index, stream, bytes) in [
            (0, &mut stdout as &mut dyn Read, &mut output),
            (1, &mut stderr as &mut dyn Read, &mut errors),
        ] {
            if eof[index] {
                continue;
            }
            let mut chunk = [0; 8192];
            match stream.read(&mut chunk) {
                Ok(0) => eof[index] = true,
                Ok(count) => {
                    if bytes.len() + count > wire::MAX_REPLY_BYTES {
                        return Err(io::Error::other("worker capture exceeded fixed bound"));
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
            return Ok((status, output, errors));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "worker fixture deadline",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn run(request: &[u8], bundle: &[u8]) -> (ExitStatus, Vec<u8>, Vec<u8>) {
    let executable = worker();
    let request = high_duplicate(&sealed(request));
    let bundle = high_duplicate(&sealed(bundle));
    let request_fd = request.as_raw_fd();
    let bundle_fd = bundle.as_raw_fd();
    let mut command = Command::new(executable);
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: only async-signal-safe operations on this new child's private
    // table. High source descriptors cannot collide with destinations 3/4.
    // CLOEXEC preserves Command's exec-error handshake until successful exec.
    // Descriptor flush/startup effects are the external provisioner's burden,
    // NOT the offline worker's guarantee.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(request_fd, 3) != 3 || libc::dup2(bundle_fd, 4) != 4 {
                return Err(io::Error::last_os_error());
            }
            if libc::syscall(
                libc::SYS_close_range,
                5u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .expect("start the explicitly provisioned worker");
    let output = collect(&mut child);
    if output.is_err() {
        stop(&mut child);
    }
    output.expect("bounded worker observation; external provisioner owns any escaped descendants")
}

/// Literal native ET_EXEC with one RX PT_LOAD, no interpreter or libraries.
/// These synthetic executables prove no real Clang/Node/Rust compatibility.
fn executable(payload: &[u8], socket: bool, spin: bool) -> Vec<u8> {
    let code = machine_code(payload.len(), socket, spin);
    let mut elf = vec![0; 120];
    elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    elf[16..18].copy_from_slice(&2u16.to_le_bytes());
    elf[18..20].copy_from_slice(&(if architecture() == 1 { 62u16 } else { 183u16 }).to_le_bytes());
    elf[20..24].copy_from_slice(&1u32.to_le_bytes());
    elf[24..32].copy_from_slice(&(0x0040_0000_u64 + 120).to_le_bytes());
    elf[32..40].copy_from_slice(&64u64.to_le_bytes());
    elf[52..54].copy_from_slice(&64u16.to_le_bytes());
    elf[54..56].copy_from_slice(&56u16.to_le_bytes());
    elf[56..58].copy_from_slice(&1u16.to_le_bytes());
    elf[64..68].copy_from_slice(&1u32.to_le_bytes());
    elf[68..72].copy_from_slice(&5u32.to_le_bytes());
    elf[80..88].copy_from_slice(&0x0040_0000_u64.to_le_bytes());
    let length = (120 + code.len() + payload.len()) as u64;
    elf[96..104].copy_from_slice(&length.to_le_bytes());
    elf[104..112].copy_from_slice(&length.to_le_bytes());
    elf[112..120].copy_from_slice(&4096u64.to_le_bytes());
    elf.extend_from_slice(&code);
    elf.extend_from_slice(payload);
    elf
}

#[cfg(target_arch = "x86_64")]
fn machine_code(length: usize, socket: bool, spin: bool) -> Vec<u8> {
    if spin {
        return vec![0xeb, 0xfe];
    } // jmp self
    let mut code = Vec::new();
    let mut rejection = None;
    if socket {
        // socket(AF_INET, SOCK_STREAM, 0); require raw -EPERM, else exit 7.
        code.extend_from_slice(&[
            0xb8, 41, 0, 0, 0, 0xbf, 2, 0, 0, 0, 0xbe, 1, 0, 0, 0, 0x31, 0xd2, 0x0f, 0x05, 0x48,
            0x83, 0xf8, 0xff, 0x75, 0,
        ]);
        rejection = Some(code.len() - 1);
    }
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0x35]);
    let address = code.len();
    code.extend_from_slice(&0i32.to_le_bytes());
    code.push(0xba);
    code.extend_from_slice(&(length as u32).to_le_bytes());
    code.extend_from_slice(&[0x0f, 0x05, 0x48, 0x3d]); // syscall; cmp rax, length
    code.extend_from_slice(&(length as u32).to_le_bytes());
    code.extend_from_slice(&[0x75, 12]); // jne failure
    code.extend_from_slice(&[0xbf, 0, 0, 0, 0, 0xb8, 60, 0, 0, 0, 0x0f, 0x05]);
    let failure = code.len();
    code.extend_from_slice(&[0xbf, 7, 0, 0, 0, 0xb8, 60, 0, 0, 0, 0x0f, 0x05]);
    let displacement = i32::try_from(code.len() - address - 4).unwrap();
    code[address..address + 4].copy_from_slice(&displacement.to_le_bytes());
    if let Some(offset) = rejection {
        code[offset] = u8::try_from(failure - offset - 1).unwrap();
    }
    code
}

#[cfg(target_arch = "aarch64")]
fn machine_code(length: usize, socket: bool, spin: bool) -> Vec<u8> {
    if spin {
        return 0x1400_0000u32.to_le_bytes().to_vec();
    } // b self
    let mov = |register: u32, value: u32| 0xd280_0000 | (value << 5) | register;
    let mut words = Vec::new();
    let mut rejection = None;
    if socket {
        // socket(AF_INET, SOCK_STREAM, 0); cmn x0,1; b.ne failure.
        words.extend([
            mov(0, 2),
            mov(1, 1),
            mov(2, 0),
            mov(8, 198),
            0xd400_0001,
            0xb100_041f,
            0,
        ]);
        rejection = Some(words.len() - 1);
    }
    words.push(mov(0, 1));
    let address = words.len();
    words.push(0); // adr x1,payload, patched below
    words.extend([
        mov(2, length as u32 & 0xffff),
        0xf2a0_0002 | (((length as u32 >> 16) & 0xffff) << 5),
        mov(8, 64),
        0xd400_0001,
        0xeb02_001f,
        0x5400_0081,
    ]);
    words.extend([mov(0, 0), mov(8, 93), 0xd400_0001]);
    let failure = words.len();
    words.extend([mov(0, 7), mov(8, 93), 0xd400_0001]);
    let offset = ((words.len() - address) * 4) as u32;
    words[address] = 0x1000_0001 | ((offset & 3) << 29) | ((offset >> 2) << 5);
    if let Some(index) = rejection {
        words[index] = 0x5400_0001 | (((failure - index) as u32) << 5);
    }
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

#[test]
#[ignore = "requires provisioned worker/private mapped namespaces/cgroup; run serially"]
fn provisioned_materializer_exec_and_socket_denial() {
    for socket in [false, true] {
        let bundle = bundle(&executable(VERSION, socket, false));
        let request = request(&bundle, 1, SELECTOR);
        let (status, output, errors) = run(&request, &bundle);
        assert!(status.success(), "worker status {status:?}");
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = wire::Request::parse(&request).unwrap();
        assert_eq!(
            wire::validate_reply(&parsed, &output).unwrap(),
            vec![(1, Ok(VERSION.to_vec()))]
        );
        assert_eq!(&output[..8], b"SPXDWR1\0");
        assert_eq!(&output[8..40], Sha256::digest(&request).as_slice());
        assert_eq!(&output[40..72], &[0x37; 32]);
        assert_eq!(
            &output[72..83],
            &[1, architecture(), 1, 1, 1, 1, 0, 20, 0, 0, 0]
        );
        assert_eq!(&output[83..], VERSION);
    }
}

#[test]
#[ignore = "requires provisioned worker/private mapped namespaces/cgroup; run serially"]
fn provisioned_overflow_and_timeout_publish_only_settled_failure() {
    for (payload, spin, expected) in [
        (vec![b'x'; 65_536], false, Ok(vec![b'x'; 65_536])),
        (vec![b'x'; 65_537], false, Err(ProbeError::OutputLimit)),
        (Vec::new(), true, Err(ProbeError::Timeout)),
    ] {
        let bundle = bundle(&executable(&payload, false, spin));
        let request = request(&bundle, 1, SELECTOR);
        let (status, output, errors) = run(&request, &bundle);
        assert!(status.success());
        assert!(errors.is_empty());
        let expected_size = 83 + expected.as_ref().map_or(0, Vec::len);
        assert_eq!(
            wire::validate_reply(&wire::Request::parse(&request).unwrap(), &output).unwrap(),
            vec![(1, expected)]
        );
        assert_eq!(output.len(), expected_size);
    }
}

#[test]
#[ignore = "requires provisioned worker/private mapped namespaces/cgroup; run serially"]
fn provisioned_missing_role_bad_hash_and_invalid_request_emit_no_frame() {
    let bundle = bundle(&executable(VERSION, false, false));
    let valid = request(&bundle, 1, SELECTOR);
    let mut bad_hash = valid.clone();
    bad_hash[52] ^= 1;
    let mut malformed = valid.clone();
    malformed.push(0);
    for request in [request(&bundle, 3, SELECTOR), bad_hash, malformed] {
        let (status, output, errors) = run(&request, &bundle);
        assert_eq!(status.code(), Some(2));
        assert!(output.is_empty(), "invalid admission emitted report bytes");
        assert!(errors.is_empty());
    }
}

#[test]
#[ignore = "requires separately provisioned real-tool bundle plus private worker context"]
fn provisioned_real_clang_node_rust_distributions() {
    let path = PathBuf::from(
        std::env::var_os("SEMAPRAX_DOCTOR_REAL_BUNDLE").expect("provision real bundle"),
    );
    assert!(path.is_absolute());
    let selector = std::env::var("SEMAPRAX_DOCTOR_REAL_SELECTOR").expect("provision real selector");
    let mut bundle = Vec::new();
    File::open(path)
        .unwrap()
        .take(512 * 1024 * 1024 + 1)
        .read_to_end(&mut bundle)
        .unwrap();
    assert!(!bundle.is_empty() && bundle.len() <= 512 * 1024 * 1024);
    let request = request(&bundle, 3, &selector);
    let (status, output, errors) = run(&request, &bundle);
    assert!(status.success());
    assert!(errors.is_empty());
    let rows = wire::validate_reply(&wire::Request::parse(&request).unwrap(), &output).unwrap();
    for ((role, value), expected) in rows.into_iter().zip([1, 2, 4]) {
        assert_eq!(role, expected);
        let bytes = value.expect("real selected tool must complete under confinement");
        assert!(!bytes.is_empty());
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(match role {
            1 => text.contains("clang version"),
            2 => text.starts_with('v'),
            4 => text.starts_with("rustc "),
            _ => false,
        });
    }
}
