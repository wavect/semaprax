use std::path::Path;

use semaprax::{graph, parse, patch};

#[test]
fn semantic_rename_is_atomic_and_updates_calls() {
    let source = r#"module patch.demo;

@id("math.add")
fn add(a: i64, b: i64) -> i64
{
    a + b
}

@id("app.main")
fn main() -> i64
{
    add(40, 2)
}
"#;
    let program = parse(source, Path::new("patch.spx")).unwrap();
    let revision = graph::revision(&program);
    let directory = std::env::temp_dir().join(format!("semaprax-patch-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("rename.spatch");
    std::fs::write(&source_path, source).unwrap();
    std::fs::write(
        &patch_path,
        format!("base {revision}\nrename math.add to sum\nrequire no-new-effects\n"),
    )
    .unwrap();
    patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    assert!(changed.contains("fn sum("));
    assert!(changed.contains("sum(40, 2)"));
    assert!(changed.contains("@id(\"math.add\")"));
}

#[test]
fn stale_patch_changes_nothing() {
    let directory = std::env::temp_dir().join(format!("semaprax-stale-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("rename.spatch");
    let source = "module stale; @id(\"app.main\") fn main() -> i64 { 42 }\n";
    std::fs::write(&source_path, source).unwrap();
    std::fs::write(&patch_path, "base fnv1a64:0000000000000000\n").unwrap();
    let error = patch::apply(&source_path, &patch_path).unwrap_err();
    assert_eq!(error[0].code, "SPX-G409");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
}
