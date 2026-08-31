//! One fatal-outcome oracle, including a deliberately broken generated-close
//! calibration. This mutates only retained test files, never production sources.
use std::path::Path;
use std::process::Command;

const COMPLETED: &str = "invocation returned to the harness";

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
    std::fs::create_dir(&calibration).unwrap();
    for (name, contents) in [
        ("sdk.rs", lib),
        ("owned_data_ffi.rs", broken.as_str()),
        (
            "provider.rs",
            include_str!("../fixtures/owned_boundary_provider.rs"),
        ),
        ("boundary.rs", harness),
    ] {
        std::fs::write(calibration.join(name), contents).unwrap();
    }
    let executable = calibration.join(format!("boundary{}", std::env::consts::EXE_SUFFIX));
    let compiled = Command::new("rustc")
        .args(["--edition=2021", "-C", &format!("opt-level={optimization}")])
        .arg(calibration.join("boundary.rs"))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = Command::new(executable).arg("22").output().unwrap();
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
}
