use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project;

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project")
}

fn temporary(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "semaprax-project-manifest-v1-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ))
}

fn copy_fixture(label: &str) -> PathBuf {
    let destination = temporary(label);
    std::fs::create_dir_all(destination.join("src")).unwrap();
    for path in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ] {
        std::fs::copy(fixture_root().join(path), destination.join(path)).unwrap();
    }
    destination.canonicalize().unwrap()
}

fn diagnostic(root: &Path) -> Vec<semaprax::diagnostic::Diagnostic> {
    project::with_authenticated_project(&root.join("semaprax.toml"), |_| Ok(())).unwrap_err()
}

#[test]
fn project_snapshot_is_deterministic_and_closures_exclude_reverse_consumers() {
    let path = fixture_root().join("semaprax.toml");
    let first = project::with_authenticated_project(&path, |snapshot| {
        snapshot.check()?;
        assert_eq!(snapshot.manifest().name(), "calculator");
        assert_eq!(snapshot.manifest().entry(), "calculator.app");
        assert_eq!(snapshot.manifest().test_module(), "calculator.tests");
        assert_eq!(snapshot.sources().len(), 3);
        assert!(snapshot
            .sources()
            .iter()
            .all(|source| source.source_graph_schema() == "semaprax.graph.v10"));

        let entry_ids = snapshot
            .entry_program()
            .functions
            .iter()
            .map(|function| function.id.as_str())
            .collect::<Vec<_>>();
        assert!(entry_ids.contains(&"calculator.app.main"));
        assert!(entry_ids.contains(&"calculator.add"));
        assert!(!entry_ids.contains(&"calculator.tests.main"));

        let test_ids = snapshot
            .test_program()
            .functions
            .iter()
            .map(|function| function.id.as_str())
            .collect::<Vec<_>>();
        assert!(test_ids.contains(&"calculator.tests.main"));
        assert!(test_ids.contains(&"calculator.add"));
        assert!(!test_ids.contains(&"calculator.app.main"));
        Ok((
            snapshot.project_revision().to_owned(),
            snapshot.workspace_revision().to_owned(),
            snapshot.workspace_manifest().to_owned(),
            entry_ids.into_iter().map(str::to_owned).collect::<Vec<_>>(),
            test_ids.into_iter().map(str::to_owned).collect::<Vec<_>>(),
        ))
    })
    .unwrap();
    let second = project::with_authenticated_project(&path, |snapshot| {
        Ok((
            snapshot.project_revision().to_owned(),
            snapshot.workspace_revision().to_owned(),
            snapshot.workspace_manifest().to_owned(),
            snapshot
                .entry_program()
                .functions
                .iter()
                .map(|function| function.id.as_str().to_owned())
                .collect::<Vec<_>>(),
            snapshot
                .test_program()
                .functions
                .iter()
                .map(|function| function.id.as_str().to_owned())
                .collect::<Vec<_>>(),
        ))
    })
    .unwrap();
    assert_eq!(first, second);
}

#[test]
fn manifest_and_source_hostility_fail_before_output_creation() {
    let mutations = [
        ("unknown", "schema =", "unknown ="),
        (
            "reordered",
            "name = \"calculator\"\nentry = \"calculator.app\"",
            "entry = \"calculator.app\"\nname = \"calculator\"",
        ),
        ("traversal", "src/app.spx", "../src/app.spx"),
        (
            "duplicate-source",
            "\"src/app.spx\", \"src/core.spx\"",
            "\"src/app.spx\", \"src/app.spx\"",
        ),
        (
            "duplicate-export",
            "\"calculator.add\", \"calculator.divide\"",
            "\"calculator.add\", \"calculator.add\"",
        ),
    ];
    for (label, from, to) in mutations {
        let root = copy_fixture(label);
        let manifest_path = root.join("semaprax.toml");
        let manifest = std::fs::read_to_string(&manifest_path).unwrap();
        let changed = manifest.replacen(from, to, 1);
        assert_ne!(changed, manifest);
        std::fs::write(&manifest_path, changed).unwrap();
        let output = root.join("out");
        let errors = project::with_authenticated_project(&manifest_path, |snapshot| {
            snapshot.build_web(&output)
        })
        .unwrap_err();
        assert!(!errors.is_empty(), "{label} unexpectedly succeeded");
        assert!(!output.exists(), "{label} created output before rejection");
        let _ = std::fs::remove_dir_all(root);
    }

    for (label, path, replacement) in [
        (
            "noncanonical-source",
            "src/core.spx",
            "module calculator.core; @id(\"calculator.add\") fn add(a:i64,b:i64)->i64{a+b}",
        ),
        (
            "missing-provider",
            "src/app.spx",
            "module calculator.app;\nuse function @id(\"missing.add\") from missing.core as add;\n\n@id(\"calculator.app.main\")\nfn main() -> i64\n{\n    add(1, 2)\n}\n",
        ),
    ] {
        let root = copy_fixture(label);
        std::fs::write(root.join(path), replacement).unwrap();
        let output = root.join("out");
        let errors = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            snapshot.build_web(&output)
        })
        .unwrap_err();
        assert!(!errors.is_empty());
        assert!(!output.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(unix)]
#[test]
fn source_alias_and_external_hardlink_are_rejected_without_output() {
    use std::os::unix::fs::symlink;

    let root = copy_fixture("symlink");
    std::fs::remove_file(root.join("src/core.spx")).unwrap();
    symlink(
        fixture_root().join("src/core.spx"),
        root.join("src/core.spx"),
    )
    .unwrap();
    assert_eq!(diagnostic(&root)[0].code, "SPX-J102");
    assert!(!root.join("out").exists());
    let _ = std::fs::remove_file(root.join("src/core.spx"));
    let _ = std::fs::remove_dir_all(root);

    let root = copy_fixture("hardlink");
    let external = temporary("external-link");
    std::fs::hard_link(root.join("src/core.spx"), &external).unwrap();
    assert_eq!(diagnostic(&root)[0].code, "SPX-J102");
    assert!(!root.join("out").exists());
    std::fs::remove_file(external).unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn windows_external_hardlink_and_declared_parent_junction_are_rejected_without_output() {
    use std::process::Command;

    let root = copy_fixture("windows-hardlink");
    let external = temporary("windows-external-link");
    std::fs::hard_link(root.join("src/core.spx"), &external).unwrap();
    assert_eq!(diagnostic(&root)[0].code, "SPX-J102");
    assert!(!root.join("out").exists());
    std::fs::remove_file(external).unwrap();
    let _ = std::fs::remove_dir_all(root);

    let root = copy_fixture("windows-junction");
    let foreign = root.join("foreign-source-target");
    std::fs::rename(root.join("src"), &foreign).unwrap();
    let sentinel = foreign.join("sentinel.txt");
    std::fs::write(&sentinel, b"foreign source bytes\n").unwrap();
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(root.join("src"))
        .arg(&foreign)
        .status()
        .unwrap();
    assert!(status.success(), "mklink /J failed");
    assert_eq!(diagnostic(&root)[0].code, "SPX-J102");
    assert!(!root.join("out").exists());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"foreign source bytes\n");
    std::fs::remove_dir(root.join("src")).unwrap();
    let _ = std::fs::remove_dir_all(root);
}
