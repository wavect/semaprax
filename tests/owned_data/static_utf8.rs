//! Internal static text carrier gates; public export profiles stay closed.
use semaprax::{codegen, hir, parse, wasm};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = r#"module static.backend;
@id("static.read")
fn read(text: borrow str, index: usize) -> i64 {
    match str_byte_at(text, index) {
        Option::Some { value: byte } => i64_from_u8(byte),
        Option::None {} => -1,
    }
}
@id("static.length")
fn text_length(text: borrow str) -> i64 { str_len_bytes(text) }
@id("static.scalar")
fn scalar() -> i64 { 42 }
@id("app.main")
fn main() -> i64 {
    let key = str"é\u{0}\"\\";
    let bytes = str_as_bytes(key);
    let empty = str"";
    let mut index = 0usize;
    let mut total = 0;
    while index < 5usize {
        total = total + read(key, index);
        index = index + 1usize;
        0
    }
    if total == 490 && str_len_bytes(key) == 5
        && byte_len(bytes) == 5usize && str_is_empty(empty)
        && str_contains(key, str"\u{0}\"")
        && str_starts_with(key, str"é")
        && read(key, 5usize) == -1
        && read(key, 18446744073709551615usize) == -1
        && read(empty, 0usize) == -1 { 42 } else { -1 }
}
"#;

