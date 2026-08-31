//! Actual production launcher entry, not the direct fixture's clone/exec path.
//! The trusted driver provisions immutable images and maps only its four files.
//! Rejection observations alone do not prove absence of child or tool effects.
use super::{fixture, launch, observe, report};
use semaprax_native_rust_interop_platform_sys::{
    create_doctor_offline_executable, create_doctor_offline_input,
};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

const IMAGE_LIMIT: usize = 512 * 1024 * 1024;
const IMMUTABLE: i32 =
    libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;

pub(super) fn context() {
    assert_eq!(
        std::env::var("SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT").as_deref(),
        Ok("private-mapped-user-mount-clean-worker-cgroup-v1")
    );
}

pub(super) fn installed_image(variable: &str) -> Vec<u8> {
    let path = launch::provisioned_path(variable);
    let file = File::open(path).unwrap();
    let metadata = file.metadata().unwrap();
    assert!(metadata.is_file());
    assert!(metadata.len() > 0 && metadata.len() <= IMAGE_LIMIT as u64);
    let mut bytes = Vec::new();
    file.take(IMAGE_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len() as u64, metadata.len());
    assert!(bytes.len() <= IMAGE_LIMIT);
    // The explicit paths and immutable startup closure are provisioner facts;
    // reading a binary here does not independently authenticate its provenance.
    bytes
}

pub(super) fn prepared_executable(bytes: &[u8]) -> File {
    assert!(!bytes.is_empty() && bytes.len() <= IMAGE_LIMIT);
    let (file, snapshot) = create_doctor_offline_executable(bytes, bytes.len()).unwrap();
    assert_eq!(snapshot.bytes(), bytes);
    // The launcher receives the actual factory-created file. A retained byte
    // snapshot cannot substitute for the executable descriptor handoff.
    drop(snapshot);
    file
}

// Independent hostile image construction: malformed bytes or absent seals
// must reach launcher admission, not be rejected by the production factory.
fn executable_file(bytes: &[u8], execution_seal: bool) -> File {
    assert!(!bytes.is_empty() && bytes.len() <= IMAGE_LIMIT);
    let fd = unsafe {
        libc::memfd_create(
            c"launcher-fixture-image".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING | libc::MFD_EXEC,
        )
    };
    assert!(fd >= 0, "{}", io::Error::last_os_error());
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes).unwrap();
    let seals = IMMUTABLE | if execution_seal { libc::F_SEAL_EXEC } else { 0 };
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, seals) }, 0);
    let actual = unsafe { libc::fcntl(fd, libc::F_GET_SEALS) };
    assert!(actual >= 0);
    assert_eq!(actual & IMMUTABLE, IMMUTABLE);
    assert_eq!(actual & libc::F_SEAL_EXEC != 0, execution_seal);
    file
}

fn transport(bytes: &[u8]) -> File {
    let (file, snapshot) = create_doctor_offline_input(bytes, bytes.len()).unwrap();
    assert_eq!(snapshot.bytes(), bytes);
    drop(snapshot);
    file
}

