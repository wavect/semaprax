use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::{graph, parse, patch};

static NEXT_A0_FIXTURE: AtomicUsize = AtomicUsize::new(0);

const A0_SOURCE: &str = r#"module patch.a0;

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

fn a0_fixture(label: &str) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let sequence = NEXT_A0_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-patch-a0-{}-{label}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("rename.spatch");
    let revision = graph::revision(&parse(A0_SOURCE, &source_path).unwrap());
    std::fs::write(&source_path, A0_SOURCE).unwrap();
    std::fs::write(
        &patch_path,
        format!("base {revision}\nrename helper.answer to computed\n"),
    )
    .unwrap();
    (source_path, patch_path, revision)
}

fn lock_path(source_path: &Path) -> std::path::PathBuf {
    source_path
        .parent()
        .unwrap()
        .join(".module.spx.semaprax-patch.lock")
}

fn stage_path(source_path: &Path, index: usize) -> std::path::PathBuf {
    source_path
        .parent()
        .unwrap()
        .join(format!(".module.spx.semaprax-stage.{index}.tmp"))
}

fn assert_no_owned_artifacts(source_path: &Path) {
    assert!(!lock_path(source_path).exists());
    for index in 0..32 {
        assert!(!stage_path(source_path, index).exists());
    }
}

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

/// A patched file keeps every `//` comment, and the comment above a renamed
/// function stays above it; the result is canonical, so `fmt --check` accepts
/// it. Canonical comments v1 owns the placement rules.
#[test]
fn semantic_patch_keeps_comments_and_stays_canonical() {
    let source = "// Arithmetic helpers.\nmodule patch.demo;\n\n// Adds two numbers.\n@id(\"math.add\")\nfn add(a: i64, b: i64) -> i64\n{\n    a + b\n    // the sum\n}\n\n// Entry point.\n@id(\"app.main\")\nfn main() -> i64\n{\n    // forty-two\n    add(40, 2)\n}\n// end of file\n";
    let expected = source
        .replace("fn add(", "fn sum(")
        .replace("add(40, 2)", "sum(40, 2)");
    let program = parse(source, Path::new("patch.spx")).unwrap();
    let revision = graph::revision(&program);
    let directory =
        std::env::temp_dir().join(format!("semaprax-patch-comments-{}", std::process::id()));
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
    assert_eq!(changed, expected);
    assert!(changed.contains("// Adds two numbers.\n@id(\"math.add\")\nfn sum("));
    // Comments never reach the graph: the revision is the comment-free one.
    let stripped: String = expected
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(
        returned_revision,
        graph::revision(&parse(&stripped, &source_path).unwrap())
    );
    let check = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["fmt", source_path.to_str().unwrap(), "--check"])
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    std::fs::remove_dir_all(&directory).unwrap();
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

#[test]
fn semantic_patch_cleans_owned_lock_and_staging_after_success() {
    let (source_path, patch_path, _) = a0_fixture("success-cleanup");
    patch::apply(&source_path, &patch_path).unwrap();
    assert!(std::fs::read_to_string(&source_path)
        .unwrap()
        .contains("fn computed()"));
    assert_no_owned_artifacts(&source_path);
}

#[test]
fn semantic_patch_lock_contention_fails_without_mutation_or_deletion() {
    let (source_path, patch_path, _) = a0_fixture("lock-contention");
    let lock = lock_path(&source_path);
    std::fs::write(&lock, "not owned by this transaction").unwrap();

    let error = patch::apply(&source_path, &patch_path).unwrap_err();
    assert_eq!(error[0].code, "SPX-I205");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), A0_SOURCE);
    assert_eq!(
        std::fs::read_to_string(&lock).unwrap(),
        "not owned by this transaction"
    );
    for index in 0..32 {
        assert!(!stage_path(&source_path, index).exists());
    }
}

#[test]
fn semantic_patch_skips_create_new_stage_collisions_without_deleting_them() {
    let (source_path, patch_path, _) = a0_fixture("stage-collisions");
    std::fs::write(stage_path(&source_path, 0), "collision zero").unwrap();
    std::fs::write(stage_path(&source_path, 1), "collision one").unwrap();

    patch::apply(&source_path, &patch_path).unwrap();
    assert_eq!(
        std::fs::read_to_string(stage_path(&source_path, 0)).unwrap(),
        "collision zero"
    );
    assert_eq!(
        std::fs::read_to_string(stage_path(&source_path, 1)).unwrap(),
        "collision one"
    );
    assert!(!lock_path(&source_path).exists());
    for index in 2..32 {
        assert!(!stage_path(&source_path, index).exists());
    }
}

