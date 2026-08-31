//! One fatal-outcome oracle, including independently broken owner/close responses.
//! Calibrations mutate retained generated test files, never production sources.
use std::path::{Path, PathBuf};
use std::process::Command;

const COMPLETED: &str = "invocation returned to the harness";
const UNWIND_OWNER: &str =
    "event:init\nevent:call\nevent:unwind\nevent:drop\nevent:drop-unwinding\n";
const UNWIND_CLOSE: &str = "event:init\nevent:call\nevent:unwind\nevent:drop\nevent:drop-unwinding\nevent:close\nevent:close-unwinding\n";
const UNWIND_TRANSCRIPT: &str = "unwind finalization transcript disagrees";

pub(super) fn check(mode: u32, success: bool, stdout: &str) -> Result<(), &'static str> {
    if stdout.contains("call-completed") {
        return Err(COMPLETED);
    }
    if success {
        return Err("fatal invocation exited successfully");
    }
    if stdout.contains("returned:") || stdout.contains("recovered") || stdout.contains("finished") {
        return Err("fatal invocation published or continued");
    }
    if matches!(mode, 28 | 29) {
        let expected = if mode == 28 {
            UNWIND_OWNER
        } else {
            UNWIND_CLOSE
        };
        // Exact lines distinguish drop from drop-unwinding, preserve phase
        // order, and exclude every extra provider effect or harness return.
        return if stdout == expected {
            Ok(())
        } else {
            Err(UNWIND_TRANSCRIPT)
        };
    }
    if matches!(mode, 9 | 10 | 26) {
        if ["event:len", "event:copy", "event:drop", "event:close"]
            .iter()
            .any(|event| stdout.contains(event))
        {
            return Err("invalid authority caused a later provider operation");
        }
    } else if matches!(mode, 7 | 27) {
        if stdout.matches("event:drop").count() != 1 || stdout.contains("event:close") {
            return Err("uncertain owner drop retried or reached context close");
        }
    } else if stdout.matches("event:close").count() != 1 {
        return Err("context close was missing or repeated");
    }
    Ok(())
}

pub(super) fn calibrate(root: &Path, lib: &str, ffi: &str, harness: &str, optimization: &str) {
    const CHECKED_CLOSE: &str =
        "if unsafe{spx_owned_data_context_drop_v1(self.raw.as_ptr())}!=0{std::process::abort()}";
    const IGNORED_CLOSE: &str = "let _=unsafe{spx_owned_data_context_drop_v1(self.raw.as_ptr())};";
    assert_eq!(ffi.matches(CHECKED_CLOSE).count(), 1);
    // Keep the actual provider close and its failure. Only the generated
    // response to that failure is deliberately wrong in this isolated control.
    let broken = ffi.replacen(CHECKED_CLOSE, IGNORED_CLOSE, 1);
    assert!(!broken.contains(CHECKED_CLOSE));
    assert_eq!(broken.matches(IGNORED_CLOSE).count(), 1);
    let calibration = root.join(format!("fatal-oracle-o{optimization}"));
    let executable = compile_control(&calibration, lib, &broken, harness, optimization);
    let output = Command::new(&executable).arg("22").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    // This is the former false-positive shape: the provider close failed, the
    // call returned an ordinary error, and a later harness assertion panicked.
    assert!(!output.status.success(), "{stdout}");
    assert_eq!(stdout.matches("event:close").count(), 1, "{stdout}");
    assert_eq!(stdout.matches("call-completed").count(), 1, "{stdout}");
    assert!(
        stdout.find("event:close").unwrap() < stdout.find("call-completed").unwrap(),
        "{stdout}"
    );
    for forbidden in ["returned:", "recovered", "finished", "event:drop"] {
        assert!(!stdout.contains(forbidden), "{stdout}");
    }
    assert_eq!(
        check(22, output.status.success(), &stdout),
        Err(COMPLETED),
        "broken close must fail the same fatal oracle specifically on API return"
    );

    let output = Command::new(&executable).arg("29").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "{stdout}");
    assert_eq!(stdout, format!("{UNWIND_CLOSE}call-completed\n"));
    assert_eq!(
        check(29, output.status.success(), &stdout),
        Err(COMPLETED),
        "caught unwind after ignored close failure is not process fail-stop"
    );

    const CHECKED_OWNER: &str = "if self.armed&&unsafe{spx_owned_bytes_drop_v1(self.context.raw.as_ptr(),self.handle)}!=0{std::process::abort()}";
    const IGNORED_OWNER: &str = "if self.armed{let _=unsafe{spx_owned_bytes_drop_v1(self.context.raw.as_ptr(),self.handle)};}";
    assert_eq!(ffi.matches(CHECKED_OWNER).count(), 1);
    let broken_owner = ffi.replacen(CHECKED_OWNER, IGNORED_OWNER, 1);
    assert!(!broken_owner.contains(CHECKED_OWNER));
    assert_eq!(broken_owner.matches(IGNORED_OWNER).count(), 1);
    assert_eq!(broken_owner.matches(CHECKED_CLOSE).count(), 1);
    // Only the owner guard's abort response is removed. The real failed drop
    // retains its owner, and the unchanged context close must then fail-stop.
    let calibration = root.join(format!("fatal-owner-oracle-o{optimization}"));
    let executable = compile_control(&calibration, lib, &broken_owner, harness, optimization);
    let output = Command::new(executable).arg("28").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "{stdout}");
    assert_eq!(stdout, format!("{UNWIND_OWNER}event:close\n"));
    assert_eq!(
        check(28, output.status.success(), &stdout),
        Err(UNWIND_TRANSCRIPT),
        "a later context abort cannot conceal ignored owner-drop uncertainty"
    );
}

