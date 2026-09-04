use std::path::Path;
use std::process::Command;

#[test]
fn standalone_rejects_private_commands_before_creating_output() {
    let root = std::env::temp_dir().join(format!("semaprax-standalone-{}", std::process::id()));
    assert!(!root.exists());
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/semaprax.toml");
    let output = root.join("nested/package");
    for arguments in [
        vec!["doctor".to_owned(), "--json".to_owned()],
        vec![
            "build".to_owned(),
            manifest.display().to_string(),
            "--target".to_owned(),
            "rust".to_owned(),
            "-o".to_owned(),
            output.display().to_string(),
        ],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr)
            .contains("unavailable in the standalone crates.io package"));
        assert!(!root.exists());
    }
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
