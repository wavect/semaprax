//! `semaprax fmt` keeps `//` comments: the CLI half of the contract owned by
//! `docs/CANONICAL-COMMENTS-V1.md`. The placement rules themselves are pinned
//! by the unit cases in `src/format/comments.rs`.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const COMMENTED: &str = "// Calculator with notes.\nmodule app.calc;\n\n// Adds two numbers.\n@id(\"calc.add\")\nfn add(left: i64, right: i64) -> i64\n    requires left >= 0 // only non-negative\n{\n    left + right // the sum\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    // start\n    let base = add(19, 23);\n    base // done\n}\n// trailing note\n";

const CANONICAL: &str = "// Calculator with notes.\nmodule app.calc;\n\n// Adds two numbers.\n// only non-negative\n@id(\"calc.add\")\nfn add(left: i64, right: i64) -> i64\n    requires left >= 0\n{\n    left + right\n    // the sum\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    // start\n    let base = add(19, 23);\n    base\n    // done\n}\n// trailing note\n";

fn fixture(label: &str) -> PathBuf {
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "semaprax-fmt-comments-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

fn cli(arguments: &[&str]) -> (i32, Vec<u8>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        output.stdout,
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn fmt_keeps_comments_and_is_idempotent() {
    let root = fixture("keep");
    let path = root.join("calc.spx");
    std::fs::write(&path, COMMENTED).unwrap();
    let text = path.to_str().unwrap();

    let (status, _, stderr) = cli(&["fmt", text, "--check"]);
    assert_eq!(status, 1, "a file with misplaced comments is not canonical");
    assert!(
        stderr.ends_with("is not canonically formatted\n"),
        "{stderr}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), COMMENTED);

    let (status, stdout, stderr) = cli(&["fmt", text]);
    assert_eq!(status, 0, "{stderr}");
    assert!(stdout.is_empty() && stderr.is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), CANONICAL);

    let (status, _, stderr) = cli(&["fmt", text, "--check"]);
    assert_eq!(status, 0, "{stderr}");
    let (status, _, _) = cli(&["fmt", text]);
    assert_eq!(status, 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), CANONICAL);

    let (status, stdout, _) = cli(&["run", text]);
    assert_eq!(status, 0);
    assert_eq!(stdout, b"42\n");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn comments_do_not_reach_the_semantic_graph() {
    let root = fixture("graph");
    let commented = root.join("calc.spx");
    let plain = root.join("plain.spx");
    std::fs::write(&commented, CANONICAL).unwrap();
    let stripped: String = CANONICAL
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(|line| format!("{line}\n"))
        .collect();
    std::fs::write(&plain, &stripped).unwrap();
    let (status, _, stderr) = cli(&["fmt", plain.to_str().unwrap(), "--check"]);
    assert_eq!(status, 0, "the comment-free form is canonical: {stderr}");

    let (status, with_comments, _) = cli(&["graph", commented.to_str().unwrap()]);
    assert_eq!(status, 0);
    let (status, without_comments, _) = cli(&["graph", plain.to_str().unwrap()]);
    assert_eq!(status, 0);
    let normalize = |bytes: Vec<u8>| {
        String::from_utf8(bytes)
            .unwrap()
            .replace("plain.spx", "calc.spx")
    };
    assert_eq!(normalize(with_comments), normalize(without_comments));
    std::fs::remove_dir_all(root).unwrap();
}

const PROJECT_MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"notes\"\nentry = \"notes.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\nweb_exports = [\"notes.add\"]\ntests = [\"notes.tests\"]\n";
const APP_DRIFTED: &str = "module notes.app;\nuse function @id(\"notes.add\") from notes.core as add;\n\n@id(\"notes.app.main\")\nfn main() -> i64\n{\n    add(40, 2) // the answer\n}\n";
const APP_CANONICAL: &str = "module notes.app;\nuse function @id(\"notes.add\") from notes.core as add;\n\n@id(\"notes.app.main\")\nfn main() -> i64\n{\n    add(40, 2)\n    // the answer\n}\n";
const CORE_CANONICAL: &str = "module notes.core;\n\n// Adds two numbers.\n@id(\"notes.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n";
const TESTS_DRIFTED: &str = "// Conformance suite.\nmodule notes.tests;\nuse function @id(\"notes.add\") from notes.core as add;\n\n@id(\"notes.tests.main\")\nfn main() -> i64\n{\n        if add(19, 23) == 42 { 0 } else { 1 }\n}\n";
const TESTS_CANONICAL: &str = "// Conformance suite.\nmodule notes.tests;\nuse function @id(\"notes.add\") from notes.core as add;\n\n@id(\"notes.tests.main\")\nfn main() -> i64\n{\n    if add(19, 23) == 42 { 0 } else { 1 }\n}\n";

fn write_project(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), PROJECT_MANIFEST).unwrap();
    std::fs::write(root.join("src/app.spx"), APP_DRIFTED).unwrap();
    std::fs::write(root.join("src/core.spx"), CORE_CANONICAL).unwrap();
    std::fs::write(root.join("src/tests.spx"), TESTS_DRIFTED).unwrap();
}

