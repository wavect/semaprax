//! Authored physical process-boundary evidence, not a network-sandbox gate.
use super::*;
use std::process::Command;
use std::sync::OnceLock;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;

static FIXTURE: OnceLock<PathBuf> = OnceLock::new();

fn fixture(name: &str) -> PathBuf {
    let root = FIXTURE.get_or_init(|| {
        let root = std::env::temp_dir().join(format!(
            "semaprax-doctor-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("fixture.rs");
        std::fs::write(&source, include_str!("fixture.rs")).unwrap();
        let binary = root.join(format!("fixture{}", std::env::consts::EXE_SUFFIX));
        let output = Command::new("rustc")
            .args(["--edition=2021", "-O"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for name in [
            "normal",
            "exact",
            "overflow",
            "stderr-overflow",
            "combined-overflow",
            "sleep",
            "closed-sleep",
            "nonzero",
            "invalid-utf8",
            "stdin-eof",
            "descendant",
            "closed-descendant",
            "fault-kill-descendant",
            "fault-settle-descendant",
            "fault-close-descendant",
            "closed-high-fd",
            "socket-control",
            "socket-guard",
            "socket-descendant",
            "socket-setup-sentinel",
        ] {
            std::fs::hard_link(
                &binary,
                root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
            )
            .unwrap();
        }
        // Bounded retained fixture: never recursively remove an ambient tree.
        root
    });
    root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

fn run(prepared: &Prepared) -> Result<Vec<u8>, ProbeError> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        super::unix::run(prepared)
    }
    #[cfg(windows)]
    {
        super::windows::run(prepared)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = prepared;
        Err(ProbeError::Unsupported)
    }
}

#[test]
fn environment_is_closed_and_rustup_auto_install_is_disabled() {
    let mut asked = Vec::new();
    let environment = sanitized_environment(|key| {
        asked.push(key.to_owned());
        Some(OsString::from(if key == "HOME" {
            "/controlled/home"
        } else {
            "controlled"
        }))
    })
    .unwrap();
    assert_eq!(asked, retained_environment_names());
    assert_eq!(
        environment.last(),
        Some(&(OsString::from("RUSTUP_AUTO_INSTALL"), OsString::from("0")))
    );
    for forbidden in [
        "NODE_OPTIONS",
        "NODE_PATH",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_LIBRARY_PATH",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "PATH",
        "RUSTUP_DIST_SERVER",
        "RUSTUP_UPDATE_ROOT",
    ] {
        assert!(!environment.iter().any(|(name, _)| name == forbidden));
    }
    assert_eq!(
        sanitized_environment(|_| Some(OsString::from("x".repeat(8193)))),
        Err(ProbeError::Invalid)
    );
}

#[test]
fn environment_total_includes_forced_row_and_block_terminator() {
    let names = &retained_environment_names()[..4];
    let fixed = "RUSTUP_AUTO_INSTALL".len() + "0".len() + 2 + 1;
    let overhead: usize = names.iter().map(|key| key.len() + 2).sum();
    let last = 32_768 - fixed - overhead - 3 * 8192;
    for (extra, expected) in [(0, true), (1, false)] {
        let result = sanitized_environment(|key| {
            names.iter().position(|name| *name == key).map(|index| {
                OsString::from("x".repeat(if index == 3 { last + extra } else { 8192 }))
            })
        });
        assert_eq!(result.is_ok(), expected);
        if let Ok(rows) = result {
            assert_eq!(
                rows.iter()
                    .map(|(key, value)| native_units(key) + native_units(value) + 2)
                    .sum::<usize>()
                    + 1,
                32_768
            );
        } else {
            assert_eq!(result, Err(ProbeError::Invalid));
        }
    }
}

#[test]
#[cfg(any(
    target_os = "macos",
    windows,
    all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn physical_probe_bounds_output_and_settles_before_return() {
    for (name, expected) in [
        ("normal", Ok(b"rustc 1.88.0 (fixture)\n".to_vec())),
        ("exact", Ok(vec![b'x'; 65_536])),
        ("overflow", Err(ProbeError::OutputLimit)),
        ("stderr-overflow", Err(ProbeError::OutputLimit)),
        ("combined-overflow", Err(ProbeError::OutputLimit)),
        ("nonzero", Err(ProbeError::Exit)),
        ("invalid-utf8", Ok(vec![0xff])),
        ("stdin-eof", Ok(b"rustc 1.88.0 (fixture)\n".to_vec())),
        ("descendant", Ok(b"rustc 1.88.0 (fixture)\n".to_vec())),
        (
            "closed-descendant",
            Ok(b"rustc 1.88.0 (fixture)\n".to_vec()),
        ),
    ] {
        let path = fixture(name);
        let mut prepared = prepare(&path).unwrap();
        prepared.limits.run = Duration::from_secs(2);
        assert_eq!(run(&prepared), expected, "{name}");
        if name.ends_with("descendant") {
            let pid = std::fs::read_to_string(path.with_extension("pid"))
                .unwrap()
                .parse::<u32>()
                .unwrap();
            assert_stopped(pid);
        }
    }
    for name in ["sleep", "closed-sleep"] {
        let mut prepared = prepare(&fixture(name)).unwrap();
        prepared.limits.run = Duration::from_millis(100);
        let start = std::time::Instant::now();
        assert_eq!(run(&prepared), Err(ProbeError::Timeout));
        assert!(start.elapsed() < Duration::from_secs(7));
    }
}

#[test]
#[cfg(any(
    target_os = "macos",
    windows,
    all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn physical_probe_faults_are_sticky_and_uncertainty_is_fail_stop() {
    for (fault, expected) in [
        (Fault::Spawn, ProbeError::Spawn),
        (Fault::Read, ProbeError::Io),
        (Fault::Wait, ProbeError::Io),
        (Fault::Deadline, ProbeError::Timeout),
    ] {
        let mut prepared = prepare(&fixture("normal")).unwrap();
        prepared.fault = Some(fault);
        assert_eq!(run(&prepared), Err(expected));
    }
    #[cfg(windows)]
    for fault in [Fault::Assign, Fault::Resume] {
        let mut prepared = prepare(&fixture("normal")).unwrap();
        prepared.fault = Some(fault);
        assert_eq!(run(&prepared), Err(ProbeError::Spawn));
    }
    for fault in ["kill", "settle", "close"] {
        let path = fixture(&format!("fault-{fault}-descendant"));
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "doctor::tests::settlement_fault_subprocess",
                "--nocapture",
            ])
            .env("SEMAPRAX_DOCTOR_FAULT", fault)
            .env("SEMAPRAX_DOCTOR_FIXTURE", &path)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{fault}");
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains("probe-returned"),
            "{fault}"
        );
        let pid = std::fs::read_to_string(path.with_extension("pid"))
            .unwrap()
            .parse::<u32>()
            .unwrap();
        assert_stopped(pid);
    }
}