#[test]
fn semantic_patch_stage_exhaustion_preserves_source_lock_and_collisions() {
    let (source_path, patch_path, _) = a0_fixture("stage-exhaustion");
    for index in 0..32 {
        std::fs::write(
            stage_path(&source_path, index),
            format!("collision {index}"),
        )
        .unwrap();
    }

    let error = patch::apply(&source_path, &patch_path).unwrap_err();
    assert_eq!(error[0].code, "SPX-I203");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), A0_SOURCE);
    assert!(!lock_path(&source_path).exists());
    for index in 0..32 {
        assert_eq!(
            std::fs::read_to_string(stage_path(&source_path, index)).unwrap(),
            format!("collision {index}")
        );
    }
}

#[test]
fn semantic_patch_rejects_a_nonregular_source() {
    let (source_path, patch_path, _) = a0_fixture("nonregular-source");
    let directory_source = source_path.parent().unwrap().join("source-directory");
    std::fs::create_dir(&directory_source).unwrap();

    let error = patch::apply(&directory_source, &patch_path).unwrap_err();
    assert_eq!(error[0].code, "SPX-I201");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), A0_SOURCE);
}

#[cfg(unix)]
#[test]
fn semantic_patch_rejects_a_symlink_source_leaf() {
    use std::os::unix::fs::symlink;

    let (source_path, patch_path, _) = a0_fixture("source-symlink");
    let alias = source_path.parent().unwrap().join("source-alias.spx");
    symlink(&source_path, &alias).unwrap();

    let error = patch::apply(&alias, &patch_path).unwrap_err();
    assert_eq!(error[0].code, "SPX-I201");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), A0_SOURCE);
    assert!(std::fs::symlink_metadata(&alias)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[cfg(unix)]
#[test]
fn semantic_patch_uses_the_authenticated_canonical_parent_for_an_alias() {
    use std::os::unix::fs::symlink;

    let (source_path, patch_path, _) = a0_fixture("parent-symlink");
    let real_parent = source_path.parent().unwrap();
    let alias_parent = real_parent.with_extension("alias");
    symlink(real_parent, &alias_parent).unwrap();
    let aliased_source = alias_parent.join("module.spx");

    patch::apply(&aliased_source, &patch_path).unwrap();
    assert!(std::fs::read_to_string(&source_path)
        .unwrap()
        .contains("fn computed()"));
    assert_no_owned_artifacts(&source_path);
}

#[cfg(unix)]
#[test]
fn semantic_patch_never_follows_or_deletes_a_planted_stage_symlink() {
    use std::os::unix::fs::symlink;

    let (source_path, patch_path, _) = a0_fixture("stage-symlink");
    let target = source_path.parent().unwrap().join("symlink-target.txt");
    std::fs::write(&target, "sentinel").unwrap();
    let planted = stage_path(&source_path, 0);
    symlink(&target, &planted).unwrap();

    patch::apply(&source_path, &patch_path).unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "sentinel");
    assert!(std::fs::symlink_metadata(&planted)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!lock_path(&source_path).exists());
    for index in 1..32 {
        assert!(!stage_path(&source_path, index).exists());
    }
}

#[cfg(unix)]
#[test]
fn semantic_patch_never_follows_or_deletes_a_planted_lock_symlink() {
    use std::os::unix::fs::symlink;

    let (source_path, patch_path, _) = a0_fixture("lock-symlink");
    let target = source_path
        .parent()
        .unwrap()
        .join("lock-symlink-target.txt");
    std::fs::write(&target, "sentinel").unwrap();
    let planted = lock_path(&source_path);
    symlink(&target, &planted).unwrap();

    let error = patch::apply(&source_path, &patch_path).unwrap_err();
    assert_eq!(error[0].code, "SPX-I205");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), A0_SOURCE);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "sentinel");
    assert!(std::fs::symlink_metadata(&planted)
        .unwrap()
        .file_type()
        .is_symlink());
    for index in 0..32 {
        assert!(!stage_path(&source_path, index).exists());
    }
}