fn read(root: &std::path::Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative)).unwrap()
}

/// `fmt <dir>` and `fmt <dir>/semaprax.toml` format every manifest source in
/// manifest order through the same comment-preserving projection.
#[test]
fn fmt_formats_every_project_source_in_manifest_order() {
    let root = fixture("project");
    write_project(&root);
    let dir = root.to_str().unwrap();
    let manifest = root.join("semaprax.toml");

    let (status, stdout, stderr) = cli(&["fmt", dir, "--check"]);
    assert_eq!(status, 1);
    assert!(stdout.is_empty());
    assert_eq!(
        stderr,
        format!(
            "{} is not canonically formatted\n{} is not canonically formatted\n",
            root.join("src/app.spx").display(),
            root.join("src/tests.spx").display()
        )
    );
    assert_eq!(
        read(&root, "src/app.spx"),
        APP_DRIFTED,
        "--check never writes"
    );

    let (status, stdout, stderr) = cli(&["fmt", dir]);
    assert_eq!(status, 0, "{stderr}");
    assert!(stdout.is_empty() && stderr.is_empty());
    assert_eq!(read(&root, "src/app.spx"), APP_CANONICAL);
    assert_eq!(read(&root, "src/core.spx"), CORE_CANONICAL);
    assert_eq!(read(&root, "src/tests.spx"), TESTS_CANONICAL);

    let (status, _, stderr) = cli(&["fmt", manifest.to_str().unwrap(), "--check"]);
    assert_eq!(status, 0, "{stderr}");
    let (status, _, stderr) = cli(&["lock", manifest.to_str().unwrap(), "--write"]);
    assert_eq!(status, 0, "{stderr}");
    let (status, stdout, _) = cli(&["test", dir]);
    assert_eq!(status, 0);
    assert_eq!(stdout, b"project tests passed\n");
    let (status, stdout, _) = cli(&["run", dir]);
    assert_eq!(status, 0);
    assert_eq!(stdout, b"42\n");
    std::fs::remove_dir_all(root).unwrap();
}

/// A parse error in one source reports that file and writes nothing, and a
/// directory without a manifest is reported as the unreadable manifest.
#[test]
fn fmt_project_failures_write_nothing() {
    let root = fixture("project-failures");
    write_project(&root);
    std::fs::write(root.join("src/app.spx"), "module notes.app;\n\nfn {\n").unwrap();
    let dir = root.to_str().unwrap();

    let (status, stdout, stderr) = cli(&["fmt", dir]);
    assert_eq!(status, 1);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("error[SPX-"), "{stderr}");
    assert!(stderr.contains("app.spx"), "{stderr}");
    assert_eq!(
        read(&root, "src/tests.spx"),
        TESTS_DRIFTED,
        "no file is rewritten when another one does not parse"
    );

    let empty = root.join("empty");
    std::fs::create_dir(&empty).unwrap();
    let (status, stdout, stderr) = cli(&["fmt", empty.to_str().unwrap()]);
    assert_eq!(status, 1);
    assert!(stdout.is_empty());
    assert!(
        stderr.starts_with(&format!(
            "cannot read {}: ",
            empty.join("semaprax.toml").display()
        )),
        "{stderr}"
    );
    std::fs::remove_dir_all(root).unwrap();
}
