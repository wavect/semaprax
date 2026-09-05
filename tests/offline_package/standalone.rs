use std::path::Path;
use std::process::Command;

#[test]
fn standalone_rejects_private_commands_before_creating_output() {
    let root = std::env::temp_dir().join(format!("semaprax-standalone-{}", std::process::id()));
    assert!(!root.exists());
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/semaprax.toml");
    let output = root.join("nested/package");
    // `doctor` moved into the root crate; only the `rust` build target still
    // needs the private host, and the standalone catalog omits it entirely.
    let result = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "build",
            &manifest.display().to_string(),
            "--target",
            "rust",
            "-o",
            &output.display().to_string(),
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("unsupported target `rust`"), "{stderr}");
    assert!(!root.exists());
}

#[test]
fn standalone_normal_dependency_closure_excludes_every_private_host() {
    let result = Command::new(env!("CARGO"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "tree",
            "--locked",
            "--offline",
            "-p",
            "semaprax",
            "--target",
            "all",
            "--edges",
            "normal",
            "--prefix",
            "none",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let tree = String::from_utf8(result.stdout).unwrap();
    for line in tree.lines().skip(1) {
        assert!(
            !line.starts_with("semaprax-"),
            "private host in standalone dependency closure: {line}"
        );
    }
}
