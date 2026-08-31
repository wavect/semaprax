//! Real v8 standalone provider argument-staging allocation evidence.
//! Process waits/captures follow the existing physical String fixture: they
//! are NOT intrinsically bounded. Run under the reviewed restricted container
//! with an external process-group deadline and memory quota. Retain all files;
//! direct-child completion grants no recursive descendant cleanup authority.
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "native_bytes_call_staging_v1/subject.rs"]
mod subject;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn run(sanitized: bool) {
    let compiler = if sanitized {
        let path = PathBuf::from(
            std::env::var_os("SEMAPRAX_STRING_SANITIZER_CLANG")
                .expect("selected gate requires provisioned SEMAPRAX_STRING_SANITIZER_CLANG"),
        );
        assert!(path.is_absolute() && path.is_file());
        path
    } else {
        std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from)
    };
    let provider = subject::provider();
    let root = std::env::temp_dir().join(format!(
        "semaprax-bytes-call-staging-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    eprintln!("retained Bytes staging evidence: {}", root.display());
    let source = root.join("probe.c");
    fs::write(
        &source,
        format!(
            "{}\n{}\n{}\n{}",
            include_str!("support/native_fixture_stdio.c"),
            include_str!("native_owned_utf8_settlement_v1/allocations.c"),
            provider,
            include_str!("native_bytes_call_staging_v1/probe.c"),
        ),
    )
    .unwrap();
    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let mut compile = Command::new(&compiler);
        compile
            .current_dir(&root)
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"]);
        if sanitized {
            compile.args([
                "-fsanitize=address,undefined",
                "-fno-sanitize-recover=all",
                "-fno-omit-frame-pointer",
            ]);
        }
        let built = compile
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}: {}",
            root.display(),
            String::from_utf8_lossy(&built.stderr)
        );
        let mut execute = Command::new(&executable);
        execute.current_dir(&root);
        if sanitized {
            execute
                .env("ASAN_OPTIONS", "halt_on_error=1")
                .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
        }
        let result = execute.output().unwrap();
        assert!(
            result.status.success(),
            "{}: stdout={} stderr={}",
            root.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"native-bytes-call-staging-settled\n");
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn bytes_call_arguments_settle_once_at_o0_and_o2() {
    run(false);
}

#[test]
#[ignore = "requires explicitly provisioned Clang ASan/UBSan runtime and external process bounds"]
fn provisioned_bytes_call_arguments_asan_ubsan() {
    run(true);
}

#[test]
fn copy_variant_owned_match_keeps_cleanup_admission_closed() {
    let source = r#"module native.closed_variant_match;
@id("closed.main") fn main() -> i64 { 0 }
@id("closed.value") fn value(input: borrow Slice<u8>, branch: bool) -> Bytes {
    let choice = if branch { Option<i64>::Some { value: 7 } } else { Option<i64>::None {} };
    match choice {
        Option::Some { value: number } => bytes_copy(input),
        Option::None {} => bytes_copy(input),
    }
}
"#;
    // Source typing alone is not executable admission: the mandatory cleanup
    // builder still rejects a Copy-variant match with a droppable result.
    let parsed = semaprax::check(source, "closed-variant-match.spx").unwrap();
    let errors = semaprax::hir::resolve(&parsed).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-H006");
    assert_eq!(
        errors[0].message,
        "cleanup plan: droppable match result reached the copy-only cleanup slice"
    );
}

#[test]
fn scalar_owned_match_keeps_cleanup_replay_admission_closed() {
    for arms in [
        "0 => bytes_copy(input), _ => bytes_copy(input),",
        "0 => bytes, _ => bytes,",
    ] {
        let binding = if arms.contains("=> bytes,") {
            "let bytes = bytes_copy(input);"
        } else {
            ""
        };
        let source = format!(
            "module native.closed_scalar_match;\n\
             @id(\"closed.main\") fn main() -> i64 {{ 0 }}\n\
             @id(\"closed.value\") fn value(input: borrow Slice<u8>, branch: i64) -> Bytes {{\n\
             {binding} match branch {{ {arms} }}\n}}\n"
        );
        let parsed = semaprax::check(&source, "closed-scalar-match.spx").unwrap();
        // Resolving includes mandatory independent cleanup-plan replay,
        // which still rejects the owned result despite successful source typing.
        let errors = semaprax::hir::resolve(&parsed).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-H006");
        assert_eq!(
            errors[0].message,
            "cleanup plan: droppable match result reached the copy-only cleanup slice"
        );
    }
}