#[test]
fn settlement_fault_subprocess() {
    let Some(fault) = std::env::var_os("SEMAPRAX_DOCTOR_FAULT") else {
        return;
    };
    let path = PathBuf::from(std::env::var_os("SEMAPRAX_DOCTOR_FIXTURE").unwrap());
    let mut prepared = prepare(&path).unwrap();
    prepared.fault = Some(match fault.to_str().unwrap() {
        "kill" => Fault::Kill,
        "settle" => Fault::Settle,
        "close" => Fault::Close,
        _ => panic!("closed fault vocabulary"),
    });
    let _ = run(&prepared);
    println!("probe-returned");
}

#[cfg(any(
    target_os = "macos",
    all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn assert_stopped(pid: u32) {
    assert!(pid > 0 && pid <= i32::MAX as u32);
    assert_eq!(
        unsafe { libc::kill(pid as i32, 0) },
        -1,
        "descendant is still present"
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}

#[cfg(windows)]
fn assert_stopped(pid: u32) {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        assert_eq!(unsafe { GetLastError() }, ERROR_INVALID_PARAMETER);
        return;
    }
    let state = unsafe { WaitForSingleObject(handle, 0) };
    assert_ne!(unsafe { CloseHandle(handle) }, 0);
    assert_eq!(state, WAIT_OBJECT_0, "descendant is still running");
}
