//! Authored creation/admission observations, not provisioning or execution proof.
use super::create_doctor_offline_input;
use crate::{DoctorOfflineInputError as Error, DOCTOR_OFFLINE_INPUT_MAX_BYTES};

#[test]
fn byte_and_limit_rejections_precede_platform_selection() {
    for (bytes, limit, error) in [
        (&b""[..], 0, Error::Invalid),
        (&b"x"[..], 0, Error::Invalid),
        (&b""[..], DOCTOR_OFFLINE_INPUT_MAX_BYTES + 1, Error::Limit),
        (&b"x"[..], DOCTOR_OFFLINE_INPUT_MAX_BYTES + 1, Error::Limit),
        (&b""[..], 1, Error::Invalid),
        (&b"xy"[..], 1, Error::Limit),
    ] {
        assert_eq!(
            create_doctor_offline_input(bytes, limit).unwrap_err(),
            error
        );
    }
}

#[cfg(not(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
#[test]
fn valid_input_has_no_unsupported_host_fallback() {
    assert_eq!(
        create_doctor_offline_input(b"x", 1).unwrap_err(),
        Error::Unsupported
    );
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod native {
    use super::super::{create_with_test, TestControl, TestFault, TestStage, TestWriteFault};
    use super::*;
    use crate::{
        encode_doctor_offline_bundle, DoctorOfflineArchitecture, DoctorOfflineBundle,
        DoctorOfflineBundleEntry, DoctorOfflineBundleRoles, DoctorOfflineInput,
        DoctorOfflineTarget, DoctorOfflineTool,
    };
    use std::fs::File;
    use std::io::Seek;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{FileExt, MetadataExt};

    // Independent kernel UAPI expectations, not production implementation constants.
    const IMMUTABLE_AND_EXEC_SEALS: libc::c_int = 1 | 2 | 4 | 8 | 0x20;

    fn completed_stages(length: usize) -> Vec<TestStage> {
        let mut stages = vec![TestStage::Create];
        for offset in (0..length).step_by(8192) {
            stages.push(TestStage::Write {
                offset,
                length: (length - offset).min(8192),
            });
        }
        stages.extend([
            TestStage::Seal,
            TestStage::GetSeals,
            TestStage::Metadata,
            TestStage::GetFlags,
            TestStage::Snapshot,
            TestStage::Compare,
        ]);
        stages
    }

    #[test]
    fn preflight_failures_have_no_creation_or_cleanup_events() {
        for (bytes, limit, error) in [
            (&b""[..], 0, Error::Invalid),
            (&b"x"[..], 0, Error::Invalid),
            (&b""[..], DOCTOR_OFFLINE_INPUT_MAX_BYTES + 1, Error::Limit),
            (&b"x"[..], DOCTOR_OFFLINE_INPUT_MAX_BYTES + 1, Error::Limit),
            (&b""[..], 1, Error::Invalid),
            (&b"xy"[..], 1, Error::Limit),
        ] {
            let mut control = TestControl::default();
            assert_eq!(
                create_with_test(bytes, limit, &mut control).unwrap_err(),
                error
            );
            assert!(control.events.is_empty());
        }
    }

    #[test]
    fn successful_native_creation_calibrates_exact_bounded_write_order_and_transfer() {
        for length in [1, 8192, 8193] {
            let bytes = vec![0xa5; length];
            let mut control = TestControl::default();
            let (mut file, input) = create_with_test(&bytes, length, &mut control).unwrap();
            assert_eq!(input.bytes(), bytes);
            assert_eq!(control.events, completed_stages(length));
            assert_file(&mut file, &bytes);
            // No Close event before transfer: the returned file remains caller-owned.
        }
    }

    #[test]
    fn injected_creation_and_later_failures_close_only_the_owned_file_once() {
        let all = completed_stages(8193);
        for (fault, final_stage, expected_error, owned) in [
            (TestFault::Create, TestStage::Create, Error::Io, false),
            (
                TestFault::CreateUnsupported,
                TestStage::Create,
                Error::Unsupported,
                false,
            ),
            (TestFault::Seal, TestStage::Seal, Error::Io, true),
            (TestFault::GetSeals, TestStage::GetSeals, Error::Io, true),
            (
                TestFault::MissingSeals,
                TestStage::GetSeals,
                Error::Invalid,
                true,
            ),
            (TestFault::Metadata, TestStage::Metadata, Error::Io, true),
            (
                TestFault::ExecutableMode,
                TestStage::Metadata,
                Error::Invalid,
                true,
            ),
            (
                TestFault::SizeMismatch,
                TestStage::Metadata,
                Error::Invalid,
                true,
            ),
            (TestFault::GetFlags, TestStage::GetFlags, Error::Io, true),
            (
                TestFault::MissingCloexec,
                TestStage::GetFlags,
                Error::Invalid,
                true,
            ),
            (TestFault::Snapshot, TestStage::Snapshot, Error::Io, true),
            (
                TestFault::Mismatch,
                TestStage::Compare,
                Error::Invalid,
                true,
            ),
        ] {
            let mut control = TestControl {
                fault: Some(fault),
                ..TestControl::default()
            };
            assert_eq!(
                create_with_test(&[0x5a; 8193], 8193, &mut control).unwrap_err(),
                expected_error,
                "{fault:?}"
            );
            let final_index = all.iter().position(|stage| *stage == final_stage).unwrap();
            let mut expected = all[..=final_index].to_vec();
            if owned {
                expected.push(TestStage::Close);
            }
            assert_eq!(control.events, expected, "{fault:?}");
        }
    }

    #[test]
    fn every_nonexact_write_stops_without_retry_or_seal_or_snapshot() {
        for call in [1, 2] {
            for outcome in [
                TestWriteFault::Short,
                TestWriteFault::Zero,
                TestWriteFault::Interrupted,
                TestWriteFault::Io,
            ] {
                let fault = TestFault::Write { call, outcome };
                let mut control = TestControl {
                    fault: Some(fault),
                    ..TestControl::default()
                };
                assert_eq!(
                    create_with_test(&[7; 8193], 8193, &mut control).unwrap_err(),
                    Error::Io
                );
                let mut expected = vec![
                    TestStage::Create,
                    TestStage::Write {
                        offset: 0,
                        length: 8192,
                    },
                ];
                if call == 2 {
                    expected.push(TestStage::Write {
                        offset: 8192,
                        length: 1,
                    });
                }
                expected.push(TestStage::Close);
                assert_eq!(control.events, expected, "{fault:?}");
            }
        }
    }

    const CLOSE_HELPER: &str =
        "doctor::offline_input::create::tests::native::helper_close_uncertainty";
    const CLOSE_CHILD: &str = "SEMAPRAX_TEST_CREATED_INPUT_CLOSE_UNCERTAINTY";

    #[test]
    #[ignore = "private subprocess helper selected by failure_close_uncertainty_is_exit_126"]
    fn helper_close_uncertainty() {
        assert_eq!(std::env::var(CLOSE_CHILD).as_deref(), Ok("1"));
        let mut control = TestControl {
            fault: Some(TestFault::Snapshot),
            close_fault: true,
            ..TestControl::default()
        };
        // The seam really closes its own newly created memfd, then substitutes
        // the failure observation. No foreign descriptor is closed/replaced.
        let _ = create_with_test(b"owned", 5, &mut control);
        panic!("uncertain close returned instead of terminating");
    }

    #[test]
    fn failure_close_uncertainty_is_exit_126() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CLOSE_HELPER, "--ignored"])
            .env(CLOSE_CHILD, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(126));
                return;
            }
            if started.elapsed() >= Duration::from_secs(10) {
                // Exit may race this request. Always attempt the bounded reap
                // even if killing the exact owned child reports an error.
                let _ = child.kill();
                let cleanup = Instant::now();
                while child.try_wait().unwrap().is_none() {
                    assert!(
                        cleanup.elapsed() < Duration::from_secs(5),
                        "close helper did not reap after kill"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                panic!("close uncertainty helper exceeded its bounded wait");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn assert_file(file: &mut File, bytes: &[u8]) {
        let fd = file.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        assert!(flags >= 0);
        assert_ne!(flags & libc::FD_CLOEXEC, 0);
        let seals = unsafe { libc::fcntl(fd, libc::F_GET_SEALS) };
        assert!(seals >= 0);
        assert_eq!(seals & IMMUTABLE_AND_EXEC_SEALS, IMMUTABLE_AND_EXEC_SEALS);
        let metadata = file.metadata().unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.mode() & 0o111, 0);
        assert_eq!(metadata.len(), bytes.len() as u64);
        assert_eq!(file.stream_position().unwrap(), 0);
        assert_eq!(
            DoctorOfflineInput::acquire(file, bytes.len())
                .unwrap()
                .bytes(),
            bytes
        );
        assert_eq!(file.stream_position().unwrap(), 0);
    }

    #[test]
    fn physical_binary_carriers_are_exact_sealed_nonexecutable_and_independently_retained() {
        // Missing mandatory MFD_NOEXEC_SEAL support fails this selected test;
        // there is no older-kernel or seccomp fallback disguised as success.
        for (length, limit) in [
            (1, 1),
            (1, DOCTOR_OFFLINE_INPUT_MAX_BYTES),
            (8192, 8192),
            (8193, 8193),
        ] {
            let bytes: Vec<_> = (0..length).map(|index| (index % 256) as u8).collect();
            let (mut file, snapshot) = create_doctor_offline_input(&bytes, limit).unwrap();
            assert_eq!(snapshot.bytes(), bytes);
            assert_file(&mut file, &bytes);
            drop(file);
            assert_eq!(snapshot.bytes(), bytes);
        }
    }

    #[test]
    fn physical_write_resize_writable_map_and_execute_bit_changes_are_denied() {
        let bytes = vec![0x81; 8192];
        let (mut file, snapshot) = create_doctor_offline_input(&bytes, bytes.len()).unwrap();
        let mode = file.metadata().unwrap().mode();
        assert!(file.write_at(b"x", 0).is_err());
        assert!(file.set_len(0).is_err());
        assert!(file.set_len(8193).is_err());
        let mapped = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                8192,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            )
        };
        if mapped != libc::MAP_FAILED {
            assert_eq!(unsafe { libc::munmap(mapped, 8192) }, 0);
            panic!("sealed carrier unexpectedly admitted a writable shared mapping");
        }
        assert_eq!(unsafe { libc::fchmod(file.as_raw_fd(), mode | 0o111) }, -1);
        assert_eq!(file.metadata().unwrap().mode(), mode);
        assert_file(&mut file, &bytes);
        assert_eq!(snapshot.bytes(), bytes);
    }

    #[test]
    fn encoded_bundle_and_request_use_created_snapshots_without_resealing_or_execution() {
        let (architecture, machine) = if cfg!(target_arch = "x86_64") {
            (DoctorOfflineArchitecture::LinuxX86_64, 62u16)
        } else {
            (DoctorOfflineArchitecture::LinuxAarch64, 183u16)
        };
        // Structurally admitted minimal ELF only: never a runnable tool claim.
        let mut elf = vec![0; 120];
        elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        elf[16..18].copy_from_slice(&2u16.to_le_bytes());
        elf[18..20].copy_from_slice(&machine.to_le_bytes());
        elf[20..24].copy_from_slice(&1u32.to_le_bytes());
        elf[32..40].copy_from_slice(&64u64.to_le_bytes());
        elf[52..54].copy_from_slice(&64u16.to_le_bytes());
        elf[54..56].copy_from_slice(&56u16.to_le_bytes());
        elf[56..58].copy_from_slice(&1u16.to_le_bytes());
        elf[64..68].copy_from_slice(&1u32.to_le_bytes());
        let bytes = encode_doctor_offline_bundle(
            architecture,
            "created-v1",
            &[DoctorOfflineBundleEntry {
                path: "bin/clang",
                bytes: &elf,
                executable: true,
            }],
            DoctorOfflineBundleRoles {
                clang: Some(0),
                node: None,
                rustc: None,
            },
            DOCTOR_OFFLINE_INPUT_MAX_BYTES,
        )
        .unwrap();
        let (mut file, snapshot) = create_doctor_offline_input(&bytes, bytes.len()).unwrap();
        assert_file(&mut file, &bytes);
        let bundle = DoctorOfflineBundle::parse(snapshot, "created-v1").unwrap();
        let request = bundle
            .encode_worker_request(DoctorOfflineTarget::Native, [3; 32])
            .unwrap();
        let (mut request_file, request_snapshot) =
            create_doctor_offline_input(&request, 149).unwrap();
        assert_file(&mut request_file, &request);
        let parsed =
            crate::doctor::offline_worker::wire::Request::parse(request_snapshot.bytes()).unwrap();
        assert_eq!(parsed.bundle_len, bytes.len());
        assert_eq!(parsed.architecture, architecture);
        assert_eq!(parsed.selector, "created-v1");
        assert_eq!(
            parsed.roles().collect::<Vec<_>>(),
            [(1, DoctorOfflineTool::Clang)]
        );
        assert_eq!(&request[12..44], &[3; 32]);
        assert_eq!(&request[44..52], &(bytes.len() as u64).to_le_bytes());
        use sha2::{Digest, Sha256};
        let expected_digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(parsed.bundle_digest, expected_digest);
        drop(file);
        drop(request_file);
        assert_eq!(request_snapshot.bytes(), request);
        assert_eq!(bundle.tool(DoctorOfflineTool::Clang).unwrap().bytes(), elf);
    }
}
