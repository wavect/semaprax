//! Physical ordinary-C evidence, not the distinct v10 provider boundary.
//! The unchanged allocator observer wraps only the generated C translation unit.
//! No interpreter String-signature or ordinary-Wasm drop parity is claimed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, format, graph, hir};

#[path = "../native_string_settlement_v1/contents.rs"]
mod contents;

static SERIAL: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = include_str!("../native_string_settlement_v1/source.spx");
const OBSERVER: &str = include_str!("../native_owned_utf8_settlement_v1/allocations.c");
const STDIO: &str = include_str!("../support/native_fixture_stdio.c");
const GENERIC: &str = r#"
module native.generic_string_settlement;
@id("s.generic")
fn generic<T>(value: T, zero: i64) -> i64 {
    let text = string_from_char('\0');
    let length = string_len_chars(text);
    let checked = 1 / zero;
    length
}
@id("s.generic-root")
fn root(zero: i64) -> i64 { generic<i64>(42, zero) }
@id("s.main")
fn main() -> i64 { root(1) }
"#;
const STDOUT: &str = r#"
module native.stdout_string_settlement;
permit { process.stdout.write }
@id("s.main")
fn main() -> i64 uses { process.stdout.write } {
    let text = "\u{0}kept-through-output\u{0}世界";
    let data = [65u8, 0u8, 66u8];
    let view = array_as_slice(data);
    let written = stdout_write(view);
    let checked = 1 / 0;
    if string_len(text) == 27 { 7 } else { 0 }
}
"#;

fn checked(source: &str) -> semaprax::ast::Program {
    let program = semaprax::check(source, "native-string-settlement.spx").unwrap();
    let canonical = format::canonical(&program);
    let reparsed = semaprax::check(&canonical, "canonical-string-settlement.spx").unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(
        graph::to_json(&program).unwrap(),
        graph::to_json(&reparsed).unwrap()
    );
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
    program
}

fn symbol(id: &str) -> String {
    format!(
        "spx_decl_{}",
        id.bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn ordinary() -> String {
    let program = checked(SOURCE);
    let json = graph::to_json(&program).unwrap();
    for id in [
        "s.post",
        "s.requires",
        "core.string.concat",
        "core.string.from_char",
    ] {
        assert!(json.contains(id));
    }
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated.contains("struct spx_string_v10"));
    assert!(!generated.contains("strlen(spx_source)"));
    let mut aliases = String::new();
    for (name, id) in [
        ("BEFORE", "s.before"),
        ("LOCAL", "s.local"),
        ("LATE", "s.late"),
        ("NESTED", "s.nested"),
        ("CALLEE", "s.callee"),
        ("CONDITION", "s.condition"),
        ("BODY", "s.body"),
        ("MIXED", "s.mixed"),
        ("PRE", "s.pre"),
        ("POST", "s.post"),
        ("CLONE", "s.clone"),
        ("BRANCH", "s.branch"),
        ("MATCH", "s.match"),
        ("PRESSURE", "s.pressure"),
        ("EMPTY", "s.empty"),
        ("OPS", "s.ops"),
        ("FROM_CHAR", "s.from-char"),
        ("EQUALITY", "s.equality"),
    ] {
        aliases.push_str(&format!("#define FIXTURE_{name} {}\n", symbol(id)));
    }
    format!(
        "{STDIO}\n{OBSERVER}\n{generated}\n{aliases}\n{}",
        include_str!("../native_string_settlement_v1/probe.c")
    )
}

fn generic() -> String {
    let program = checked(GENERIC);
    let resolved = hir::resolve(&program).unwrap();
    assert_eq!(resolved.function_instances.len(), 1);
    assert_eq!(
        resolved.function_instances[0].template.as_str(),
        "s.generic"
    );
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated.contains("spx_string_from_char"));
    assert!(generated.contains("spx_string_len_chars"));
    let root = symbol("s.generic-root");
    format!(
        r#"{STDIO}
{OBSERVER}
{generated}
#undef malloc
#undef free
int main(void) {{
    REQUIRE(fixture_binary_stdout());
    struct spx_status_entry entries[32];
    struct spx_context context = {{0}};
    REQUIRE(spx_context_init(&context, 17, entries, 32, NULL, NULL, NULL));
    for (unsigned repetition = 0; repetition < 32; ++repetition) {{
        size_t before = fixture_allocations, freed = fixture_frees;
        int64_t value = INT64_MIN;
        spx_status_token status = {root}(&context, 0, &value);
        const struct spx_normalized_status *normalized = spx_status_resolve(&context, status);
        REQUIRE(status != 0 && normalized != NULL && normalized->code == 4);
        REQUIRE(strcmp(normalized->domain_id, "semaprax.arithmetic.v1") == 0);
        REQUIRE(value == INT64_MIN && fixture_allocations - before == 2);
        REQUIRE(fixture_frees - freed == 2 && fixture_live == 0);
        REQUIRE({root}(&context, 1, &value) == 0 && value == 1);
        REQUIRE(fixture_allocations - before == 4 && fixture_frees - freed == 4);
        REQUIRE(fixture_live == 0);
        REQUIRE(context.status_arena.length == repetition + 1);
    }}
    REQUIRE(fixture_allocations == fixture_frees);
    (void)puts("native-ordinary-strings-settled");
    return 0;
}}
"#
    )
}