pub(super) fn run(
    request: &File,
    bundle: &File,
    worker: &File,
    collector: &File,
) -> observe::Observation {
    context();
    let launcher = launch::provisioned_path("SEMAPRAX_DOCTOR_LAUNCHER");
    let sources = [request, bundle, worker, collector].map(|file| launch::high(file.as_raw_fd()));
    let fds = sources.each_ref().map(AsRawFd::as_raw_fd);
    // Reserve before std creates its exec-error pipe. The trusted serial
    // harness excludes other descriptor mutators throughout spawn.
    let _reservations = launch::reserve_destinations(fds[0]);
    let mut command = Command::new(launcher);
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Only the dedicated production launcher clones a worker or maps collector
    // endpoints. This parent-side harness merely transfers the agreed inputs.
    unsafe {
        command.pre_exec(move || {
            for (index, source) in fds.iter().enumerate() {
                let destination = 3 + index as i32;
                if libc::dup2(*source, destination) != destination {
                    return Err(io::Error::last_os_error());
                }
            }
            // Preserve std's private startup handshake until successful exec;
            // all high duplicates and reservations then disappear atomically.
            if libc::syscall(
                libc::SYS_close_range,
                7u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut owned =
        observe::OwnedCollector(command.spawn().expect(
            "start provisioned launcher; reconcile external fixture cgroup on uncertainty",
        ));
    observe::collect(&mut owned.0)
        .expect("bounded launcher observation; reconcile external cgroup on uncertainty")
}

fn healthy(request: &File, bundle: &File, worker: &File, collector: &File, all: bool) {
    let tools = [
        ("clang", "ok", "/bin/clang (clang version 1.0.0)"),
        ("node", "ok", "v22.0.0"),
        ("rust", "ok", "rustc 1.88.0"),
    ];
    report::require(
        run(request, bundle, worker, collector),
        if all { "all" } else { "native" },
        &tools[..if all { 3 } else { 1 }],
        0,
    );
}

#[test]
#[ignore = "requires current-head production launcher/worker/collector and fully provisioned native context"]
fn production_launcher_reports_native_and_all_from_literal_transport_files() {
    context();
    let worker = prepared_executable(&installed_image("SEMAPRAX_DOCTOR_WORKER"));
    let collector = prepared_executable(&installed_image("SEMAPRAX_DOCTOR_COLLECTOR"));
    for all in [false, true] {
        // Independent existing literal helpers, not launcher-private encoders.
        let bytes = if all {
            fixture::all_bundle(fixture::Ending::Exit(0))
        } else {
            fixture::bundle()
        };
        let request = transport(&fixture::request_target(&bytes, if all { 3 } else { 1 }));
        let bundle = transport(&bytes);
        healthy(&request, &bundle, &worker, &collector, all);
    }
}

#[test]
#[ignore = "requires production launcher and strict executable/non-executable sealed-memfd support"]
fn production_launcher_rejects_both_image_defects_and_digest_drift() {
    context();
    let worker_bytes = installed_image("SEMAPRAX_DOCTOR_WORKER");
    let collector_bytes = installed_image("SEMAPRAX_DOCTOR_COLLECTOR");
    let worker = prepared_executable(&worker_bytes);
    let collector = prepared_executable(&collector_bytes);
    let bundle_bytes = fixture::bundle();
    let request_bytes = fixture::request(&bundle_bytes);
    let request = transport(&request_bytes);
    let bundle = transport(&bundle_bytes);
    healthy(&request, &bundle, &worker, &collector, false);
    for replace_worker in [true, false] {
        let original: &[u8] = if replace_worker {
            &worker_bytes
        } else {
            &collector_bytes
        };
        let mut malformed = original.to_vec();
        malformed[0] ^= 1;
        let mut wrong_arch = original.to_vec();
        let foreign = if fixture::architecture() == 1 {
            183u16
        } else {
            62u16
        };
        wrong_arch[18..20].copy_from_slice(&foreign.to_le_bytes());
        let variants = [
            executable_file(&malformed, true),
            executable_file(&wrong_arch, true),
            executable_file(b"#!/bin/sh\nexit 0\n", true),
            executable_file(original, false),
            transport(original),
        ];
        for replacement in variants {
            let (selected_worker, selected_collector) = if replace_worker {
                (&replacement, &collector)
            } else {
                (&worker, &replacement)
            };
            super::rejected(run(&request, &bundle, selected_worker, selected_collector));
        }
        healthy(&request, &bundle, &worker, &collector, false);
    }
    let mut drifted = request_bytes;
    drifted[52] ^= 1;
    let wrong_request = transport(&drifted); // Deliberately not repaired/rehashed.
    super::rejected(run(&wrong_request, &bundle, &worker, &collector));
    healthy(&request, &bundle, &worker, &collector, false);
    // These observations establish rejection/no report, not zero child creation
    // or descendant settlement. The production state scripts own action order;
    // the external provisioner still owns aggregate cgroup reconciliation.
}

#[test]
#[ignore = "requires production launcher and quiescent provisioned loader namespace"]
fn production_launcher_rejects_structural_collector_with_missing_loader() {
    context();
    let missing = "/semaprax-launcher-fixture-missing-loader";
    assert_eq!(
        std::fs::symlink_metadata(missing).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    let worker = prepared_executable(&installed_image("SEMAPRAX_DOCTOR_WORKER"));
    let collector = prepared_executable(&installed_image("SEMAPRAX_DOCTOR_COLLECTOR"));
    let bytes = fixture::bundle();
    let request = transport(&fixture::request(&bytes));
    let bundle = transport(&bytes);
    healthy(&request, &bundle, &worker, &collector, false);

    let mut interpreter = missing.as_bytes().to_vec();
    interpreter.push(0);
    let mut image = fixture::image(&[], &interpreter);
    // A single native ELF64 PT_INTERP row, with bounded canonical absolute
    // interpreter bytes. The shared structural validator admits this shape;
    // the kernel cannot load its explicitly absent interpreter. There is no
    // executable tool body or fallback loader in this synthetic collector.
    image[64..68].copy_from_slice(&3u32.to_le_bytes());
    image[72..80].copy_from_slice(&120u64.to_le_bytes());
    image[96..104].copy_from_slice(&(interpreter.len() as u64).to_le_bytes());
    image[104..112].copy_from_slice(&(interpreter.len() as u64).to_le_bytes());
    let unlaunchable = executable_file(&image, true);
    super::rejected(run(&request, &bundle, &worker, &unlaunchable));
    healthy(&request, &bundle, &worker, &collector, false);
    assert_eq!(
        std::fs::symlink_metadata(missing).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    // Sys admission tests separately calibrate structural acceptance of this
    // header. Exit 126/no output alone does not identify the rejection stage,
    // prove exact child reaping, or replace external cgroup reconciliation.
}
