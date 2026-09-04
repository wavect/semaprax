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
    let root = std::env::temp_dir().join(format!(
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
