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

#[test]
fn fresh_but_invalid_patch_changes_nothing() {
    let source = r#"module patch.collision;

@id("helper.answer")
fn answer() -> i64
{
    42
}

@id("app.main")
fn main() -> i64
{
    answer()
}
"#;
    let program = parse(source, Path::new("collision.spx")).unwrap();
    let revision = graph::revision(&program);
    let directory =
        std::env::temp_dir().join(format!("semaprax-invalid-patch-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("rename.spatch");
    std::fs::write(&source_path, source).unwrap();
    std::fs::write(
        &patch_path,
        format!("base {revision}\nrename helper.answer to main\n"),
    )
    .unwrap();

    let errors = patch::apply(&source_path, &patch_path).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-S101"));
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
}

#[test]
fn semantic_resource_rename_updates_ownership_boundaries() {
    let source = r#"module patch.resource;

@id("buffer.type")
resource Buffer;

@id("buffer.consume")
fn consume(buffer: own Buffer) -> Buffer
{
    buffer
}

@id("app.main")
fn main() -> i64
{
    42
}
"#;
    let program = parse(source, Path::new("resource.spx")).unwrap();
    let revision = graph::revision(&program);
    let directory =
        std::env::temp_dir().join(format!("semaprax-resource-patch-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("rename.spatch");
    std::fs::write(&source_path, source).unwrap();
    std::fs::write(
        &patch_path,
        format!("base {revision}\nrename buffer.type to ByteBuffer\nrequire no-new-effects\n"),
    )
    .unwrap();
    patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    assert!(changed.contains("resource ByteBuffer;"));
    assert!(changed.contains("buffer: own ByteBuffer"));
    assert!(changed.contains("-> ByteBuffer"));
    assert!(changed.contains("@id(\"buffer.type\")"));
}
