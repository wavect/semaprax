//! Injected observations follow real owned creation; they are not kernel faults.
use super::*;

#[test]
fn preflight_limits_have_no_creation_or_cleanup_events() {
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
            create_executable_with_test(bytes, limit, &mut control).unwrap_err(),
            error
        );
        assert!(control.events.is_empty());
    }
    let bytes = image(120);
    let mut control = TestControl::default();
    assert_eq!(
        create_executable_with_test(&bytes, 119, &mut control).unwrap_err(),
        Error::Limit
    );
    assert!(control.events.is_empty());
}

#[test]
fn every_mode_property_and_acquisition_failure_stops_at_its_exact_prefix() {
    let bytes = image(8193);
    let all = completed_stages(bytes.len());
    for (fault, final_stage, expected_error, owned) in [
        (TestFault::Create, TestStage::Create, Error::Io, false),
        (
            TestFault::CreateUnsupported,
            TestStage::Create,
            Error::Unsupported,
            false,
        ),
        (TestFault::Mode, TestStage::Mode, Error::Io, true),
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
            TestFault::WrongMode,
            TestStage::Metadata,
            Error::Invalid,
            true,
        ),
        (
            TestFault::ExcessMode,
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
            create_executable_with_test(&bytes, bytes.len(), &mut control).unwrap_err(),
            expected_error,
            "{fault:?}"
        );
        let index = all.iter().position(|stage| *stage == final_stage).unwrap();
        let mut expected = all[..=index].to_vec();
        if owned {
            expected.push(TestStage::Close);
        }
        assert_eq!(control.events, expected, "{fault:?}");
    }
}

#[test]
fn first_and_second_nonexact_writes_never_retry_seal_or_acquire() {
    let bytes = image(8193);
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
                create_executable_with_test(&bytes, bytes.len(), &mut control).unwrap_err(),
                Error::Io
            );
            let mut expected = vec![
                TestStage::Create,
                TestStage::Mode,
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

const CLOSE_HELPER: &str = "doctor::offline_input::create::executable_tests::native::faults::helper_executable_close_uncertainty";
const CLOSE_CHILD: &str = "SEMAPRAX_TEST_CREATED_EXECUTABLE_CLOSE_UNCERTAINTY";

#[test]
#[ignore = "private subprocess helper selected by executable_close_uncertainty_exits_126"]
fn helper_executable_close_uncertainty() {
    assert_eq!(std::env::var(CLOSE_CHILD).as_deref(), Ok("1"));
    let bytes = image(120);
    let mut control = TestControl {
        fault: Some(TestFault::Snapshot),
        close_fault: true,
        ..TestControl::default()
    };
    // The seam first closes only its newly owned descriptor, then substitutes
    // uncertainty. This does not simulate a foreign FD or physical close errno.
    let _ = create_executable_with_test(&bytes, bytes.len(), &mut control);
    panic!("uncertain executable close returned");
}

#[test]
fn executable_close_uncertainty_exits_126() {
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
            let _ = child.kill();
            let cleanup = Instant::now();
            while child.try_wait().unwrap().is_none() {
                assert!(
                    cleanup.elapsed() < Duration::from_secs(5),
                    "executable close helper did not reap after kill"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("executable close helper exceeded bounded wait");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