fn stdout(empty_success: bool) -> String {
    let source = if empty_success {
        STDOUT
            .replace(
                "let written = stdout_write(view);",
                "let written = if false { stdout_write(view) } else { 0usize };",
            )
            .replace("1 / 0", "1 / 1")
    } else {
        STDOUT.to_owned()
    };
    let program = checked(&source);
    let json = graph::to_json(&program).unwrap();
    assert!(json.contains("core.host.stdout-write"));
    assert!(json.contains("terminal-success-only"));
    let generated = codegen::emit_c_with_stdout_transcript(&program).unwrap();
    assert_eq!(
        generated,
        codegen::emit_c_with_stdout_transcript(&program).unwrap()
    );
    let success = u8::from(empty_success);
    let expected = if empty_success { 7 } else { 0 };
    // Success additionally clones the retained text for its byte-length read.
    // Failure settles the local before reaching that read.
    let allocations = if empty_success { 2 } else { 1 };
    format!(
        r#"{STDIO}
{OBSERVER}
{generated}
#undef malloc
#undef free
int main(void) {{
    REQUIRE(fixture_binary_stdout());
    for (unsigned repetition = 0; repetition < 32; ++repetition) {{
        struct spx_stdout_transcript_result_v1 result;
        memset(&result, 0xa5, sizeof(result));
        size_t before = fixture_allocations, freed = fixture_frees;
        REQUIRE(spx_stdout_transcript_run_v1(&result) == {success});
        REQUIRE(result.value == {expected} && result.transcript_length == 0);
        for (size_t i = 0; i < sizeof(result.transcript); ++i) REQUIRE(result.transcript[i] == 0);
        REQUIRE(fixture_allocations - before == {allocations} && fixture_frees - freed == {allocations});
        REQUIRE(fixture_live == 0 && fixture_allocations == fixture_frees);
    }}
    (void)puts("native-ordinary-strings-settled");
    return 0;
}}
"#
    )
}

fn compile_and_run(kind: &str, source: &str, sanitized: bool) {
    let compiler = if sanitized {
        let configured = PathBuf::from(
            std::env::var_os("SEMAPRAX_STRING_SANITIZER_CLANG")
                .expect("selected sanitizer gate requires SEMAPRAX_STRING_SANITIZER_CLANG"),
        );
        assert!(configured.is_absolute() && configured.is_file());
        configured
    } else {
        std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from)
    };
    let root = std::env::temp_dir().join(format!(
        "semaprax-ordinary-string-{kind}-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let path = root.join("probe.c");
    fs::write(&path, source).unwrap();
    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let mut compiler = Command::new(&compiler);
        compiler.current_dir(&root).args([
            "-std=c11",
            optimization,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-DSPX_NO_ENTRY_WRAPPER",
        ]);
        if sanitized {
            compiler.args([
                "-fsanitize=address,undefined",
                "-fno-sanitize-recover=all",
                "-fno-omit-frame-pointer",
            ]);
        }
        let compiled = compiler
            .arg(&path)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Clang is required for ordinary native String physical evidence");
        assert!(
            compiled.status.success(),
            "{}: {}",
            root.display(),
            String::from_utf8_lossy(&compiled.stderr)
        );
        let mut command = Command::new(&executable);
        command.current_dir(&root);
        if sanitized {
            command.env("ASAN_OPTIONS", "halt_on_error=1");
            command.env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}: stdout={} stderr={}",
            root.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"native-ordinary-strings-settled\n");
        assert!(output.stderr.is_empty());
    }
    remove_successful_fixture(&root);
}

fn remove_successful_fixture(root: &Path) {
    let mut permitted = vec!["probe.c".to_owned()];
    for optimization in ["-O0", "-O2"] {
        permitted.push(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        if cfg!(windows) {
            for extension in ["lib", "exp", "pdb", "ilk"] {
                permitted.push(format!("probe{optimization}.{extension}"));
            }
        }
    }
    let entries = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    assert!(entries.len() <= permitted.len());
    for entry in &entries {
        assert!(permitted.contains(&entry.file_name().into_string().unwrap()));
        let metadata = fs::symlink_metadata(entry.path()).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            assert_eq!(metadata.file_attributes() & 0x400, 0);
        }
    }
    for entry in entries {
        fs::remove_file(entry.path()).unwrap();
    }
    fs::remove_dir(root).unwrap();
}

#[test]
fn ordinary_native_strings_settle_at_o0_and_o2() {
    compile_and_run("ordinary", &ordinary(), false);
}

#[test]
fn generic_instance_only_string_runtime_settles_at_o0_and_o2() {
    compile_and_run("generic", &generic(), false);
}

#[test]
fn stdout_failure_and_empty_success_settle_strings_at_o0_and_o2() {
    compile_and_run("stdout-failure", &stdout(false), false);
    compile_and_run("stdout-empty-success", &stdout(true), false);
}

#[test]
#[ignore = "requires explicitly provisioned Clang ASan/UBSan runtime"]
fn provisioned_ordinary_native_string_asan_ubsan() {
    compile_and_run("ordinary-sanitized", &ordinary(), true);
    compile_and_run("generic-sanitized", &generic(), true);
    compile_and_run("stdout-failure-sanitized", &stdout(false), true);
    compile_and_run("stdout-empty-sanitized", &stdout(true), true);
}

#[test]
fn ordinary_string_evidence_preserves_move_and_stdout_authority_diagnostics() {
    let moved = r#"module native.invalid_move;
@id("s.main") fn main() -> i64 {
    let a = string_concat("hello", "world");
    let b = string_concat(a, "!");
    if string_starts_with(a, "he") { 7 } else { string_len_chars(b) }
}"#;
    let program = semaprax::parse(moved, Path::new("invalid-move.spx")).unwrap();
    assert!(hir::resolve(&program)
        .unwrap_err()
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-O101"));
    let missing = STDOUT.replace(
        "fn main() -> i64 uses { process.stdout.write }",
        "fn main() -> i64",
    );
    let program = semaprax::parse(&missing, Path::new("missing-authority.spx")).unwrap();
    assert!(semaprax::verify::verify(&program)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-E102"));
}
