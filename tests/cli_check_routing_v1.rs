//! Actual CLI selector routing; no compiler build or target runtime is invoked.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const VALID: &str = "module route.valid; @id(\"route.main\") fn main() -> i64 { 42 }\n";
const INVALID: &str = "not a module\n";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-check-routing-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    eprintln!("retained CLI check fixture: {}", path.display());
    path
}

fn cli(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("check")
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap()
}

fn same_result(first: &Output, second: &Output) {
    assert_eq!(first.status.code(), second.status.code());
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.stderr, second.stderr);
}

#[test]
fn source_json_flag_order_cannot_select_a_shadow_file() {
    let root = fixture();
    fs::write(root.join("valid.spx"), VALID).unwrap();
    fs::write(root.join("invalid.spx"), INVALID).unwrap();
    fs::write(root.join("--json"), INVALID).unwrap();
    let before = cli(&root, &["--json", "valid.spx"]);
    let after = cli(&root, &["valid.spx", "--json"]);
    same_result(&before, &after);
    assert!(before.status.success());
    assert!(before.stdout.is_empty() && before.stderr.is_empty());
    assert_eq!(fs::read_to_string(root.join("--json")).unwrap(), INVALID);

    // A valid shadow would make the old argument-reparsing route report
    // success for this invalid requested source. The parsed path must win.
    fs::write(root.join("--json"), VALID).unwrap();
    let before = cli(&root, &["--json", "invalid.spx"]);
    let after = cli(&root, &["invalid.spx", "--json"]);
    same_result(&before, &after);
    assert_eq!(before.status.code(), Some(1));
    assert!(before.stderr.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&before.stdout).unwrap();
    assert_eq!(diagnostic["code"], "SPX-P104");
    assert!(String::from_utf8(before.stdout)
        .unwrap()
        .contains("invalid.spx"));
    let human = cli(&root, &["invalid.spx"]);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stdout.is_empty());
    let human_stderr = String::from_utf8_lossy(&human.stderr);
    assert!(human_stderr.contains(" at invalid.spx:1:1"));
    assert!(!human_stderr.contains("--help"));
    assert_eq!(fs::read_to_string(root.join("--json")).unwrap(), VALID);
    assert_eq!(fs::read_to_string(root.join("valid.spx")).unwrap(), VALID);
    assert_eq!(
        fs::read_to_string(root.join("invalid.spx")).unwrap(),
        INVALID
    );
    let plain = cli(&root, &["valid.spx"]);
    assert!(plain.status.success() && plain.stderr.is_empty());
    assert!(String::from_utf8(plain.stdout)
        .unwrap()
        .starts_with("verified valid.spx (sha256:"));
}

#[test]
fn project_default_and_explicit_selectors_keep_the_same_result() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    let plain = cli(&root, &[]);
    assert!(plain.status.success());
    for arguments in [
        &["semaprax.toml"][..],
        &["--manifest-path", "semaprax.toml"][..],
    ] {
        same_result(&plain, &cli(&root, arguments));
    }
    let json = cli(&root, &["--json"]);
    assert!(json.status.success() && json.stdout.is_empty() && json.stderr.is_empty());
    for arguments in [
        &["--json", "semaprax.toml"][..],
        &["semaprax.toml", "--json"][..],
        &["--json", "--manifest-path", "semaprax.toml"][..],
        &["--manifest-path", "semaprax.toml", "--json"][..],
    ] {
        same_result(&json, &cli(&root, arguments));
    }
}

#[test]
fn malformed_selectors_reject_before_attempting_source_io() {
    let root = fixture();
    for arguments in [
        &["--json", "--json", "missing.spx"][..],
        &["missing.spx", "--unknown"][..],
        &["missing.spx", "--manifest-path", "semaprax.toml"][..],
        &["missing.spx", "second.spx"][..],
    ] {
        let output = cli(&root, arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8(output.stderr)
            .unwrap()
            .contains("SPX-I001"));
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }
}