fn compile_control(
    directory: &Path,
    lib: &str,
    ffi: &str,
    harness: &str,
    optimization: &str,
) -> PathBuf {
    std::fs::create_dir(directory).unwrap();
    for (name, contents) in [
        ("sdk.rs", lib),
        ("owned_data_ffi.rs", ffi),
        (
            "provider.rs",
            include_str!("../fixtures/owned_boundary_provider.rs"),
        ),
        ("boundary.rs", harness),
    ] {
        std::fs::write(directory.join(name), contents).unwrap();
    }
    let executable = directory.join(format!("boundary{}", std::env::consts::EXE_SUFFIX));
    let compiled = Command::new("rustc")
        .args([
            "--edition=2021",
            "-C",
            "panic=unwind",
            "-C",
            &format!("opt-level={optimization}"),
        ])
        .arg(directory.join("boundary.rs"))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    executable
}

#[test]
fn joint_unwind_fatal_oracle_requires_exact_ordered_phase_witnesses() {
    for (mode, expected) in [(28, UNWIND_OWNER), (29, UNWIND_CLOSE)] {
        assert_eq!(check(mode, false, expected), Ok(()));
        assert_eq!(
            check(mode, true, expected),
            Err("fatal invocation exited successfully")
        );
        let lines = expected.split_inclusive('\n').collect::<Vec<_>>();
        for index in 0..lines.len() {
            let mut missing = lines.clone();
            missing.remove(index);
            assert_eq!(
                check(mode, false, &missing.concat()),
                Err(UNWIND_TRANSCRIPT)
            );
            let mut duplicate = lines.clone();
            duplicate.insert(index, lines[index]);
            assert_eq!(
                check(mode, false, &duplicate.concat()),
                Err(UNWIND_TRANSCRIPT)
            );
        }
        for index in 1..lines.len() {
            let mut reordered = lines.clone();
            reordered.swap(index - 1, index);
            assert_eq!(
                check(mode, false, &reordered.concat()),
                Err(UNWIND_TRANSCRIPT)
            );
        }
        for extra in [
            "event:len\n",
            "event:copy\n",
            "event:drop\n",
            "event:close\n",
            "event:init\n",
        ] {
            assert_eq!(
                check(mode, false, &format!("{expected}{extra}")),
                Err(UNWIND_TRANSCRIPT)
            );
        }
        for extra in ["returned:value\n", "recovered\n", "finished\n"] {
            assert_eq!(
                check(mode, false, &format!("{expected}{extra}")),
                Err("fatal invocation published or continued")
            );
        }
        for success in [false, true] {
            assert_eq!(
                check(mode, success, &format!("{expected}call-completed\n")),
                Err(COMPLETED)
            );
        }
        assert_eq!(check(mode, false, ""), Err(UNWIND_TRANSCRIPT));
    }
}