fn directory() -> std::path::PathBuf {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-static-utf8-{}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn native_static_utf8_reads_repeat_without_any_heap_constructor_at_o0_o2() {
    native_probe(SOURCE, 0);
}

fn native_probe(source: &str, owned_allocations: u32) {
    let ast = parse(source, Path::new("static-native.spx")).unwrap();
    let resolved = hir::resolve(&ast).unwrap();
    hir::validate(&resolved).unwrap();
    if owned_allocations == 0 {
        assert!(resolved
            .functions
            .iter()
            .all(|f| f.cleanup_plan.slots.is_empty()));
    }
    let generated = codegen::emit_c(&ast).unwrap();
    assert_eq!(generated, codegen::emit_c(&ast).unwrap());
    assert!(generated.contains("\\303\\251\\000"));
    assert!(!generated.contains("spx_string_from_literal"));
    assert!(!generated.contains("spx_string_drop"));
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let path = directory();
    let preamble = include_str!("static_utf8/allocations.c");
    let probe = include_str!("static_utf8/probe.c");
    let input = path.join("probe.c");
    std::fs::write(&input, format!("{preamble}\n{generated}\n#define SPX_PROBE_EXPECTED_OWNER_ALLOCATIONS {owned_allocations}u\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let executable = path.join(format!(
            "probe{}{}",
            optimization,
            std::env::consts::EXE_SUFFIX
        ));
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&input)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        assert!(
            executed.status.success(),
            "static literal probe {optimization}: {:?} {}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn wasm_static_utf8_pool_reads_repeat_without_string_or_bytes_allocation() {
    let ast = parse(SOURCE, Path::new("static-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&ast).unwrap();
    assert_eq!(bytes, wasm::emit_module(&ast).unwrap());
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let path = directory();
    std::fs::write(path.join("probe.wasm"), bytes).unwrap();
    std::fs::write(
        path.join("probe.mjs"),
        include_str!("static_utf8/probe.mjs"),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn static_utf8_backends_replay_types_producers_and_keep_public_providers_closed() {
    let ast = parse(SOURCE, Path::new("static-hostile.spx")).unwrap();
    let resolved = hir::resolve(&ast).unwrap();
    for forged in 0..4 {
        let mut candidate = resolved.clone();
        let main = candidate
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let unrelated = main.body.id.clone();
        let hir::ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
            panic!("block")
        };
        let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
            panic!("literal")
        };
        match forged {
            0 => value.ty = hir::ResolvedType::String,
            1 => value.ownership = hir::OwnershipMode::Own,
            2 => value.id = unrelated,
            _ => value.kind = hir::ResolvedExprKind::String("owned".to_owned()),
        }
        assert_eq!(
            codegen::emit_hir_c(&candidate).unwrap_err().code,
            "SPX-H006"
        );
        assert_eq!(
            wasm::emit_resolved_module(&candidate).unwrap_err().code,
            "SPX-H006"
        );
    }
    assert_eq!(
        wasm::emit_module_with_scalar_exports(&ast, &["static.scalar".to_owned()])
            .unwrap_err()
            .code,
        "SPX-W119"
    );
    assert_eq!(
        wasm::emit_module_with_text_exports(&ast, &["static.length".to_owned()])
            .unwrap_err()
            .code,
        "SPX-W119"
    );
}

#[test]
fn static_utf8_writer_helpers_before_and_inside_loops_have_one_settled_owner_at_o0_o2_and_wasm() {
    let source = include_str!("static_utf8_writer.spx");
    native_probe(source, 1);
    let ast = parse(source, Path::new("static-writer.spx")).unwrap();
    let bytes = wasm::emit_module(&ast).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let path = directory();
    std::fs::write(path.join("probe.wasm"), bytes).unwrap();
    std::fs::write(
        path.join("probe.mjs"),
        include_str!("static_utf8/writer.mjs"),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn authenticated_project_retains_static_source_sites_across_revision_and_drift() {
    use semaprax::project::{self, ProjectExecutionOptions, ProjectExecutionOutcome};
    let root = directory();
    std::fs::create_dir(root.join("src")).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/owned-leaf-command-project");
    std::fs::copy(example.join("semaprax.toml"), root.join("semaprax.toml")).unwrap();
    let write = |path: &str, source: &str| {
        let program = semaprax::parse(source, path).unwrap();
        let canonical = semaprax::format::canonical(&program);
        assert_eq!(
            canonical,
            semaprax::format::canonical(&semaprax::parse(&canonical, path).unwrap())
        );
        std::fs::write(root.join(path), canonical).unwrap();
    };
    let app = std::fs::read_to_string(example.join("src/app.spx"))
        .unwrap()
        .replace(
            "@id(\"owned.data.verify\") from owned.data",
            "@id(\"static.writer\") from static.writer",
        );
    write("src/app.spx", &app);
    let data = include_str!("static_utf8_writer.spx")
        .replace("@id(\"app.main\") fn main()->i64 {run()}", "");
    write("src/data.spx", &data);
    write("src/tests.spx", "module owned.tests; use function @id(\"static.writer\") from static.writer as run; @id(\"owned.tests.main\") fn main()->i64 {if run()==42 {0}else{1}} @id(\"owned.tests.run\") fn test()->i64 {if run()==42 {0}else{1}}");
    let manifest = root.join("semaprax.toml");
    let before = project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        hir::validate(snapshot.entry_program()).map_err(|e| vec![e])?;
        let options =
            ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000).map_err(|e| vec![e])?;
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &ProjectExecutionOutcome::Returned(42)
        );
        assert!(snapshot.semantic_graph().contains("static.writer"));
        assert!(snapshot.semantic_graph().contains("static.write"));
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    write("src/data.spx", &data.replace("aé", "bé"));
    let after =
        project::with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
            .unwrap();
    assert_ne!(before.project_revision(), after.project_revision());
    assert_ne!(before.semantic_graph(), after.semantic_graph());
    let drift = project::with_authenticated_project(&manifest, |_snapshot| {
        write("src/data.spx", &data);
        Ok(())
    })
    .unwrap_err();
    assert!(
        !drift.is_empty(),
        "held authoritative source drift must fail closed"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn authenticated_project_replays_static_utf8_and_owned_bytes_iterator_runtime() {
    use semaprax::project::{self, ProjectExecutionOptions, ProjectExecutionOutcome};
    let root = directory();
    std::fs::create_dir(root.join("src")).unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/owned-leaf-command-project");
    std::fs::copy(example.join("semaprax.toml"), root.join("semaprax.toml")).unwrap();
    let write = |path: &str, source: &str| {
        let ast = semaprax::parse(source, path).unwrap();
        let canonical = semaprax::format::canonical(&ast);
        assert_eq!(
            canonical,
            semaprax::format::canonical(&semaprax::parse(&canonical, path).unwrap()),
        );
        std::fs::write(root.join(path), canonical).unwrap();
    };
    let app = std::fs::read_to_string(example.join("src/app.spx"))
        .unwrap()
        .replace(
            "use function @id(\"owned.data.verify\") from owned.data as verify;",
            "use function @id(\"static.lexical\") from static.writer as verify;
             use function @id(\"static.scratch\") from static.writer as scratch;",
        )
        .replacen("verify()", "verify() + scratch()", 1);
    write("src/app.spx", &app);
    let (data, _) = include_str!("static_utf8_iterator.spx")
        .split_once("@id(\"app.main\")")
        .unwrap();
    write("src/data.spx", data);
    write(
        "src/tests.spx",
        r#"module owned.tests;
        use function @id("static.lexical") from static.writer as run;
        use function @id("static.scratch") from static.writer as scratch;
        @id("owned.tests.main") fn main()->i64 {if run()+scratch()==10 {0}else{1}}
        @id("owned.tests.run") fn test()->i64 {if run()+scratch()==10 {0}else{1}}"#,
    );
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        hir::validate(snapshot.entry_program()).map_err(|e| vec![e])?;
        let options =
            ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000).map_err(|e| vec![e])?;
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &ProjectExecutionOutcome::Returned(10)
        );
        assert!(snapshot.semantic_graph().contains("static.scratch"));
        let source = snapshot
            .sources()
            .iter()
            .find(|source| source.path() == "src/data.spx")
            .expect("retained iterator source");
        assert_eq!(source.source_graph_schema(), "semaprax.graph.v78");
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
