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
    let returned_revision = patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    assert!(changed.contains("fn sum("));
    assert!(changed.contains("sum(40, 2)"));
    assert!(changed.contains("@id(\"math.add\")"));
    let reparsed = parse(&changed, &source_path).unwrap();
    assert_eq!(returned_revision, graph::revision(&reparsed));
}

#[test]
fn function_rename_does_not_retarget_same_named_interface_import() {
    let source = r#"module patch.import_collision;

@id("file.type")
resource File {
    @id("file.type.drop")
    drop trivial;
}

@id("file.host")
interface FileHost permits {} {
    @id("file.process")
    import fn process(file: own File) -> unit
        effects {}
        failure infallible
        consumes file always;
}

@id("app.process")
fn process() -> i64
{
    42
}

@id("app.main")
fn main() -> i64
{
    process()
}
"#;
    let program = parse(source, Path::new("function-import-collision.spx")).unwrap();
    let revision = graph::revision(&program);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-function-import-collision-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("rename.spatch");
    std::fs::write(&source_path, source).unwrap();
    std::fs::write(
        &patch_path,
        format!("base {revision}\nrename app.process to execute\n"),
    )
    .unwrap();

    patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    assert!(changed.contains("import fn process(file: own File)"));
    assert!(changed.contains("fn execute() -> i64"));
    assert!(changed.contains("execute()\n}"));
    assert!(changed.contains("@id(\"file.process\")"));
    assert!(changed.contains("@id(\"app.process\")"));
    graph::to_json(&parse(&changed, &source_path).unwrap()).unwrap();
}

#[test]
fn legacy_fnv_patch_is_rejected_without_changing_source() {
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
fn stale_sha256_patch_is_rejected_without_changing_source() {
    let directory = std::env::temp_dir().join(format!("semaprax-stale-sha-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("rename.spatch");
    let source = "module stale_sha; @id(\"app.main\") fn main() -> i64 { 42 }\n";
    let program = parse(source, &source_path).unwrap();
    let revision = graph::revision(&program);
    let mut stale = revision.into_bytes();
    let last = stale.last_mut().unwrap();
    *last = if *last == b'0' { b'1' } else { b'0' };
    let stale = String::from_utf8(stale).unwrap();
    std::fs::write(&source_path, source).unwrap();
    std::fs::write(&patch_path, format!("base {stale}\n")).unwrap();

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
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}

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
    assert!(changed.contains("resource ByteBuffer {"));
    assert!(changed.contains("buffer: own ByteBuffer"));
    assert!(changed.contains("-> ByteBuffer"));
    assert!(changed.contains("@id(\"buffer.type\")"));
}

#[test]
fn resource_rename_does_not_retarget_record_initializer_values() {
    let source = r#"module patch.record_resource;

@id("handle.type")
resource Handle {
    @id("handle.type.drop")
    drop trivial;
}

@id("wrapper.type")
record Wrapper {
    @id("wrapper.handle")
    handle: Handle,
}

@id("wrapper.wrap")
fn wrap(Handle: own Handle, other: own Handle) -> Wrapper
{
    Wrapper { handle: Handle }
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let program = parse(source, Path::new("record-resource.spx")).unwrap();
    let revision = graph::revision(&program);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-record-resource-patch-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("rename.spatch");
    std::fs::write(&source_path, source).unwrap();
    std::fs::write(
        &patch_path,
        format!("base {revision}\nrename handle.type to other\nrequire no-new-effects\n"),
    )
    .unwrap();

    patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    assert!(changed.contains("resource other {"));
    assert!(changed.contains("handle: other,"));
    assert!(changed.contains("Handle: own other, other: own other"));
    assert!(changed.contains("Wrapper { handle: Handle }"));
    assert!(!changed.contains("Wrapper { handle: other }"));
    let reparsed = parse(&changed, &source_path).unwrap();
    graph::to_json(&reparsed).unwrap();
}

#[test]
fn resource_rename_updates_import_parameter_without_retargeting_lifecycle_keys() {
    let source = r#"module patch.lifecycle;

@id("file.type")
resource File {
    @id("file.type.drop")
    drop import "file.finalize";
}

@id("file.host")
interface FileHost
    permits { file.release }
{
    @id("file.finalize")
    import fn finalize(file: own File) -> unit
        effects { file.release }
        failure infallible
        consumes file always;
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;
    let program = parse(source, Path::new("resource-lifecycle.spx")).unwrap();
    let revision = graph::revision(&program);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-resource-lifecycle-patch-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("rename.spatch");
    std::fs::write(&source_path, source).unwrap();
    std::fs::write(
        &patch_path,
        format!("base {revision}\nrename file.type to Handle\nrequire no-new-effects\n"),
    )
    .unwrap();

    let returned = patch::apply(&source_path, &patch_path).unwrap();
    let changed = std::fs::read_to_string(&source_path).unwrap();
    assert!(changed.contains("resource Handle {"));
    assert!(changed.contains("finalize(file: own Handle)"));
    assert!(changed.contains("@id(\"file.type.drop\")"));
    assert!(changed.contains("drop import \"file.finalize\";"));
    assert!(changed.contains("@id(\"file.finalize\")"));
    let reparsed = parse(&changed, &source_path).unwrap();
    assert_eq!(returned, graph::revision(&reparsed));
    graph::to_json(&reparsed).unwrap();
}
